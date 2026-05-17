use axum::{
    Router,
    extract::{
        DefaultBodyLimit, Json, Multipart, Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;
use litm_common::{NeighborInfo, NodeId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::info;

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
    images: Arc<Mutex<std::collections::HashMap<u32, (String, Vec<u8>)>>>,
    video_ch: Arc<VideoChannel>,
    active_streams:
        Arc<Mutex<std::collections::HashMap<NodeId, (u64, Vec<u8>)>>>,
}

#[derive(Serialize, Clone)]
struct ApiMessage {
    id: u32,
    source: NodeId,
    text: Option<String>,
    image_id: Option<u32>,
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
struct ChannelHopRequest {
    channel: u8,
}

async fn get_data(State(state): State<AppState>) -> Json<GodData> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
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
    mut multipart: Multipart,
) -> Json<serde_json::Value> {
    let mut text: Option<String> = None;
    let mut image_mime: Option<String> = None;
    let mut image_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("text") => {
                if let Ok(t) = field.text().await {
                    if !t.trim().is_empty() {
                        text = Some(t);
                    }
                }
            }
            Some("image") => {
                image_mime = field.content_type().map(|s| s.to_string());
                if let Ok(b) = field.bytes().await {
                    image_bytes = Some(b.to_vec());
                }
            }
            _ => {}
        }
    }

    let mut sent = false;
    let mut sent_id: Option<u32> = None;

    if let Some(t) = text {
        info!("Sending text message");
        match state.node.send(MessagePayload::Text { content: t }) {
            Ok(id) => { sent_id = Some(id); sent = true; }
            Err(e) => {
                tracing::error!("Failed to send text: {}", e);
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
        }
    }

    if let (Some(bytes), Some(mime)) = (image_bytes, image_mime) {
        info!("Sending image ({} bytes)", bytes.len());
        match state.node.send(MessagePayload::Image { mime, bytes }) {
            Ok(id) => { sent_id = Some(id); sent = true; }
            Err(e) => {
                tracing::error!("Failed to send image: {}", e);
                return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
            }
        }
    }

    if sent {
        let mut resp = serde_json::json!({ "status": "ok" });
        if let Some(id) = sent_id {
            resp["id"] = serde_json::json!(id);
        }
        Json(resp)
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Nothing to send" }))
    }
}

async fn send_video_frame(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Json<serde_json::Value> {
    match state.video_ch.send_frame(&body) {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })),
        Err(e) => {
            tracing::error!("Failed to send video frame: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

async fn get_image(Path(id): Path<u32>, State(state): State<AppState>) -> impl IntoResponse {
    let map = state.images.lock().unwrap();
    if let Some((mime, bytes)) = map.get(&id) {
        let mut headers = HeaderMap::new();
        if let Ok(val) = mime.parse() {
            headers.insert(header::CONTENT_TYPE, val);
        }
        (StatusCode::OK, headers, bytes.clone()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn get_latest_frame(Path(id): Path<u32>, State(state): State<AppState>) -> impl IntoResponse {
    let bytes = {
        let map = state.active_streams.lock().unwrap();
        map.get(&id).map(|(_, bytes)| bytes.clone())
    };

    if let Some(bytes) = bytes {
        if bytes.is_empty() {
            return StatusCode::NO_CONTENT.into_response();
        }
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("image/jpeg"));
        headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-cache, no-store"));
        (StatusCode::OK, headers, bytes).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
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

async fn channel_hop(
    State(state): State<AppState>,
    Json(req): Json<ChannelHopRequest>,
) -> Json<serde_json::Value> {
    info!("Channel hop requested to CH{}", req.channel);
    match state.node.request_channel_hop(req.channel) {
        Ok(_) => Json(serde_json::json!({ "status": "ok" })),
        Err(e) => {
            tracing::error!("Channel hop failed: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    info!(
        "Starting LITM API for Node {} on interface {}",
        cli.id, cli.iface
    );

    let node = NodeBuilder::new(cli.id, &cli.password, &cli.iface)
        .build()
        .expect("Failed to start node. Is the interface in monitor mode?");

    let messages = Arc::new(Mutex::new(Vec::<ApiMessage>::new()));
    let images: Arc<Mutex<std::collections::HashMap<u32, (String, Vec<u8>)>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));
    let messages_clone = Arc::clone(&messages);
    let images_clone = Arc::clone(&images);
    let mut rx = node.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let (text, image_id) = match msg.payload {
                        MessagePayload::Text { content } => {
                            info!("Received text message from {}", msg.source);
                            (Some(content), None)
                        }
                        MessagePayload::Image { mime, bytes } => {
                            info!(
                                "Received image message from {} ({} bytes)",
                                msg.source,
                                bytes.len()
                            );
                            images_clone.lock().unwrap().insert(msg.id, (mime, bytes));
                            (None, Some(msg.id))
                        }
                        _ => {
                            info!("Received other message payload from {}", msg.source);
                            (None, None)
                        }
                    };

                    if text.is_some() || image_id.is_some() {
                        let mut msgs = messages_clone.lock().unwrap();
                        msgs.push(ApiMessage {
                            id: msg.id,
                            source: msg.source,
                            text,
                            image_id,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                        if msgs.len() > 50 {
                            let pruned = msgs.remove(0);
                            if let Some(iid) = pruned.image_id {
                                images_clone.lock().unwrap().remove(&iid);
                            }
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
    let active_streams: Arc<Mutex<std::collections::HashMap<NodeId, (u64, Vec<u8>)>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));
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
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut map = streams_clone.lock().unwrap();

            let entry = map.entry(frame.source).or_insert_with(|| {
                info!(source = frame.source, "New active stream registered");
                (now, Vec::new())
            });
            entry.0 = now;
            entry.1 = frame.jpeg_bytes;

            map.retain(|_, (ts, _)| now - *ts < 10);
        }
        tracing::warn!("Video stream receiver task ended — channel closed");
    });

    let state = AppState {
        node,
        messages,
        images,
        video_ch,
        active_streams,
    };

    let app = Router::new()
        .route("/api/data", get(get_data))
        .route("/api/send", post(send_message).layer(DefaultBodyLimit::max(20 * 1024 * 1024)))
        .route("/api/video/frame", post(send_video_frame).layer(DefaultBodyLimit::max(2 * 1024 * 1024)))
        .route("/api/video/latest/:id", get(get_latest_frame))
        .route("/api/image/:id", get(get_image))
        .route("/api/raptor/ws", get(raptor_ws_handler))
        .route("/api/channel/hop", post(channel_hop))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("API listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
