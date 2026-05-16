use std::sync::Arc;
use std::time::Instant;

pub struct PacketMeta {
    pub sender_id: u32,
    pub rssi_dbm: i8,
    pub recv_time: Instant,
}

pub trait Transport: Send + Sync {
    fn broadcast(&self, payload: &[u8]) -> anyhow::Result<()>;
    fn subscribe(&self, handler: Arc<dyn Fn(PacketMeta, &[u8]) + Send + Sync>);
}
