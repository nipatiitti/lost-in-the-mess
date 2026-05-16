use std::sync::{Arc, Mutex};
use std::time::Duration;

use delivery::{RaptorQDelivery, ReliableBroadcast, SendPolicy};
use delivery::mock::MockNetwork;

#[test]
fn test_reliable_broadcast_with_drop() {
    let network = MockNetwork::new(0.2); // 20% drop rate

    let sender_transport = Arc::new(network.create_transport(1));
    let receiver_transport = Arc::new(network.create_transport(2));

    let sender = RaptorQDelivery::new(sender_transport);
    let receiver = RaptorQDelivery::new(receiver_transport);

    let received_payload = Arc::new(Mutex::new(None));
    let received_clone = received_payload.clone();

    receiver.on_complete(Arc::new(move |id, payload| {
        assert_eq!(id, 42);
        *received_clone.lock().unwrap() = Some(payload);
    }));

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

    sender.send_object(42, payload.clone(), policy);

    // Wait for the receiver to get the message
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if received_payload.lock().unwrap().is_some() {
            break;
        }
    }

    let result = received_payload.lock().unwrap().take().expect("Failed to receive and decode payload");
    assert_eq!(result, payload);
}
