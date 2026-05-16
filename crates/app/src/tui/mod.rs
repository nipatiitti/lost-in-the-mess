mod app;
mod camera;
mod events;
mod ui;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use app_sdk::{MessagePayload, NodeBuilder, ReceivedMessage, SdkError, VideoChannel, VideoQuality};
use crossterm::{
    event::EventStream,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use image::DynamicImage;
use litm_common::NodeId;
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use tokio::sync::mpsc;

use app::App;
use events::{AppEvent, DecodedVideoFrame};

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

pub async fn run(id: NodeId, password: &str, iface: &str) -> anyhow::Result<()> {
    let node = NodeBuilder::new(id, password, iface).build()?;

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    // Video channel: bypasses FEC/delivery, goes straight to transport.
    let video_ch = VideoChannel::new(node.transport());

    let (cam_cmd_tx, cam_cmd_rx) = mpsc::channel::<bool>(4);
    let (preview_raw_tx, mut preview_raw_rx) = mpsc::channel::<Vec<u8>>(2);
    let (preview_decoded_tx, mut preview_decoded_rx) = mpsc::channel::<DynamicImage>(2);

    // Camera capture runs in a blocking thread; it writes JPEG into VideoChannel.
    let video_ch_for_cam = Arc::clone(&video_ch);
    let rt_handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        let _guard = rt_handle.enter();
        camera::run_capture(video_ch_for_cam, VideoQuality::Balanced, cam_cmd_rx, preview_raw_tx);
    });

    // Decode local camera preview for the bottom half of the Video panel.
    tokio::spawn(async move {
        while let Some(jpeg) = preview_raw_rx.recv().await {
            let result = tokio::task::spawn_blocking(move || {
                image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
                    .ok()
                    .map(|img| img.thumbnail(480, 480))
            })
            .await;
            if let Ok(Some(img)) = result {
                let _ = preview_decoded_tx.try_send(img);
            }
        }
    });

    // Subscribe to the video channel for frames arriving from remote nodes.
    let mut video_frame_rx = video_ch.subscribe();
    let (video_decoded_tx, mut video_decoded_rx) = mpsc::channel::<DecodedVideoFrame>(1);

    tokio::spawn(async move {
        while let Some(frame) = video_frame_rx.recv().await {
            let source = frame.source;
            let frame_id = frame.frame_id;
            let chunks_received = frame.chunks_received;
            let chunks_total = frame.chunks_total;
            let jpeg = frame.jpeg_bytes;
            let result = tokio::task::spawn_blocking(move || {
                image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
                    .ok()
                    .map(|img| DecodedVideoFrame {
                        source,
                        frame_id,
                        image: img.thumbnail(480, 480),
                        chunks_received,
                        chunks_total,
                    })
            })
            .await;
            if let Ok(Some(decoded)) = result {
                let _ = video_decoded_tx.try_send(decoded);
            }
        }
    });

    let mut app = App::new(id, node.clone(), cam_cmd_tx);
    let mut mesh_rx = node.subscribe();
    let mut telemetry_rx = node.subscribe_telemetry();
    let mut term_events = EventStream::new();
    let mut topo_tick = tokio::time::interval(Duration::from_secs(1));

    let mut local_preview_proto: Option<StatefulProtocol> = None;
    let mut remote_video_proto: Option<StatefulProtocol> = None;

    loop {
        terminal.draw(|f| ui::render(f, &app, &mut local_preview_proto, &mut remote_video_proto))?;

        let event = tokio::select! {
            maybe = term_events.next() => match maybe {
                Some(Ok(e)) => AppEvent::Terminal(e),
                _ => break,
            },
            msg = mesh_rx.recv() => match msg {
                Ok(ReceivedMessage { source, payload: MessagePayload::Text { content }, .. }) => {
                    AppEvent::MeshMessage { source, content }
                }
                Ok(_) => continue,
                Err(SdkError::Lagged) => continue,
                Err(_) => AppEvent::MeshClosed,
            },
            maybe_decoded = video_decoded_rx.recv() => match maybe_decoded {
                Some(frame) => {
                    remote_video_proto = Some(picker.new_resize_protocol(frame.image.clone()));
                    AppEvent::VideoFrame(frame)
                }
                None => continue,
            },
            maybe_preview = preview_decoded_rx.recv() => match maybe_preview {
                Some(img) => {
                    local_preview_proto = Some(picker.new_resize_protocol(img.clone()));
                    AppEvent::LocalPreview(img)
                }
                None => continue,
            },
            telemetry = telemetry_rx.recv() => match telemetry {
                Some(event) => AppEvent::RaptorTelemetry(event),
                None => continue,
            },
            _ = topo_tick.tick() => AppEvent::TopologyTick,
        };

        app.handle_event(event);

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
