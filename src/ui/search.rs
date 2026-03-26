use crate::gh::{self, Repo};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};
use std::sync::mpsc;
use std::thread;

use super::*;

pub struct SearchView {
    pub query: String,
    pub editing: bool,
    pub results: Vec<Repo>,
    pub state: ListState,
    pub loading: bool,
    pub error: Option<String>,
    pub searched: bool,

    rx: Option<mpsc::Receiver<Result<Vec<Repo>, String>>>,
}

impl SearchView {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            editing: true,
            results: Vec::new(),
            state: ListState::default(),
            loading: false,
            error: None,
            searched: false,
            rx: None,
        }
    }

    pub fn search(&mut self) {
        if self.query.trim().is_empty() {
            return;
        }
        self.loading = true;
        self.error = None;
        self.searched = true;

        let q = self.query.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let result = gh::search_repos(&q, 30).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    pub fn poll(&mut self) {
        if let Some(ref rx) = self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                self.loading = false;
                match result {
                    Ok(repos) => {
                        self.results = repos;
                        if !self.results.is_empty() {
                            self.state.select(Some(0));
                        } else {
                            self.state.select(None);
                        }
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        if let Some(i) = self.state.selected() {
            if i + 1 < self.results.len() {
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

    pub fn selected_repo(&self) -> Option<&Repo> {
        self.state.selected().and_then(|i| self.results.get(i))
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        let chunks = Layout::vertical([
            Constraint::Length(1), // search input
            Constraint::Min(1),   // results
        ])
        .split(area);

        // Search input
        let cursor = if self.editing { "\u{2588}" } else { "" };
        let input_line = Line::from(vec![
            Span::styled(" / ", style_accent()),
            Span::styled(format!("{}{cursor}", self.query), style_normal()),
        ]);
        f.render_widget(input_line, chunks[0]);

        // Results
        if self.loading {
            f.render_widget(spinner_line(tick, "Searching..."), chunks[1]);
            return;
        }
        if let Some(ref err) = self.error {
            let line = Line::from(Span::styled(
                format!(" Error: {err}"),
                ratatui::style::Style::default().fg(red()),
            ));
            f.render_widget(line, chunks[1]);
            return;
        }
        if !self.searched {
            let line = Line::from(Span::styled(
                " Type a query and press Enter to search",
                style_dim(),
            ));
            f.render_widget(line, chunks[1]);
            return;
        }
        if self.results.is_empty() {
            let line = Line::from(Span::styled(" No results", style_dim()));
            f.render_widget(line, chunks[1]);
            return;
        }

        let items: Vec<ListItem> = self.results.iter().map(|repo| {
            let mut spans = vec![
                Span::styled(&repo.full_name, style_normal()),
            ];
            if repo.is_private {
                spans.push(Span::styled(" [private]", style_purple()));
            }
            if repo.star_count > 0 {
                spans.push(Span::styled(format!(" *{}", repo.star_count), style_dim()));
            }
            if let Some(ref desc) = repo.description {
                if !desc.is_empty() {
                    spans.push(Span::styled(format!(" — {desc}"), style_dim()));
                }
            }
            ListItem::new(Line::from(spans))
        }).collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, chunks[1], &mut self.state);
    }
}
