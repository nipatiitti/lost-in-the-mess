//! Authenticated, replay-protected, unreliable broadcast over kova-wfb-rs.

mod crypto;
mod frame;
mod radio;
mod replay;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tracing::{trace, warn};

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

pub struct WifiTransport {
    local_id: NodeId,
    keys: Arc<KeyStore>,
    replay: Arc<ReplayDb>,
    send_counter: AtomicU64,
    radio: Radio,
    subscribers: Arc<Mutex<Subscribers>>,
}

#[derive(Default)]
struct Subscribers {
    beacon: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
    fec: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
    control: Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>>,
}

impl Subscribers {
    fn list_mut(&mut self, k: Kind) -> &mut Vec<mpsc::Sender<(PacketMeta, Vec<u8>)>> {
        match k {
            Kind::Beacon => &mut self.beacon,
            Kind::Fec => &mut self.fec,
            Kind::Control => &mut self.control,
        }
    }
}

impl WifiTransport {
    pub fn start(cfg: WifiTransportConfig) -> Result<Arc<Self>> {
        let epoch = current_epoch();
        let keys = Arc::new(KeyStore::new(cfg.root_key, epoch));
        let radio = Radio::start(cfg.radio)?;

        let me = Arc::new(Self {
            local_id: cfg.local_id,
            keys,
            replay: Arc::new(ReplayDb::new()),
            send_counter: AtomicU64::new(1),
            radio,
            subscribers: Arc::new(Mutex::new(Subscribers::default())),
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
        let pt = self.keys.open(header.epoch, &header.nonce(), aad, ct)?;

        // Replay check AFTER AEAD verification.
        if self
            .replay
            .check_and_mark(header.sender, header.counter)
            .is_err()
        {
            return Err(Error::Replay);
        }

        // Plaintext layout: Kind(1) | payload_len(2 BE) | payload | padding.
        if pt.len() < 3 {
            return Err(Error::BadFrame("plaintext too short"));
        }
        let kind = match pt[0] {
            0 => Kind::Beacon,
            1 => Kind::Fec,
            2 => Kind::Control,
            _ => return Err(Error::BadFrame("unknown kind")),
        };
        let payload_len = u16::from_be_bytes([pt[1], pt[2]]) as usize;
        if 3 + payload_len > pt.len() {
            return Err(Error::BadFrame("payload length overflow"));
        }
        let inner = pt[3..3 + payload_len].to_vec();
        let meta = PacketMeta {
            sender_id: header.sender,
            counter: header.counter,
            rssi_dbm,
            recv_time: Instant::now(),
        };

        let mut subs = self.subscribers.lock().unwrap();
        subs.list_mut(kind)
            .retain(|s| match s.try_send((meta.clone(), inner.clone())) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(?kind, "subscriber full, dropping");
                    true
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            });
        Ok(())
    }
}

impl Transport for WifiTransport {
    fn local_id(&self) -> NodeId {
        self.local_id
    }

    fn broadcast(&self, kind: Kind, payload: &[u8]) -> Result<()> {
        // Plaintext layout: Kind(1) | payload_len(2 BE) | payload | zero-pad → MAX_PLAINTEXT.
        // All frames are padded to the same size so a passive observer cannot distinguish
        // a small beacon from a large FEC symbol by frame length (LPI/LPD, design decision 7).
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
            counter,
        };

        let mut wire = vec![0u8; HEADER_LEN];
        header.encode(&mut wire)?;

        let mut pt = vec![0u8; MAX_PLAINTEXT]; // pre-zeroed; trailing zeros are the pad
        pt[0] = kind as u8;
        let len = payload.len() as u16;
        pt[1..3].copy_from_slice(&len.to_be_bytes());
        pt[3..3 + payload.len()].copy_from_slice(payload);

        let ct = self.keys.seal(epoch, &header.nonce(), &wire, &pt)?;
        wire.extend_from_slice(&ct);

        self.radio
            .tx_queue
            .try_send(wire)
            .map_err(|_| Error::Backpressure)?;
        Ok(())
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
