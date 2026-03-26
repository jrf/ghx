use crate::gh::{self, Repo};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepoSource {
    Mine,
    Starred,
    Org(usize),
}

pub struct RepoList {
    pub repos: Vec<Repo>,
    pub orgs: Vec<String>,
    pub source: RepoSource,
    pub state: ListState,
    pub loading: bool,
    pub error: Option<String>,
    pub filter: String,
    pub filtering: bool,
    pub filtered_indices: Vec<usize>,

    cache: HashMap<RepoSource, Vec<Repo>>,
    rx: Option<mpsc::Receiver<LoadResult>>,
}

struct LoadResult {
    source: RepoSource,
    result: Result<Vec<Repo>, String>,
}

impl RepoList {
    pub fn new() -> Self {
        Self {
            repos: Vec::new(),
            orgs: Vec::new(),
            source: RepoSource::Mine,
            state: ListState::default(),
            loading: true,
            error: None,
            filter: String::new(),
            filtering: false,
            filtered_indices: Vec::new(),
            cache: HashMap::new(),
            rx: None,
        }
    }

    pub fn load(&mut self) {
        // Check cache first
        if let Some(repos) = self.cache.get(&self.source) {
            self.repos = repos.clone();
            self.loading = false;
            self.error = None;
            self.refilter();
            return;
        }

        self.loading = true;
        self.error = None;
        self.repos.clear();
        self.filtered_indices.clear();

        let source = self.source;
        let orgs = self.orgs.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let result = match source {
                RepoSource::Mine => gh::list_repos(50),
                RepoSource::Starred => gh::list_starred(50),
                RepoSource::Org(idx) => {
                    if let Some(org) = orgs.get(idx) {
                        gh::list_org_repos(org, 50)
                    } else {
                        Ok(Vec::new())
                    }
                }
            };
            let _ = tx.send(LoadResult {
                source,
                result: result.map_err(|e| e.to_string()),
            });
        });
    }

    /// Call this each frame to check for completed loads.
    pub fn poll(&mut self) {
        if let Some(ref rx) = self.rx {
            if let Ok(msg) = rx.try_recv() {
                self.rx = None;
                if msg.source == self.source {
                    match msg.result {
                        Ok(repos) => {
                            self.cache.insert(msg.source, repos.clone());
                            self.repos = repos;
                            self.refilter();
                            self.error = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                    self.loading = false;
                }
            }
        }
    }

    pub fn load_orgs(&mut self) {
        if let Ok(orgs) = gh::list_orgs() {
            self.orgs = orgs;
        }
    }

    pub fn refilter(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.repos.len()).collect();
        } else {
            let query = self.filter.to_lowercase();
            self.filtered_indices = self.repos.iter().enumerate()
                .filter(|(_, r)| {
                    r.full_name.to_lowercase().contains(&query)
                        || r.description.as_deref().unwrap_or("").to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if self.filtered_indices.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    pub fn selected_repo(&self) -> Option<&Repo> {
        self.state.selected()
            .and_then(|i| self.filtered_indices.get(i))
            .and_then(|&idx| self.repos.get(idx))
    }

    pub fn move_down(&mut self) {
        if let Some(i) = self.state.selected() {
            if i + 1 < self.filtered_indices.len() {
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

    pub fn source_labels(&self) -> Vec<String> {
        let mut labels = vec!["My Repos".into(), "Starred".into()];
        for org in &self.orgs {
            labels.push(org.clone());
        }
        labels
    }

    pub fn active_source_index(&self) -> usize {
        match self.source {
            RepoSource::Mine => 0,
            RepoSource::Starred => 1,
            RepoSource::Org(i) => 2 + i,
        }
    }

    pub fn total_sources(&self) -> usize {
        2 + self.orgs.len()
    }

    pub fn set_source_by_index(&mut self, idx: usize) {
        self.source = match idx {
            0 => RepoSource::Mine,
            1 => RepoSource::Starred,
            n => RepoSource::Org(n - 2),
        };
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.loading {
            f.render_widget(spinner_line(tick, "Loading repositories..."), area);
            return;
        }
        if let Some(ref err) = self.error {
            let line = Line::from(Span::styled(
                format!(" Error: {err}"),
                Style::default().fg(red()),
            ));
            f.render_widget(line, area);
            return;
        }

        let items: Vec<ListItem> = self.filtered_indices.iter().map(|&idx| {
            let repo = &self.repos[idx];
            let mut spans = vec![
                Span::styled(&repo.full_name, style_normal()),
            ];
            if repo.is_private {
                spans.push(Span::styled(" [private]", style_purple()));
            }
            if repo.star_count > 0 {
                spans.push(Span::styled(format!(" *{}", repo.star_count), style_dim()));
            }
            if let Some(ref ts) = repo.updated_at {
                spans.push(Span::styled(format!(" · {}", timeago(ts)), style_dim()));
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

        f.render_stateful_widget(list, area, &mut self.state);
    }
}
