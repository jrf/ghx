use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};

use super::*;
use crate::ui::repo_list::RepoList;

pub struct SourcePicker {
    pub visible: bool,
    pub filtering: bool,
    filter: String,
    filtered_indices: Vec<usize>,
    selected: usize,
}

impl SourcePicker {
    pub fn new() -> Self {
        Self {
            visible: false,
            filtering: false,
            filter: String::new(),
            filtered_indices: Vec::new(),
            selected: 0,
        }
    }

    pub fn open(&mut self, repos: &RepoList) {
        self.visible = true;
        self.filtering = false;
        self.filter.clear();
        self.refilter(repos);
        self.selected = self
            .filtered_indices
            .iter()
            .position(|&index| index == repos.active_source_index())
            .unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.filtering = false;
        self.filter.clear();
    }

    pub fn start_filtering(&mut self) {
        self.filtering = true;
    }

    pub fn on_filter_key(&mut self, key: char, repos: &RepoList) {
        self.filter.push(key);
        self.refilter(repos);
    }

    pub fn on_filter_backspace(&mut self, repos: &RepoList) {
        self.filter.pop();
        self.refilter(repos);
    }

    pub fn clear_filter(&mut self, repos: &RepoList) {
        self.filter.clear();
        self.filtering = false;
        self.refilter(repos);
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.filtered_indices.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_to_first(&mut self) {
        self.selected = 0;
    }

    pub fn move_to_last(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = self.filtered_indices.len() - 1;
        }
    }

    pub fn selected_source_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected).copied()
    }

    fn refilter(&mut self, repos: &RepoList) {
        let query = self.filter.to_lowercase();
        self.filtered_indices = repos
            .source_labels()
            .iter()
            .enumerate()
            .filter(|(_, label)| query.is_empty() || label.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect();
        self.selected = if self.filtered_indices.is_empty() {
            0
        } else {
            self.selected.min(self.filtered_indices.len() - 1)
        };
    }

    pub fn render(&mut self, f: &mut Frame, repos: &RepoList, area: Rect) {
        // Organizations can finish loading while the picker is already open.
        self.refilter(repos);
        let labels = repos.source_labels();
        let width = 52u16.min(area.width.saturating_sub(2));
        let desired_height = (self.filtered_indices.len() as u16 + 6).clamp(8, 18);
        let height = desired_height.min(area.height.saturating_sub(2));
        let popup = Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        );

        f.render_widget(Clear, popup);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent()))
            .title(Span::styled(
                " Repository source ",
                style_bold().fg(accent()),
            ))
            .style(Style::default().bg(bg()));
        let inner = block.inner(popup);
        f.render_widget(block, popup);

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
        let filter_line = if self.filtering {
            Line::from(vec![
                Span::styled(" / ", style_key()),
                Span::styled(format!("{}\u{2588}", self.filter), style_normal()),
            ])
        } else if self.filter.is_empty() {
            Line::from(Span::styled(
                " Select My Repos, Starred, or an organization",
                style_dim(),
            ))
        } else {
            Line::from(Span::styled(
                format!(
                    " filter: {} ({}/{})",
                    self.filter,
                    self.filtered_indices.len(),
                    labels.len()
                ),
                style_dim(),
            ))
        };
        f.render_widget(filter_line, chunks[0]);

        if self.filtered_indices.is_empty() {
            f.render_widget(
                Line::from(Span::styled(
                    format!(" No sources match ‘{}’", self.filter),
                    style_dim(),
                )),
                chunks[1],
            );
        } else {
            let items: Vec<_> = self
                .filtered_indices
                .iter()
                .map(|&index| {
                    let label = &labels[index];
                    let source_kind = match index {
                        0 => "personal",
                        1 => "starred",
                        _ => "organization",
                    };
                    let mut spans = vec![
                        Span::styled(
                            if index == repos.active_source_index() {
                                "● "
                            } else {
                                "  "
                            },
                            if index == repos.active_source_index() {
                                Style::default().fg(green())
                            } else {
                                style_dim()
                            },
                        ),
                        Span::styled(label.clone(), style_normal()),
                        Span::styled(format!("  {source_kind}"), style_dim()),
                    ];
                    if index == repos.active_source_index() {
                        spans.push(Span::styled("  active", Style::default().fg(green())));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();
            let mut state = ListState::default().with_selected(Some(self.selected));
            let list = List::new(items)
                .highlight_style(style_selected())
                .highlight_symbol("> ");
            f.render_stateful_widget(list, chunks[1], &mut state);
        }

        let status = if self.filtering {
            vec![
                Span::styled(" Enter", style_key()),
                Span::styled(" select    ", style_dim()),
                Span::styled("Esc", style_key()),
                Span::styled(" clear", style_dim()),
            ]
        } else {
            vec![
                Span::styled(" Enter", style_key()),
                Span::styled(" select    ", style_dim()),
                Span::styled("/", style_key()),
                Span::styled(" filter    ", style_dim()),
                Span::styled("Esc", style_key()),
                Span::styled(" close", style_dim()),
            ]
        };
        f.render_widget(Line::from(status), chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_sources_and_preserves_underlying_index() {
        let mut repos = RepoList::new();
        repos.orgs = vec!["Synthetic Alpha".into(), "Synthetic Beta".into()];
        let mut picker = SourcePicker::new();
        picker.open(&repos);

        picker.on_filter_key('b', &repos);
        picker.on_filter_key('e', &repos);
        picker.on_filter_key('t', &repos);
        picker.on_filter_key('a', &repos);

        assert_eq!(picker.filtered_indices, vec![3]);
        assert_eq!(picker.selected_source_index(), Some(3));
    }
}
