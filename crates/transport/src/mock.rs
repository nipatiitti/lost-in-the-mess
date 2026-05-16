use litm_common::{Kind, NodeId, PacketMeta, Result, Transport};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc};

pub struct MockTransport {
    local_id: NodeId,
    socket: Arc<UdpSocket>,
    tx_beacon: broadcast::Sender<(PacketMeta, Vec<u8>)>,
    tx_fec: broadcast::Sender<(PacketMeta, Vec<u8>)>,
    tx_control: broadcast::Sender<(PacketMeta, Vec<u8>)>,
    counter: AtomicU64,
}

impl MockTransport {
    pub async fn new(local_id: NodeId) -> Arc<Self> {
        let socket = {
            let socket2 = socket2::Socket::new(
                socket2::Domain::IPV4,
                socket2::Type::DGRAM,
                Some(socket2::Protocol::UDP),
            )
            .unwrap();
            socket2.set_reuse_address(true).unwrap();
            #[cfg(unix)]
            socket2.set_reuse_port(true).unwrap();
            socket2.set_broadcast(true).unwrap();
            socket2
                .bind(
                    &"0.0.0.0:9999"
                        .parse::<std::net::SocketAddr>()
                        .unwrap()
                        .into(),
                )
                .unwrap();
            socket2.set_nonblocking(true).unwrap();
            UdpSocket::from_std(socket2.into()).unwrap()
        };

        let (tx_beacon, _) = broadcast::channel(100);
        let (tx_fec, _) = broadcast::channel(100);
        let (tx_control, _) = broadcast::channel(100);

        let t = Arc::new(Self {
            local_id,
            socket: Arc::new(socket),
            tx_beacon,
            tx_fec,
            tx_control,
            counter: AtomicU64::new(0),
        });

        Self::spawn_receiver(Arc::clone(&t));
        t
    }

    fn spawn_receiver(self_ref: Arc<Self>) {
        let socket = Arc::clone(&self_ref.socket);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            loop {
                if let Ok((len, _)) = socket.recv_from(&mut buf).await {
                    if len < 13 {
                        continue;
                    }
                    let mut sid_bytes = [0; 4];
                    sid_bytes.copy_from_slice(&buf[0..4]);
                    let sender_id = u32::from_le_bytes(sid_bytes);

                    if sender_id == self_ref.local_id {
                        continue;
                    }

                    let mut c_bytes = [0; 8];
                    c_bytes.copy_from_slice(&buf[4..12]);
                    let counter = u64::from_le_bytes(c_bytes);

                    let kind = match buf[12] {
                        0 => Kind::Beacon,
                        1 => Kind::Fec,
                        2 => Kind::Control,
                        _ => continue,
                    };

                    let meta = PacketMeta {
                        sender_id,
                        counter,
                        rssi_dbm: -50,
                        recv_time: Instant::now(),
                    };

                    let payload = buf[13..len].to_vec();

                    match kind {
                        Kind::Beacon => { let _ = self_ref.tx_beacon.send((meta, payload)); }
                        Kind::Fec => { let _ = self_ref.tx_fec.send((meta, payload)); }
                        Kind::Control => { let _ = self_ref.tx_control.send((meta, payload)); }
                    }
                }
            }
        });
    }
}

impl Transport for MockTransport {
    fn local_id(&self) -> NodeId {
        self.local_id
    }

    fn broadcast(&self, kind: Kind, payload: &[u8]) -> Result<()> {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&self.local_id.to_le_bytes());
        pkt.extend_from_slice(&counter.to_le_bytes());
        pkt.push(kind as u8);
        pkt.extend_from_slice(payload);

        let socket = Arc::clone(&self.socket);
        tokio::spawn(async move {
            let _ = socket.send_to(&pkt, "255.255.255.255:9999").await;
        });
        Ok(())
    }

    fn subscribe(&self, kind: Kind) -> mpsc::Receiver<(PacketMeta, Vec<u8>)> {
        let mut rx = match kind {
            Kind::Beacon => self.tx_beacon.subscribe(),
            Kind::Fec => self.tx_fec.subscribe(),
            Kind::Control => self.tx_control.subscribe(),
        };
        let (tx, client_rx) = mpsc::channel(100);
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if tx.send(msg).await.is_err() {
                    break;
                }
            }
        });
        client_rx
    }

    fn set_channel(&self, _ch: u8) -> Result<()> {
        Ok(())
    }
}
