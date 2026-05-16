mod app;
mod events;
mod ui;

use std::io::stdout;
use std::time::Duration;

use app_sdk::{MessagePayload, NodeBuilder, ReceivedMessage, SdkError};
use crossterm::{
    event::EventStream,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use litm_common::NodeId;
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;
use events::AppEvent;

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

    let mut app = App::new(id, node.clone());
    let mut mesh_rx = node.subscribe();
    let mut term_events = EventStream::new();
    let mut topo_tick = tokio::time::interval(Duration::from_secs(1));

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

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
            _ = topo_tick.tick() => AppEvent::TopologyTick,
        };

        app.handle_event(event);

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
