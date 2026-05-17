use std::collections::HashMap;
use std::sync::Arc;

use litm_common::{Delivery, Mesh, NeighborInfo, NodeId, ObjectId, SendPolicy, Transport};
use tokio::sync::broadcast;

use crate::error::{Result, SdkError};
use crate::message::{MessagePayload, ReceivedMessage, decode_message, encode_message};
use crate::policy;

#[derive(serde::Serialize, Clone, Debug)]
pub struct RadioInfo {
    pub channel: u32,
    pub frequency_mhz: u32,
    pub width_mhz: u32,
    pub txpower_dbm: f32,
    pub pending_channel: Option<u8>,
    pub pending_epoch: Option<u32>,
    pub remaining_seconds: Option<u64>,
}

/// High-level handle for a mesh node. Clone is cheap — all fields are `Arc` or `Copy`.
#[derive(Clone)]
pub struct Node {
    local_id: NodeId,
    transport: Arc<dyn Transport>,
    delivery: Arc<dyn Delivery>,
    mesh: Arc<dyn Mesh>,
    tx: broadcast::Sender<litm_common::DeliveredObject>,
}

impl Node {
    pub(crate) fn new(
        local_id: NodeId,
        transport: Arc<dyn Transport>,
        delivery: Arc<dyn Delivery>,
        mesh: Arc<dyn Mesh>,
        tx: broadcast::Sender<litm_common::DeliveredObject>,
    ) -> Self {
        Self { local_id, transport, delivery, mesh, tx }
    }

    // --- identity ---

    pub fn local_id(&self) -> NodeId {
        self.local_id
    }

    /// Raw transport — use to create a `VideoChannel` for the unreliable video lane.
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::clone(&self.transport)
    }

    pub fn radio_info(&self) -> RadioInfo {
        let ch = self.mesh.current_channel();
        let (pending_channel, pending_epoch, remaining_seconds) = if let Some((next_ch, at_epoch)) = self.mesh.pending_channel_switch() {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let rem = (at_epoch as u64 * 60).saturating_sub(now_secs);
            (Some(next_ch), Some(at_epoch), Some(rem))
        } else {
            (None, None, None)
        };

        RadioInfo {
            channel: ch as u32,
            frequency_mhz: 2407 + (ch as u32) * 5,
            width_mhz: 20,
            txpower_dbm: 22.0,
            pending_channel,
            pending_epoch,
            remaining_seconds,
        }
    }

    /// Broadcast a coordinated channel-switch control packet to all mesh nodes.
    /// The switch is applied at `current_epoch + 1` on every node that receives it.
    pub fn request_channel_hop(&self, channel: u8) -> Result<()> {
        self.mesh.request_channel_hop(channel).map_err(SdkError::Stack)
    }

    // --- sending ---

    /// Send a typed message using the default reliable policy.
    pub fn send(&self, payload: MessagePayload) -> Result<ObjectId> {
        self.send_with_policy(payload, policy::reliable())
    }

    /// Send a typed message with an explicit policy.
    pub fn send_with_policy(
        &self,
        payload: MessagePayload,
        policy: SendPolicy,
    ) -> Result<ObjectId> {
        let bytes = encode_message(&payload)?;
        let id: ObjectId = rand::random();
        self.delivery.send_object(id, bytes, policy).map_err(SdkError::Stack)?;
        Ok(id)
    }

    // --- receiving ---

    /// Returns a new independent subscriber. Each call creates a separate receiver;
    /// messages sent before this call are not replayed.
    pub fn subscribe(&self) -> MessageReceiver {
        MessageReceiver { inner: self.tx.subscribe() }
    }

    /// Subscribe to real-time telemetry events for the RaptorQ delivery engine.
    pub fn subscribe_telemetry(&self) -> tokio::sync::mpsc::Receiver<litm_common::RaptorEvent> {
        self.delivery.subscribe_telemetry()
    }

    // --- mesh queries ---

    pub fn neighbors(&self) -> Vec<NeighborInfo> {
        self.mesh.neighbors()
    }

    pub fn topology(&self) -> HashMap<NodeId, Vec<(NodeId, f32)>> {
        self.mesh.topology()
    }

    // --- escape hatches for advanced use ---

    pub fn delivery(&self) -> Arc<dyn Delivery> {
        Arc::clone(&self.delivery)
    }

    pub fn mesh(&self) -> Arc<dyn Mesh> {
        Arc::clone(&self.mesh)
    }
}

/// Wraps a `broadcast::Receiver<DeliveredObject>` and decodes SDK envelopes on the fly.
/// Non-SDK payloads (from nodes not yet using app_sdk) are silently skipped.
pub struct MessageReceiver {
    inner: broadcast::Receiver<litm_common::DeliveredObject>,
}

