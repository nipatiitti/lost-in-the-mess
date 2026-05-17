pub mod frame;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation, PayloadId};

use crate::frame::FecFrame;
use litm_common::{
    DeliveredObject, Delivery, Kind, NodeId, ObjectBitmap, ObjectId, Result, SendPolicy, Transport, RaptorEvent
};

/// Bounded set of recently-completed object IDs. O(1) lookup, O(1) eviction.
struct CompletedSet {
    ids: HashSet<ObjectId>,
    order: VecDeque<ObjectId>,
    cap: usize,
}

impl CompletedSet {
    fn new(cap: usize) -> Self {
        Self { ids: HashSet::with_capacity(cap + 1), order: VecDeque::with_capacity(cap + 1), cap }
    }

    fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }

    fn insert(&mut self, id: ObjectId) {
        if self.ids.insert(id) {
            self.order.push_back(id);
            if self.order.len() > self.cap {
                if let Some(evicted) = self.order.pop_front() {
                    self.ids.remove(&evicted);
                }
            }
        }
    }
}

pub struct RaptorQDelivery<T: Transport + ?Sized> {
    transport: Arc<T>,
    subscribers: Arc<Mutex<Vec<mpsc::Sender<DeliveredObject>>>>,
    telemetry_subscribers: Arc<Mutex<Vec<mpsc::Sender<RaptorEvent>>>>,
    /// Value is (decoder, time of last packet received for this object, packets_received, k_symbols).
    decoders: Arc<Mutex<HashMap<ObjectId, (Arc<Mutex<Decoder>>, Instant, u32, u32)>>>,
    completed_objects: Arc<Mutex<CompletedSet>>,
    /// Exact object IDs confirmed decoded by each peer (from beacon recent lists).
    /// Used by send_object for accurate coverage counting — no bitmap hash collisions.
    peer_exact: Arc<Mutex<HashMap<NodeId, HashSet<ObjectId>>>>,
    peer_prr: Arc<Mutex<HashMap<NodeId, f32>>>,
    local_bitmap: Arc<Mutex<ObjectBitmap>>,
    /// Last RECENT_COMPLETED_CAP decoded object IDs, in insertion order.
    /// Broadcast in beacons so peers can perform exact-ID coverage checks.
    recent_completed: Arc<Mutex<VecDeque<ObjectId>>>,
}

/// Incomplete decoders older than this are dropped (no more packets expected after TTL expires).
/// Matches the default send policy TTL — a sender won't retransmit after that point.
const DECODER_STALE_SECS: u64 = 30;
/// How many recently-completed object IDs to remember to suppress re-delivery.
const COMPLETED_CAP: usize = 4096;
/// How many recently-decoded object IDs to keep for exact-ID coverage broadcasting.
const RECENT_COMPLETED_CAP: usize = 64;

