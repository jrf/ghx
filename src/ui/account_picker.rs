use crate::gh::{self, GitHubAccount};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};
use std::{sync::mpsc, thread};

use super::*;

enum PickerMsg {
    Accounts {
        generation: u64,
        result: Result<Vec<GitHubAccount>, String>,
    },
    Switched {
        generation: u64,
        result: Result<(), String>,
    },
}

pub struct AccountPicker {
    pub visible: bool,
    accounts: Vec<GitHubAccount>,
    selected: usize,
    loading: bool,
    switching: bool,
    error: Option<String>,
    generation: u64,
    tx: mpsc::Sender<PickerMsg>,
    rx: mpsc::Receiver<PickerMsg>,
}

impl AccountPicker {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            visible: false,
            accounts: Vec::new(),
            selected: 0,
            loading: false,
            switching: false,
            error: None,
            generation: 0,
            tx,
            rx,
        }
    }

    pub fn open(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.visible = true;
        self.accounts.clear();
        self.selected = 0;
        self.loading = true;
        self.switching = false;
        self.error = None;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = gh::list_accounts().map_err(|error| error.to_string());
            let _ = tx.send(PickerMsg::Accounts { generation, result });
        });
    }

    pub fn close(&mut self) {
        if !self.switching {
            self.visible = false;
        }
    }

    pub fn move_down(&mut self) {
        if !self.switching && self.selected + 1 < self.accounts.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        if !self.switching {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn move_to_first(&mut self) {
        if !self.switching {
            self.selected = 0;
        }
    }

    pub fn move_to_last(&mut self) {
        if !self.switching && !self.accounts.is_empty() {
            self.selected = self.accounts.len() - 1;
        }
    }

    pub fn confirm(&mut self) {
        if self.loading || self.switching {
            return;
        }
        let Some(account) = self.accounts.get(self.selected).cloned() else {
            return;
        };
        if account.active {
            self.visible = false;
            return;
        }

        self.switching = true;
        self.error = None;
        let generation = self.generation;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = gh::switch_account(&account.host, &account.login)
                .map_err(|error| error.to_string());
            let _ = tx.send(PickerMsg::Switched { generation, result });
        });
    }

    /// Returns true after the active account changes successfully.
    pub fn poll(&mut self) -> bool {
        let mut switched = false;
        while let Ok(message) = self.rx.try_recv() {
            match message {
                PickerMsg::Accounts { generation, result } if generation == self.generation => {
                    self.loading = false;
                    match result {
                        Ok(accounts) => {
                            self.accounts = accounts;
                            self.selected = self
                                .accounts
                                .iter()
                                .position(|account| account.active)
                                .unwrap_or(0);
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                PickerMsg::Switched { generation, result } if generation == self.generation => {
                    self.switching = false;
                    match result {
                        Ok(()) => {
                            self.visible = false;
                            switched = true;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                _ => {}
            }
        }
        switched
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, tick: usize) {
        let width = 60u16.min(area.width.saturating_sub(2));
        let desired_height = (self.accounts.len() as u16 + 6).clamp(8, 16);
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
            .title(Span::styled(" GitHub account ", style_bold().fg(accent())))
            .style(Style::default().bg(bg()));
        let inner = block.inner(popup);
        f.render_widget(block, popup);

        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
        f.render_widget(
            Line::from(Span::styled(
                " Select the account ghx should use",
                style_dim(),
            )),
            chunks[0],
        );

        if self.loading {
            f.render_widget(spinner_line(tick, "Loading gh accounts..."), chunks[1]);
        } else if self.accounts.is_empty() {
            let message = self
                .error
                .as_deref()
                .unwrap_or("No authenticated accounts. Run `gh auth login` first.");
            f.render_widget(
                Line::from(Span::styled(
                    format!(" {message}"),
                    Style::default().fg(red()),
                )),
                chunks[1],
            );
        } else {
            let items: Vec<_> = self
                .accounts
                .iter()
                .map(|account| {
                    let mut spans = vec![
                        Span::styled(
                            if account.active { "● " } else { "  " },
                            if account.active {
                                Style::default().fg(green())
                            } else {
                                style_dim()
                            },
                        ),
                        Span::styled(account.login.clone(), style_normal()),
                        Span::styled(format!(" @ {}", account.host), style_dim()),
                    ];
                    if account.active {
                        spans.push(Span::styled("  active", Style::default().fg(green())));
                    }
                    if account.uses_environment_token() {
                        spans.push(Span::styled(
                            "  environment token",
                            Style::default().fg(yellow()),
                        ));
                    }
                    if account.state != "Logged in" {
                        spans.push(Span::styled(
                            format!("  {}", account.state),
                            Style::default().fg(yellow()),
                        ));
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

        let status = if self.switching {
            spinner_line(tick, "Switching account...")
        } else if let Some(error) = self.error.as_deref() {
            Line::from(Span::styled(
                format!(" Error: {error}"),
                Style::default().fg(red()),
            ))
        } else {
            Line::from(vec![
                Span::styled(" Enter", style_accent()),
                Span::styled(" switch    ", style_dim()),
                Span::styled("Esc", style_accent()),
                Span::styled(" close", style_dim()),
            ])
        };
        f.render_widget(status, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_stays_within_available_accounts() {
        let mut picker = AccountPicker::new();
        picker.accounts = vec![
            GitHubAccount {
                host: "github.com".into(),
                login: "active-example".into(),
                active: true,
                state: "Logged in".into(),
                token_source: "keyring".into(),
            },
            GitHubAccount {
                host: "github.com".into(),
                login: "other-example".into(),
                active: false,
                state: "Logged in".into(),
                token_source: "keyring".into(),
            },
        ];

        picker.move_to_last();
        picker.move_down();
        assert_eq!(picker.selected, 1);
        picker.move_to_first();
        picker.move_up();
        assert_eq!(picker.selected, 0);
    }
}
