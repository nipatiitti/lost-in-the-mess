use axum::{
    Router,
    extract::{Json, State, Path, ws::{WebSocketUpgrade, WebSocket, Message}},
    routing::{get, post},
    response::IntoResponse,
    body::Body,
};
use tokio_stream::wrappers::ReceiverStream;
use clap::Parser;
use litm_common::{NeighborInfo, NodeId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::info;
use base64::{Engine as _, prelude::BASE64_STANDARD};

use app_sdk::{MessagePayload, Node, NodeBuilder, VideoChannel};

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
    video_ch: Arc<VideoChannel>,
    active_streams: Arc<Mutex<std::collections::HashMap<NodeId, (u64, tokio::sync::watch::Sender<Vec<u8>>)> >>,
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
    active_streams: Vec<NodeId>,
    radio: app_sdk::RadioInfo,
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
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let mut active_streams = Vec::new();
    {
        let mut map = state.active_streams.lock().unwrap();
        map.retain(|_, (ts, _)| now - *ts < 10);
        active_streams.extend(map.keys().copied());
    }
    active_streams.sort();

    Json(GodData {
        local_id: state.node.local_id(),
        neighbors: state.node.neighbors(),
        topology: state.node.topology(),
        messages: state.messages.lock().unwrap().clone(),
        active_streams,
        radio: state.node.radio_info(),
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

    match state.video_ch.send_frame(&data) {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })),
        Err(e) => {
            tracing::error!("Failed to send video frame: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

async fn get_video_stream(
    Path(id): Path<u32>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rx = {
        let map = state.active_streams.lock().unwrap();
        map.get(&id).map(|(_, tx)| tx.subscribe())
    };

    if let Some(mut rx) = rx {
        let (tx, axum_rx) = tokio::sync::mpsc::channel::<Result<axum::body::Bytes, std::convert::Infallible>>(4);
        
        tokio::spawn(async move {
            let mut last_len = 0;
            loop {
                let bytes = rx.borrow_and_update().clone();
                if !bytes.is_empty() && bytes.len() != last_len {
                    let mut data = Vec::new();
                    data.extend_from_slice(b"--frame\r\nContent-Type: image/jpeg\r\n\r\n");
                    data.extend_from_slice(&bytes);
                    data.extend_from_slice(b"\r\n");
                    if tx.send(Ok(axum::body::Bytes::from(data))).await.is_err() {
                        break;
                    }
                    last_len = bytes.len();
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
        });
        
        let body = Body::from_stream(ReceiverStream::new(axum_rx));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::header::HeaderValue::from_static("multipart/x-mixed-replace; boundary=frame"),
        );
        (headers, body).into_response()
    } else {
        axum::http::StatusCode::NOT_FOUND.into_response()
    }
}

async fn raptor_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut rx = state.node.subscribe_telemetry();
    ws.on_upgrade(move |mut socket: WebSocket| async move {
        while let Some(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    })
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

    let video_ch = VideoChannel::new(node.transport());
    let mut video_rx = video_ch.subscribe();
    let active_streams: Arc<Mutex<std::collections::HashMap<NodeId, (u64, tokio::sync::watch::Sender<Vec<u8>>)> >> = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let streams_clone = active_streams.clone();

    tokio::spawn(async move {
        info!("Video stream receiver task started");
        while let Some(frame) = video_rx.recv().await {
            info!(
                source = frame.source,
                frame_id = frame.frame_id,
                chunks = %format!("{}/{}", frame.chunks_received, frame.chunks_total),
                jpeg_len = frame.jpeg_bytes.len(),
                "Received video frame from mesh"
            );
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let mut map = streams_clone.lock().unwrap();
            
            if let Some((ts, tx)) = map.get_mut(&frame.source) {
                *ts = now;
                let _ = tx.send(frame.jpeg_bytes);
            } else {
                let (tx, _rx) = tokio::sync::watch::channel(frame.jpeg_bytes);
                map.insert(frame.source, (now, tx));
                info!(source = frame.source, "New active stream registered");
            }
            
            map.retain(|_, (ts, _)| now - *ts < 10);
        }
        tracing::warn!("Video stream receiver task ended — channel closed");
    });

    let state = AppState { node, messages, video_ch, active_streams };

    let app = Router::new()
        .route("/api/data", get(get_data))
        .route("/api/send", post(send_message))
        .route("/api/video/frame", post(send_video_frame))
        .route("/api/video/stream/:id", get(get_video_stream))
        .route("/api/raptor/ws", get(raptor_ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("API listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
