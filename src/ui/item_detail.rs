use crate::gh::{self, IssueDetail, ItemKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use std::sync::mpsc;
use std::thread;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTab {
    Conversation,
    Diff,
}

enum LoadMsg {
    Detail(Result<IssueDetail, String>),
    Diff(Result<String, String>),
}

pub struct ItemDetailView {
    pub repo_name: String,
    pub number: u32,
    pub kind: ItemKind,
    pub tab: ItemTab,
    pub body_only: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub detail: Option<IssueDetail>,
    pub diff_loading: bool,
    pub diff_error: Option<String>,
    pub diff: Option<String>,
    pub scroll: u16,
    pub lines_count: usize,
    visible_height: u16,
    tx: mpsc::Sender<LoadMsg>,
    rx: mpsc::Receiver<LoadMsg>,
}

impl ItemDetailView {
    pub fn new(repo_name: String, number: u32, kind: ItemKind) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut view = Self {
            repo_name,
            number,
            kind,
            tab: ItemTab::Conversation,
            body_only: false,
            loading: true,
            error: None,
            detail: None,
            diff_loading: false,
            diff_error: None,
            diff: None,
            scroll: 0,
            lines_count: 0,
            visible_height: 0,
            tx,
            rx,
        };
        view.load_detail();
        view
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            ItemKind::Issue => "Issue",
            ItemKind::PullRequest => "Pull Request",
        }
    }

    pub fn conversation_label(&self) -> &'static str {
        if self.body_only {
            "Reader"
        } else {
            "Conversation"
        }
    }

    pub fn has_diff(&self) -> bool {
        self.kind == ItemKind::PullRequest
    }

    pub fn next_tab(&mut self) {
        if !self.has_diff() {
            return;
        }
        self.tab = match self.tab {
            ItemTab::Conversation => ItemTab::Diff,
            ItemTab::Diff => ItemTab::Conversation,
        };
        self.body_only = false;
        self.scroll = 0;
        if self.tab == ItemTab::Diff {
            self.ensure_diff_loaded();
        }
    }

    pub fn prev_tab(&mut self) {
        self.next_tab();
    }

    pub fn toggle_reader(&mut self) {
        if self.tab == ItemTab::Conversation && self.detail.is_some() {
            self.body_only = !self.body_only;
            self.scroll = 0;
        }
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let max = (self.lines_count as u16).saturating_sub(self.visible_height);
        self.scroll = self.scroll.saturating_add(amount).min(max);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = (self.lines_count as u16).saturating_sub(self.visible_height);
    }

    pub fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                LoadMsg::Detail(result) => {
                    self.loading = false;
                    match result {
                        Ok(detail) => self.detail = Some(detail),
                        Err(error) => self.error = Some(error),
                    }
                }
                LoadMsg::Diff(result) => {
                    self.diff_loading = false;
                    match result {
                        Ok(diff) => self.diff = Some(diff),
                        Err(error) => self.diff_error = Some(error),
                    }
                }
            }
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        self.visible_height = area.height;
        match self.tab {
            ItemTab::Conversation => self.render_conversation(f, area, tick),
            ItemTab::Diff => self.render_diff(f, area, tick),
        }
    }

    fn load_detail(&mut self) {
        let repo = self.repo_name.clone();
        let number = self.number;
        let kind = self.kind;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = match kind {
                ItemKind::Issue => gh::view_issue(&repo, number),
                ItemKind::PullRequest => gh::view_pr(&repo, number),
            }
            .map_err(|error| error.to_string());
            let _ = tx.send(LoadMsg::Detail(result));
        });
    }

    fn ensure_diff_loaded(&mut self) {
        if !self.has_diff() || self.diff.is_some() || self.diff_loading || self.diff_error.is_some()
        {
            return;
        }

        self.diff_loading = true;
        let repo = self.repo_name.clone();
        let number = self.number;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = gh::pr_diff(&repo, number).map_err(|error| error.to_string());
            let _ = tx.send(LoadMsg::Diff(result));
        });
    }

    fn render_conversation(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.loading {
            f.render_widget(spinner_line(tick, "Loading conversation..."), area);
            return;
        }
        if let Some(ref error) = self.error {
            render_error(f, area, error);
            return;
        }
        let Some(detail) = self.detail.as_ref() else {
            return;
        };

        let markdown = build_conversation_markdown(detail, self.body_only);
        let styled = mdr::markdown::parse_markdown(
            &markdown,
            ghx_to_mdr_theme(),
            area.width.saturating_sub(2),
        );
        let lines: Vec<Line<'static>> = styled.into_iter().map(|line| line.line).collect();
        self.lines_count = lines.len();
        f.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), area);
    }

    fn render_diff(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        if self.diff_loading {
            f.render_widget(spinner_line(tick, "Loading pull request diff..."), area);
            return;
        }
        if let Some(ref error) = self.diff_error {
            render_error(f, area, error);
            return;
        }
        let Some(diff) = self.diff.as_ref() else {
            self.ensure_diff_loaded();
            return;
        };

        let lines: Vec<Line<'static>> = diff
            .lines()
            .map(|line| {
                let style = if line.starts_with("+++") || line.starts_with("---") {
                    Style::default().fg(yellow())
                } else if line.starts_with('+') {
                    Style::default().fg(green())
                } else if line.starts_with('-') {
                    Style::default().fg(red())
                } else if line.starts_with("@@") {
                    style_key()
                } else if line.starts_with("diff --git") {
                    style_bold().fg(purple())
                } else {
                    style_normal()
                };
                Line::from(Span::styled(line.to_string(), style))
            })
            .collect();
        self.lines_count = lines.len();
        f.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), area);
    }
}

