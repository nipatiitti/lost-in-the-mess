use clap::{Parser, Subcommand};
use litm_common::{Delivery, Mesh, NodeId, SendPolicy, Transport};
use litm_delivery::RaptorQDelivery;
use litm_mesh::MeshService;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

use litm_transport::derive_root_key;

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
        #[arg(long)]
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
    StreamVideo {
        #[arg(short, long)]
        id: NodeId,
        #[arg(short = 'd', long, default_value = "/dev/video0")]
        device: String,
    },
    ViewVideo {
        #[arg(short, long)]
        id: NodeId,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Launcher {
            nodes,
            password,
            iface,
        } => {
            info!("Launching {} nodes on interface {}", nodes, iface);
            let mut children = Vec::new();
            for i in 1..=nodes {
                let id = i as NodeId;
                let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
                cmd.arg("node")
                    .arg("--id")
                    .arg(id.to_string())
                    .arg("--password")
                    .arg(&password)
                    .arg("--iface")
                    .arg(&iface);
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

        Commands::Node {
            id,
            password,
            iface,
        } => {
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

            // Count received objects for the status logger
            let objects_rx = Arc::new(AtomicU64::new(0));

            // Log received objects (delivery layer)
            let mut delivery_rx = delivery.subscribe();
            let objects_rx_recv = Arc::clone(&objects_rx);
            tokio::spawn(async move {
                while let Some(obj) = delivery_rx.recv().await {
                    objects_rx_recv.fetch_add(1, Ordering::Relaxed);
                    let preview = if let Ok(text) = String::from_utf8(obj.payload.clone()) {
                        format!("\"{}\"", text)
                    } else {
                        format!("{} bytes (binary)", obj.payload.len())
                    };
                    info!(
                        "Node {} received object {} from {}: {}",
                        id, obj.id, obj.source, preview
                    );
                }
            });

            // Log mesh status every 5 s; log neighbor changes immediately.
            let mesh_log = Arc::clone(&mesh);
            let objects_rx_log = Arc::clone(&objects_rx);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                let mut known_neighbors: std::collections::HashSet<NodeId> =
                    std::collections::HashSet::new();
                loop {
                    interval.tick().await;
                    let mut neighbors = mesh_log.neighbors();
                    neighbors.sort_by_key(|n| n.id);
                    let rx_count = objects_rx_log.load(Ordering::Relaxed);

                    let current_ids: std::collections::HashSet<NodeId> =
                        neighbors.iter().map(|n| n.id).collect();

                    for &appeared in current_ids.difference(&known_neighbors) {
                        info!("▲ node {id} peer {appeared} UP");
                    }
                    for &lost in known_neighbors.difference(&current_ids) {
                        info!("▼ node {id} peer {lost} DOWN");
                    }
                    known_neighbors = current_ids;

                    let topo = mesh_log.topology();
                    let direct_ids: std::collections::HashSet<NodeId> =
                        neighbors.iter().map(|n| n.id).collect();

                    info!(
                        "node {id} | {} neighbor(s) | rx={rx_count}",
                        neighbors.len()
                    );
                    for (i, n) in neighbors.iter().enumerate() {
                        let branch = if i + 1 == neighbors.len() { "└" } else { "├" };
                        let two_hop: Vec<String> = topo
                            .get(&n.id)
                            .map(|links| {
                                links
                                    .iter()
                                    .filter(|(dst, _)| *dst != id && !direct_ids.contains(dst))
                                    .map(|(dst, prr)| format!("{dst}@{:.0}%", prr * 100.0))
                                    .collect()
                            })
                            .unwrap_or_default();

                        if two_hop.is_empty() {
                            info!(
                                "  {branch} peer {peer}  prr={prr:.0}%  rssi={rssi}dBm  seen={seen:.1}s",
                                peer = n.id,
                                prr  = n.prr * 100.0,
                                rssi = n.rssi_dbm,
                                seen = n.last_seen.elapsed().as_secs_f32(),
                            );
                        } else {
                            info!(
                                "  {branch} peer {peer}  prr={prr:.0}%  rssi={rssi}dBm  seen={seen:.1}s  2-hop:[{hops}]",
                                peer = n.id,
                                prr  = n.prr * 100.0,
                                rssi = n.rssi_dbm,
                                seen = n.last_seen.elapsed().as_secs_f32(),
                                hops = two_hop.join(" "),
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
                let mut buf = [0; 4096];
                if let Ok(n) = stream.read(&mut buf).await {
                    let cmd_str = String::from_utf8_lossy(&buf[..n]);
                    if cmd_str.starts_with("send-image") {
                        let path = cmd_str
                            .strip_prefix("send-image ")
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        match tokio::fs::read(&path).await {
                            Ok(bytes) => {
                                info!("Node {} sending image {} ({} bytes)", id, path, bytes.len());
                                let _ = delivery.send_object(
                                    rand::random(),
                                    bytes,
                                    SendPolicy::default(),
                                );
                            }
                            Err(e) => {
                                warn!("Node {} cannot read {}: {}", id, path, e);
                            }
                        }
                    } else if cmd_str.starts_with("send-text") {
                        let text = cmd_str
                            .strip_prefix("send-text ")
                            .unwrap_or("Hello Mesh!")
                            .trim()
                            .to_string();
                        info!("Node {} sending text: {}", id, text);
                        let _ = delivery.send_object(
                            rand::random(),
                            text.into_bytes(),
                            SendPolicy::default(),
                        );
                    } else if cmd_str.starts_with("stream-video") {
                        let device = cmd_str
                            .strip_prefix("stream-video ")
                            .unwrap_or("/dev/video0")
                            .trim()
                            .to_string();
                        info!("Node {} starting video stream from {}", id, device);
                        litm_app::video::stream_video(delivery.clone(), device);
                    } else if cmd_str.starts_with("view-video") {
                        info!("Node {} starting video viewer for a client", id);
                        let delivery_clone = delivery.clone();
                        tokio::spawn(async move {
                            litm_app::video::view_video_tcp(delivery_clone, stream).await;
                        });
                        continue;
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

        Commands::StreamVideo { id, device } => {
            if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", 10000 + id)).await {
                let _ = stream
                    .write_all(format!("stream-video {}", device).as_bytes())
                    .await;
                info!("Triggered stream-video on node {} from {}", id, device);
            } else {
                warn!("Failed to connect to node {}", id);
            }
        }

        Commands::ViewVideo { id } => {
            if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", 10000 + id)).await {
                let _ = stream.write_all(b"view-video\n").await;
                info!("Connected to node {}. Piping video to stdout...", id);
                let mut stdout = tokio::io::stdout();
                if let Err(e) = tokio::io::copy(&mut stream, &mut stdout).await {
                    warn!("Stream ended: {}", e);
                }
            } else {
                warn!("Failed to connect to node {}", id);
            }
        }
    }
}
