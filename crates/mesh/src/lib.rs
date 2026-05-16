use litm_common::{
    ControlPayload, Delivery, Epoch, Kind, Mesh, NeighborInfo, NodeId, ObjectBitmap, Transport,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BeaconPayload {
    pub epoch: u32,
    pub beacon_seq: u32,
    pub neighbors_heard: Vec<(NodeId, u8)>, // (NodeId, PRR * 255)
    pub decoded: ObjectBitmap,
}

#[derive(Clone, Debug)]
struct NeighborState {
    prr: f32,
    rssi_dbm: i8,
    last_seen: Instant,
    bitmap: ObjectBitmap,
    expected_beacon_seq: Option<u32>,
}

/// Maps raw RSSI from the RTL8812AU to a [0.0, 1.0] quality estimate.
/// -90 dBm → 0.0 (dead), -50 dBm → 1.0 (excellent), linear between.
/// Returns None when rssi ≤ -100 dBm, which is the driver sentinel for "not available".
fn rssi_to_prr(rssi: i8) -> Option<f32> {
    if rssi <= -100 {
        None
    } else {
        Some(((rssi as f32 + 90.0) / 40.0).clamp(0.0, 1.0))
    }
}

pub struct MeshService {
    transport: Arc<dyn Transport>,
    neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
    /// link_state[node_id] = that node's own neighbor list (from its most recent beacon).
    /// Enables building the full topology graph for Dijkstra routing.
    link_state: Arc<RwLock<HashMap<NodeId, Vec<(NodeId, u8)>>>>,
    pending_channel_switch: Arc<RwLock<Option<(u8, Epoch)>>>,
    current_channel: Arc<AtomicU8>,
}

impl MeshService {
    pub fn new(transport: Arc<dyn Transport>, delivery: Arc<dyn Delivery>) -> Arc<Self> {
        let neighbor_table = Arc::new(RwLock::new(HashMap::new()));
        let link_state = Arc::new(RwLock::new(HashMap::new()));
        let pending_channel_switch = Arc::new(RwLock::new(None));
        let current_channel = Arc::new(AtomicU8::new(6));

        let service = Arc::new(Self {
            transport: Arc::clone(&transport),
            neighbor_table: Arc::clone(&neighbor_table),
            link_state: Arc::clone(&link_state),
            pending_channel_switch: Arc::clone(&pending_channel_switch),
            current_channel: Arc::clone(&current_channel),
        });

        Self::spawn_beacon_sender(
            Arc::clone(&transport),
            Arc::clone(&delivery),
            Arc::clone(&neighbor_table),
        );
        Self::spawn_beacon_receiver(
            Arc::clone(&transport),
            Arc::clone(&delivery),
            Arc::clone(&neighbor_table),
            Arc::clone(&link_state),
        );
        Self::spawn_neighbor_eviction(Arc::clone(&neighbor_table), Arc::clone(&link_state));

        Self::spawn_flooding_task(Arc::clone(&transport), Kind::Fec);
        Self::spawn_flooding_task(Arc::clone(&transport), Kind::Control);
        Self::spawn_control_handler(
            Arc::clone(&transport),
            Arc::clone(&pending_channel_switch),
        );
        Self::spawn_channel_switch_watcher(
            Arc::clone(&transport),
            Arc::clone(&pending_channel_switch),
            Arc::clone(&current_channel),
        );

        service
    }

    fn spawn_beacon_sender(
        transport: Arc<dyn Transport>,
        delivery: Arc<dyn Delivery>,
        neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(20));
            let mut next_beacon = Instant::now() + Duration::from_millis(100);
            let mut beacon_seq: u32 = 0;

            loop {
                interval.tick().await;
                if Instant::now() >= next_beacon {
                    let epoch = current_epoch();

                    let neighbors_heard = {
                        let table = neighbor_table.read().unwrap();
                        table
                            .iter()
                            .map(|(id, state)| (*id, (state.prr * 255.0).clamp(0.0, 255.0) as u8))
                            .collect()
                    };

                    let decoded = delivery.decoded_bitmap();

                    let payload = BeaconPayload {
                        epoch,
                        beacon_seq,
                        neighbors_heard,
                        decoded,
                    };

                    if let Ok(encoded) = postcard::to_allocvec(&payload) {
                        let _ = transport.broadcast(Kind::Beacon, &encoded);
                    }

                    beacon_seq = beacon_seq.wrapping_add(1);
                    let jitter = rand::thread_rng().gen_range(90..=110);
                    next_beacon = Instant::now() + Duration::from_millis(jitter);
                }
            }
        });
    }

    fn spawn_beacon_receiver(
        transport: Arc<dyn Transport>,
        delivery: Arc<dyn Delivery>,
        neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
        link_state: Arc<RwLock<HashMap<NodeId, Vec<(NodeId, u8)>>>>,
    ) {
        let local_id = transport.local_id();
        let mut rx = transport.subscribe(Kind::Beacon);
        tokio::spawn(async move {
            while let Some((meta, payload)) = rx.recv().await {
                if meta.sender_id == local_id {
                    continue;
                }
                if let Ok(beacon) = postcard::from_bytes::<BeaconPayload>(&payload) {
                    {
                        let mut table = neighbor_table.write().unwrap();
                        let is_new = !table.contains_key(&meta.sender_id);
                        let state = table.entry(meta.sender_id).or_insert(NeighborState {
                            prr: 0.5, // neutral prior — let EMA converge rather than starting optimistic
                            rssi_dbm: meta.rssi_dbm,
                            last_seen: Instant::now(),
                            bitmap: beacon.decoded,
                            expected_beacon_seq: None,
                        });

                        if is_new {
                            tracing::info!(
                                peer = meta.sender_id,
                                rssi = meta.rssi_dbm,
                                "new neighbor discovered"
                            );
                        }

                        state.last_seen = Instant::now();
                        state.bitmap = beacon.decoded;
                        // Smooth RSSI with fast EMA — responds within a couple of beacons.
                        state.rssi_dbm =
                            (state.rssi_dbm as f32 * 0.4 + meta.rssi_dbm as f32 * 0.6) as i8;

                        match state.expected_beacon_seq {
                            None => {
                                // First beacon: use RSSI as the quality signal since we have
                                // no delivery history yet (no missed beacons to count).
                                // Fall back to neutral (0.5) if RSSI is unavailable.
                                let inst_prr = rssi_to_prr(meta.rssi_dbm).unwrap_or(0.5);
                                state.prr = (state.prr * 0.3 + inst_prr * 0.7).clamp(0.0, 1.0);
                            }
                            Some(expected) => {
                                if beacon.beacon_seq >= expected {
                                    let gap = (beacon.beacon_seq - expected + 1) as f32;
                                    let delivery_prr = 1.0 / gap;
                                    // Conservative blend: report the worse of radio signal
                                    // quality and actual delivery ratio. Falls back to
                                    // delivery-only when RSSI is unavailable (-128 sentinel).
                                    let inst_prr = match rssi_to_prr(meta.rssi_dbm) {
                                        Some(rssi_prr) => f32::min(rssi_prr, delivery_prr),
                                        None => delivery_prr,
                                    };
                                    state.prr = (state.prr * 0.3 + inst_prr * 0.7).clamp(0.0, 1.0);
                                } else {
                                    // Received seq is behind expected.
                                    // Large gap → genuine peer restart; small gap → late/reordered.
                                    let backward = expected.wrapping_sub(beacon.beacon_seq);
                                    if backward > 1000 {
                                        tracing::info!(
                                            peer = meta.sender_id,
                                            "peer restarted (seq reset), reinitialising PRR"
                                        );
                                        state.prr = 0.5;
                                        state.rssi_dbm = meta.rssi_dbm;
                                        state.expected_beacon_seq = None;
                                    }
                                    // Either way: do not advance expected_beacon_seq here.
                                    continue;
                                }
                            }
                        }
                        state.expected_beacon_seq = Some(beacon.beacon_seq.wrapping_add(1));
                    }

                    // Store this node's link-state advertisement for topology graph
                    link_state
                        .write()
                        .unwrap()
                        .insert(meta.sender_id, beacon.neighbors_heard.clone());

                    let prr = {
                        let table = neighbor_table.read().unwrap();
                        table.get(&meta.sender_id).map(|s| s.prr).unwrap_or(0.5)
                    };
                    delivery.note_peer_coverage(meta.sender_id, beacon.decoded, prr);
                }
            }
        });
    }

    fn spawn_neighbor_eviction(
        neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
        link_state: Arc<RwLock<HashMap<NodeId, Vec<(NodeId, u8)>>>>,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                let now = Instant::now();
                let mut table = neighbor_table.write().unwrap();
                table.retain(|id, state| {
                    let alive = now.duration_since(state.last_seen) <= Duration::from_millis(2000);
                    if !alive {
                        tracing::info!(peer = id, "neighbor evicted (no beacon for 2000 ms)");
                    }
                    alive
                });

                // Remove link-state entries for evicted neighbors
                let live: HashSet<NodeId> = table.keys().copied().collect();
                link_state
                    .write()
                    .unwrap()
                    .retain(|id, _| live.contains(id));
            }
        });
    }

    fn spawn_control_handler(
        transport: Arc<dyn Transport>,
        pending: Arc<RwLock<Option<(u8, Epoch)>>>,
    ) {
        let mut rx = transport.subscribe(Kind::Control);
        tokio::spawn(async move {
            while let Some((_meta, payload)) = rx.recv().await {
                match postcard::from_bytes::<ControlPayload>(&payload) {
                    Ok(ControlPayload::ChannelSwitch { next_channel, at_epoch }) => {
                        tracing::info!(
                            channel = next_channel,
                            at_epoch,
                            "channel switch scheduled from peer"
                        );
                        *pending.write().unwrap() = Some((next_channel, at_epoch));
                    }
                    Err(e) => {
                        tracing::debug!("unknown control payload: {}", e);
                    }
                }
            }
        });
    }

    fn spawn_channel_switch_watcher(
        transport: Arc<dyn Transport>,
        pending: Arc<RwLock<Option<(u8, Epoch)>>>,
        current_channel: Arc<AtomicU8>,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(200));
            loop {
                interval.tick().await;
                let snapshot = *pending.read().unwrap();
                if let Some((ch, at_epoch)) = snapshot {
                    if current_epoch() >= at_epoch {
                        if let Err(e) = transport.set_channel(ch) {
                            tracing::error!(channel = ch, "set_channel failed: {}", e);
                        } else {
                            current_channel.store(ch, Ordering::Relaxed);
                            tracing::info!(channel = ch, "coordinated channel switch applied");
                        }
                        *pending.write().unwrap() = None;
                    }
                }
            }
        });
    }

    fn spawn_flooding_task(transport: Arc<dyn Transport>, kind: Kind) {
        let seen_hashes: Arc<RwLock<HashMap<u64, (u32, Instant)>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let hashes_for_cleanup = Arc::clone(&seen_hashes);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                hashes_for_cleanup
                    .write()
                    .unwrap()
                    .retain(|_, (_, ts)| ts.elapsed() < Duration::from_secs(10));
            }
        });

        let mut rx = transport.subscribe(kind);
        let t_transport = Arc::clone(&transport);
        let hashes = Arc::clone(&seen_hashes);

        tokio::spawn(async move {
            while let Some((meta, payload)) = rx.recv().await {
                let Some(hash) = flood_key(kind, &payload) else { continue; };
                let origin_id = meta.origin_id;

                let is_new = {
                    let mut lock = hashes.write().unwrap();
                    let entry = lock.entry(hash).or_insert((0, Instant::now()));
                    entry.0 += 1;
                    entry.1 = Instant::now();
                    entry.0 == 1
                };

                if is_new {
                    let payload_clone = payload.clone();
                    let t_transport_clone = Arc::clone(&t_transport);
                    let hashes_clone = Arc::clone(&seen_hashes);

                    tokio::spawn(async move {
                        let delay = rand::thread_rng().gen_range(0..=50);
                        tokio::time::sleep(Duration::from_millis(delay)).await;

                        let should_forward = {
                            let lock = hashes_clone.read().unwrap();
                            lock.get(&hash).map(|(c, _)| *c).unwrap_or(0) < 2
                        };

                        if should_forward {
                            // Pre-mark as already forwarded so the echo of our own
                            // re-broadcast doesn't trigger another forward cycle.
                            hashes_clone
                                .write()
                                .unwrap()
                                .entry(hash)
                                .and_modify(|e| e.0 = 2);
                            let _ = t_transport_clone.broadcast_forwarded(kind, &payload_clone, origin_id);
                        }
                    });
                }
            }
        });
    }
}

