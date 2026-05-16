use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};
use ratatui_image::{StatefulImage, protocol::StatefulProtocol};

use super::app::{ActivePanel, App};

pub fn render(
    f: &mut Frame,
    app: &App,
    local_preview_proto: &mut Option<StatefulProtocol>,
    remote_video_proto: &mut Option<StatefulProtocol>,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(f.area());

    render_header(f, rows[0], app);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)])
        .split(rows[1]);

    render_neighbors(f, cols[0], app);

    match app.active_panel {
        ActivePanel::Topology => render_topology(f, cols[1], app),
        ActivePanel::Messages => render_messages_compose(f, cols[1], app),
        ActivePanel::Video => render_video(f, cols[1], app, local_preview_proto, remote_video_proto),
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let highlight = Style::default().add_modifier(Modifier::REVERSED);
    let normal = Style::default();

    let mut spans = vec![
        Span::styled(
            format!(" Node {} ", app.local_id),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ];

    for (label, panel) in [
        ("Topology", ActivePanel::Topology),
        ("Messages", ActivePanel::Messages),
        ("Video",    ActivePanel::Video),
    ] {
        let style = if app.active_panel == panel { highlight } else { normal };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled("[Tab] cycle  [^Q] Quit", normal));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_neighbors(f: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(["ID", "PRR", "RSSI"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app.neighbors.iter().map(|n| {
        Row::new([
            Cell::from(n.id.to_string()),
            Cell::from(format!("{:.0}%", n.prr * 100.0)),
            Cell::from(format!("{}dBm", n.rssi_dbm)),
        ])
    }).collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Min(7),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Neighbors "));

    f.render_widget(table, area);
}

fn render_topology(f: &mut Frame, area: Rect, app: &App) {
    let mut edges: Vec<ListItem> = Vec::new();
    let mut sorted_keys: Vec<_> = app.topology.keys().copied().collect();
    sorted_keys.sort();

    for src in sorted_keys {
        if let Some(dests) = app.topology.get(&src) {
            let mut dests = dests.clone();
            dests.sort_by_key(|(id, _)| *id);
            for (dst, prr) in dests {
                let style = if src == app.local_id || dst == app.local_id {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                edges.push(ListItem::new(Line::from(Span::styled(
                    format!("{src} → {dst}  ({:.0}%)", prr * 100.0),
                    style,
                ))));
            }
        }
    }

    let list = List::new(edges)
        .block(Block::default().borders(Borders::ALL).title(" Mesh Topology "));

    f.render_widget(list, area);
}

fn render_messages_compose(f: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    render_messages(f, rows[0], app);
    render_compose(f, rows[1], app);
}

fn render_messages(f: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app.messages.iter().map(|m| {
        Line::from(vec![
            Span::styled(
                m.received_at.format("%H:%M:%S").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(
                format!("from:{}", m.source),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::raw(m.content.clone()),
        ])
    }).collect();

    let scroll = app.messages_scroll as u16;
    let para = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Messages "))
        .scroll((scroll, 0));

    f.render_widget(para, area);
}

fn render_compose(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Compose  Enter=send  Esc=clear ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prefix = "> ";
    let display = format!("{}{}", prefix, app.compose_input);
    f.render_widget(Paragraph::new(display.as_str()), inner);

    let cursor_x = inner.x + prefix.len() as u16 + app.compose_cursor as u16;
    f.set_cursor_position((cursor_x, inner.y));
}

fn render_video(
    f: &mut Frame,
    area: Rect,
    app: &App,
    local_preview_proto: &mut Option<StatefulProtocol>,
    remote_video_proto: &mut Option<StatefulProtocol>,
) {
    let halves = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_received_video(f, halves[0], app, remote_video_proto);
    render_local_camera(f, halves[1], app, local_preview_proto);
}

fn render_received_video(f: &mut Frame, area: Rect, app: &App, remote_video_proto: &mut Option<StatefulProtocol>) {
    let title = match &app.latest_video {
        Some(v) => {
            let loss_pct = if v.chunks_total > 0 {
                100u32.saturating_sub(
                    v.chunks_received as u32 * 100 / v.chunks_total as u32,
                )
            } else {
                0
            };
            format!(
                " Received  from:{}  frame:{}  loss:{}% ",
                v.source, v.frame_id, loss_pct
            )
        }
        None => " Received  (waiting...) ".to_string(),
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(proto) = remote_video_proto {
        f.render_stateful_widget(StatefulImage::default(), inner, proto);
    } else {
        f.render_widget(
            Paragraph::new("No stream received")
                .style(Style::default().fg(Color::DarkGray)),
            inner,
        );
    }
}

fn render_local_camera(f: &mut Frame, area: Rect, app: &App, local_preview_proto: &mut Option<StatefulProtocol>) {
    let (status, status_style) = if app.streaming {
        (
            format!(" Camera  \u{25cf} LIVE  {} frames  [s] stop ", app.stream_frames_sent),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            " Camera  \u{25cb} idle  [s] start streaming ".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };

    let block = Block::default().borders(Borders::ALL)
        .title(Span::styled(status, status_style));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(proto) = local_preview_proto {
        f.render_stateful_widget(StatefulImage::default(), inner, proto);
    } else if app.streaming {
        f.render_widget(
            Paragraph::new("Opening camera...")
                .style(Style::default().fg(Color::Yellow)),
            inner,
        );
    } else {
        f.render_widget(
            Paragraph::new("Press [s] to start streaming your camera")
                .style(Style::default().fg(Color::DarkGray)),
            inner,
        );
    }
}
