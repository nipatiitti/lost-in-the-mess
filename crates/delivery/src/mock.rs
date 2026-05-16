use litm_common::{Kind, NodeId, PacketMeta, Result, Transport};
use rand::RngExt;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

pub struct MockNetwork {
    drop_rate: f64,
    handlers: Arc<Mutex<Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>>>,
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
    pub network_handlers: Arc<Mutex<Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>>>,
}

impl Transport for MockTransport {
    fn local_id(&self) -> NodeId {
        self.id
    }

    fn broadcast(&self, _kind: Kind, payload: &[u8]) -> Result<()> {
        let mut rng = rand::rng();
        let handlers: Vec<_> = self
            .network_handlers
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect();

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
            let _ = handler.try_send((meta, payload_copy));
        }

        Ok(())
    }

    fn subscribe(&self, _kind: Kind) -> mpsc::Receiver<(PacketMeta, Vec<u8>)> {
        let (tx, rx) = mpsc::channel(100);
        self.network_handlers.lock().unwrap().push(tx);
        rx
    }
}