/// Extract a u64 dedup key for the flooding seen-set.
/// For Fec: encodes (object_id, esi) — the two fields that uniquely identify a symbol.
/// For everything else: first 8 bytes of BLAKE3(payload).
fn flood_key(kind: Kind, payload: &[u8]) -> Option<u64> {
    match kind {
        Kind::Fec => {
            // FEC frame layout: object_id(4) | block(1) | oti(12) | esi(4) | …
            if payload.len() < 21 {
                return None;
            }
            let oid = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as u64;
            let esi = u32::from_be_bytes(payload[17..21].try_into().unwrap()) as u64;
            Some((oid << 32) | esi)
        }
        _ => {
            let h = blake3::hash(payload);
            Some(u64::from_le_bytes(h.as_bytes()[0..8].try_into().unwrap()))
        }
    }
}

fn current_epoch() -> Epoch {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 60) as Epoch
}

impl Mesh for MeshService {
    fn neighbors(&self) -> Vec<NeighborInfo> {
        let table = self.neighbor_table.read().unwrap();
        table
            .iter()
            .map(|(id, state)| NeighborInfo {
                id: *id,
                prr: state.prr,
                rssi_dbm: state.rssi_dbm,
                last_seen: state.last_seen,
                bitmap: state.bitmap,
            })
            .collect()
    }

