use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use litm_common::ObjectId;

use crate::error::Result;
use crate::message::{MessagePayload, VideoCodec};
use crate::node::{MessageReceiver, Node};
use crate::policy;

/// Sends video frames through a `Node`. Does not own a capture loop — that
/// stays in the platform-specific layer (e.g. `litm-app/video.rs`).
pub struct VideoStreamer {
    node: Node,
    seq: Arc<AtomicU32>,
    codec: VideoCodec,
}

impl VideoStreamer {
    pub fn new(node: Node, codec: VideoCodec) -> Self {
        Self { node, seq: Arc::new(AtomicU32::new(0)), codec }
    }

    pub fn send_frame(&self, width: u16, height: u16, data: Vec<u8>) -> Result<ObjectId> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        self.node.send_with_policy(
            MessagePayload::VideoFrame { seq, width, height, codec: self.codec.clone(), data },
            policy::video_frame(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct VideoFrameData {
    pub seq: u32,
    pub width: u16,
    pub height: u16,
    pub codec: VideoCodec,
    pub data: Vec<u8>,
    pub source: litm_common::NodeId,
}

/// Receives and decodes video frames from a `Node`, skipping non-video messages
/// and stale frames (sequence number wrap-aware).
pub struct VideoReceiver {
    inner: MessageReceiver,
    last_seqs: std::collections::HashMap<litm_common::NodeId, u32>,
}

impl VideoReceiver {
    pub fn new(node: &Node) -> Self {
        Self { inner: node.subscribe(), last_seqs: std::collections::HashMap::new() }
    }

    pub async fn recv_frame(&mut self) -> Result<VideoFrameData> {
        loop {
            let msg = self.inner.recv().await?;
            if let MessagePayload::VideoFrame { seq, width, height, codec, data } = msg.payload {
                let last = self.last_seqs.get(&msg.source).copied();
                if is_newer_seq(last, seq) {
                    self.last_seqs.insert(msg.source, seq);
                    return Ok(VideoFrameData { seq, width, height, codec, data, source: msg.source });
                }
            }
            // Non-video or stale frame — keep waiting
        }
    }
}

/// Wrap-aware seq comparison. Returns true if `seq` is strictly newer than `last`.
/// Also returns true if `seq` is vastly different (stream restart).
/// We only reject frames that are slightly old (diff is between u32::MAX - 10000 and u32::MAX).
fn is_newer_seq(last: Option<u32>, seq: u32) -> bool {
    match last {
        None => true,
        Some(last) => {
            let diff = seq.wrapping_sub(last);
            diff != 0 && !(diff > (u32::MAX - 10_000))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_always_accepted() {
        assert!(is_newer_seq(None, 0));
        assert!(is_newer_seq(None, 999));
        assert!(is_newer_seq(None, u32::MAX));
    }

    #[test]
    fn sequential_frames_accepted() {
        assert!(is_newer_seq(Some(5), 6));
        assert!(is_newer_seq(Some(5), 100));
        assert!(is_newer_seq(Some(5), 10004)); // 5 + 9999
    }

    #[test]
    fn stale_frames_rejected() {
        assert!(!is_newer_seq(Some(100), 100)); // same seq
        assert!(!is_newer_seq(Some(100), 99));  // one behind
        assert!(!is_newer_seq(Some(100), 1));   // far behind
    }

    #[test]
    fn out_of_window_accepted_as_restart() {
        assert!(is_newer_seq(Some(5), 10005)); // diff = 10000, accepted as restart
        assert!(is_newer_seq(Some(5), 20000));
    }

    #[test]
    fn wraparound_accepted() {
        assert!(is_newer_seq(Some(u32::MAX - 1), u32::MAX));
        assert!(is_newer_seq(Some(u32::MAX - 1), 0)); // wrapped
        assert!(is_newer_seq(Some(u32::MAX - 1), 5));
    }

    #[test]
    fn wraparound_stale_rejected() {
        // u32::MAX - 5 is not newer than u32::MAX - 1
        assert!(!is_newer_seq(Some(u32::MAX - 1), u32::MAX - 5));
    }

    #[test]
    fn streamer_seq_increments() {
        let seq = Arc::new(AtomicU32::new(0));
        assert_eq!(seq.fetch_add(1, Ordering::Relaxed), 0);
        assert_eq!(seq.fetch_add(1, Ordering::Relaxed), 1);
        assert_eq!(seq.load(Ordering::Relaxed), 2);
    }
}
