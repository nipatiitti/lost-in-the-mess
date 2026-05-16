use std::sync::{Arc, Mutex};
use std::time::Instant;
use transport::{PacketMeta, Transport};
use rand::{Rng, RngExt};

pub struct MockNetwork {
    drop_rate: f64,
    handlers: Arc<Mutex<Vec<Arc<dyn Fn(PacketMeta, &[u8]) + Send + Sync>>>>,
}

impl MockNetwork {
    pub fn new(drop_rate: f64) -> Self {
        Self {
            drop_rate,
            handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn create_transport(&self, id: u32) -> MockTransport {
        MockTransport {
            id,
            drop_rate: self.drop_rate,
            network_handlers: self.handlers.clone(),
        }
    }
}

pub struct MockTransport {
    pub id: u32,
    pub drop_rate: f64,
    pub network_handlers: Arc<Mutex<Vec<Arc<dyn Fn(PacketMeta, &[u8]) + Send + Sync>>>>,
}

impl Transport for MockTransport {
    fn broadcast(&self, payload: &[u8]) -> anyhow::Result<()> {
        let mut rng = rand::rng();
        let handlers: Vec<_> = self.network_handlers.lock().unwrap().iter().cloned().collect();
        
        for handler in handlers {
            if rng.random_bool(self.drop_rate) {
                continue; // Drop packet
            }
            
            let meta = PacketMeta {
                sender_id: self.id,
                rssi_dbm: -50,
                recv_time: Instant::now(),
            };
            
            let payload_copy = payload.to_vec();
            std::thread::spawn(move || {
                handler(meta, &payload_copy);
            });
        }
        
        Ok(())
    }

    fn subscribe(&self, handler: Arc<dyn Fn(PacketMeta, &[u8]) + Send + Sync>) {
        self.network_handlers.lock().unwrap().push(handler);
    }
}
