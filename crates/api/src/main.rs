use axum::{
    Router,
    extract::{Json, State},
    routing::{get, post},
};
use clap::Parser;
use litm_common::{NeighborInfo, NodeId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::info;

use app_sdk::{MessagePayload, Node, NodeBuilder};

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
    node: Node,
    messages: Arc<Mutex<Vec<ApiMessage>>>,
}

#[derive(Serialize, Clone)]
struct ApiMessage {
    id: u32,
    source: NodeId,
    text: String,
    timestamp: u64,
}

#[derive(Serialize)]
struct GodData {
    local_id: NodeId,
    neighbors: Vec<NeighborInfo>,
    messages: Vec<ApiMessage>,
    topology: std::collections::HashMap<NodeId, Vec<(NodeId, f32)>>,
}

#[derive(Deserialize)]
struct SendRequest {
    text: String,
}

async fn get_data(State(state): State<AppState>) -> Json<GodData> {
    Json(GodData {
        local_id: state.node.local_id(),
        neighbors: state.node.neighbors(),
        topology: state.node.topology(),
        messages: state.messages.lock().unwrap().clone(),
    })
}

async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Json<serde_json::Value> {
    info!("Sending message: {}", req.text);
    let _ = state.node.send(MessagePayload::Text { content: req.text });
    Json(serde_json::json!({ "status": "ok" }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    info!("Starting LITM API for Node {} on interface {}", cli.id, cli.iface);

    let node = NodeBuilder::new(cli.id, &cli.password, &cli.iface)
        .build()
        .expect("Failed to start node. Is the interface in monitor mode?");

    let messages = Arc::new(Mutex::new(Vec::<ApiMessage>::new()));
    let messages_clone = Arc::clone(&messages);
    let mut rx = node.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if let MessagePayload::Text { content } = msg.payload {
                        let mut msgs = messages_clone.lock().unwrap();
                        msgs.push(ApiMessage {
                            id: msg.id,
                            source: msg.source,
                            text: content,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                        if msgs.len() > 50 {
                            msgs.remove(0);
                        }
                    }
                }
                Err(app_sdk::SdkError::Lagged) => {
                    tracing::warn!("message subscriber lagged — some messages dropped");
                }
                Err(_) => break,
            }
        }
    });

    let state = AppState { node, messages };

    let app = Router::new()
        .route("/api/data", get(get_data))
        .route("/api/send", post(send_message))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("API listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
