mod app;
#[allow(dead_code)]
mod gh;
mod theme;
mod ui;

use app::{App, Screen, Tab};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::time::Duration;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
    DefaultTerminal, Frame,
};
use std::io;
use ui::*;

fn main() -> anyhow::Result<()> {
    let context_repo = gh::current_repo();

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = ratatui::init();

    let mut app = App::new(context_repo);
    app.init();

    let result = run(&mut terminal, &mut app);

    ratatui::restore();
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    loop {
        app.tick = app.tick.wrapping_add(1);

        // Poll for async data
        app.repo_list.poll();
        app.notif_list.poll();
        app.search.poll();
        if let Some(ref mut detail) = app.repo_detail {
            detail.poll();
        }

        terminal.draw(|f| draw(f, app))?;

        // Poll for events with timeout so we can check async results
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            // Help overlay — any key dismisses
            if app.show_help {
                app.show_help = false;
                continue;
            }

            // Filter input mode (only on home/repos)
            if app.screen == Screen::Home && app.tab == Tab::Repos && app.repo_list.filtering {
                match key.code {
                    KeyCode::Esc => {
                        app.repo_list.filtering = false;
                        app.repo_list.filter.clear();
                        app.repo_list.refilter();
                    }
                    KeyCode::Backspace => {
                        app.repo_list.filter.pop();
                        app.repo_list.refilter();
                    }
                    KeyCode::Enter => {
                        app.repo_list.filtering = false;
                    }
                    KeyCode::Char(c) => {
                        app.repo_list.filter.push(c);
                        app.repo_list.refilter();
                    }
                    _ => {}
                }
                continue;
            }

            // Search input mode
            if app.screen == Screen::Home && app.tab == Tab::Search && app.search.editing {
                match key.code {
                    KeyCode::Esc => {
                        app.search.editing = false;
                    }
                    KeyCode::Backspace => {
                        app.search.query.pop();
                    }
                    KeyCode::Enter => {
                        app.search.editing = false;
                        app.search.search();
                    }
                    KeyCode::Char(c) => {
                        app.search.query.push(c);
                    }
                    KeyCode::Tab => {
                        app.search.editing = false;
                        app.next_tab();
                    }
                    KeyCode::BackTab => {
                        app.search.editing = false;
                        app.prev_tab();
                    }
                    _ => {}
                }
                continue;
            }

            // Global keys
            match key.code {
                KeyCode::Char('q') => {
                    if app.screen == Screen::Home {
                        app.should_quit = true;
                    } else {
                        app.go_back();
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.should_quit = true
                }
                KeyCode::Char('?') => app.show_help = true,
                KeyCode::Char('o') => app.on_open(),
                KeyCode::Esc | KeyCode::Backspace => {
                    if app.screen != Screen::Home {
                        app.go_back();
                    } else if !app.repo_list.filter.is_empty() {
                        app.repo_list.filter.clear();
                        app.repo_list.refilter();
                    }
                }
                _ => {}
            }

            // Screen-specific keys
            match app.screen {
                Screen::Home => match key.code {
                    KeyCode::Char('j') | KeyCode::Down => match app.tab {
                        Tab::Notifications => app.notif_list.move_down(),
                        Tab::Search => app.search.move_down(),
                        _ => app.repo_list.move_down(),
                    },
                    KeyCode::Char('k') | KeyCode::Up => match app.tab {
                        Tab::Notifications => app.notif_list.move_up(),
                        Tab::Search => app.search.move_up(),
                        _ => app.repo_list.move_up(),
                    },
                    KeyCode::Char('g') => match app.tab {
                        Tab::Notifications => {
                            if !app.notif_list.notifs.is_empty() {
                                app.notif_list.state.select(Some(0));
                            }
                        }
                        Tab::Search => {
                            if !app.search.results.is_empty() {
                                app.search.state.select(Some(0));
                            }
                        }
                        _ => app.repo_list.state.select(Some(0)),
                    },
                    KeyCode::Char('G') => match app.tab {
                        Tab::Notifications => {
                            let len = app.notif_list.notifs.len();
                            if len > 0 { app.notif_list.state.select(Some(len - 1)); }
                        }
                        Tab::Search => {
                            let len = app.search.results.len();
                            if len > 0 { app.search.state.select(Some(len - 1)); }
                        }
                        _ => {
                            let len = app.repo_list.filtered_indices.len();
                            if len > 0 { app.repo_list.state.select(Some(len - 1)); }
                        }
                    },
                    KeyCode::Char('m') => {
                        if app.tab == Tab::Notifications {
                            app.notif_list.mark_selected_read();
                        }
                    }
                    KeyCode::Char('/') => match app.tab {
                        Tab::Repos => app.repo_list.filtering = true,
                        Tab::Search => app.search.editing = true,
                        _ => {}
                    },
                    KeyCode::Enter => app.on_enter(),
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::BackTab => app.prev_tab(),
                    _ => {}
                },
                Screen::RepoDetail => {
                    use ui::repo_detail::RepoTab;
                    if let Some(ref mut d) = app.repo_detail {
                        let is_list = matches!(d.tab, RepoTab::Issues | RepoTab::PullRequests);
                        match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                if is_list { d.move_down(); } else { d.scroll_down(1); }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if is_list { d.move_up(); } else { d.scroll_up(1); }
                            }
                            KeyCode::Char('d') => {
                                if !is_list { d.scroll_down(10); }
                            }
                            KeyCode::Char('u') => {
                                if !is_list { d.scroll_up(10); }
                            }
                            KeyCode::Char('g') => {
                                if is_list {
                                    d.list_state.select(if d.current_list_len() > 0 { Some(0) } else { None });
                                } else {
                                    d.scroll = 0;
                                }
                            }
                            KeyCode::Char('G') => {
                                if is_list {
                                    let len = d.current_list_len();
                                    if len > 0 { d.list_state.select(Some(len - 1)); }
                                } else {
                                    d.scroll_down(d.lines_count as u16);
                                }
                            }
                            KeyCode::Tab => d.next_tab(),
                            KeyCode::BackTab => d.prev_tab(),
                            _ => {}
                        }
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Clear every cell to bg color + space
    let bg_style = Style::default().fg(bg()).bg(bg());
    let buf = f.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &mut buf[(x, y)];
            cell.set_char(' ');
            cell.set_style(bg_style);
        }
    }

    // Outer border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border()))
        .style(Style::default().bg(bg()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Layout: tabs, divider, content, status bar
    let chunks = Layout::vertical([
        Constraint::Length(1), // tab bar
        Constraint::Length(1), // divider
        Constraint::Min(1),   // content
        Constraint::Length(1), // status bar
    ])
    .split(inner);

    draw_tabs(f, app, chunks[0]);
    let divider = "─".repeat(chunks[1].width as usize);
    f.render_widget(
        Line::from(Span::styled(divider, Style::default().fg(border()))),
        chunks[1],
    );
    draw_content(f, app, chunks[2]);
    draw_status(f, app, chunks[3]);

    if app.show_help {
        draw_help(f, area);
    }
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    if app.screen == Screen::RepoDetail {
        if let Some(ref detail) = app.repo_detail {
            let mut spans = vec![Span::raw(" ")];
            for tab in ui::repo_detail::RepoTab::ALL {
                if tab == detail.tab {
                    spans.push(Span::styled(
                        format!("[{}]", tab.label()),
                        style_accent().add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(tab.label(), style_dim()));
                }
                spans.push(Span::raw("    "));
            }
            f.render_widget(Line::from(spans), area);
        }
        return;
    }

    let mut titles: Vec<String> = app.repo_list.source_labels();
    titles.push("Search".into());
    titles.push("Notifications".into());

    let active = match app.tab {
        Tab::Repos => app.repo_list.active_source_index(),
        Tab::Search => titles.len() - 2,
        Tab::Notifications => titles.len() - 1,
    };

    let mut spans = vec![Span::raw(" ")];
    for (i, title) in titles.iter().enumerate() {
        if i == active {
            spans.push(Span::styled(
                format!("[{title}]"),
                style_accent().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(title.as_str(), style_dim()));
        }
        spans.push(Span::raw("    "));
    }

    f.render_widget(Line::from(spans), area);
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let tick = app.tick;

    if app.screen == Screen::RepoDetail {
        if let Some(ref mut detail) = app.repo_detail {
            detail.render(f, area, tick);
        }
        return;
    }

    match app.tab {
        Tab::Repos => {
            if app.repo_list.filtering {
                let chunks = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Min(1),
                ])
                .split(area);

                let filter_line = Line::from(vec![
                    Span::styled(" / ", style_accent()),
                    Span::styled(format!("{}\u{2588}", app.repo_list.filter), style_normal()),
                ]);
                f.render_widget(filter_line, chunks[0]);
                app.repo_list.render(f, chunks[1], tick);
            } else if !app.repo_list.filter.is_empty() {
                let chunks = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Min(1),
                ])
                .split(area);

                let info = Line::from(Span::styled(
                    format!(
                        " filter: {} ({}/{})",
                        app.repo_list.filter,
                        app.repo_list.filtered_indices.len(),
                        app.repo_list.repos.len()
                    ),
                    style_dim(),
                ));
                f.render_widget(info, chunks[0]);
                app.repo_list.render(f, chunks[1], tick);
            } else {
                app.repo_list.render(f, area, tick);
            }
        }
        Tab::Search => {
            app.search.render(f, area, tick);
        }
        Tab::Notifications => {
            app.notif_list.ensure_loaded();
            app.notif_list.render(f, area, tick);
        }
    }
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let sep = Span::styled(" │ ", style_dim());

    let mut spans = vec![
        Span::styled(" ghx", style_bold().fg(accent())),
        sep.clone(),
    ];

    if let Some(ref repo) = app.selected_repo {
        spans.push(Span::styled(repo.as_str(), style_normal()));
        spans.push(sep.clone());
    }

    spans.push(Span::styled("?:help", style_dim()));

    f.render_widget(Line::from(spans), area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_lines = vec![
        ("Navigation", vec![
            ("j/k, ↑/↓", "Move up/down"),
            ("g/G", "Jump to top/bottom"),
            ("d/u", "Half-page down/up (overview)"),
            ("Tab/S-Tab", "Next/previous tab"),
            ("Enter", "Open selected item"),
            ("Esc/Bksp", "Go back"),
        ]),
        ("Actions", vec![
            ("o", "Open in browser"),
            ("/", "Filter repos / edit search"),
            ("m", "Mark notification read"),
            ("r", "Read in mdr (detail view)"),
            ("?", "Toggle this help"),
            ("q", "Quit / go back"),
        ]),
    ];

    // Calculate popup size
    let width = 54u16;
    let height = help_lines.iter().map(|(_, items)| items.len() + 2).sum::<usize>() as u16 + 3;

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width.min(area.width), height.min(area.height));

    // Clear popup area
    let bg_style = Style::default().bg(bg()).fg(bg());
    let buf = f.buffer_mut();
    for py in popup.y..popup.y + popup.height {
        for px in popup.x..popup.x + popup.width {
            if px < area.x + area.width && py < area.y + area.height {
                let cell = &mut buf[(px, py)];
                cell.set_char(' ');
                cell.set_style(bg_style);
            }
        }
    }

    // Draw border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent()))
        .title(Span::styled(" Help ", style_bold().fg(accent())))
        .style(Style::default().bg(bg()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    // Render help content
    let mut lines: Vec<Line> = Vec::new();
    for (section, items) in &help_lines {
        lines.push(Line::from(Span::styled(
            format!(" {section}"),
            style_bold().fg(accent()),
        )));
        for (key, desc) in items {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<14}"), style_accent()),
                Span::styled(*desc, style_normal()),
            ]));
        }
        lines.push(Line::default());
    }
    lines.push(Line::from(Span::styled(
        " Press any key to close",
        style_dim(),
    )));

    for (i, line) in lines.iter().enumerate() {
        let ly = inner.y + i as u16;
        if ly >= inner.y + inner.height {
            break;
        }
        f.render_widget(line.clone(), Rect::new(inner.x, ly, inner.width, 1));
    }
}
