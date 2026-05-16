use litm_common::{Delivery, SendPolicy};
use litm_delivery::RaptorQDelivery;
use litm_delivery::mock::MockNetwork;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_reliable_broadcast_with_drop() {
    let network = MockNetwork::new(0.2); // 20% drop rate

    let sender_transport = Arc::new(network.create_transport(1));
    let receiver_transport = Arc::new(network.create_transport(2));

    let sender = RaptorQDelivery::new(sender_transport);
    let receiver = RaptorQDelivery::new(receiver_transport);

    let mut rx = receiver.subscribe();

    // Generate 100KB payload
    let mut payload = vec![0u8; 100_000];
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }

    let policy = SendPolicy {
        desired_coverage: 100,
        ttl: Duration::from_secs(10),
        priority: 1,
    };

    sender.send_object(42, payload.clone(), policy).unwrap();

    // Wait for the receiver to get the message
    let result = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("Test timed out waiting for payload")
        .expect("Channel closed");

    assert_eq!(result.id, 42);
    assert_eq!(result.payload, payload);
}