impl<T: Transport + 'static + ?Sized> RaptorQDelivery<T> {
    pub fn new(transport: Arc<T>) -> Arc<Self> {
        let delivery = Arc::new(Self {
            transport: transport.clone(),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            telemetry_subscribers: Arc::new(Mutex::new(Vec::new())),
            decoders: Arc::new(Mutex::new(HashMap::new())),
            completed_objects: Arc::new(Mutex::new(CompletedSet::new(COMPLETED_CAP))),
            peer_exact: Arc::new(Mutex::new(HashMap::new())),
            peer_prr: Arc::new(Mutex::new(HashMap::new())),
            local_bitmap: Arc::new(Mutex::new(ObjectBitmap::default())),
            recent_completed: Arc::new(Mutex::new(VecDeque::with_capacity(RECENT_COMPLETED_CAP + 1))),
        });

        let mut rx = transport.subscribe(Kind::Fec);
        let delivery_clone = delivery.clone();
        tokio::spawn(async move {
            while let Some((meta, payload)) = rx.recv().await {
                let task_delivery = delivery_clone.clone();
                tokio::spawn(async move {
                    task_delivery.handle_packet(meta.origin_id, payload).await;
                });
            }
        });

        // Periodically evict incomplete decoders for objects we stopped receiving packets for.
        let decoders_ref = delivery.decoders.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(DECODER_STALE_SECS));
            loop {
                interval.tick().await;
                let cutoff = Instant::now() - Duration::from_secs(DECODER_STALE_SECS);
                decoders_ref.lock().unwrap().retain(|_, (_, last_seen, _, _)| *last_seen > cutoff);
            }
        });

        delivery
    }

    async fn handle_packet(self: Arc<Self>, source: NodeId, payload: Vec<u8>) {
        if let Some(frame) = FecFrame::decode(&payload) {
            {
                let completed = self.completed_objects.lock().unwrap();
                if completed.contains(frame.object_id) {
                    return;
                }
            }

            let (decoder_arc, k, emit_events) = {
                let mut decoders = self.decoders.lock().unwrap();
                let mut emit_events = Vec::new();

                let (decoder_arc, last_seen, packets_received, k) = decoders.entry(frame.object_id).or_insert_with(|| {
                    let mut tl_bytes = [0u8; 8];
                    tl_bytes.copy_from_slice(&frame.oti[0..8]);
                    let transfer_length = u64::from_be_bytes(tl_bytes);
                    let k = (transfer_length as f64 / frame.sym_sz as f64).ceil() as u32;
                    (
                        Arc::new(Mutex::new(Decoder::new(ObjectTransmissionInformation::with_defaults(
                            transfer_length,
                            frame.sym_sz,
                        )))),
                        Instant::now(),
                        0,
                        k,
                    )
                });
                *last_seen = Instant::now();
                *packets_received += 1;
                
                let is_repair = frame.esi >= *k;
                emit_events.push(RaptorEvent::PacketReceived {
                    id: frame.object_id,
                    is_repair,
                    source_block: frame.block as u32,
                });
                
                let progress = (*packets_received as f32 / *k as f32).min(1.0);
                let overhead = if *packets_received > *k { *packets_received - *k } else { 0 };
                
                emit_events.push(RaptorEvent::DecoderStatus {
                    progress,
                    overhead_symbols: overhead,
                });
                
                // Faked matrix density for visualization purposes
                emit_events.push(RaptorEvent::MatrixState {
                    rows: *k as usize,
                    cols: *k as usize,
                    density: progress, // Simplification for visual density
                });

                (decoder_arc.clone(), *k, emit_events)
            };

            // Emit collected events immediately so the UI doesn't block
            let mut telemetry = self.telemetry_subscribers.lock().unwrap();
            telemetry.retain(|tx| {
                let mut ok = true;
                for event in &emit_events {
                    if tx.try_send(event.clone()).is_err() {
                        ok = false;
                        break;
                    }
                }
                ok
            });
            drop(telemetry);

            let payload_id = PayloadId::new(frame.block, frame.esi);
            let packet = EncodingPacket::new(payload_id, frame.payload);

            let decoded_payload = tokio::task::spawn_blocking(move || {
                let mut decoder = decoder_arc.lock().unwrap();
                decoder.decode(packet)
            }).await.unwrap();

            if let Some(decoded_payload) = decoded_payload {
                {
                    let mut completed = self.completed_objects.lock().unwrap();
                    if completed.contains(frame.object_id) {
                        return; // Beaten by a concurrent decoder task
                    }
                    completed.insert(frame.object_id);
                }

                self.decoders.lock().unwrap().remove(&frame.object_id);
                
                let mut telemetry = self.telemetry_subscribers.lock().unwrap();
                telemetry.retain(|tx| tx.try_send(RaptorEvent::DecodingSuccess).is_ok());
                drop(telemetry);

                self.local_bitmap.lock().unwrap().set(frame.object_id);
                {
                    let mut recent = self.recent_completed.lock().unwrap();
                    if recent.back().copied() != Some(frame.object_id) {
                        recent.push_back(frame.object_id);
                        if recent.len() > RECENT_COMPLETED_CAP {
                            recent.pop_front();
                        }
                    }
                }

                let mut subscribers = self.subscribers.lock().unwrap();
                subscribers.retain(|tx| {
                    tx.try_send(DeliveredObject {
                        id: frame.object_id,
                        source,
                        payload: decoded_payload.clone(),
                    })
                    .is_ok()
                });
            }
        }
    }
}

