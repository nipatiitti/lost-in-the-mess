//! Shared contract between transport, delivery, mesh, and app.
//! No business logic lives here — only the types and traits at the seams.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

// ---------- Identifiers ----------

pub type NodeId = u32;
pub type ObjectId = u32;
pub type Epoch = u32;

// ---------- Wire / sizing constants ----------

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_PLAINTEXT: usize = 1400; // budget after radiotap + AEAD overhead
pub const BITMAP_WORDS: usize = 4; // 4 * u64 = 256 object_id slots

// ---------- Error ----------

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("transport io: {0}")]
    Io(String),
    #[error("auth failed")]
    AuthFailed,
    #[error("replay detected")]
    Replay,
    #[error("bad frame: {0}")]
    BadFrame(&'static str),
    #[error("decode failed")]
    DecodeFailed,
    #[error("backpressure")]
    Backpressure,
    #[error("other: {0}")]
    Other(String),
}
pub type Result<T> = std::result::Result<T, Error>;

// ---------- Packet kinds ----------
// One byte inside the encrypted envelope. Transport adds/strips it transparently.

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Kind {
    Beacon = 0,  // mesh -> mesh
    Fec = 1,     // delivery -> delivery
    Control = 2, // mesh -> mesh (channel hop, rekey, eviction)
    Video = 3,   // unreliable video bypass — no FEC, no ACK, fire-and-forget
}

// ---------- Control message payload ----------
// Postcard-serialized, lives inside a Kind::Control encrypted envelope.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ControlPayload {
    /// Coordinated channel switch: all nodes apply at `at_epoch` boundary.
    ChannelSwitch { next_channel: u8, at_epoch: Epoch },
}

// ---------- Per-packet metadata ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketMeta {
    /// Last-hop forwarder (or the original sender if no hops).
    pub sender_id: NodeId,
    /// Node that first created this packet; preserved through all flood hops.
    pub origin_id: NodeId,
    pub counter: u64,
    pub rssi_dbm: i8,
    #[serde(skip, default = "Instant::now")]
    pub recv_time: Instant,
}

// ---------- Object-id bitmap (piggybacked ACKs in beacons) ----------
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObjectBitmap(pub [u64; BITMAP_WORDS]);

impl ObjectBitmap {
    #[inline]
    pub fn set(&mut self, id: ObjectId) {
        let b = (id as usize) % (BITMAP_WORDS * 64);
        self.0[b / 64] |= 1u64 << (b & 63);
    }
    #[inline]
    pub fn contains(&self, id: ObjectId) -> bool {
        let b = (id as usize) % (BITMAP_WORDS * 64);
        (self.0[b / 64] >> (b & 63)) & 1 == 1
    }
}

// ---------- Send-side policy ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SendPolicy {
    pub desired_coverage: u8, // stop once this many peers' beacons confirm
    pub ttl: Duration,
    pub priority: u8, // higher = sent first when queued
}

impl Default for SendPolicy {
    fn default() -> Self {
        Self {
            desired_coverage: 1,
            ttl: Duration::from_secs(30),
            priority: 128,
        }
    }
}

// ---------- Delivered output ----------

#[derive(Clone, Debug)]
pub struct DeliveredObject {
    pub id: ObjectId,
    pub source: NodeId,
    pub payload: Vec<u8>,
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum RaptorEvent {
    PacketReceived { id: u32, is_repair: bool, source_block: u32 },
    DecoderStatus { progress: f32, overhead_symbols: u32 },
    MatrixState { rows: usize, cols: usize, density: f32 },
    DecodingSuccess,
    DecodingFailed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeighborInfo {
    pub id: NodeId,
    pub prr: f32,      // 0.0..=1.0, blended RSSI + delivery ratio
    pub rssi_dbm: i8,  // smoothed RSSI from radio hardware
    #[serde(skip, default = "Instant::now")]
    pub last_seen: Instant,
    pub bitmap: ObjectBitmap,
}

// ---------- The three seams ----------

/// Person A (transport crate) implements this.
/// Authenticated, replay-protected, unreliable broadcast.
pub trait Transport: Send + Sync + 'static {
    fn local_id(&self) -> NodeId;
    /// Broadcast as this node (sets origin = local_id).
    fn broadcast(&self, kind: Kind, payload: &[u8]) -> Result<()>;
    /// Re-broadcast a flooded packet, preserving the original `origin`.
    fn broadcast_forwarded(&self, kind: Kind, payload: &[u8], origin: NodeId) -> Result<()>;
    fn subscribe(&self, kind: Kind) -> mpsc::Receiver<(PacketMeta, Vec<u8>)>;
    fn set_channel(&self, ch: u8) -> Result<()>;
    /// Read the currently configured channel of the physical radio (defaults to 6).
    fn get_channel(&self) -> u8 {
        6
    }
}

/// Person B (delivery crate) implements this.
/// Reliable object transfer via RaptorQ over a `Transport`.
pub trait Delivery: Send + Sync + 'static {
    fn send_object(&self, id: ObjectId, payload: Vec<u8>, policy: SendPolicy) -> Result<()>;
    fn subscribe(&self) -> mpsc::Receiver<DeliveredObject>;
    fn decoded_bitmap(&self) -> ObjectBitmap;
    fn note_peer_coverage(&self, peer: NodeId, bitmap: ObjectBitmap, prr: f32);
    fn subscribe_telemetry(&self) -> mpsc::Receiver<RaptorEvent> {
        mpsc::channel(1).1 // Default empty receiver
    }
}

/// Person C (mesh crate) implements this.
/// Neighbor discovery, link quality, and beacon-borne state distribution.
pub trait Mesh: Send + Sync + 'static {
    fn neighbors(&self) -> Vec<NeighborInfo>;
    fn topology(&self) -> std::collections::HashMap<NodeId, Vec<(NodeId, f32)>>;
    /// Broadcast a coordinated channel-switch; all nodes apply at `current_epoch + 1`.
    fn request_channel_hop(&self, next_channel: u8) -> Result<()>;
    /// Current radio channel (updated after each coordinated hop applies).
    fn current_channel(&self) -> u8;
}
