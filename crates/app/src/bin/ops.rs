use clap::{Parser, Subcommand};
use litm_common::{
    Delivery, Kind, NodeId, PacketMeta, Result, SendPolicy,
    Transport,
};
use litm_delivery::RaptorQDelivery;
use litm_mesh::MeshService;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::broadcast;
use tracing::{info, warn};

// A mock transport that uses UDP broadcast to simulate the radio
struct MockTransport {
    local_id: NodeId,
    socket: Arc<UdpSocket>,
    ignored_nodes: Vec<NodeId>,
    tx_beacon: broadcast::Sender<(PacketMeta, Vec<u8>)>,
    tx_fec: broadcast::Sender<(PacketMeta, Vec<u8>)>,
    tx_control: broadcast::Sender<(PacketMeta, Vec<u8>)>,
    counter: AtomicU64,
}

impl MockTransport {
    async fn new(local_id: NodeId, ignored_nodes: Vec<NodeId>) -> Arc<Self> {
        // Bind to a random port for sending, but receive on a common broadcast port if possible?
        // Actually, UDP broadcast: bind to 0.0.0.0:9999 with SO_REUSEADDR and SO_REUSEPORT
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
            ignored_nodes,
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
                        continue; // too short
                    }
                    // parse mock header: [sender_id(4)][counter(8)][kind(1)][payload...]
                    let mut sid_bytes = [0; 4];
                    sid_bytes.copy_from_slice(&buf[0..4]);
                    let sender_id = u32::from_le_bytes(sid_bytes);

                    if sender_id == self_ref.local_id || self_ref.ignored_nodes.contains(&sender_id)
                    {
                        continue;
                    }

                    let mut c_bytes = [0; 8];
                    c_bytes.copy_from_slice(&buf[4..12]);
                    let counter = u64::from_le_bytes(c_bytes);

