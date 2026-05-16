use axum::{
    extract::{State, Json},
    routing::{get, post},
    Router,
};
use litm_common::{Delivery, Mesh, NodeId, NeighborInfo, SendPolicy};
use litm_delivery::RaptorQDelivery;
use litm_mesh::MeshService;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::info;

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
}

#[derive(Deserialize)]
struct SendRequest {
    text: String,
}

async fn get_data(State(state): State<AppState>) -> Json<GodData> {
    let neighbors = state.mesh.neighbors();
    let messages = state.messages.lock().unwrap().clone();
    Json(GodData {
        local_id: state.local_id,
        neighbors,
        messages,
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

    // For hackathon simplicity, we hardcode some defaults or use env/args.
    let id: NodeId = std::env::var("NODE_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .unwrap();

    info!("Starting LITM API for Node {}", id);

    // Initialize transport (using MockTransport for simulation if needed, 
    // but here we'll assume we can use the real one or a mock).
    // For the demo, we'll use a simple loopback or real radio if available.
    
    // TODO: Connect to real transport or mock. 
    // For now, let's assume we are running in simulation mode or have a radio.
    // In a real hackathon, you'd pass the interface name.
    
    let transport: Arc<dyn litm_common::Transport> = if std::env::var("SIMULATION").is_ok() {
        litm_transport::MockTransport::new(id).await
    } else {
        let root_key = [0u8; 32];
        let cfg = litm_transport::WifiTransportConfig {
            local_id: id,
            radio: litm_transport::RadioConfig {
                iface: std::env::var("IFACE").unwrap_or_else(|_| "wlan0".to_string()),
                ..litm_transport::RadioConfig::default()
            },
            root_key,
        };
        litm_transport::WifiTransport::start(cfg)
            .expect("Failed to start transport. Is wlan0 in monitor mode? (Try SIMULATION=1)")
    };

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
        local_id: id,
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
