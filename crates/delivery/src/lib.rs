pub mod frame;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation, PayloadId};

use crate::frame::FecFrame;
use litm_common::{
    DeliveredObject, Delivery, Kind, NodeId, ObjectBitmap, ObjectId, Result, SendPolicy, Transport,
};

pub struct RaptorQDelivery<T: Transport + ?Sized> {
    transport: Arc<T>,
    subscribers: Arc<Mutex<Vec<mpsc::Sender<DeliveredObject>>>>,
    decoders: Arc<Mutex<HashMap<ObjectId, Decoder>>>,
    completed_objects: Arc<Mutex<Vec<ObjectId>>>,
    peer_coverage: Arc<Mutex<HashMap<NodeId, ObjectBitmap>>>,
    local_bitmap: Arc<Mutex<ObjectBitmap>>,
}

impl<T: Transport + 'static + ?Sized> RaptorQDelivery<T> {
    pub fn new(transport: Arc<T>) -> Arc<Self> {
        let delivery = Arc::new(Self {
            transport: transport.clone(),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            decoders: Arc::new(Mutex::new(HashMap::new())),
            completed_objects: Arc::new(Mutex::new(Vec::new())),
            peer_coverage: Arc::new(Mutex::new(HashMap::new())),
            local_bitmap: Arc::new(Mutex::new(ObjectBitmap::default())),
        });

        let mut rx = transport.subscribe(Kind::Fec);
        let delivery_clone = delivery.clone();

        tokio::spawn(async move {
            while let Some((meta, payload)) = rx.recv().await {
                delivery_clone.handle_packet(meta.sender_id, &payload).await;
            }
        });

        delivery
    }

    async fn handle_packet(&self, source: NodeId, payload: &[u8]) {
        if let Some(frame) = FecFrame::decode(payload) {
            let mut completed = self.completed_objects.lock().unwrap();
            if completed.contains(&frame.object_id) {
                return;
            }

            let mut decoders = self.decoders.lock().unwrap();
            let decoder = decoders.entry(frame.object_id).or_insert_with(|| {
                let mut tl_bytes = [0u8; 8];
                tl_bytes.copy_from_slice(&frame.oti[0..8]);
                let transfer_length = u64::from_be_bytes(tl_bytes);
                Decoder::new(ObjectTransmissionInformation::with_defaults(
                    transfer_length,
                    frame.sym_sz,
                ))
            });

            let payload_id = PayloadId::new(0, frame.esi);
            let packet = EncodingPacket::new(payload_id, frame.payload);

            if let Some(decoded_payload) = decoder.decode(packet) {
                completed.push(frame.object_id);
                self.local_bitmap.lock().unwrap().set(frame.object_id);

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
        let peer_coverage = self.peer_coverage.clone();

        tokio::spawn(async move {
            let symbol_size = 1024;
            let encoder = Encoder::with_defaults(&payload, symbol_size);

            let k = (payload_len as f64 / symbol_size as f64).ceil() as u32;
            let target_symbols = (((k + 4) as f64 * 1.2) / 0.8).ceil() as u32;

            let packets = encoder.get_encoded_packets(target_symbols);

            let mut oti = [0u8; 12];
            oti[0..8].copy_from_slice(&payload_len.to_be_bytes());

            for packet in packets {
                let coverage_count = {
                    let coverage = peer_coverage.lock().unwrap();
                    coverage.values().filter(|b| b.contains(id)).count()
                };

                if coverage_count >= policy.desired_coverage as usize {
                    break;
                }

                let esi = packet.payload_id().encoding_symbol_id();
                let frame = FecFrame {
                    object_id: id,
                    oti,
                    esi,
                    sym_sz: symbol_size,
                    payload: packet.data().to_vec(),
                };

                let encoded_frame = frame.encode();
                let _ = transport.broadcast(Kind::Fec, &encoded_frame);

                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        Ok(())
    }

    fn subscribe(&self) -> mpsc::Receiver<DeliveredObject> {
        let (tx, rx) = mpsc::channel(100);
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    fn decoded_bitmap(&self) -> ObjectBitmap {
        *self.local_bitmap.lock().unwrap()
    }

    fn note_peer_coverage(&self, peer: NodeId, bitmap: ObjectBitmap) {
        self.peer_coverage.lock().unwrap().insert(peer, bitmap);
    }
}
