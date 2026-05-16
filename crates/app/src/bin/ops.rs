use clap::{Parser, Subcommand};
use litm_common::{Delivery, Mesh, NodeId, SendPolicy, Transport};
use litm_delivery::RaptorQDelivery;
use litm_mesh::MeshService;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

fn derive_root_key(password: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(password.as_bytes()).into()
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
        #[arg(short = 'p', long, default_value = "litm")]
        password: String,
        #[arg(short, long, default_value = "wlan0")]
        iface: String,
    },
    Node {
        #[arg(short, long)]
        id: NodeId,
        #[arg(short = 'p', long, default_value = "litm")]
        password: String,
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
        Commands::Launcher { nodes, password, iface } => {
            info!("Launching {} nodes on interface {}", nodes, iface);
            let mut children = Vec::new();
            for i in 1..=nodes {
                let id = i as NodeId;
                let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
                cmd.arg("node")
                    .arg("--id").arg(id.to_string())
                    .arg("--password").arg(&password)
                    .arg("--iface").arg(&iface);
                let child = cmd.spawn().expect("failed to spawn node");
                children.push(child);
            }

            info!("All nodes launched. Press Ctrl+C to stop.");
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

        Commands::Node { id, password, iface } => {
            info!("Starting Node {} on interface {}", id, iface);

            let transport: Arc<dyn Transport> = {
                let cfg = litm_transport::WifiTransportConfig {
                    local_id: id,
                    radio: litm_transport::RadioConfig {
                        iface,
                        ..litm_transport::RadioConfig::default()
                    },
                    root_key: derive_root_key(&password),
                };
                litm_transport::WifiTransport::start(cfg).expect("Failed to start WifiTransport")
            };

            let delivery = RaptorQDelivery::new(Arc::clone(&transport));
            let mesh = MeshService::new(Arc::clone(&transport), delivery.clone());

            // Log received objects (delivery layer)
            let mut delivery_rx = delivery.subscribe();
            tokio::spawn(async move {
                while let Some(obj) = delivery_rx.recv().await {
                    let preview = if let Ok(text) = String::from_utf8(obj.payload.clone()) {
                        format!("\"{}\"", text)
                    } else {
                        format!("{} bytes (binary)", obj.payload.len())
                    };
                    info!("Node {} received object {} from {}: {}", id, obj.id, obj.source, preview);
                }
            });

            // Log mesh neighbor table every 5 s (mesh layer)
            let mesh_log = Arc::clone(&mesh);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    let neighbors = mesh_log.neighbors();
                    if neighbors.is_empty() {
                        info!("Node {} mesh: no neighbors", id);
                    } else {
                        for n in &neighbors {
                            info!(
                                "Node {} neighbor {} PRR={:.0}% last_seen={:.1}s",
                                id,
                                n.id,
                                n.prr * 100.0,
                                n.last_seen.elapsed().as_secs_f32(),
                            );
                        }
                    }
                }
            });

            // TCP command socket on 127.0.0.1:(10000 + id)
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

            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0; 1024];
                if let Ok(n) = stream.read(&mut buf).await {
                    let cmd_str = String::from_utf8_lossy(&buf[..n]);
                    if cmd_str.starts_with("send-image") {
                        info!("Node {} sending fake image (5 KB)", id);
                        let _ = delivery.send_object(
                            rand::random(),
                            vec![0xAA; 5000],
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
                let _ = stream.write_all(format!("send-image {}", path).as_bytes()).await;
                info!("Triggered send-image on node {}", id);
            } else {
                warn!("Failed to connect to node {}", id);
            }
        }

        Commands::SendText { id, message } => {
            if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", 10000 + id)).await {
                let _ = stream.write_all(format!("send-text {}", message).as_bytes()).await;
                info!("Triggered send-text on node {}", id);
            } else {
                warn!("Failed to connect to node {}", id);
            }
        }
    }
}
