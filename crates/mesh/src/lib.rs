use litm_common::{
    Delivery, Kind, Mesh, NeighborInfo, NodeId, ObjectBitmap, Transport,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

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
    last_seen: Instant,
    bitmap: ObjectBitmap,
    expected_beacon_seq: Option<u32>,
}

pub struct MeshService {
    neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
    /// link_state[node_id] = that node's own neighbor list (from its most recent beacon).
    /// Enables building the full topology graph for Dijkstra routing.
    link_state: Arc<RwLock<HashMap<NodeId, Vec<(NodeId, u8)>>>>,
}

impl MeshService {
    pub fn new(transport: Arc<dyn Transport>, delivery: Arc<dyn Delivery>) -> Arc<Self> {
        let neighbor_table = Arc::new(RwLock::new(HashMap::new()));
        let link_state = Arc::new(RwLock::new(HashMap::new()));

        let service = Arc::new(Self {
            neighbor_table: Arc::clone(&neighbor_table),
            link_state: Arc::clone(&link_state),
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
        Self::spawn_neighbor_eviction(
            Arc::clone(&neighbor_table),
            Arc::clone(&link_state),
        );

        Self::spawn_flooding_task(Arc::clone(&transport), Kind::Fec);
        Self::spawn_flooding_task(Arc::clone(&transport), Kind::Control);

        service
    }

    /// Returns the full link-state graph as known from received beacons.
    /// `topology()[node_id]` = list of (neighbor_id, prr) that `node_id` reported hearing.
    pub fn topology(&self) -> HashMap<NodeId, Vec<(NodeId, f32)>> {
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

    fn spawn_beacon_sender(
        transport: Arc<dyn Transport>,
        delivery: Arc<dyn Delivery>,
        neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            let mut next_beacon = Instant::now() + Duration::from_millis(500);
            let mut beacon_seq: u32 = 0;

            loop {
                interval.tick().await;
                if Instant::now() >= next_beacon {
                    // TODO: use transport epoch when the Transport trait exposes it
                    let epoch = 0u32;

                    let neighbors_heard = {
                        let table = neighbor_table.read().unwrap();
                        table
                            .iter()
                            .map(|(id, state)| {
                                (*id, (state.prr * 255.0).clamp(0.0, 255.0) as u8)
                            })
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
                    let jitter = rand::thread_rng().gen_range(450..=550);
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
                        let state = table.entry(meta.sender_id).or_insert(NeighborState {
                            prr: 1.0,
                            last_seen: Instant::now(),
                            bitmap: beacon.decoded,
                            expected_beacon_seq: None,
                        });

                        state.last_seen = Instant::now();
                        state.bitmap = beacon.decoded;

                        match state.expected_beacon_seq {
                            None => {
                                state.prr = 1.0;
                            }
                            Some(expected) => {
                                if beacon.beacon_seq >= expected {
                                    let gap = (beacon.beacon_seq - expected + 1) as f32;
                                    let inst_prr = 1.0 / gap;
                                    state.prr =
                                        (state.prr * 0.8 + inst_prr * 0.2).clamp(0.0, 1.0);
                                } else {
                                    // Sequence reset (peer restarted) — re-initialise
                                    state.prr = 1.0;
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

                    delivery.note_peer_coverage(meta.sender_id, beacon.decoded);
                }
            }
        });
    }

    fn spawn_neighbor_eviction(
        neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
        link_state: Arc<RwLock<HashMap<NodeId, Vec<(NodeId, u8)>>>>,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                let now = Instant::now();
                let mut table = neighbor_table.write().unwrap();
                table.retain(|_, state| {
                    // 4 × max-jitter beacon interval (4 × 550 ms) so jitter alone cannot
                    // trigger spurious eviction
                    now.duration_since(state.last_seen) <= Duration::from_millis(2200)
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

    fn spawn_flooding_task(transport: Arc<dyn Transport>, kind: Kind) {
        let seen_hashes: Arc<RwLock<HashMap<[u8; 32], (u32, Instant)>>> =
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
            while let Some((_meta, payload)) = rx.recv().await {
                let hash: [u8; 32] = blake3::hash(&payload).into();

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

                        let count = hashes_clone
                            .read()
                            .unwrap()
                            .get(&hash)
                            .map(|(c, _)| *c)
                            .unwrap_or(0);

                        if count < 2 {
                            let _ = t_transport_clone.broadcast(kind, &payload_clone);
                        }
                    });
                }
            }
        });
    }
}

impl Mesh for MeshService {
    fn neighbors(&self) -> Vec<NeighborInfo> {
        let table = self.neighbor_table.read().unwrap();
        table
            .iter()
            .map(|(id, state)| NeighborInfo {
                id: *id,
                prr: state.prr,
                last_seen: state.last_seen,
                bitmap: state.bitmap,
            })
            .collect()
    }
}

/// Pure PRR calculation logic, extracted for unit testing.
#[cfg(test)]
fn compute_prr(current_prr: f32, expected_seq: Option<u32>, received_seq: u32) -> (f32, u32) {
    let new_prr = match expected_seq {
        None => 1.0,
        Some(expected) => {
            if received_seq >= expected {
                let gap = (received_seq - expected + 1) as f32;
                let inst_prr = 1.0 / gap;
                (current_prr * 0.8 + inst_prr * 0.2).clamp(0.0, 1.0)
            } else {
                // Sequence reset (peer restarted)
                1.0
            }
        }
    };
    let next_expected = received_seq.wrapping_add(1);
    (new_prr, next_expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prr_first_beacon() {
        let (prr, next) = compute_prr(0.0, None, 0);
        assert_eq!(prr, 1.0);
        assert_eq!(next, 1);
    }

    #[test]
    fn test_prr_consecutive_no_loss() {
        // Receive seq 1 when expected 1 → gap=1 → inst_prr=1.0
        let (prr, next) = compute_prr(1.0, Some(1), 1);
        assert!((prr - 1.0).abs() < 1e-6, "prr={}", prr);
        assert_eq!(next, 2);
    }

    #[test]
    fn test_prr_gap_of_two() {
        // Expected seq=5, received seq=6 → one packet dropped → gap=2 → inst_prr=0.5
        let (prr, next) = compute_prr(1.0, Some(5), 6);
        let expected_prr = 1.0f32 * 0.8 + 0.5f32 * 0.2;
        assert!((prr - expected_prr).abs() < 1e-6, "prr={}", prr);
        assert_eq!(next, 7);
    }

    #[test]
    fn test_prr_sequence_reset() {
        // Peer restarted: expected=10, received=2 → reset to 1.0
        let (prr, next) = compute_prr(0.5, Some(10), 2);
        assert_eq!(prr, 1.0);
        assert_eq!(next, 3);
    }

    #[test]
    fn test_prr_degrades_over_gaps() {
        // Simulate losing every other beacon: seq 0, 2, 4, ...
        let mut prr = 1.0f32;
        let mut expected = Some(0u32);
        for i in 0..20u32 {
            let seq = i * 2; // skip odd sequence numbers
            let (new_prr, next) = compute_prr(prr, expected, seq);
            prr = new_prr;
            expected = Some(next);
        }
        // Losing half the beacons should bring PRR well below 0.8
        assert!(prr < 0.8, "prr={} should degrade with 50% loss", prr);
    }
}
