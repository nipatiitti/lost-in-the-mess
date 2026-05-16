use litm_common::{Delivery, SendPolicy, Transport};
use litm_delivery::RaptorQDelivery;
use litm_transport::{WifiTransport, WifiTransportConfig, RadioConfig};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_reliable_broadcast_with_real_radio() {
    let root_key = [0u8; 32];
    
    let cfg1 = WifiTransportConfig {
        local_id: 1,
        radio: RadioConfig::default(),
        root_key,
    };
    
    let cfg2 = WifiTransportConfig {
        local_id: 2,
        radio: RadioConfig::default(),
        root_key,
    };

    let sender_transport: Arc<dyn Transport> = WifiTransport::start(cfg1).expect("Sender WifiTransport failed");
    let receiver_transport: Arc<dyn Transport> = WifiTransport::start(cfg2).expect("Receiver WifiTransport failed");

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
    let result = tokio::time::timeout(Duration::from_secs(15), rx.recv())
        .await
        .expect("Test timed out waiting for payload")
        .expect("Channel closed");

    assert_eq!(result.id, 42);
    assert_eq!(result.payload, payload);
}
