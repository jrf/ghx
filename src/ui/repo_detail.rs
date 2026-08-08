use crate::gh::{self, CheckStatus, Issue, PR, RepoDetail};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use std::sync::mpsc;
use std::thread;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepoTab {
    Overview,
    Issues,
    PullRequests,
}

impl RepoTab {
    pub const ALL: [RepoTab; 3] = [RepoTab::Overview, RepoTab::Issues, RepoTab::PullRequests];

    pub fn label(self) -> &'static str {
        match self {
            RepoTab::Overview => "Overview",
            RepoTab::Issues => "Issues",
            RepoTab::PullRequests => "Pull Requests",
        }
    }
}

enum LoadMsg {
    RepoDetail(Box<Result<RepoDetail, String>>),
    Readme(Result<String, String>),
    Issues(Result<Vec<Issue>, String>),
    Prs(Result<Vec<PR>, String>),
}

pub struct RepoDetailView {
    #[allow(dead_code)]
    pub repo_name: String,
    pub detail: Option<RepoDetail>,
    pub tab: RepoTab,
    pub loading: bool,
    pub error: Option<String>,
    pub scroll: u16,
    pub lines_count: usize,

    pub readme_raw: Option<String>,
    pub issues: Vec<Issue>,
    pub prs: Vec<PR>,
    pub issues_loading: bool,
    pub prs_loading: bool,
    pub issues_error: Option<String>,
    pub prs_error: Option<String>,
    pub list_state: ListState,
    pub filter: String,
    pub filtering: bool,
    pub filtered_indices: Vec<usize>,

    rx: Option<mpsc::Receiver<LoadMsg>>,
}

