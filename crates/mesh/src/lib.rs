use litm_common::{
    Delivery, Kind, Mesh, NeighborInfo, NodeId, ObjectBitmap, Transport,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BeaconPayload {
    pub epoch: u32,
    pub neighbors_heard: Vec<(NodeId, u8)>, // (NodeId, PRR * 255)
    pub decoded: ObjectBitmap,
}

#[derive(Clone, Debug)]
struct NeighborState {
    prr: f32,
    last_seen: Instant,
    bitmap: ObjectBitmap,
    expected_counter: Option<u64>,
}

pub struct MeshService {
    neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
}

impl MeshService {
    pub fn new(transport: Arc<dyn Transport>, delivery: Arc<dyn Delivery>) -> Arc<Self> {
        let neighbor_table = Arc::new(RwLock::new(HashMap::new()));

        let service = Arc::new(Self {
            neighbor_table: Arc::clone(&neighbor_table),
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
        );
        Self::spawn_neighbor_eviction(Arc::clone(&neighbor_table));

        Self::spawn_flooding_task(Arc::clone(&transport), Kind::Fec);
        Self::spawn_flooding_task(Arc::clone(&transport), Kind::Control);
        Self::spawn_channel_hopper(Arc::clone(&transport));

        service
    }

    fn spawn_beacon_sender(
        transport: Arc<dyn Transport>,
        delivery: Arc<dyn Delivery>,
        neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(50));
            let mut next_beacon = Instant::now() + Duration::from_millis(500);

            loop {
                interval.tick().await;
                if Instant::now() >= next_beacon {
                    let epoch = 0; // TODO: obtain real epoch if implemented

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
                        neighbors_heard,
                        decoded,
                    };

                    if let Ok(encoded) = postcard::to_allocvec(&payload) {
                        let _ = transport.broadcast(Kind::Beacon, &encoded);
                    }

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
    ) {
        let mut rx = transport.subscribe(Kind::Beacon);
        tokio::spawn(async move {
            while let Some((meta, payload)) = rx.recv().await {
                if let Ok(beacon) = postcard::from_bytes::<BeaconPayload>(&payload) {
                    let mut table = neighbor_table.write().unwrap();
                    let state = table.entry(meta.sender_id).or_insert(NeighborState {
                        prr: 0.0,
                        last_seen: Instant::now(),
                        bitmap: beacon.decoded,
                        expected_counter: None,
                    });

                    state.last_seen = Instant::now();
                    state.bitmap = beacon.decoded;

                    if let Some(expected) = state.expected_counter {
                        if meta.counter >= expected {
                            let total = (meta.counter - expected + 1) as f32;
                            let inst_prr = 1.0 / total;
                            state.prr = state.prr * 0.8 + inst_prr * 0.2;
                        }
                    } else {
                        state.prr = 1.0;
                    }
                    state.expected_counter = Some(meta.counter + 1);

                    delivery.note_peer_coverage(meta.sender_id, beacon.decoded);
                }
            }
        });
    }

    fn spawn_neighbor_eviction(neighbor_table: Arc<RwLock<HashMap<NodeId, NeighborState>>>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                let now = Instant::now();
                let mut table = neighbor_table.write().unwrap();
                table.retain(|_, state| {
                    now.duration_since(state.last_seen) <= Duration::from_millis(1500)
                });
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
                    let hashes_clone = Arc::clone(&hashes);

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

    fn spawn_channel_hopper(transport: Arc<dyn Transport>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(200));
            let root_key = b"hardcoded-root-key-for-demo-----"; // 32 bytes
            let available_channels = vec![1, 6, 11, 36, 40, 44, 48]; // standard 2.4/5GHz channels

            loop {
                interval.tick().await;
                
                let epoch = 0u32;
                let slot = (std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::default())
                    .as_millis() / 200) as u32;

                use hkdf::Hkdf;
                use sha2::Sha256;

                let hkdf = Hkdf::<Sha256>::new(None, root_key);
                let mut info = Vec::new();
                info.extend_from_slice(b"hop");
                info.extend_from_slice(&epoch.to_le_bytes());
                info.extend_from_slice(&slot.to_le_bytes());

                let mut okm = [0u8; 4];
                if hkdf.expand(&info, &mut okm).is_ok() {
                    let rand_val = u32::from_le_bytes(okm);
                    let ch_idx = (rand_val as usize) % available_channels.len();
                    let ch = available_channels[ch_idx];
                    let _ = transport.set_channel(ch);
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