impl<T: Transport + 'static + ?Sized> Delivery for RaptorQDelivery<T> {
    fn send_object(&self, id: ObjectId, payload: Vec<u8>, policy: SendPolicy) -> Result<()> {
        let transport = self.transport.clone();
        let payload_len = payload.len() as u64;
        let peer_exact = self.peer_exact.clone();
        let peer_prr = self.peer_prr.clone();

        tokio::spawn(async move {
            let symbol_size = 1374;
            let encoder = Encoder::with_defaults(&payload, symbol_size);

            let k = (payload_len as f64 / symbol_size as f64).ceil() as u32;
            let effective_prr = {
                let prr_map = peer_prr.lock().unwrap();
                if prr_map.is_empty() {
                    0.8_f32
                } else {
                    prr_map.values().cloned().fold(f32::INFINITY, f32::min).max(0.05)
                }
            };
            let target_symbols = (((k + 4) as f64 * 1.2) / effective_prr as f64).ceil() as u32;

            let packets = encoder.get_encoded_packets(target_symbols);

            let mut oti = [0u8; 12];
            oti[0..8].copy_from_slice(&payload_len.to_be_bytes());

            // Honour the send policy TTL: abort if we cannot achieve coverage in time.
            let _ = tokio::time::timeout(policy.ttl, async {
                let mut sent = 0u32;
                for packet in packets {
                    // Only check coverage after sending at least k source symbols —
                    // the receiver cannot attempt decoding with fewer than k packets.
                    if sent >= k {
                        let coverage_count = {
                            let exact = peer_exact.lock().unwrap();
                            exact.values().filter(|set| set.contains(&id)).count()
                        };

                        if coverage_count >= policy.desired_coverage as usize {
                            break;
                        }
                    }

                    let block = packet.payload_id().source_block_number();
                    let esi = packet.payload_id().encoding_symbol_id();
                    let frame = FecFrame {
                        object_id: id,
                        block,
                        oti,
                        esi,
                        sym_sz: symbol_size,
                        payload: packet.data().to_vec(),
                    };

                    let encoded_frame = frame.encode();
                    loop {
                        match transport.broadcast(Kind::Fec, &encoded_frame) {
                            Ok(()) => break,
                            Err(litm_common::Error::Backpressure) => {
                                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                            }
                            Err(_) => break,
                        }
                    }
                    sent += 1;
                }
            })
            .await;
        });

        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<DeliveredObject> {
        let (tx, rx) = mpsc::channel(512);
        self.subscribers.lock().unwrap().push(tx);
        rx
    }
    
    fn subscribe_telemetry(&self) -> mpsc::Receiver<RaptorEvent> {
        let (tx, rx) = mpsc::channel(512);
        self.telemetry_subscribers.lock().unwrap().push(tx);
        rx
    }

    fn decoded_bitmap(&self) -> ObjectBitmap {
        *self.local_bitmap.lock().unwrap()
    }

    fn decoded_recent(&self) -> Vec<ObjectId> {
        self.recent_completed.lock().unwrap().iter().copied().collect()
    }

    fn note_peer_coverage(&self, peer: NodeId, bitmap: ObjectBitmap, recent: Vec<ObjectId>, prr: f32) {
        let _ = bitmap; // retained in signature for telemetry callers; delivery uses exact IDs
        self.peer_exact
            .lock()
            .unwrap()
            .insert(peer, recent.into_iter().collect::<HashSet<ObjectId>>());
        self.peer_prr.lock().unwrap().insert(peer, prr);
    }
}