    /// Returns the full link-state graph as known from received beacons.
    /// `topology()[node_id]` = list of (neighbor_id, prr) that `node_id` reported hearing.
    fn topology(&self) -> std::collections::HashMap<NodeId, Vec<(NodeId, f32)>> {
        self.link_state
            .read()
            .unwrap()
            .iter()
            .map(|(src, links)| {
                let converted = links
                    .iter()
                    .map(|(dst, prr_byte)| (*dst, *prr_byte as f32 / 255.0))
                    .collect();
                (*src, converted)
            })
            .collect()
    }

    fn request_channel_hop(&self, next_channel: u8) -> litm_common::Result<()> {
        let at_epoch = current_epoch() + 1;
        // Schedule locally — we won't receive our own broadcast back.
        *self.pending_channel_switch.write().unwrap() = Some((next_channel, at_epoch));
        tracing::info!(channel = next_channel, at_epoch, "broadcasting channel switch");
        let payload = postcard::to_allocvec(&ControlPayload::ChannelSwitch { next_channel, at_epoch })
            .map_err(|e| litm_common::Error::Other(e.to_string()))?;
        self.transport.broadcast(Kind::Control, &payload)
    }

    fn current_channel(&self) -> u8 {
        self.current_channel.load(Ordering::Relaxed)
    }
}

