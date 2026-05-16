use std::collections::{HashMap, HashSet};

use app_sdk::{MessagePayload, Node};
use chrono::{DateTime, Local};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use litm_common::{NeighborInfo, NodeId};

use tokio::sync::mpsc;

use super::events::{AppEvent, DecodedVideoFrame};

#[derive(Clone, Copy, PartialEq)]
pub enum ActivePanel {
    Topology,
    Messages,
    Video,
    Telemetry,
}

pub struct MessageEntry {
    pub received_at: DateTime<Local>,
    pub source: NodeId,
    pub content: String,
}

pub struct EventEntry {
    pub at: DateTime<Local>,
    pub text: String,
    pub is_warning: bool,
}

pub struct App {
    pub local_id: NodeId,
    pub node: Node,
    pub active_panel: ActivePanel,
    pub should_quit: bool,
    pub neighbors: Vec<NeighborInfo>,
    pub topology: HashMap<NodeId, Vec<(NodeId, f32)>>,
    pub messages: Vec<MessageEntry>,
    pub messages_scroll: usize,
    pub compose_input: String,
    pub compose_cursor: usize,
    pub latest_video: Option<DecodedVideoFrame>,
    pub streaming: bool,
    pub local_preview: Option<image::DynamicImage>,
    pub stream_frames_sent: u64,
    pub events: Vec<EventEntry>,
    pub objects_received: u64,
    pub raptor_progress: f32,
    pub raptor_overhead: u32,
    pub raptor_matrix_rows: usize,
    pub raptor_matrix_cols: usize,
    pub raptor_density: f32,
    cam_cmd_tx: mpsc::Sender<bool>,
    prev_neighbor_ids: HashSet<NodeId>,
}

impl App {
    pub fn new(local_id: NodeId, node: Node, cam_cmd_tx: mpsc::Sender<bool>) -> Self {
        Self {
            local_id,
            node,
            active_panel: ActivePanel::Topology,
            should_quit: false,
            neighbors: Vec::new(),
            topology: HashMap::new(),
            messages: Vec::new(),
            messages_scroll: 0,
            compose_input: String::new(),
            compose_cursor: 0,
            latest_video: None,
            streaming: false,
            local_preview: None,
            stream_frames_sent: 0,
            events: Vec::new(),
            objects_received: 0,
            raptor_progress: 0.0,
            raptor_overhead: 0,
            raptor_matrix_rows: 0,
            raptor_matrix_cols: 0,
            raptor_density: 0.0,
            cam_cmd_tx,
            prev_neighbor_ids: HashSet::new(),
        }
    }

    fn push_event(&mut self, text: String, is_warning: bool) {
        self.events.push(EventEntry { at: Local::now(), text, is_warning });
        if self.events.len() > 200 {
            self.events.drain(..self.events.len() - 200);
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::TopologyTick => {
                let new_neighbors = self.node.neighbors();
                let new_ids: HashSet<NodeId> = new_neighbors.iter().map(|n| n.id).collect();

                let joined: Vec<NodeId> = new_ids.iter()
                    .filter(|id| !self.prev_neighbor_ids.contains(id))
                    .copied().collect();
                let left: Vec<NodeId> = self.prev_neighbor_ids.iter()
                    .filter(|id| !new_ids.contains(id))
                    .copied().collect();

                for id in joined {
                    self.push_event(format!("Node {} joined", id), false);
                }
                for id in left {
                    self.push_event(format!("Node {} left", id), true);
                }
                self.prev_neighbor_ids = new_ids;

                self.neighbors = new_neighbors;
                self.neighbors.sort_by_key(|n| n.id);
                self.topology = self.node.topology();
            }

            AppEvent::MeshMessage { source, content } => {
                self.objects_received += 1;
                self.messages.push(MessageEntry {
                    received_at: Local::now(),
                    source,
                    content,
                });
                if self.active_panel == ActivePanel::Messages {
                    self.messages_scroll = self.messages.len().saturating_sub(1);
                }
            }

            AppEvent::VideoFrame(frame) => {
                self.latest_video = Some(frame);
            }

            AppEvent::LocalPreview(img) => {
                self.local_preview = Some(img);
                self.stream_frames_sent += 1;
            }

            AppEvent::MeshClosed => {
                self.should_quit = true;
            }

            AppEvent::RaptorTelemetry(telemetry) => {
                match telemetry {
                    litm_common::RaptorEvent::DecoderStatus { progress, overhead_symbols } => {
                        self.raptor_progress = progress;
                        self.raptor_overhead = overhead_symbols;
                    }
                    litm_common::RaptorEvent::MatrixState { rows, cols, density } => {
                        self.raptor_matrix_rows = rows;
                        self.raptor_matrix_cols = cols;
                        self.raptor_density = density;
                    }
                    litm_common::RaptorEvent::DecodingSuccess => {
                        self.raptor_progress = 1.0;
                    }
                    _ => {}
                }
            }

            AppEvent::Terminal(Event::Key(key)) => self.handle_key(key),

            AppEvent::Terminal(_) => {}
        }
    }

