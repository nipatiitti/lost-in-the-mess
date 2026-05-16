use image::DynamicImage;
use litm_common::NodeId;

pub struct DecodedVideoFrame {
    pub source: NodeId,
    pub seq: u32,
    pub image: DynamicImage,
}

pub enum AppEvent {
    Terminal(crossterm::event::Event),
    MeshMessage { source: NodeId, content: String },
    TopologyTick,
    MeshClosed,
    VideoFrame(DecodedVideoFrame),
}
