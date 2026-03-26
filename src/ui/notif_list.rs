use crate::gh::{self, Notification};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};
use std::sync::mpsc;
use std::thread;

use super::*;

pub struct NotifList {
    pub notifs: Vec<Notification>,
    pub state: ListState,
    pub loading: bool,
    pub error: Option<String>,
    loaded: bool,
    rx: Option<mpsc::Receiver<Result<Vec<Notification>, String>>>,
}

impl NotifList {
    pub fn new() -> Self {
        Self {
            notifs: Vec::new(),
            state: ListState::default(),
            loading: false,
            error: None,
            loaded: false,
            rx: None,
        }
    }

    pub fn load(&mut self) {
        self.loading = true;
        self.error = None;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let result = gh::list_notifications().map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    pub fn ensure_loaded(&mut self) {
        if !self.loaded && !self.loading {
            self.loaded = true;
            self.load();
        }
    }

    pub fn poll(&mut self) {
        if let Some(ref rx) = self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                self.loading = false;
                match result {
                    Ok(notifs) => {
                        self.notifs = notifs;
                        if !self.notifs.is_empty() {
                            self.state.select(Some(0));
                        }
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        if let Some(i) = self.state.selected() {
            if i + 1 < self.notifs.len() {
                self.state.select(Some(i + 1));
            }
        }
    }

    pub fn move_up(&mut self) {
        if let Some(i) = self.state.selected() {
            if i > 0 {
                self.state.select(Some(i - 1));
            }
        }
    }

    pub fn move_to_first(&mut self) {
        if !self.notifs.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn move_to_last(&mut self) {
        let len = self.notifs.len();
        if len > 0 {
            self.state.select(Some(len - 1));
        }
    }

    pub fn page_down(&mut self, page_size: usize) {
        if let Some(i) = self.state.selected() {
            let last = self.notifs.len().saturating_sub(1);
            self.state.select(Some((i + page_size).min(last)));
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        if let Some(i) = self.state.selected() {
            self.state.select(Some(i.saturating_sub(page_size)));
        }
    }

    pub fn selected(&self) -> Option<&Notification> {
        self.state.selected().and_then(|i| self.notifs.get(i))
    }

    pub fn mark_selected_read(&mut self) {
        if let Some(notif) = self.selected() {
            let id = notif.id.clone();
            let _ = gh::mark_notification_read(&id);
            if let Some(i) = self.state.selected() {
                self.notifs.remove(i);
                if self.notifs.is_empty() {
                    self.state.select(None);
                } else if i >= self.notifs.len() {
                    self.state.select(Some(self.notifs.len() - 1));
                }
            }
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.loading {
            f.render_widget(spinner_line(tick, "Loading notifications..."), area);
            return;
        }
        if let Some(ref err) = self.error {
            let line = Line::from(Span::styled(
                format!(" Error: {err}"),
                ratatui::style::Style::default().fg(red()),
            ));
            f.render_widget(line, area);
            return;
        }
        if self.notifs.is_empty() {
            let line = Line::from(Span::styled(" No notifications", style_dim()));
            f.render_widget(line, area);
            return;
        }

        let items: Vec<ListItem> = self.notifs.iter().map(|n| {
            let kind_icon = match n.subject.kind.as_str() {
                "Issue" => "●",
                "PullRequest" => "⑂",
                "Release" => "▲",
                _ => "•",
            };
            let kind_style = match n.subject.kind.as_str() {
                "Issue" => ratatui::style::Style::default().fg(green()),
                "PullRequest" => ratatui::style::Style::default().fg(purple()),
                _ => style_dim(),
            };
            let mut spans = vec![
                Span::styled(format!("{kind_icon} "), kind_style),
                Span::styled(&n.subject.title, style_normal()),
                Span::styled(format!("  {}", n.repository.full_name), style_dim()),
            ];
            if let Some(ref ts) = n.updated_at {
                spans.push(Span::styled(format!(" · {}", timeago(ts)), style_dim()));
            }
            ListItem::new(Line::from(spans))
        }).collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }
}
