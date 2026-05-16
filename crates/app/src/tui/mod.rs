mod app;
mod camera;
mod events;
mod ui;

use std::io::stdout;
use std::time::Duration;

use app_sdk::{MessagePayload, NodeBuilder, ReceivedMessage, SdkError, VideoCodec};
use crossterm::{
    event::EventStream,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use image::DynamicImage;
use litm_common::NodeId;
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::picker::Picker;
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

    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let (cam_cmd_tx, cam_cmd_rx) = mpsc::channel::<bool>(4);
    let (preview_tx, mut preview_rx) = mpsc::channel::<DynamicImage>(2);

    let node_for_cam = node.clone();
    std::thread::spawn(move || {
        camera::run_capture(node_for_cam, cam_cmd_rx, preview_tx);
    });

    let mut app = App::new(id, node.clone(), cam_cmd_tx);
    let mut mesh_rx = node.subscribe();
    let mut term_events = EventStream::new();
    let mut topo_tick = tokio::time::interval(Duration::from_secs(1));

    loop {
        terminal.draw(|f| ui::render(f, &app, &mut picker))?;

        let event = tokio::select! {
            maybe = term_events.next() => match maybe {
                Some(Ok(e)) => AppEvent::Terminal(e),
                _ => break,
            },
            msg = mesh_rx.recv() => match msg {
                Ok(ReceivedMessage {
                    source,
                    payload: MessagePayload::VideoFrame { seq, width, height, codec, data },
                    ..
                }) => {
                    let decoded: Option<DynamicImage> = match codec {
                        VideoCodec::Mjpeg => {
                            image::load_from_memory_with_format(&data, image::ImageFormat::Jpeg).ok()
                        }
                        VideoCodec::Raw => {
                            image::RgbImage::from_raw(width as u32, height as u32, data)
                                .map(DynamicImage::ImageRgb8)
                        }
                        VideoCodec::H264 => None,
                    };
                    match decoded {
                        Some(img) => AppEvent::VideoFrame(DecodedVideoFrame { source, seq, image: img }),
                        None => continue,
                    }
                }
                Ok(ReceivedMessage { source, payload: MessagePayload::Text { content }, .. }) => {
                    AppEvent::MeshMessage { source, content }
                }
                Ok(_) => continue,
                Err(SdkError::Lagged) => continue,
                Err(_) => AppEvent::MeshClosed,
            },
            maybe_preview = preview_rx.recv() => match maybe_preview {
                Some(img) => AppEvent::LocalPreview(img),
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
