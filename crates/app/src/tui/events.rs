use image::DynamicImage;
use litm_common::NodeId;

pub struct DecodedVideoFrame {
    pub source: NodeId,
    pub frame_id: u32,
    pub image: DynamicImage,
    /// Chunks that arrived for this frame (vs. total expected). Used for loss display.
    pub chunks_received: u8,
    pub chunks_total: u8,
}

pub enum AppEvent {
    Terminal(crossterm::event::Event),
    MeshMessage { source: NodeId, content: String },
    TopologyTick,
    MeshClosed,
    VideoFrame(DecodedVideoFrame),
    LocalPreview(image::DynamicImage),
    RaptorTelemetry(litm_common::RaptorEvent),
}
