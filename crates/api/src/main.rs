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
use base64::{Engine as _, prelude::BASE64_STANDARD};

use app_sdk::{MessagePayload, Node, NodeBuilder, VideoCodec, VideoStreamer};

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
    video_streamer: Arc<Mutex<Option<VideoStreamer>>>,
}

#[derive(Serialize, Clone)]
struct ApiMessage {
    id: u32,
    source: NodeId,
    text: Option<String>,
    image: Option<String>, // Data URI
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
    text: Option<String>,
    image: Option<String>, // Data URI
}

#[derive(Deserialize)]
struct SendVideoFrameRequest {
    width: u16,
    height: u16,
    data: String, // base64-encoded JPEG
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
    let mut sent = false;
    
    if let Some(text) = req.text {
        if !text.trim().is_empty() {
            info!("Sending text message");
            if let Err(e) = state.node.send(MessagePayload::Text { content: text }) {
                tracing::error!("Failed to send text: {}", e);
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
            sent = true;
        }
    }
    
    if let Some(image_uri) = req.image {
        info!("Sending image ({} bytes)", image_uri.len());
        if let Some((mime, data)) = parse_data_uri(&image_uri) {
            if let Err(e) = state.node.send(MessagePayload::Image { mime, bytes: data }) {
                tracing::error!("Failed to send image: {}", e);
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
            sent = true;
        } else {
            tracing::warn!("Failed to parse image data URI");
        }
    }
    
    if sent {
        Json(serde_json::json!({ "status": "ok" }))
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Nothing to send" }))
    }
}

async fn send_video_frame(
    State(state): State<AppState>,
    Json(req): Json<SendVideoFrameRequest>,
) -> Json<serde_json::Value> {
    let data = match BASE64_STANDARD.decode(&req.data) {
        Ok(d) => d,
        Err(e) => {
            return Json(serde_json::json!({ "status": "error", "message": format!("bad base64: {}", e) }));
        }
    };

    let mut guard = state.video_streamer.lock().unwrap();
    let streamer = guard.get_or_insert_with(|| {
        info!("Initializing VideoStreamer (MJPEG)");
        VideoStreamer::new(state.node.clone(), VideoCodec::Mjpeg)
    });

    match streamer.send_frame(req.width, req.height, data) {
        Ok(obj_id) => {
            Json(serde_json::json!({ "status": "ok", "object_id": obj_id }))
        }
        Err(e) => {
            tracing::error!("Failed to send video frame: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

fn parse_data_uri(uri: &str) -> Option<(String, Vec<u8>)> {
    if !uri.starts_with("data:") { return None; }
    let comma_idx = uri.find(',')?;
    let header = &uri[5..comma_idx];
    let data_str = &uri[comma_idx+1..];
    
    let mime = header.split(';').next()?.to_string();
    let is_base64 = header.contains(";base64");
    
    if is_base64 {
        BASE64_STANDARD.decode(data_str).ok().map(|d| (mime, d))
    } else {
        None
    }
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
                    let (text, image) = match msg.payload {
                        MessagePayload::Text { content } => {
                            info!("Received text message from {}", msg.source);
                            (Some(content), None)
                        }
                        MessagePayload::Image { mime, bytes } => {
                            info!("Received image message from {} ({} bytes)", msg.source, bytes.len());
                            let b64 = BASE64_STANDARD.encode(&bytes);
                            (None, Some(format!("data:{};base64,{}", mime, b64)))
                        }
                        _ => {
                            info!("Received other message payload from {}", msg.source);
                            (None, None)
                        }
                    };

                    if text.is_some() || image.is_some() {
                        let mut msgs = messages_clone.lock().unwrap();
                        msgs.push(ApiMessage {
                            id: msg.id,
                            source: msg.source,
                            text,
                            image,
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

    let state = AppState { node, messages, video_streamer: Arc::new(Mutex::new(None)) };

    let app = Router::new()
        .route("/api/data", get(get_data))
        .route("/api/send", post(send_message))
        .route("/api/video/frame", post(send_video_frame))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("API listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
