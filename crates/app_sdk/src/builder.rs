use std::sync::Arc;

use litm_common::{Delivery, Mesh, NodeId, Transport};
use litm_delivery::RaptorQDelivery;
use litm_mesh::MeshService;
use litm_transport::{RadioConfig, WifiTransport, WifiTransportConfig, derive_root_key};

use crate::broadcast::spawn_delivery_bridge;
use crate::error::{Result, SdkError};
use crate::node::Node;

pub struct NodeBuilder {
    node_id: NodeId,
    password: String,
    iface: String,
    channel: Option<u8>,
}

impl NodeBuilder {
    pub fn new(
        node_id: NodeId,
        password: impl Into<String>,
        iface: impl Into<String>,
    ) -> Self {
        Self { node_id, password: password.into(), iface: iface.into(), channel: None }
    }

    /// Override the 802.11 channel (default: whatever the interface is already on).
    pub fn channel(mut self, ch: u8) -> Self {
        self.channel = Some(ch);
        self
    }

    /// Assemble the full concrete stack and return a `Node`.
    ///
    /// Must be called inside a tokio runtime — `MeshService::new` spawns async tasks.
    /// Hardware-dependent: requires a monitor-mode interface with packet injection.
    pub fn build(self) -> Result<Node> {
        let root_key = derive_root_key(&self.password);

        let radio = RadioConfig { iface: self.iface, ..RadioConfig::default() };
        let cfg = WifiTransportConfig { local_id: self.node_id, radio, root_key };

        let transport: Arc<dyn Transport> =
            WifiTransport::start(cfg).map_err(SdkError::Stack)?;

        let delivery: Arc<dyn Delivery> = RaptorQDelivery::new(Arc::clone(&transport));
        let mesh: Arc<dyn Mesh> =
            MeshService::new(Arc::clone(&transport), Arc::clone(&delivery));

        if let Some(ch) = self.channel {
            transport.set_channel(ch).map_err(SdkError::Stack)?;
        }

        let tx = spawn_delivery_bridge(Arc::clone(&delivery));
        Ok(Node::new(self.node_id, transport, delivery, mesh, tx))
    }
}