    fn cycle_panel(&mut self, dir: i8) {
        let panels = [ActivePanel::Topology, ActivePanel::Messages, ActivePanel::Video, ActivePanel::Telemetry];
        let idx = panels.iter().position(|p| *p == self.active_panel).unwrap_or(0);
        let next = (idx as i8 + dir).rem_euclid(panels.len() as i8) as usize;
        self.active_panel = panels[next];
        if self.active_panel == ActivePanel::Messages {
            self.messages_scroll = self.messages.len().saturating_sub(1);
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') if ctrl => {
                self.should_quit = true;
            }
            KeyCode::Tab => self.cycle_panel(1),
            KeyCode::BackTab => self.cycle_panel(-1),
            KeyCode::Char('s') if self.active_panel == ActivePanel::Video => {
                self.streaming = !self.streaming;
                let _ = self.cam_cmd_tx.try_send(self.streaming);
                if !self.streaming {
                    self.local_preview = None;
                    self.stream_frames_sent = 0;
                }
            }
            KeyCode::Down | KeyCode::Char('j') if self.compose_input.is_empty() => {
                if self.active_panel == ActivePanel::Messages {
                    let max = self.messages.len().saturating_sub(1);
                    self.messages_scroll = (self.messages_scroll + 1).min(max);
                }
            }
            KeyCode::Up | KeyCode::Char('k') if self.compose_input.is_empty() => {
                if self.active_panel == ActivePanel::Messages {
                    self.messages_scroll = self.messages_scroll.saturating_sub(1);
                }
            }

            // Compose input — active whenever Messages panel is shown
            KeyCode::Esc => {
                self.compose_input.clear();
                self.compose_cursor = 0;
            }
            KeyCode::Enter if self.active_panel == ActivePanel::Messages => {
                if !self.compose_input.is_empty() {
                    let _ = self.node.send(MessagePayload::Text {
                        content: self.compose_input.clone(),
                    });
                    self.compose_input.clear();
                    self.compose_cursor = 0;
                    self.messages_scroll = self.messages.len().saturating_sub(1);
                }
            }
            KeyCode::Backspace if self.active_panel == ActivePanel::Messages => {
                if self.compose_cursor > 0 {
                    let prev = self.compose_input[..self.compose_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.compose_input.remove(prev);
                    self.compose_cursor = prev;
                }
            }
            KeyCode::Left if self.active_panel == ActivePanel::Messages => {
                if self.compose_cursor > 0 {
                    let prev = self.compose_input[..self.compose_cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.compose_cursor = prev;
                }
            }
            KeyCode::Right if self.active_panel == ActivePanel::Messages => {
                if self.compose_cursor < self.compose_input.len() {
                    let next = self.compose_input[self.compose_cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.compose_cursor + i)
                        .unwrap_or(self.compose_input.len());
                    self.compose_cursor = next;
                }
            }
            KeyCode::Char(c) if self.active_panel == ActivePanel::Messages => {
                self.active_panel = ActivePanel::Messages;
                self.compose_input.insert(self.compose_cursor, c);
                self.compose_cursor += c.len_utf8();
            }

            _ => {}
        }
    }
}