                    let kind_byte = buf[12];
                    let kind = match kind_byte {
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
                        Kind::Beacon => {
                            let _ = self_ref.tx_beacon.send((meta, payload));
                        }
                        Kind::Fec => {
                            let _ = self_ref.tx_fec.send((meta, payload));
                        }
                        Kind::Control => {
                            let _ = self_ref.tx_control.send((meta, payload));
                        }
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

    fn subscribe(&self, kind: Kind) -> tokio::sync::mpsc::Receiver<(PacketMeta, Vec<u8>)> {
        let mut rx = match kind {
            Kind::Beacon => self.tx_beacon.subscribe(),
            Kind::Fec => self.tx_fec.subscribe(),
            Kind::Control => self.tx_control.subscribe(),
        };
        let (tx, client_rx) = tokio::sync::mpsc::channel(100);
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

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Launcher {
        #[arg(short, long)]
        nodes: usize,
        #[arg(short, long)]
        simulation: bool,
        #[arg(short, long, default_value = "wlan0")]
        iface: String,
    },
    Node {
        #[arg(short, long)]
        id: NodeId,
        #[arg(long, default_value = "")]
        ignore: String,
        #[arg(short, long)]
        simulation: bool,
        #[arg(short, long, default_value = "wlan0")]
        iface: String,
    },
    SendImage {
        #[arg(short, long)]
        id: NodeId,
        #[arg(short, long)]
        path: String,
    },
    SendText {
        #[arg(short, long)]
        id: NodeId,
        #[arg(short, long)]
        message: String,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Launcher { nodes, simulation, iface } => {
            info!("Launching {} nodes on interface {}", nodes, iface);
            let mut children = Vec::new();
            for i in 1..=nodes {
                let id = i as NodeId;
                let mut ignore = Vec::new();
                // Simple topology: line topology (1-2-3-4-5)
                for j in 1..=nodes {
                    let j = j as NodeId;
                    if id.abs_diff(j) > 1 && id != j {
                        ignore.push(j.to_string());
                    }
                }
                let ignore_str = ignore.join(",");

                let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
                cmd.arg("node").arg("--id").arg(id.to_string());
                if !ignore_str.is_empty() {
                    cmd.arg("--ignore").arg(ignore_str);
                }
                if simulation {
                    cmd.arg("--simulation");
                }
                cmd.arg("--iface").arg(&iface);
                let child = cmd.spawn().expect("failed to spawn node");
                children.push(child);
            }

            info!("All nodes launched. Press Ctrl+C to stop.");

            // Wait for Ctrl+C
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down nodes...");
                }
                _ = std::future::pending::<()>() => {}
            }

            for mut child in children {
                let _ = child.kill();
            }
        }
        Commands::Node { id, ignore, simulation, iface } => {
            info!("Starting Node {} on interface {}", id, iface);
            let ignored_nodes = ignore
                .split(',')
                .filter_map(|s| s.parse::<NodeId>().ok())
                .collect::<Vec<_>>();

            let transport: Arc<dyn Transport> = if simulation {
                MockTransport::new(id, ignored_nodes).await
            } else {
                let mut root_key = [0u8; 32];
                let cfg = litm_transport::WifiTransportConfig {
                    local_id: id,
                    radio: litm_transport::RadioConfig {
                        iface,
                        ..litm_transport::RadioConfig::default()
                    },
                    root_key,
                };
                litm_transport::WifiTransport::start(cfg).expect("Failed to start WifiTransport")
            };

            let delivery = RaptorQDelivery::new(Arc::clone(&transport));
            let _mesh = MeshService::new(
                Arc::clone(&transport),
                delivery.clone(),
            );

            // LOGGING: Listen for received objects and print them
            let mut delivery_rx = delivery.subscribe();
            tokio::spawn(async move {
                while let Some(obj) = delivery_rx.recv().await {
                    let content_preview = if let Ok(text) = String::from_utf8(obj.payload.clone()) {
                        format!("\"{}\"", text)
                    } else {
                        format!("{} bytes (binary)", obj.payload.len())
                    };
                    info!(
                        "Node {} RECEIVED object {} from Node {}: {}",
                        id, obj.id, obj.source, content_preview
                    );
                }
            });

            // Command socket for send-image
            let listener = {
                let socket = socket2::Socket::new(
                    socket2::Domain::IPV4,
                    socket2::Type::STREAM,
                    Some(socket2::Protocol::TCP),
                )
                .unwrap();
                socket.set_reuse_address(true).unwrap();
                socket
                    .bind(
                        &format!("127.0.0.1:{}", 10000 + id)
                            .parse::<std::net::SocketAddr>()
                            .unwrap()
                            .into(),
                    )
                    .unwrap();
                socket.listen(128).unwrap();
                socket.set_nonblocking(true).unwrap();
                TcpListener::from_std(socket.into()).unwrap()
            };

            // Wait for commands
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0; 1024];
                if let Ok(n) = stream.read(&mut buf).await {
                    let cmd_str = String::from_utf8_lossy(&buf[..n]);
                    if cmd_str.starts_with("send-image") {
                        info!(
                            "Node {} triggering real RaptorQ send_object (fake image)",
                            id
                        );
                        let _ = delivery.send_object(
                            rand::random(),
                            vec![0xAA; 5000], // 5KB fake image
                            SendPolicy::default(),
                        );
                    } else if cmd_str.starts_with("send-text") {
                        let text = cmd_str.strip_prefix("send-text ").unwrap_or("Hello Mesh!");
                        info!("Node {} sending text: {}", id, text);
                        let _ = delivery.send_object(
                            rand::random(),
                            text.as_bytes().to_vec(),
                            SendPolicy::default(),
                        );
                    }
                }
            }
        }
        Commands::SendImage { id, path } => {
            if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", 10000 + id)).await {
                let _ = stream
                    .write_all(format!("send-image {}", path).as_bytes())
                    .await;
                info!("Triggered send-image on node {}", id);
            } else {
                warn!("Failed to connect to node {}", id);
            }
        }
        Commands::SendText { id, message } => {
            if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", 10000 + id)).await {
                let _ = stream
                    .write_all(format!("send-text {}", message).as_bytes())
                    .await;
                info!("Triggered send-text on node {}", id);
            } else {
                warn!("Failed to connect to node {}", id);
            }
        }
    }
}
