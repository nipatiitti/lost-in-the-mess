use std::sync::Arc;

use litm_common::{Delivery, DeliveredObject};
use tokio::sync::broadcast;
use tracing::warn;

pub const BROADCAST_CAPACITY: usize = 256;

/// Bridges the one-to-one `mpsc::Receiver` from `Delivery::subscribe()` into a
/// `broadcast::Sender` so any number of callers can independently receive objects.
///
/// Spawns one background task. Returns the sender; callers call `.subscribe()` on it.
/// Slow consumers receive `RecvError::Lagged` rather than blocking the bridge.
pub fn spawn_delivery_bridge(delivery: Arc<dyn Delivery>) -> broadcast::Sender<DeliveredObject> {
    let (tx, _rx) = broadcast::channel::<DeliveredObject>(BROADCAST_CAPACITY);
    let tx_clone = tx.clone();
    let mut mpsc_rx = delivery.subscribe();

    tokio::spawn(async move {
        while let Some(obj) = mpsc_rx.recv().await {
            // send() returns Err only when there are zero subscribers — that is fine.
            let _ = tx_clone.send(obj);
        }
        warn!("delivery bridge task exited — delivery layer was dropped");
    });

    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use litm_common::{NodeId, ObjectBitmap, ObjectId, Result, SendPolicy};
    use std::sync::Mutex;
    use tokio::sync::mpsc;

    struct MockDelivery {
        senders: Mutex<Vec<mpsc::Sender<DeliveredObject>>>,
    }

    impl MockDelivery {
        fn new() -> Self {
            Self { senders: Mutex::new(vec![]) }
        }

        fn inject(&self, obj: DeliveredObject) {
            let senders = self.senders.lock().unwrap();
            for tx in senders.iter() {
                let _ = tx.try_send(obj.clone());
            }
        }
    }

    impl Delivery for MockDelivery {
        fn send_object(&self, _id: ObjectId, _payload: Vec<u8>, _policy: SendPolicy) -> Result<()> {
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
        fn note_peer_coverage(&self, _peer: NodeId, _bitmap: ObjectBitmap, _prr: f32) {}
    }

    #[tokio::test]
    async fn two_subscribers_both_receive() {
        let delivery = Arc::new(MockDelivery::new());
        let tx = spawn_delivery_bridge(Arc::clone(&delivery) as Arc<dyn Delivery>);
        let mut rx1 = tx.subscribe();
        let mut rx2 = tx.subscribe();

        // Give the bridge task a moment to start
        tokio::task::yield_now().await;

        delivery.inject(DeliveredObject { id: 1, source: 42, payload: b"hello".to_vec() });

        let a = rx1.recv().await.unwrap();
        let b = rx2.recv().await.unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.source, b.source);
    }

    #[tokio::test]
    async fn late_subscriber_misses_earlier_messages() {
        let delivery = Arc::new(MockDelivery::new());
        let tx = spawn_delivery_bridge(Arc::clone(&delivery) as Arc<dyn Delivery>);

        tokio::task::yield_now().await;
        delivery.inject(DeliveredObject { id: 1, source: 1, payload: vec![] });
        tokio::task::yield_now().await;

        // Subscribe after the message was sent — should not see it
        let mut rx = tx.subscribe();
        delivery.inject(DeliveredObject { id: 2, source: 1, payload: vec![] });

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.id, 2); // only the second message
    }
}
