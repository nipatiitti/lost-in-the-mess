use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};
use ratatui_image::{StatefulImage, protocol::StatefulProtocol};

fn prr_color(prr: f32) -> Color {
    if prr >= 0.7 { Color::Green }
    else if prr >= 0.4 { Color::Yellow }
    else { Color::Red }
}

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
        ActivePanel::Telemetry => render_telemetry(f, cols[1], app),
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let highlight = Style::default().add_modifier(Modifier::REVERSED);
    let normal = Style::default();

    let avg_prr = if app.neighbors.is_empty() {
        0.0f32
    } else {
        app.neighbors.iter().map(|n| n.prr).sum::<f32>() / app.neighbors.len() as f32
    };
    let (health_label, health_color) = if app.neighbors.is_empty() {
        ("ISOLATED", Color::Red)
    } else if avg_prr >= 0.7 {
        ("MESH OK", Color::Green)
    } else if avg_prr >= 0.4 {
        ("DEGRADED", Color::Yellow)
    } else {
        ("CRITICAL", Color::Red)
    };

    let mut spans = vec![
        Span::styled(
            format!(" Node {} ", app.local_id),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ", health_label),
            Style::default().fg(health_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}n  obj:{} ", app.neighbors.len(), app.objects_received),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
    ];

    for (label, panel) in [
        ("Topology", ActivePanel::Topology),
        ("Messages", ActivePanel::Messages),
        ("Video",    ActivePanel::Video),
        ("Telemetry", ActivePanel::Telemetry),
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
            Cell::from(format!("{:.0}%", n.prr * 100.0))
                .style(Style::default().fg(prr_color(n.prr))),
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
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(7)])
        .split(area);

    // --- topology edges ---
    let mut edges: Vec<ListItem> = Vec::new();
    let mut sorted_keys: Vec<_> = app.topology.keys().copied().collect();
    sorted_keys.sort();

    for src in sorted_keys {
        if let Some(dests) = app.topology.get(&src) {
            let mut dests = dests.clone();
            dests.sort_by_key(|(id, _)| *id);
            for (dst, prr) in dests {
                let is_local = src == app.local_id || dst == app.local_id;
                let prr_col = prr_color(prr);
                let node_style = if is_local {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let prr_style = Style::default().fg(prr_col);
                edges.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("{src} → {dst}  "), node_style),
                    Span::styled(format!("{:.0}%", prr * 100.0), prr_style),
                ])));
            }
        }
    }

    let list = List::new(edges)
        .block(Block::default().borders(Borders::ALL).title(" Mesh Topology "));
    f.render_widget(list, split[0]);

    // --- event log ---
    let visible_rows = split[1].height.saturating_sub(2) as usize;
    let skip = app.events.len().saturating_sub(visible_rows);
    let event_lines: Vec<Line> = app.events.iter().skip(skip).map(|e| {
        let color = if e.is_warning { Color::Yellow } else { Color::Green };
        Line::from(vec![
            Span::styled(
                e.at.format("%H:%M:%S").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(e.text.clone(), Style::default().fg(color)),
        ])
    }).collect();

    let log = Paragraph::new(Text::from(event_lines))
        .block(Block::default().borders(Borders::ALL).title(" Network Events "));
    f.render_widget(log, split[1]);
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
    let direct_ids: std::collections::HashSet<litm_common::NodeId> =
        app.neighbors.iter().map(|n| n.id).collect();

    let lines: Vec<Line> = app.messages.iter().map(|m| {
        let is_relay = !direct_ids.contains(&m.source);
        let mut spans = vec![
            Span::styled(
                m.received_at.format("%H:%M:%S").to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(
                format!("from:{}", m.source),
                Style::default().fg(Color::Yellow),
            ),
        ];
        if is_relay {
            spans.push(Span::styled(
                " ↻relay",
                Style::default().fg(Color::Magenta),
            ));
        }
        spans.push(Span::raw("  "));
        spans.push(Span::raw(m.content.clone()));
        Line::from(spans)
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

fn render_telemetry(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" RaptorQ Fountain Decoder Telemetry ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(0)
        ])
        .split(inner);

    let progress_pct = (app.raptor_progress * 100.0) as u16;
    let gauge = ratatui::widgets::Gauge::default()
        .block(Block::default().title(" Matrix Assembly Progress ").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .percent(progress_pct);
    f.render_widget(gauge, chunks[0]);
    
    let stats = vec![
        Line::from(vec![
            Span::styled("Matrix Density: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}%", app.raptor_density * 100.0), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Overhead Symbols: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", app.raptor_overhead), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Matrix Dimensions: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}x{}", app.raptor_matrix_rows, app.raptor_matrix_cols), Style::default().fg(Color::Magenta)),
        ]),
    ];
    
    let para = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title(" Statistics "));
    f.render_widget(para, chunks[1]);
}
