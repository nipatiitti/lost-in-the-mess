//! Unreliable video bypass lane: Kind::Video, no FEC, no ACK, fire-and-forget.
//!
//! Each JPEG frame is sliced into ≤1391-byte chunks. Every chunk is an
//! independent encrypted broadcast. The receiver reassembles chunks by
//! frame_id; incomplete frames are silently discarded and the last good
//! frame is held in the TUI.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use image::ImageEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use litm_common::{Kind, NodeId, Transport};
use tokio::sync::mpsc;

// MAX_PLAINTEXT(1400) - kind(1) - payload_len(2) - chunk_header(6) = 1391
const CHUNK_DATA: usize = 1391;
// frame_id(4) + chunk_idx(1) + total_chunks(1)
const HDR: usize = 6;

// ── Public types ──────────────────────────────────────────────────────────────

/// A fully reassembled video frame received from a remote node.
#[derive(Clone)]
pub struct VideoFrame {
    pub frame_id: u32,
    pub source: NodeId,
    pub jpeg_bytes: Vec<u8>,
    /// How many chunks arrived vs. how many were expected (for loss display).
    pub chunks_received: u8,
    pub chunks_total: u8,
}

/// Resolution and quality preset for the outgoing stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoQuality {
    /// 160×120 @ JPEG q10 — typically 1–2 radio packets. Lowest latency.
    Fast,
    /// 320×240 @ JPEG q20 — typically 3–5 radio packets. Good balance.
    Balanced,
    /// 640×480 @ JPEG q30 — 10–25 packets. Use only on clean links.
    Rich,
}

impl VideoQuality {
    fn dims(self) -> (u32, u32) {
        match self {
            Self::Fast => (160, 120),
            Self::Balanced => (320, 240),
            Self::Rich => (640, 480),
        }
    }
    fn jpeg_quality(self) -> u8 {
        match self {
            Self::Fast => 10,
            Self::Balanced => 20,
            Self::Rich => 30,
        }
    }
}

// ── VideoChannel ──────────────────────────────────────────────────────────────

/// Unreliable video lane over a `Transport`. Create once; clone the `Arc`.
///
/// TX: call `send_frame` with raw JPEG bytes — it fragments and broadcasts.
/// RX: call `subscribe` to get a channel of reassembled `VideoFrame`s.
pub struct VideoChannel {
    transport: Arc<dyn Transport>,
    frame_id: AtomicU32,
    subscribers: Mutex<Vec<mpsc::Sender<VideoFrame>>>,
}

impl VideoChannel {
    pub fn new(transport: Arc<dyn Transport>) -> Arc<Self> {
        let ch = Arc::new(Self {
            transport: transport.clone(),
            frame_id: AtomicU32::new(rand::random()),
            subscribers: Mutex::new(Vec::new()),
        });

        let ch2 = ch.clone();
        let mut rx = transport.subscribe(Kind::Video);
        tokio::spawn(async move {
            // Per-source state: current frame_id + chunk accumulator
            let mut buffers: HashMap<NodeId, (u32, u8, HashMap<u8, Vec<u8>>)> = HashMap::new();

            while let Some((meta, payload)) = rx.recv().await {
                if payload.len() < HDR {
                    continue;
                }
                let frame_id =
                    u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let chunk_idx = payload[4];
                let total = payload[5];
                if total == 0 {
                    continue;
                }
                let data = payload[HDR..].to_vec();
                let source = meta.sender_id;

                let entry = buffers.entry(source).or_insert((frame_id, total, HashMap::new()));

                if frame_id > entry.0 {
                    // New frame — drop old incomplete buffer
                    *entry = (frame_id, total, HashMap::new());
                } else if frame_id < entry.0 {
                    // Stale chunk
                    continue;
                }

                entry.2.insert(chunk_idx, data);

                let received = entry.2.len() as u8;
                let frame_total = entry.1;

                if received >= frame_total {
                    let mut assembled = Vec::with_capacity(frame_total as usize * CHUNK_DATA);
                    for i in 0..frame_total {
                        if let Some(chunk) = entry.2.get(&i) {
                            assembled.extend_from_slice(chunk);
                        }
                    }
                    let frame = VideoFrame {
                        frame_id,
                        source,
                        jpeg_bytes: assembled,
                        chunks_received: received,
                        chunks_total: frame_total,
                    };
                    ch2.emit(frame);
                    buffers.remove(&source);
                }
            }
        });

        ch
    }

    /// Fragment `jpeg_bytes` into chunks and broadcast each over the radio.
    /// Fire-and-forget — no ACK, no retry.
    pub fn send_frame(&self, jpeg_bytes: &[u8]) -> litm_common::Result<()> {
        let frame_id = self.frame_id.fetch_add(1, Ordering::Relaxed);
        let raw_chunks: Vec<&[u8]> = jpeg_bytes.chunks(CHUNK_DATA).collect();
        let total = raw_chunks.len().min(255) as u8;
        if total == 0 {
            return Ok(());
        }
        for (idx, chunk) in raw_chunks.iter().enumerate().take(255) {
            let mut payload = Vec::with_capacity(HDR + chunk.len());
            payload.extend_from_slice(&frame_id.to_be_bytes());
            payload.push(idx as u8);
            payload.push(total);
            payload.extend_from_slice(chunk);
            self.transport.broadcast(Kind::Video, &payload)?;
        }
        Ok(())
    }

    /// Get a receiver for fully reassembled remote frames.
    pub fn subscribe(&self) -> mpsc::Receiver<VideoFrame> {
        let (tx, rx) = mpsc::channel(4);
        self.subscribers.lock().unwrap().push(tx);
        rx
    }

    fn emit(&self, frame: VideoFrame) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| match tx.try_send(frame.clone()) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Subscriber is slow — drop this frame but keep the subscriber alive.
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        });
    }
}

// ── VideoStreamer ─────────────────────────────────────────────────────────────

/// Encodes raw RGB frames from the camera and sends them over a `VideoChannel`.
///
/// Resize + JPEG encode happen here so the camera capture loop stays simple.
pub struct VideoStreamer {
    channel: Arc<VideoChannel>,
    quality: VideoQuality,
}

impl VideoStreamer {
    pub fn new(channel: Arc<VideoChannel>, quality: VideoQuality) -> Self {
        Self { channel, quality }
    }

    /// Encode and transmit one camera frame.
    ///
    /// `rgb` must be tightly packed RGB24 (width * height * 3 bytes).
    pub fn send_frame(&self, rgb: &[u8], width: u32, height: u32) -> litm_common::Result<()> {
        let jpeg = encode_jpeg(rgb, width, height, self.quality);
        self.channel.send_frame(&jpeg)
    }
}

fn encode_jpeg(rgb: &[u8], width: u32, height: u32, quality: VideoQuality) -> Vec<u8> {
    let (tw, th) = quality.dims();
    let img = image::RgbImage::from_raw(width, height, rgb.to_vec())
        .map(image::DynamicImage::ImageRgb8)
        .unwrap_or_else(|| image::DynamicImage::new_rgb8(width, height));

    let resized = img.resize(tw, th, FilterType::Nearest);
    let rgb8 = resized.to_rgb8();

    let mut buf = Vec::new();
    let enc = JpegEncoder::new_with_quality(&mut buf, quality.jpeg_quality());
    let _ = enc.write_image(rgb8.as_raw(), rgb8.width(), rgb8.height(), image::ExtendedColorType::Rgb8);
    buf
}
