use litm_common::NodeId;

pub enum AppEvent {
    Terminal(crossterm::event::Event),
    MeshMessage { source: NodeId, content: String },
    TopologyTick,
    MeshClosed,
}
