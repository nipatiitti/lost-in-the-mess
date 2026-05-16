//! Authenticated, replay-protected, unreliable broadcast over kova-wfb-rs.

mod crypto;
mod frame;
mod radio;
mod replay;

pub fn derive_root_key(password: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(password.as_bytes()).into()
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tracing::{info, trace, warn};

use litm_common::{
    Epoch, Error, Kind, MAX_PLAINTEXT, NodeId, PROTOCOL_VERSION, PacketMeta, Result, Transport,
};

pub use radio::RadioConfig;

use crypto::KeyStore;
use frame::{HEADER_LEN, Header};
use radio::Radio;
use replay::ReplayDb;

pub const EPOCH_SECONDS: u64 = 60;
const SUBSCRIBER_QUEUE: usize = 1024;

pub struct WifiTransportConfig {
    pub local_id: NodeId,
    pub radio: RadioConfig,
    /// 32-byte pre-shared group root key.
    pub root_key: [u8; 32],
}

#[derive(Default)]
struct TransportStats {
    // Drop counters
    radio_queue_full: AtomicU64,
    subscriber_full: AtomicU64,
    decrypt_fail: AtomicU64,
    replay: AtomicU64,
    frame_gaps: AtomicU64,
    // Per-kind receive counters — lets us separate beacon loss from FEC/control loss
    rx_beacon: AtomicU64,
    rx_fec: AtomicU64,
    rx_control: AtomicU64,
    rx_video: AtomicU64,
}

pub struct WifiTransport {
    local_id: NodeId,
    keys: Arc<KeyStore>,
    replay: Arc<ReplayDb>,
    send_counter: AtomicU64,
    radio: Radio,
    subscribers: Arc<Mutex<Subscribers>>,
    stats: Arc<TransportStats>,
    /// Last seen counter per remote sender — for gap detection.
    last_counter: Arc<Mutex<HashMap<NodeId, u64>>>,
}

#[derive(Default)]
struct Subscribers {
    beacon: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
    fec: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
    control: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
    video: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
}

impl Subscribers {
    fn list_mut(&mut self, k: Kind) -> &mut Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>> {
        match k {
            Kind::Beacon => &mut self.beacon,
            Kind::Fec => &mut self.fec,
            Kind::Control => &mut self.control,
            Kind::Video => &mut self.video,
        }
    }
}

impl WifiTransport {
    pub fn start(cfg: WifiTransportConfig) -> Result<Arc<Self>> {
        let epoch = current_epoch();

        tracing::info!(epoch, "initializing transport");
        let keys = Arc::new(KeyStore::new(cfg.root_key, epoch));

        tracing::info!(epoch, "starting radio");
        let radio = Radio::start(cfg.radio)?;

        let stats = Arc::new(TransportStats::default());
        let me = Arc::new(Self {
            local_id: cfg.local_id,
            keys,
            replay: Arc::new(ReplayDb::new()),
            send_counter: AtomicU64::new(1),
            radio,
            subscribers: Arc::new(Mutex::new(Subscribers::default())),
            stats: stats.clone(),
            last_counter: Arc::new(Mutex::new(HashMap::new())),
        });

        // Epoch advance: every 60s, slide the key window.
        {
            let me2 = me.clone();
            tokio::spawn(async move {
                let mut iv = tokio::time::interval(std::time::Duration::from_secs(EPOCH_SECONDS));
                iv.tick().await; // immediate tick — skip
                loop {
                    iv.tick().await;
                    me2.keys.advance(current_epoch());
                }
            });
        }

        // Periodic stats report every 10 s.
        {
            tokio::spawn(async move {
                let mut iv = tokio::time::interval(std::time::Duration::from_secs(10));
                iv.tick().await; // skip immediate
                loop {
                    iv.tick().await;
                    let radio = stats.radio_queue_full.load(Ordering::Relaxed);
                    let sub = stats.subscriber_full.load(Ordering::Relaxed);
                    let decrypt = stats.decrypt_fail.load(Ordering::Relaxed);
                    let replay = stats.replay.load(Ordering::Relaxed);
                    let gaps = stats.frame_gaps.load(Ordering::Relaxed);
                    let rx_beacon = stats.rx_beacon.load(Ordering::Relaxed);
                    let rx_fec = stats.rx_fec.load(Ordering::Relaxed);
                    let rx_control = stats.rx_control.load(Ordering::Relaxed);
                    let rx_video = stats.rx_video.load(Ordering::Relaxed);
                    let level = if radio + sub + decrypt + replay + gaps > 0 { "WARN" } else { "INFO" };
                    if level == "WARN" {
                        warn!(
                            rx_beacon, rx_fec, rx_control, rx_video,
                            radio_queue_full = radio,
                            subscriber_full = sub,
                            decrypt_fail = decrypt,
                            replay = replay,
                            frame_gaps = gaps,
                            "transport stats (cumulative)"
                        );
                    } else {
                        info!(
                            rx_beacon, rx_fec, rx_control, rx_video,
                            "transport stats (cumulative, no drops)"
                        );
                    }
                }
            });
        }

        // RX decrypt + fanout: drain radio queue on a blocking-friendly task.
        {
            let me2 = me.clone();
            let rxq = me.radio.rx_queue.clone();
            tokio::task::spawn_blocking(move || {
                while let Ok(frame) = rxq.recv() {
                    if let Err(e) = me2.handle_rx_frame(&frame.bytes, frame.rssi_dbm) {
                        trace!(error = ?e, "drop rx frame");
                    }
                }
            });
        }

        Ok(me)
    }

    fn handle_rx_frame(&self, bytes: &[u8], rssi_dbm: i8) -> Result<()> {
        if bytes.len() < HEADER_LEN + 16 {
            return Err(Error::BadFrame("undersize frame"));
        }
        let header = Header::decode(&bytes[..HEADER_LEN])?;
        if header.sender == self.local_id {
            // ignore_self_injected should prevent this; defensive
            return Err(Error::BadFrame("self frame"));
        }
        let aad = &bytes[..HEADER_LEN];
        let ct = &bytes[HEADER_LEN..];
        let pt = self.keys.open(header.epoch, &header.nonce(), aad, ct).map_err(|e| {
            self.stats.decrypt_fail.fetch_add(1, Ordering::Relaxed);
            warn!(sender = header.sender, epoch = header.epoch, counter = header.counter, "AEAD decrypt failed — wrong key or corrupted frame");
            e
        })?;

        // Replay check AFTER AEAD verification.
        if self
            .replay
            .check_and_mark(header.sender, header.counter)
            .is_err()
        {
            self.stats.replay.fetch_add(1, Ordering::Relaxed);
            warn!(sender = header.sender, counter = header.counter, "replay detected");
            return Err(Error::Replay);
        }

        // Gap detection: accumulate missed frames per sender; individual gaps logged at debug.
        {
            let mut last = self.last_counter.lock().unwrap();
            let entry = last.entry(header.sender).or_insert(header.counter);
            if header.counter > *entry + 1 {
                let missed = header.counter - *entry - 1;
                self.stats.frame_gaps.fetch_add(missed, Ordering::Relaxed);
                tracing::debug!(
                    sender = header.sender,
                    last_counter = *entry,
                    current_counter = header.counter,
                    missed_frames = missed,
                    "counter gap"
                );
            }
            *entry = header.counter;
        }

        // Plaintext layout: Kind(1) | payload_len(2 BE) | payload | padding.
        if pt.len() < 3 {
            return Err(Error::BadFrame("plaintext too short"));
        }
        let kind = match pt[0] {
            0 => Kind::Beacon,
            1 => Kind::Fec,
            2 => Kind::Control,
            3 => Kind::Video,
            _ => return Err(Error::BadFrame("unknown kind")),
        };
        match kind {
            Kind::Beacon => self.stats.rx_beacon.fetch_add(1, Ordering::Relaxed),
            Kind::Fec => self.stats.rx_fec.fetch_add(1, Ordering::Relaxed),
            Kind::Control => self.stats.rx_control.fetch_add(1, Ordering::Relaxed),
            Kind::Video => self.stats.rx_video.fetch_add(1, Ordering::Relaxed),
        };
        let payload_len = u16::from_be_bytes([pt[1], pt[2]]) as usize;
        if 3 + payload_len > pt.len() {
            return Err(Error::BadFrame("payload length overflow"));
        }
        let inner = pt[3..3 + payload_len].to_vec();
        let meta = PacketMeta {
            sender_id: header.sender,
            origin_id: header.origin,
            counter: header.counter,
            rssi_dbm,
            recv_time: Instant::now(),
        };

        let mut subs = self.subscribers.lock().unwrap();
        subs.list_mut(kind)
            .retain(|s| match s.try_send((meta.clone(), inner.clone())) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    self.stats.subscriber_full.fetch_add(1, Ordering::Relaxed);
                    warn!(
                        ?kind,
                        sender = header.sender,
                        counter = header.counter,
                        queue_capacity = SUBSCRIBER_QUEUE,
                        "subscriber channel full — frame dropped, consumer too slow"
                    );
                    true
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            });
        Ok(())
    }
}

