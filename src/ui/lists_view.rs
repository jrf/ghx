use crate::gh::{self, Repo, UserList};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};
use std::sync::mpsc;
use std::thread;

use super::*;

enum ListsMode {
    Names,
    Repos,
}

pub struct ListsView {
    lists: Vec<UserList>,
    name_state: ListState,
    repo_state: ListState,
    mode: ListsMode,
    selected_list: usize,
    loading: bool,
    error: Option<String>,
    filter: String,
    pub filtering: bool,
    filtered_indices: Vec<usize>,
    rx: Option<mpsc::Receiver<Result<Vec<UserList>, String>>>,
}

impl ListsView {
    pub fn new() -> Self {
        Self {
            lists: Vec::new(),
            name_state: ListState::default(),
            repo_state: ListState::default(),
            mode: ListsMode::Names,
            selected_list: 0,
            loading: false,
            error: None,
            filter: String::new(),
            filtering: false,
            filtered_indices: Vec::new(),
            rx: None,
        }
    }

    pub fn ensure_loaded(&mut self) {
        if !self.lists.is_empty() || self.loading || self.error.is_some() {
            return;
        }
        self.load();
    }

    fn load(&mut self) {
        self.loading = true;
        self.error = None;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let result = gh::list_user_lists().map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    pub fn poll(&mut self) {
        if let Some(ref rx) = self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                self.loading = false;
                match result {
                    Ok(lists) => {
                        self.lists = lists;
                        if !self.lists.is_empty() {
                            self.name_state.select(Some(0));
                        }
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

    pub fn is_browsing_repos(&self) -> bool {
        matches!(self.mode, ListsMode::Repos)
    }

    pub fn selected_repo(&self) -> Option<&Repo> {
        if let ListsMode::Repos = self.mode {
            let list = self.lists.get(self.selected_list)?;
            let idx = self.repo_state.selected()?;
            let &real = self.filtered_indices.get(idx)?;
            list.repos.get(real)
        } else {
            None
        }
    }

    pub fn enter(&mut self) {
        match self.mode {
            ListsMode::Names => {
                if let Some(i) = self.name_state.selected() {
                    self.selected_list = i;
                    self.mode = ListsMode::Repos;
                    self.filter.clear();
                    self.filtering = false;
                    self.refilter();
                }
            }
            ListsMode::Repos => {}
        }
    }

    pub fn go_back(&mut self) -> bool {
        match self.mode {
            ListsMode::Repos => {
                self.mode = ListsMode::Names;
                self.filter.clear();
                self.filtering = false;
                true
            }
            ListsMode::Names => false,
        }
    }

    fn refilter(&mut self) {
        if let Some(list) = self.lists.get(self.selected_list) {
            if self.filter.is_empty() {
                self.filtered_indices = (0..list.repos.len()).collect();
            } else {
                let query = self.filter.to_lowercase();
                self.filtered_indices = list
                    .repos
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| {
                        r.full_name.to_lowercase().contains(&query)
                            || r.description
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|(i, _)| i)
                    .collect();
            }
            if self.filtered_indices.is_empty() {
                self.repo_state.select(None);
            } else {
                self.repo_state.select(Some(0));
            }
        }
    }

    pub fn on_filter_key(&mut self, c: char) {
        self.filter.push(c);
        self.refilter();
    }

    pub fn on_filter_backspace(&mut self) {
        self.filter.pop();
        self.refilter();
    }

    pub fn on_filter_clear(&mut self) {
        self.filter.clear();
        self.filtering = false;
        self.refilter();
    }

    fn current_state(&mut self) -> &mut ListState {
        match self.mode {
            ListsMode::Names => &mut self.name_state,
            ListsMode::Repos => &mut self.repo_state,
        }
    }

    fn current_len(&self) -> usize {
        match self.mode {
            ListsMode::Names => self.lists.len(),
            ListsMode::Repos => self.filtered_indices.len(),
        }
    }

    pub fn move_down(&mut self) {
        let len = self.current_len();
        let state = self.current_state();
        if let Some(i) = state.selected() {
            if i + 1 < len {
                state.select(Some(i + 1));
            }
        }
    }

    pub fn move_up(&mut self) {
        let state = self.current_state();
        if let Some(i) = state.selected() {
            if i > 0 {
                state.select(Some(i - 1));
            }
        }
    }

    pub fn move_to_first(&mut self) {
        if self.current_len() > 0 {
            self.current_state().select(Some(0));
        }
    }

    pub fn move_to_last(&mut self) {
        let len = self.current_len();
        if len > 0 {
            self.current_state().select(Some(len - 1));
        }
    }

    pub fn page_down(&mut self, page_size: usize) {
        let len = self.current_len();
        let state = self.current_state();
        if let Some(i) = state.selected() {
            state.select(Some((i + page_size).min(len.saturating_sub(1))));
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        let state = self.current_state();
        if let Some(i) = state.selected() {
            state.select(Some(i.saturating_sub(page_size)));
        }
    }

    pub fn current_list_name(&self) -> Option<&str> {
        if let ListsMode::Repos = self.mode {
            self.lists.get(self.selected_list).map(|l| l.name.as_str())
        } else {
            None
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.loading {
            f.render_widget(spinner_line(tick, "Loading lists..."), area);
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

        match self.mode {
            ListsMode::Names => self.render_names(f, area),
            ListsMode::Repos => self.render_repos(f, area),
        }
    }

    fn render_names(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .lists
            .iter()
            .map(|list| {
                let count = list.repos.len();
                ListItem::new(Line::from(vec![
                    Span::styled(&list.name, style_normal()),
                    Span::styled(format!(" ({count})"), style_dim()),
                ]))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.name_state);
    }

    fn render_repos(&mut self, f: &mut Frame, area: Rect) {
        let Some(user_list) = self.lists.get(self.selected_list) else {
            return;
        };

        // Show filter bar if active
        let content_area = if self.filtering {
            let chunks = ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(area);

            let filter_line = Line::from(vec![
                Span::styled(" / ", style_accent()),
                Span::styled(format!("{}\u{2588}", self.filter), style_normal()),
            ]);
            f.render_widget(filter_line, chunks[0]);
            chunks[1]
        } else if !self.filter.is_empty() {
            let chunks = ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(area);

            let info = Line::from(Span::styled(
                format!(
                    " filter: {} ({}/{})",
                    self.filter,
                    self.filtered_indices.len(),
                    user_list.repos.len()
                ),
                style_dim(),
            ));
            f.render_widget(info, chunks[0]);
            chunks[1]
        } else {
            area
        };

        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&idx| {
                let repo = &user_list.repos[idx];
                let mut spans = vec![Span::styled(&repo.full_name, style_normal())];
                if repo.is_private {
                    spans.push(Span::styled(" ⊝", style_purple()));
                }
                if repo.star_count > 0 {
                    spans.push(Span::styled(
                        format!(" *{}", repo.star_count),
                        style_dim(),
                    ));
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
            })
            .collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, content_area, &mut self.repo_state);
    }
}