impl RepoDetailView {
    pub fn new(repo_name: String) -> Self {
        let mut view = Self {
            repo_name: repo_name.clone(),
            detail: None,
            tab: RepoTab::Overview,
            loading: true,
            error: None,
            scroll: 0,
            lines_count: 0,
            readme_raw: None,
            issues: Vec::new(),
            prs: Vec::new(),
            issues_loading: true,
            prs_loading: true,
            issues_error: None,
            prs_error: None,
            list_state: ListState::default(),
            filter: String::new(),
            filtering: false,
            filtered_indices: Vec::new(),
            rx: None,
        };
        view.load_all(repo_name);
        view
    }

    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            RepoTab::Overview => RepoTab::Issues,
            RepoTab::Issues => RepoTab::PullRequests,
            RepoTab::PullRequests => RepoTab::Overview,
        };
        self.scroll = 0;
        self.filter.clear();
        self.filtering = false;
        self.refilter();
        self.list_state.select(if self.current_list_len() > 0 {
            Some(0)
        } else {
            None
        });
    }

    pub fn prev_tab(&mut self) {
        self.tab = match self.tab {
            RepoTab::Overview => RepoTab::PullRequests,
            RepoTab::Issues => RepoTab::Overview,
            RepoTab::PullRequests => RepoTab::Issues,
        };
        self.scroll = 0;
        self.filter.clear();
        self.filtering = false;
        self.refilter();
        self.list_state.select(if self.current_list_len() > 0 {
            Some(0)
        } else {
            None
        });
    }

    pub fn current_list_len(&self) -> usize {
        match self.tab {
            RepoTab::Issues | RepoTab::PullRequests => self.filtered_indices.len(),
            RepoTab::Overview => 0,
        }
    }

    pub fn move_down(&mut self) {
        let len = self.current_list_len();
        if len == 0 {
            return;
        }
        if let Some(i) = self.list_state.selected()
            && i + 1 < len
        {
            self.list_state.select(Some(i + 1));
        }
    }

    pub fn move_up(&mut self) {
        if let Some(i) = self.list_state.selected()
            && i > 0
        {
            self.list_state.select(Some(i - 1));
        }
    }

    pub fn move_to_first(&mut self) {
        if self.current_list_len() > 0 {
            self.list_state.select(Some(0));
        }
    }

    pub fn move_to_last(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            self.list_state.select(Some(len - 1));
        }
    }

    pub fn page_down_list(&mut self, page_size: usize) {
        if let Some(i) = self.list_state.selected() {
            let last = self.current_list_len().saturating_sub(1);
            self.list_state.select(Some((i + page_size).min(last)));
        }
    }

    pub fn page_up_list(&mut self, page_size: usize) {
        if let Some(i) = self.list_state.selected() {
            self.list_state.select(Some(i.saturating_sub(page_size)));
        }
    }

    pub fn selected_issue_number(&self) -> Option<u32> {
        let i = self.list_state.selected()?;
        let &i = self.filtered_indices.get(i)?;
        self.issues.get(i).map(|issue| issue.number)
    }

    pub fn selected_pr_number(&self) -> Option<u32> {
        let i = self.list_state.selected()?;
        let &i = self.filtered_indices.get(i)?;
        self.prs.get(i).map(|pr| pr.number)
    }

    pub fn refilter(&mut self) {
        let query = self.filter.to_lowercase();
        self.filtered_indices = match self.tab {
            RepoTab::Issues => self
                .issues
                .iter()
                .enumerate()
                .filter(|(_, issue)| {
                    query.is_empty()
                        || issue.title.to_lowercase().contains(&query)
                        || issue.number.to_string().contains(&query)
                        || issue
                            .author
                            .as_ref()
                            .is_some_and(|author| author.login.to_lowercase().contains(&query))
                        || issue
                            .labels
                            .iter()
                            .any(|label| label.name.to_lowercase().contains(&query))
                })
                .map(|(index, _)| index)
                .collect(),
            RepoTab::PullRequests => self
                .prs
                .iter()
                .enumerate()
                .filter(|(_, pr)| {
                    query.is_empty()
                        || pr.title.to_lowercase().contains(&query)
                        || pr.number.to_string().contains(&query)
                        || pr
                            .author
                            .as_ref()
                            .is_some_and(|author| author.login.to_lowercase().contains(&query))
                })
                .map(|(index, _)| index)
                .collect(),
            RepoTab::Overview => Vec::new(),
        };
        self.list_state.select(if self.filtered_indices.is_empty() {
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

    fn load_all(&mut self, repo: String) {
        self.loading = true;
        self.issues_loading = true;
        self.prs_loading = true;
        self.error = None;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        let r1 = repo.clone();
        let tx1 = tx.clone();
        thread::spawn(move || {
            let result = gh::view_repo(&r1).map_err(|e| e.to_string());
            let _ = tx1.send(LoadMsg::RepoDetail(Box::new(result)));
        });

        let r2 = repo.clone();
        let tx2 = tx.clone();
        thread::spawn(move || {
            let result = gh::fetch_readme(&r2).map_err(|e| e.to_string());
            let _ = tx2.send(LoadMsg::Readme(result));
        });

        let r3 = repo.clone();
        let tx3 = tx.clone();
        thread::spawn(move || {
            let result = gh::list_issues(&r3, 50).map_err(|e| e.to_string());
            let _ = tx3.send(LoadMsg::Issues(result));
        });

        let r4 = repo;
        thread::spawn(move || {
            let result = gh::list_prs(&r4, 50).map_err(|e| e.to_string());
            let _ = tx.send(LoadMsg::Prs(result));
        });
    }

    pub fn poll(&mut self) {
        let mut list_changed = false;
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    LoadMsg::RepoDetail(result) => {
                        self.loading = false;
                        match *result {
                            Ok(detail) => self.detail = Some(detail),
                            Err(e) => self.error = Some(e),
                        }
                    }
                    LoadMsg::Readme(result) => {
                        if let Ok(readme) = result {
                            self.readme_raw = Some(readme);
                        }
                    }
                    LoadMsg::Issues(result) => {
                        self.issues_loading = false;
                        match result {
                            Ok(issues) => self.issues = issues,
                            Err(e) => self.issues_error = Some(e),
                        }
                        list_changed = true;
                    }
                    LoadMsg::Prs(result) => {
                        self.prs_loading = false;
                        match result {
                            Ok(prs) => self.prs = prs,
                            Err(e) => self.prs_error = Some(e),
                        }
                        list_changed = true;
                    }
                }
            }
        }
        if list_changed {
            self.refilter();
        }
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let max = (self.lines_count as u16).saturating_sub(1);
        self.scroll = (self.scroll + amount).min(max);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        let is_list = matches!(self.tab, RepoTab::Issues | RepoTab::PullRequests);
        let content_area = if is_list && (self.filtering || !self.filter.is_empty()) {
            let chunks = ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(area);
            let line = if self.filtering {
                Line::from(vec![
                    Span::styled(" / ", style_key()),
                    Span::styled(format!("{}\u{2588}", self.filter), style_normal()),
                ])
            } else {
                let total = match self.tab {
                    RepoTab::Issues => self.issues.len(),
                    RepoTab::PullRequests => self.prs.len(),
                    RepoTab::Overview => 0,
                };
                Line::from(Span::styled(
                    format!(
                        " filter: {} ({}/{total})",
                        self.filter,
                        self.filtered_indices.len()
                    ),
                    style_dim(),
                ))
            };
            f.render_widget(line, chunks[0]);
            chunks[1]
        } else {
            area
        };
        match self.tab {
            RepoTab::Overview => self.render_overview(f, content_area, tick),
            RepoTab::Issues => self.render_issues(f, content_area, tick),
            RepoTab::PullRequests => self.render_prs(f, content_area, tick),
        }
    }

    fn render_overview(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.loading {
            f.render_widget(spinner_line(tick, "Loading repo details..."), area);
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

        let detail = match &self.detail {
            Some(d) => d,
            None => return,
        };

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            format!(" {}", detail.full_name),
            style_bold().fg(heading()),
        )));
        lines.push(Line::default());

        if let Some(ref desc) = detail.description
            && !desc.is_empty()
        {
            lines.push(Line::from(Span::styled(format!(" {desc}"), style_normal())));
            lines.push(Line::default());
        }

        let mut stats = Vec::new();
        if let Some(ref lang) = detail.primary_language {
            stats.push(Span::styled(format!(" {} ", lang.name), style_key()));
        }
        stats.push(Span::styled(
            format!("★ {} ", detail.star_count),
            style_normal(),
        ));
        stats.push(Span::styled(
            format!("⑂ {} ", detail.fork_count),
            style_normal(),
        ));
        stats.push(Span::styled(
            format!("Issues: {} ", detail.issues.total_count),
            style_normal(),
        ));
        stats.push(Span::styled(
            format!("PRs: {}", detail.pull_requests.total_count),
            style_normal(),
        ));
        lines.push(Line::from(stats));

        let mut badges = Vec::new();
        if detail.is_private {
            badges.push(Span::styled(" ⊝", style_dim()));
        }
        if detail.is_fork {
            badges.push(Span::styled(" [fork]", style_dim()));
        }
        if detail.is_archived {
            badges.push(Span::styled(" [archived]", style_dim()));
        }
        if let Some(ref license) = detail.license_info {
            badges.push(Span::styled(format!(" {}", license.name), style_dim()));
        }
        if let Some(ref branch) = detail.default_branch_ref {
            badges.push(Span::styled(
                format!(" branch:{}", branch.name),
                style_dim(),
            ));
        }
        if !badges.is_empty() {
            lines.push(Line::from(badges));
        }

        if !detail.topics.is_empty() {
            let topic_str: Vec<&str> = detail.topics.iter().map(|t| t.name.as_str()).collect();
            lines.push(Line::from(Span::styled(
                format!(" topics: {}", topic_str.join(", ")),
                style_dim(),
            )));
        }

        if let Some(ref url) = detail.homepage_url
            && !url.is_empty()
        {
            lines.push(Line::from(Span::styled(
                format!(" homepage: {url}"),
                style_dim(),
            )));
        }

        lines.push(Line::default());

        if let Some(ref readme) = self.readme_raw {
            let mdr_theme = ghx_to_mdr_theme();
            let styled =
                mdr::markdown::parse_markdown(readme, mdr_theme, area.width.saturating_sub(2));
            for sl in &styled {
                lines.push(sl.line.clone());
            }
        }

        self.lines_count = lines.len();
        render_scrollable(f, area, &lines, self.scroll);
    }

    fn render_issues(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.issues_loading {
            f.render_widget(spinner_line(tick, "Loading issues..."), area);
            return;
        }
        if let Some(ref err) = self.issues_error {
            let line = Line::from(Span::styled(
                format!(" Error: {err}"),
                ratatui::style::Style::default().fg(red()),
            ));
            f.render_widget(line, area);
            return;
        }
        if self.filtered_indices.is_empty() {
            let message = if self.filter.is_empty() {
                " No open issues".to_string()
            } else {
                format!(" No issues match ‘{}’", self.filter)
            };
            let line = Line::from(Span::styled(message, style_dim()));
            f.render_widget(line, area);
            return;
        }

        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&index| {
                let issue = &self.issues[index];
                let state_style = match issue.state.as_str() {
                    "OPEN" => ratatui::style::Style::default().fg(green()),
                    "CLOSED" => ratatui::style::Style::default().fg(red()),
                    _ => style_dim(),
                };
                let mut spans = vec![
                    Span::styled(format!("#{} ", issue.number), style_dim()),
                    Span::styled(&issue.title, style_normal()),
                ];
                if !issue.labels.is_empty() {
                    let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
                    spans.push(Span::styled(
                        format!(" [{}]", labels.join(", ")),
                        style_purple(),
                    ));
                }
                if let Some(ref author) = issue.author {
                    spans.push(Span::styled(format!(" @{}", author.login), style_dim()));
                }
                if let Some(ref ts) = issue.updated_at {
                    spans.push(Span::styled(format!(" · {}", timeago(ts)), style_dim()));
                }
                let state_icon = match issue.state.as_str() {
                    "OPEN" => "● ",
                    "CLOSED" => "✓ ",
                    _ => "  ",
                };
                let mut all_spans = vec![Span::styled(state_icon, state_style)];
                all_spans.extend(spans);
                ListItem::new(Line::from(all_spans))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_prs(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.prs_loading {
            f.render_widget(spinner_line(tick, "Loading pull requests..."), area);
            return;
        }
        if let Some(ref err) = self.prs_error {
            let line = Line::from(Span::styled(
                format!(" Error: {err}"),
                ratatui::style::Style::default().fg(red()),
            ));
            f.render_widget(line, area);
            return;
        }
        if self.filtered_indices.is_empty() {
            let message = if self.filter.is_empty() {
                " No open pull requests".to_string()
            } else {
                format!(" No pull requests match ‘{}’", self.filter)
            };
            let line = Line::from(Span::styled(message, style_dim()));
            f.render_widget(line, area);
            return;
        }

        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&index| {
                let pr = &self.prs[index];
                let check = pr.overall_check_status();
                let check_span = match check {
                    CheckStatus::Pass => {
                        Span::styled("✓ ", ratatui::style::Style::default().fg(green()))
                    }
                    CheckStatus::Fail => {
                        Span::styled("✗ ", ratatui::style::Style::default().fg(red()))
                    }
                    CheckStatus::Pending => {
                        Span::styled("● ", ratatui::style::Style::default().fg(yellow()))
                    }
                    CheckStatus::None => Span::raw("  "),
                };
                let mut spans = vec![
                    check_span,
                    Span::styled(format!("#{} ", pr.number), style_dim()),
                    Span::styled(&pr.title, style_normal()),
                ];
                if pr.is_draft {
                    spans.push(Span::styled(" [draft]", style_dim()));
                }
                if let Some(ref author) = pr.author {
                    spans.push(Span::styled(format!(" @{}", author.login), style_dim()));
                }
                if let Some(ref ts) = pr.updated_at {
                    spans.push(Span::styled(format!(" · {}", timeago(ts)), style_dim()));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(style_selected())
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn ghx_to_mdr_theme() -> mdr::theme::Theme {
    let t = crate::theme::current();
    let base = mdr::theme::default_theme();
    mdr::theme::Theme {
        border: t.border,
        accent: t.accent,
        text: t.fg,
        text_bright: t.fg,
        text_dim: t.dim,
        text_muted: t.border,
        heading: t.heading,
        error: t.red,
        cursor_bg: base.cursor_bg,
        labels: base.labels,
    }
}

fn render_scrollable(f: &mut Frame, area: Rect, lines: &[Line], scroll: u16) {
    let para = Paragraph::new(lines.to_vec()).scroll((scroll, 0));
    f.render_widget(para, area);
}
