use crate::gh::{self, Notification};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};
use std::sync::mpsc;
use std::thread;

use super::*;

pub struct NotifList {
    pub notifs: Vec<Notification>,
    pub state: ListState,
    pub loading: bool,
    pub error: Option<String>,
    pub filter: String,
    pub filtering: bool,
    pub filtered_indices: Vec<usize>,
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
            filter: String::new(),
            filtering: false,
            filtered_indices: Vec::new(),
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
        if let Some(ref rx) = self.rx
            && let Ok(result) = rx.try_recv()
        {
            self.rx = None;
            self.loading = false;
            match result {
                Ok(notifs) => {
                    self.notifs = notifs;
                    self.refilter();
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    pub fn move_down(&mut self) {
        if let Some(i) = self.state.selected()
            && i + 1 < self.filtered_indices.len()
        {
            self.state.select(Some(i + 1));
        }
    }

    pub fn move_up(&mut self) {
        if let Some(i) = self.state.selected()
            && i > 0
        {
            self.state.select(Some(i - 1));
        }
    }

    pub fn move_to_first(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn move_to_last(&mut self) {
        let len = self.filtered_indices.len();
        if len > 0 {
            self.state.select(Some(len - 1));
        }
    }

    pub fn page_down(&mut self, page_size: usize) {
        if let Some(i) = self.state.selected() {
            let last = self.filtered_indices.len().saturating_sub(1);
            self.state.select(Some((i + page_size).min(last)));
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        if let Some(i) = self.state.selected() {
            self.state.select(Some(i.saturating_sub(page_size)));
        }
    }

    pub fn selected(&self) -> Option<&Notification> {
        self.state
            .selected()
            .and_then(|i| self.filtered_indices.get(i))
            .and_then(|&i| self.notifs.get(i))
    }

    pub fn refilter(&mut self) {
        let query = self.filter.to_lowercase();
        self.filtered_indices = self
            .notifs
            .iter()
            .enumerate()
            .filter(|(_, notification)| {
                query.is_empty()
                    || notification.subject.title.to_lowercase().contains(&query)
                    || notification
                        .repository
                        .full_name
                        .to_lowercase()
                        .contains(&query)
                    || notification.reason.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect();
        self.state.select(if self.filtered_indices.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    pub fn on_filter_key(&mut self, key: char) {
        self.filter.push(key);
        self.refilter();
    }

    pub fn on_filter_backspace(&mut self) {
        self.filter.pop();
        self.refilter();
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filtering = false;
        self.refilter();
    }

    pub fn mark_selected_read(&mut self) {
        if let Some(notif) = self.selected() {
            let id = notif.id.clone();
            let selected = self.state.selected();
            let _ = gh::mark_notification_read(&id);
            if let Some(real_index) = selected.and_then(|i| self.filtered_indices.get(i).copied()) {
                self.notifs.remove(real_index);
                self.refilter();
                if !self.filtered_indices.is_empty() {
                    self.state.select(Some(
                        selected.unwrap_or(0).min(self.filtered_indices.len() - 1),
                    ));
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
        if self.filtered_indices.is_empty() {
            let message = if self.filter.is_empty() {
                " No notifications".to_string()
            } else {
                format!(" No notifications match ‘{}’", self.filter)
            };
            let line = Line::from(Span::styled(message, style_dim()));
            f.render_widget(line, area);
            return;
        }

        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&index| {
                let n = &self.notifs[index];
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
            })
            .collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gh::{NotifRepo, NotifSubject};

    fn notification(title: &str, repo: &str, reason: &str) -> Notification {
        Notification {
            id: format!("{repo}-{title}"),
            reason: reason.into(),
            subject: NotifSubject {
                title: title.into(),
                kind: "Issue".into(),
                url: None,
            },
            repository: NotifRepo {
                full_name: repo.into(),
            },
            unread: true,
            updated_at: None,
        }
    }

    #[test]
    fn filters_notifications_by_title_repository_and_reason() {
        let mut list = NotifList::new();
        list.notifs = vec![
            notification("Synthetic bug", "example/one", "mention"),
            notification("Documentation", "example/two", "review_requested"),
        ];

        list.filter = "review".into();
        list.refilter();
        assert_eq!(list.filtered_indices, vec![1]);
        assert_eq!(list.selected().unwrap().subject.title, "Documentation");

        list.filter = "one".into();
        list.refilter();
        assert_eq!(list.filtered_indices, vec![0]);
    }
}