impl WifiTransport {
    fn broadcast_with_origin(&self, kind: Kind, payload: &[u8], origin: NodeId) -> Result<()> {
        // Plaintext layout: Kind(1) | payload_len(2 BE) | payload | zero-pad.
        // FEC and Control frames are padded to MAX_PLAINTEXT for LPI/LPD.
        // Beacons are sent at their natural size (hackathon trade-off: saves beacon
        // radio budget at the cost of distinguishable frame lengths for beacons).
        if 3 + payload.len() > MAX_PLAINTEXT {
            return Err(Error::BadFrame("payload too large"));
        }
        let counter = self.send_counter.fetch_add(1, Ordering::Relaxed);
        let epoch = self.keys.current_epoch();
        let header = Header {
            version: PROTOCOL_VERSION,
            flags: 0,
            epoch,
            sender: self.local_id,
            origin,
            counter,
        };

        let mut wire = vec![0u8; HEADER_LEN];
        header.encode(&mut wire)?;

        let pt_len = match kind {
            // Beacons and video chunks are sent at natural size — no LPD padding needed.
            // Video: the stream itself is the observable; padding wastes ~1370 bytes/chunk.
            Kind::Beacon | Kind::Video => 3 + payload.len(),
            _ => MAX_PLAINTEXT,
        };
        let mut pt = vec![0u8; pt_len];
        pt[0] = kind as u8;
        let len = payload.len() as u16;
        pt[1..3].copy_from_slice(&len.to_be_bytes());
        pt[3..3 + payload.len()].copy_from_slice(payload);

        let ct = self.keys.seal(epoch, &header.nonce(), &wire, &pt)?;
        wire.extend_from_slice(&ct);

        self.radio.tx_queue.try_send(wire).map_err(|e| {
            let depth = self.radio.tx_queue.len();
            warn!(
                ?kind,
                tx_queue_depth = depth,
                capacity = 256,
                "TX queue full — broadcast dropped due to backpressure: {e}"
            );
            Error::Backpressure
        })?;
        Ok(())
    }
}

impl Transport for WifiTransport {
    fn local_id(&self) -> NodeId {
        self.local_id
    }

    fn broadcast(&self, kind: Kind, payload: &[u8]) -> Result<()> {
        self.broadcast_with_origin(kind, payload, self.local_id)
    }

    fn broadcast_forwarded(&self, kind: Kind, payload: &[u8], origin: NodeId) -> Result<()> {
        self.broadcast_with_origin(kind, payload, origin)
    }

    fn subscribe(&self, kind: Kind) -> mpsc::Receiver<(PacketMeta, Vec<u8>)> {
        let (tx, rx) = mpsc::channel(SUBSCRIBER_QUEUE);
        self.subscribers.lock().unwrap().list_mut(kind).push(tx);
        rx
    }

    fn set_channel(&self, ch: u8) -> Result<()> {
        self.radio.set_channel(ch)
    }
}

fn current_epoch() -> Epoch {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / EPOCH_SECONDS) as Epoch
}