impl MessageReceiver {
    pub async fn recv(&mut self) -> Result<ReceivedMessage> {
        loop {
            match self.inner.recv().await {
                Ok(obj) => match decode_message(&obj.payload) {
                    Ok(payload) => {
                        return Ok(ReceivedMessage { id: obj.id, source: obj.source, payload });
                    }
                    Err(e) => {
                        tracing::debug!(
                            source = obj.source,
                            "skipping non-SDK payload: {}",
                            e
                        );
                        // Continue loop — tolerate mixed SDK/non-SDK nodes
                    }
                },
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SDK subscriber lagged by {} messages", n);
                    return Err(SdkError::Lagged);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(SdkError::NotStarted);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::spawn_delivery_bridge;
    use crate::message::encode_message;
    use litm_common::{
        DeliveredObject, Kind, NodeId, ObjectBitmap, ObjectId, PacketMeta, Result as CommonResult,
        SendPolicy,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tokio::sync::mpsc;

    // --- MockTransport ---

    struct MockTransport;
    impl Transport for MockTransport {
        fn local_id(&self) -> NodeId { 1 }
        fn broadcast(&self, _kind: Kind, _payload: &[u8]) -> CommonResult<()> { Ok(()) }
        fn broadcast_forwarded(&self, _kind: Kind, _payload: &[u8], _origin: NodeId) -> CommonResult<()> { Ok(()) }
        fn subscribe(&self, _kind: Kind) -> mpsc::Receiver<(PacketMeta, Vec<u8>)> {
            mpsc::channel(1).1
        }
        fn set_channel(&self, _ch: u8) -> CommonResult<()> { Ok(()) }
    }

    // --- MockDelivery ---

    struct MockDelivery {
        senders: Mutex<Vec<mpsc::Sender<DeliveredObject>>>,
        sent: Mutex<Vec<(ObjectId, Vec<u8>)>>,
    }

    impl MockDelivery {
        fn new() -> Self {
            Self { senders: Mutex::new(vec![]), sent: Mutex::new(vec![]) }
        }

        fn inject(&self, obj: DeliveredObject) {
            for tx in self.senders.lock().unwrap().iter() {
                let _ = tx.try_send(obj.clone());
            }
        }

        fn last_sent(&self) -> Option<(ObjectId, Vec<u8>)> {
            self.sent.lock().unwrap().last().cloned()
        }
    }

    impl Delivery for MockDelivery {
        fn send_object(
            &self,
            id: ObjectId,
            payload: Vec<u8>,
            _policy: SendPolicy,
        ) -> CommonResult<()> {
            self.sent.lock().unwrap().push((id, payload));
            Ok(())
        }
        fn subscribe(&self) -> mpsc::Receiver<DeliveredObject> {
            let (tx, rx) = mpsc::channel(64);
            self.senders.lock().unwrap().push(tx);
            rx
        }
        fn decoded_bitmap(&self) -> ObjectBitmap {
            ObjectBitmap::default()
        }
        fn decoded_recent(&self) -> Vec<ObjectId> {
            vec![]
        }
        fn note_peer_coverage(&self, _peer: NodeId, _bitmap: ObjectBitmap, _recent: Vec<ObjectId>, _prr: f32) {}
    }

    // --- MockMesh ---

    struct MockMesh;

    impl Mesh for MockMesh {
        fn neighbors(&self) -> Vec<NeighborInfo> {
            vec![]
        }
        fn topology(&self) -> HashMap<NodeId, Vec<(NodeId, f32)>> {
            HashMap::new()
        }
        fn request_channel_hop(&self, _next_channel: u8) -> CommonResult<()> {
            Ok(())
        }
        fn current_channel(&self) -> u8 {
            6
        }
    }

    fn make_node() -> (Arc<MockDelivery>, Node) {
        let transport = Arc::new(MockTransport) as Arc<dyn Transport>;
        let delivery = Arc::new(MockDelivery::new());
        let mesh = Arc::new(MockMesh);
        let tx = spawn_delivery_bridge(Arc::clone(&delivery) as Arc<dyn Delivery>);
        let node = Node::new(
            1,
            transport,
            Arc::clone(&delivery) as Arc<dyn Delivery>,
            mesh as Arc<dyn Mesh>,
            tx,
        );
        (delivery, node)
    }

    #[tokio::test]
    async fn send_encodes_and_stores_payload() {
        let (delivery, node) = make_node();
        node.send(MessagePayload::Text { content: "hi".into() }).unwrap();
        let (_, bytes) = delivery.last_sent().unwrap();
        let decoded = decode_message(&bytes).unwrap();
        assert!(matches!(decoded, MessagePayload::Text { content } if content == "hi"));
    }

    #[tokio::test]
    async fn subscribe_receives_typed_message() {
        let (delivery, node) = make_node();
        let mut rx = node.subscribe();

        tokio::task::yield_now().await;

        let raw = encode_message(&MessagePayload::Text { content: "mesh".into() }).unwrap();
        delivery.inject(DeliveredObject { id: 7, source: 3, payload: raw });

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.id, 7);
        assert_eq!(msg.source, 3);
        assert!(matches!(msg.payload, MessagePayload::Text { content } if content == "mesh"));
    }

    #[tokio::test]
    async fn non_sdk_payload_skipped() {
        let (delivery, node) = make_node();
        let mut rx = node.subscribe();

        tokio::task::yield_now().await;

        // Inject garbage first — should be skipped
        delivery.inject(DeliveredObject { id: 1, source: 99, payload: b"not-sdk".to_vec() });
        // Then inject a valid SDK message
        let raw = encode_message(&MessagePayload::Text { content: "ok".into() }).unwrap();
        delivery.inject(DeliveredObject { id: 2, source: 99, payload: raw });

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.id, 2); // skipped id=1
    }

    #[test]
    fn local_id_matches() {
        let (_, node) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { make_node() });
        assert_eq!(node.local_id(), 1);
    }

    #[test]
    fn clone_is_independent_subscriber() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (delivery, node) = make_node();
            let node2 = node.clone();
            let mut rx1 = node.subscribe();
            let mut rx2 = node2.subscribe();

            tokio::task::yield_now().await;

            let raw = encode_message(&MessagePayload::Text { content: "x".into() }).unwrap();
            delivery.inject(DeliveredObject { id: 5, source: 0, payload: raw });

            let m1 = rx1.recv().await.unwrap();
            let m2 = rx2.recv().await.unwrap();
            assert_eq!(m1.id, m2.id);
        });
    }
}
