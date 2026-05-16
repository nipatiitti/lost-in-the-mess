use axum::{
    extract::{State, Json},
    routing::{get, post},
    Router,
};
use clap::Parser;
use litm_common::{Delivery, Mesh, NodeId, NeighborInfo, SendPolicy};
use litm_delivery::RaptorQDelivery;
use litm_mesh::MeshService;
use litm_transport::{derive_root_key, WifiTransport, WifiTransportConfig, RadioConfig};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::info;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'n', long)]
    id: NodeId,
    #[arg(short, long, default_value = "litm")]
    password: String,
    #[arg(short, long, default_value = "wlan0")]
    iface: String,
}

#[derive(Clone)]
struct AppState {
    local_id: NodeId,
    mesh: Arc<dyn Mesh>,
    delivery: Arc<dyn Delivery>,
    messages: Arc<Mutex<Vec<ReceivedMessage>>>,
}

#[derive(Serialize, Clone)]
struct ReceivedMessage {
    id: u32,
    source: NodeId,
    text: String,
    timestamp: u64,
}

#[derive(Serialize)]
struct GodData {
    local_id: NodeId,
    neighbors: Vec<NeighborInfo>,
    messages: Vec<ReceivedMessage>,
    topology: std::collections::HashMap<NodeId, Vec<(NodeId, f32)>>,
}

#[derive(Deserialize)]
struct SendRequest {
    text: String,
}

async fn get_data(State(state): State<AppState>) -> Json<GodData> {
    let neighbors = state.mesh.neighbors();
    let topology = state.mesh.topology();
    let messages = state.messages.lock().unwrap().clone();
    Json(GodData {
        local_id: state.local_id,
        neighbors,
        messages,
        topology,
    })
}

async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Json<serde_json::Value> {
    info!("Sending message: {}", req.text);
    let _ = state.delivery.send_object(
        rand::random(),
        req.text.as_bytes().to_vec(),
        SendPolicy::default(),
    );
    Json(serde_json::json!({ "status": "ok" }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    info!("Starting LITM API for Node {} on interface {}", cli.id, cli.iface);

    let root_key = derive_root_key(&cli.password);
    let cfg = WifiTransportConfig {
        local_id: cli.id,
        radio: RadioConfig {
            iface: cli.iface,
            ..RadioConfig::default()
        },
        root_key,
    };

    let transport: Arc<dyn litm_common::Transport> = WifiTransport::start(cfg)
        .expect("Failed to start transport. Is the interface in monitor mode?");

    let delivery = RaptorQDelivery::new(Arc::clone(&transport));
    let mesh = MeshService::new(
        Arc::clone(&transport),
        delivery.clone(),
    );

    let messages = Arc::new(Mutex::new(Vec::new()));
    let messages_clone = Arc::clone(&messages);
    let mut delivery_rx = delivery.subscribe();

    tokio::spawn(async move {
        while let Some(obj) = delivery_rx.recv().await {
            let text = String::from_utf8_lossy(&obj.payload).to_string();
            let mut msgs = messages_clone.lock().unwrap();
            msgs.push(ReceivedMessage {
                id: obj.id,
                source: obj.source,
                text,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            });
            if msgs.len() > 50 {
                msgs.remove(0);
            }
        }
    });

    let state = AppState {
        local_id: cli.id,
        mesh,
        delivery,
        messages,
    };

    let app = Router::new()
        .route("/api/data", get(get_data))
        .route("/api/send", post(send_message))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("API listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