/// Pure PRR calculation logic, extracted for unit testing.
/// Returns (new_prr, next_expected_seq).
/// Returns None for next_expected when the packet should be skipped (minor backward seq).
#[cfg(test)]
fn compute_prr(
    current_prr: f32,
    expected_seq: Option<u32>,
    received_seq: u32,
    rssi_dbm: i8,
) -> (f32, Option<u32>) {
    match expected_seq {
        None => {
            let inst_prr = rssi_to_prr(rssi_dbm).unwrap_or(0.5);
            let new_prr = (current_prr * 0.3 + inst_prr * 0.7).clamp(0.0, 1.0);
            (new_prr, Some(received_seq.wrapping_add(1)))
        }
        Some(expected) => {
            if received_seq >= expected {
                let gap = (received_seq - expected + 1) as f32;
                let delivery_prr = 1.0 / gap;
                let inst_prr = match rssi_to_prr(rssi_dbm) {
                    Some(rssi_prr) => f32::min(rssi_prr, delivery_prr),
                    None => delivery_prr,
                };
                let new_prr = (current_prr * 0.3 + inst_prr * 0.7).clamp(0.0, 1.0);
                (new_prr, Some(received_seq.wrapping_add(1)))
            } else {
                let backward = expected.wrapping_sub(received_seq);
                if backward > 1000 {
                    (0.5, None)
                } else {
                    (current_prr, Some(expected))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -50 dBm maps to rssi_prr=1.0, so min(1.0, delivery_prr)=delivery_prr.
    // Using this as the default rssi in tests keeps the delivery-path assertions unchanged.
    const GOOD_RSSI: i8 = -50;

    #[test]
    fn test_prr_first_beacon() {
        // First beacon from neutral prior (0.5): RSSI-only path since no seq history.
        let (prr, next) = compute_prr(0.5, None, 0, GOOD_RSSI);
        let expected_prr = 0.5f32 * 0.3 + 1.0f32 * 0.7; // rssi_prr=1.0 for -50 dBm
        assert!((prr - expected_prr).abs() < 1e-6, "prr={}", prr);
        assert_eq!(next, Some(1));
    }

    #[test]
    fn test_prr_consecutive_no_loss() {
        // Receive seq 1 when expected 1 → gap=1 → delivery_prr=1.0; good RSSI → min=1.0
        let (prr, next) = compute_prr(1.0, Some(1), 1, GOOD_RSSI);
        assert!((prr - 1.0).abs() < 1e-6, "prr={}", prr);
        assert_eq!(next, Some(2));
    }

    #[test]
    fn test_prr_gap_of_two() {
        // Expected seq=5, received seq=6 → gap=2 → delivery_prr=0.5; good RSSI → min=0.5
        let (prr, next) = compute_prr(1.0, Some(5), 6, GOOD_RSSI);
        let expected_prr = 1.0f32 * 0.3 + 0.5f32 * 0.7;
        assert!((prr - expected_prr).abs() < 1e-6, "prr={}", prr);
        assert_eq!(next, Some(7));
    }

    #[test]
    fn test_prr_rssi_caps_delivery() {
        // Good delivery (gap=1) but weak RSSI (-80 dBm → rssi_prr=0.25).
        // min(0.25, 1.0) = 0.25 → RSSI caps the reported PRR.
        let rssi: i8 = -80;
        let rssi_prr = rssi_to_prr(rssi).unwrap(); // (−80 + 90) / 40 = 0.25
        let (prr, next) = compute_prr(0.5, Some(0), 0, rssi);
        let expected_prr = 0.5f32 * 0.3 + rssi_prr * 0.7;
        assert!((prr - expected_prr).abs() < 1e-5, "prr={}", prr);
        assert_eq!(next, Some(1));
    }

    #[test]
    fn test_prr_small_backward_seq_is_skipped() {
        // Small backward movement (reorder/late packet) — PRR and expected unchanged.
        let (prr, next) = compute_prr(0.8, Some(10), 9, GOOD_RSSI);
        assert!(
            (prr - 0.8).abs() < 1e-6,
            "prr should be unchanged, got {}",
            prr
        );
        assert_eq!(next, Some(10), "expected should be unchanged");
    }

    #[test]
    fn test_prr_large_backward_seq_is_restart() {
        // Large backward movement — genuine restart, PRR resets to neutral.
        let (prr, next) = compute_prr(0.9, Some(2000), 5, GOOD_RSSI);
        assert!(
            (prr - 0.5).abs() < 1e-6,
            "prr should reset to 0.5, got {}",
            prr
        );
        assert_eq!(next, None, "next_expected should be None on restart");
    }

    #[test]
    fn test_prr_degrades_over_gaps() {
        // Simulate losing every other beacon: seq 0, 2, 4, ...
        let mut prr = 0.5f32;
        let mut expected: Option<u32> = None;
        for i in 0..20u32 {
            let seq = i * 2; // skip odd sequence numbers
            let (new_prr, next) = compute_prr(prr, expected, seq, GOOD_RSSI);
            prr = new_prr;
            expected = next;
        }
        // Losing half the beacons should stabilise PRR near 0.5, well below 0.8
        assert!(prr < 0.8, "prr={} should degrade with 50% loss", prr);
    }

    #[test]
    fn test_prr_recovers_quickly() {
        // Start degraded, then receive perfect beacons — should recover within a few steps.
        let mut prr = 0.1f32;
        let mut expected: Option<u32> = Some(0);
        for i in 0..5u32 {
            let (new_prr, next) = compute_prr(prr, expected, i, GOOD_RSSI);
            prr = new_prr;
            expected = next;
        }
        // After 5 consecutive perfect beacons from degraded state, should be > 0.9
        assert!(
            prr > 0.9,
            "prr={} should recover quickly with perfect beacons",
            prr
        );
    }
}