fn build_conversation_markdown(detail: &IssueDetail, body_only: bool) -> String {
    let mut markdown = format!("# {}\n\n", detail.title);
    if !body_only {
        let author = detail
            .author
            .as_ref()
            .map(|author| format!("@{}", author.login))
            .unwrap_or_else(|| "unknown author".into());
        markdown.push_str(&format!("**{}** · {}", detail.state, author));
        if !detail.labels.is_empty() {
            let labels = detail
                .labels
                .iter()
                .map(|label| format!("`{}`", label.name))
                .collect::<Vec<_>>()
                .join(" ");
            markdown.push_str(&format!(" · {labels}"));
        }
        markdown.push_str("\n\n");
    }

    match detail
        .body
        .as_deref()
        .filter(|body| !body.trim().is_empty())
    {
        Some(body) => markdown.push_str(body),
        None => markdown.push_str("_No description provided._"),
    }

    if !body_only {
        markdown.push_str(&format!(
            "\n\n---\n\n## Comments ({})\n",
            detail.comments.len()
        ));
        if detail.comments.is_empty() {
            markdown.push_str("\n_No comments yet._\n");
        } else {
            for comment in &detail.comments {
                let author = comment
                    .author
                    .as_ref()
                    .map(|author| format!("@{}", author.login))
                    .unwrap_or_else(|| "unknown author".into());
                markdown.push_str(&format!("\n### {author}\n\n{}\n", comment.body));
            }
        }
    }
    markdown
}

fn render_error(f: &mut Frame, area: Rect, error: &str) {
    f.render_widget(
        Line::from(Span::styled(
            format!(" Error: {error}"),
            Style::default().fg(red()),
        )),
        area,
    );
}

fn ghx_to_mdr_theme() -> mdr::theme::Theme {
    let theme = crate::theme::current();
    let base = mdr::theme::default_theme();
    mdr::theme::Theme {
        border: theme.border,
        accent: theme.accent,
        text: theme.fg,
        text_bright: theme.fg,
        text_dim: theme.dim,
        text_muted: theme.border,
        heading: theme.heading,
        error: theme.red,
        cursor_bg: base.cursor_bg,
        labels: base.labels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gh::{Author, Comment, Label};

    fn synthetic_detail() -> IssueDetail {
        IssueDetail {
            number: 12,
            title: "Synthetic issue".into(),
            state: "OPEN".into(),
            body: Some("A synthetic body.".into()),
            author: Some(Author {
                login: "example-user".into(),
            }),
            labels: vec![Label {
                name: "test-only".into(),
            }],
            comments: vec![Comment {
                author: Some(Author {
                    login: "reviewer".into(),
                }),
                body: "A synthetic comment.".into(),
            }],
        }
    }

    #[test]
    fn conversation_includes_metadata_and_comments() {
        let markdown = build_conversation_markdown(&synthetic_detail(), false);

        assert!(markdown.contains("**OPEN** · @example-user · `test-only`"));
        assert!(markdown.contains("## Comments (1)"));
        assert!(markdown.contains("### @reviewer"));
    }

    #[test]
    fn reader_mode_contains_only_title_and_body() {
        let markdown = build_conversation_markdown(&synthetic_detail(), true);

        assert!(markdown.contains("# Synthetic issue"));
        assert!(markdown.contains("A synthetic body."));
        assert!(!markdown.contains("Comments"));
        assert!(!markdown.contains("@example-user"));
    }
}
