pub mod frame;
pub mod mock;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation, PayloadId};

use crate::frame::FecFrame;
use transport::{PacketMeta, Transport};

pub struct SendPolicy {
    pub desired_coverage: u8,
    pub ttl: Duration,
    pub priority: u8,
}

pub type ObjectBitmap = [u64; 4]; // last 256 object_ids

pub trait ReliableBroadcast: Send + Sync {
    fn send_object(&self, id: u32, payload: Vec<u8>, policy: SendPolicy);
    fn on_complete(&self, handler: Arc<dyn Fn(u32, Vec<u8>) + Send + Sync>);
    fn decoded_bitmap(&self) -> ObjectBitmap;
    fn note_peer_coverage(&self, peer_id: u32, bitmap: &ObjectBitmap);
}

pub struct RaptorQDelivery<T: Transport> {
    transport: Arc<T>,
    on_complete_handler: Arc<Mutex<Option<Arc<dyn Fn(u32, Vec<u8>) + Send + Sync>>>>,
    decoders: Arc<Mutex<HashMap<u32, Decoder>>>,
    completed_objects: Arc<Mutex<Vec<u32>>>,
}

impl<T: Transport + 'static> RaptorQDelivery<T> {
    pub fn new(transport: Arc<T>) -> Arc<Self> {
        let delivery = Arc::new(Self {
            transport: transport.clone(),
            on_complete_handler: Arc::new(Mutex::new(None)),
            decoders: Arc::new(Mutex::new(HashMap::new())),
            completed_objects: Arc::new(Mutex::new(Vec::new())),
        });

        let delivery_clone = delivery.clone();
        transport.subscribe(Arc::new(move |_meta, payload| {
            delivery_clone.handle_packet(payload);
        }));

        delivery
    }

    fn handle_packet(&self, payload: &[u8]) {
        if let Some(frame) = FecFrame::decode(payload) {
            let mut completed = self.completed_objects.lock().unwrap();
            if completed.contains(&frame.object_id) {
                return;
            }

            let mut decoders = self.decoders.lock().unwrap();
            let decoder = decoders.entry(frame.object_id).or_insert_with(|| {
                Decoder::new(ObjectTransmissionInformation::with_defaults(
                    frame.oti as u64,
                    frame.sym_sz,
                ))
            });

            let payload_id = PayloadId::new(0, frame.esi);
            let packet = EncodingPacket::new(payload_id, frame.payload);

            if let Some(decoded_payload) = decoder.decode(packet) {
                completed.push(frame.object_id);
                if let Some(handler) = self.on_complete_handler.lock().unwrap().as_ref() {
                    handler(frame.object_id, decoded_payload);
                }
            }
        }
    }
}

impl<T: Transport + 'static> ReliableBroadcast for RaptorQDelivery<T> {
    fn send_object(&self, id: u32, payload: Vec<u8>, _policy: SendPolicy) {
        let transport = self.transport.clone();
        let payload_len = payload.len() as u64;

        // Spawn a thread to send the object
        std::thread::spawn(move || {
            let symbol_size = 1024;
            let encoder = Encoder::with_defaults(&payload, symbol_size);
            let oti = encoder.get_config().transfer_length();

            // For a basic MVP, we just send all source symbols and some repair symbols.
            let symbols_count = (payload_len as f64 / symbol_size as f64).ceil() as u32;
            let target_symbols = (symbols_count as f64 * 1.5).ceil() as u32; // 50% overhead for 20% drop

            let repair_symbols = target_symbols.saturating_sub(symbols_count);
            let packets = encoder.get_encoded_packets(repair_symbols);

            for packet in packets {
                let esi = packet.payload_id().encoding_symbol_id();
                let frame = FecFrame {
                    object_id: id,
                    oti: payload_len,
                    esi,
                    sym_sz: symbol_size,
                    payload: packet.data().to_vec(),
                };

                let encoded_frame = frame.encode();
                let _ = transport.broadcast(&encoded_frame);

                // Add a small delay to simulate network pacing and prevent flooding
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
    }

    fn on_complete(&self, handler: Arc<dyn Fn(u32, Vec<u8>) + Send + Sync>) {
        *self.on_complete_handler.lock().unwrap() = Some(handler);
    }

    fn decoded_bitmap(&self) -> ObjectBitmap {
        [0; 4] // Mock implementation
    }

    fn note_peer_coverage(&self, _peer_id: u32, _bitmap: &ObjectBitmap) {
        // Mock implementation
    }
}
