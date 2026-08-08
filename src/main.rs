mod app;
#[allow(dead_code)]
mod gh;
mod theme;
mod ui;

use app::{App, Screen, Tab};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders},
};
use std::time::Duration;
use ui::*;

fn main() -> anyhow::Result<()> {
    let context_repo = gh::current_repo();

    theme::init();

    let mut terminal = ratatui::try_init()?;

    let mut app = App::new(context_repo);
    app.init();

    let result = run(&mut terminal, &mut app);

    ratatui::restore();

    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    loop {
        app.tick = app.tick.wrapping_add(1);

        // Poll for async data
        app.repo_list.poll();
        app.lists_view.poll();
        app.notif_list.poll();
        app.search.poll();
        if app.account_picker.poll() {
            app.reload_after_account_switch();
        }
        if let Some(ref mut detail) = app.repo_detail {
            detail.poll();
        }
        if let Some(ref mut detail) = app.item_detail {
            detail.poll();
        }

        terminal.draw(|f| draw(f, app))?;

        // Poll for events with timeout so we can check async results
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            // Repository source picker input
            if app.source_picker.visible {
                if app.source_picker.filtering {
                    match key.code {
                        KeyCode::Esc => app.source_picker.clear_filter(&app.repo_list),
                        KeyCode::Backspace => {
                            app.source_picker.on_filter_backspace(&app.repo_list);
                        }
                        KeyCode::Up => app.source_picker.move_up(),
                        KeyCode::Down => app.source_picker.move_down(),
                        KeyCode::Enter => {
                            if let Some(index) = app.source_picker.selected_source_index() {
                                app.source_picker.close();
                                app.select_repo_source(index);
                            }
                        }
                        KeyCode::Char(c) => {
                            app.source_picker.on_filter_key(c, &app.repo_list);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('j') | KeyCode::Down => app.source_picker.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.source_picker.move_up(),
                        KeyCode::Char('g') | KeyCode::Home => app.source_picker.move_to_first(),
                        KeyCode::Char('G') | KeyCode::End => app.source_picker.move_to_last(),
                        KeyCode::Char('/') => app.source_picker.start_filtering(),
                        KeyCode::Enter => {
                            if let Some(index) = app.source_picker.selected_source_index() {
                                app.source_picker.close();
                                app.select_repo_source(index);
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => {
                            app.source_picker.close();
                        }
                        _ => {}
                    }
                }
                continue;
            }

            // Account picker input
            if app.account_picker.visible {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => app.account_picker.move_down(),
                    KeyCode::Char('k') | KeyCode::Up => app.account_picker.move_up(),
                    KeyCode::Char('g') | KeyCode::Home => app.account_picker.move_to_first(),
                    KeyCode::Char('G') | KeyCode::End => app.account_picker.move_to_last(),
                    KeyCode::Enter => app.account_picker.confirm(),
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('A') => {
                        app.account_picker.close();
                    }
                    _ => {}
                }
                continue;
            }

            // Help overlay — any key dismisses
            if app.show_help {
                app.show_help = false;
                continue;
            }

            // Actions overlay — close it, then allow a displayed shortcut to run.
            if app.show_actions {
                app.show_actions = false;
                if matches!(key.code, KeyCode::Char('a') | KeyCode::Esc) {
                    continue;
                }
            }

            // Theme picker input
            if app.show_theme_picker {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        let next = (app.theme_index + 1).min(app.themes.len().saturating_sub(1));
                        app.theme_picker_select(next);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let prev = app.theme_index.saturating_sub(1);
                        app.theme_picker_select(prev);
                    }
                    KeyCode::Char('g') | KeyCode::Home => {
                        app.theme_picker_select(0);
                    }
                    KeyCode::Char('G') | KeyCode::End => {
                        let last = app.themes.len().saturating_sub(1);
                        app.theme_picker_select(last);
                    }
                    KeyCode::Enter => app.theme_picker_confirm(),
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('t') => {
                        app.theme_picker_cancel();
                    }
                    _ => {}
                }
                continue;
            }

            // Filter input mode for lists repos
            if app.screen == Screen::Home && app.tab == Tab::Lists && app.lists_view.filtering {
                match key.code {
                    KeyCode::Esc => app.lists_view.on_filter_clear(),
                    KeyCode::Backspace => app.lists_view.on_filter_backspace(),
                    KeyCode::Enter => app.lists_view.filtering = false,
                    KeyCode::Char(c) => app.lists_view.on_filter_key(c),
                    _ => {}
                }
                continue;
            }

            // Filter input mode for notifications
            if app.screen == Screen::Home
                && app.tab == Tab::Notifications
                && app.notif_list.filtering
            {
                match key.code {
                    KeyCode::Esc => app.notif_list.clear_filter(),
                    KeyCode::Backspace => app.notif_list.on_filter_backspace(),
                    KeyCode::Enter => app.notif_list.filtering = false,
                    KeyCode::Char(c) => app.notif_list.on_filter_key(c),
                    _ => {}
                }
                continue;
            }

            // Filter input mode for repository issues and pull requests
            if app.screen == Screen::RepoDetail
                && app
                    .repo_detail
                    .as_ref()
                    .is_some_and(|detail| detail.filtering)
            {
                if let Some(ref mut detail) = app.repo_detail {
                    match key.code {
                        KeyCode::Esc => detail.clear_filter(),
                        KeyCode::Backspace => detail.on_filter_backspace(),
                        KeyCode::Enter => detail.filtering = false,
                        KeyCode::Char(c) => detail.on_filter_key(c),
                        _ => {}
                    }
                }
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
                KeyCode::Char('a') => app.show_actions = true,
                KeyCode::Char('A') => app.account_picker.open(),
                KeyCode::Char('s') if app.screen == Screen::Home => app.open_source_picker(),
                KeyCode::Char('t') => app.open_theme_picker(),
                KeyCode::Char('o') => app.on_open(),
                KeyCode::Esc | KeyCode::Backspace => {
                    if app.screen == Screen::RepoDetail
                        && app
                            .repo_detail
                            .as_ref()
                            .is_some_and(|detail| !detail.filter.is_empty())
                    {
                        if let Some(ref mut detail) = app.repo_detail {
                            detail.clear_filter();
                        }
                    } else if app.screen != Screen::Home {
                        app.go_back();
                    } else if app.tab == Tab::Repos && !app.repo_list.filter.is_empty() {
                        app.repo_list.filter.clear();
                        app.repo_list.refilter();
                    } else if app.tab == Tab::Lists && app.lists_view.has_filter() {
                        app.lists_view.on_filter_clear();
                    } else if app.tab == Tab::Lists && app.lists_view.go_back() {
                        // went back from list repos to list names
                    } else if app.tab == Tab::Notifications && !app.notif_list.filter.is_empty() {
                        app.notif_list.clear_filter();
                    }
                }
                _ => {}
            }

            // Compute page size from terminal height
            let page_size = crossterm::terminal::size()
                .map(|(_, h)| (h as usize).saturating_sub(6))
                .unwrap_or(20);

            // Screen-specific keys
            match app.screen {
                Screen::Home => match key.code {
                    KeyCode::Char('j') | KeyCode::Down => match app.tab {
                        Tab::Lists => app.lists_view.move_down(),
                        Tab::Notifications => app.notif_list.move_down(),
                        Tab::Search => app.search.move_down(),
                        _ => app.repo_list.move_down(),
                    },
                    KeyCode::Char('k') | KeyCode::Up => match app.tab {
                        Tab::Lists => app.lists_view.move_up(),
                        Tab::Notifications => app.notif_list.move_up(),
                        Tab::Search => app.search.move_up(),
                        _ => app.repo_list.move_up(),
                    },
                    KeyCode::Char('g') | KeyCode::Home => match app.tab {
                        Tab::Lists => app.lists_view.move_to_first(),
                        Tab::Notifications => app.notif_list.move_to_first(),
                        Tab::Search => app.search.move_to_first(),
                        _ => app.repo_list.move_to_first(),
                    },
                    KeyCode::Char('G') | KeyCode::End => match app.tab {
                        Tab::Lists => app.lists_view.move_to_last(),
                        Tab::Notifications => app.notif_list.move_to_last(),
                        Tab::Search => app.search.move_to_last(),
                        _ => app.repo_list.move_to_last(),
                    },
                    KeyCode::PageDown | KeyCode::Char('f')
                        if key.code == KeyCode::PageDown
                            || key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        match app.tab {
                            Tab::Lists => app.lists_view.page_down(page_size),
                            Tab::Notifications => app.notif_list.page_down(page_size),
                            Tab::Search => app.search.page_down(page_size),
                            _ => app.repo_list.page_down(page_size),
                        }
                    }
                    KeyCode::PageUp | KeyCode::Char('b')
                        if key.code == KeyCode::PageUp
                            || key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        match app.tab {
                            Tab::Lists => app.lists_view.page_up(page_size),
                            Tab::Notifications => app.notif_list.page_up(page_size),
                            Tab::Search => app.search.page_up(page_size),
                            _ => app.repo_list.page_up(page_size),
                        }
                    }
                    KeyCode::Char('m') if app.tab == Tab::Notifications => {
                        app.notif_list.mark_selected_read();
                    }
                    KeyCode::Char('r') if app.tab == Tab::Lists => {
                        app.lists_view.retry();
                    }
                    KeyCode::Char('/') => match app.tab {
                        Tab::Repos => app.repo_list.filtering = true,
                        Tab::Lists => app.lists_view.filtering = true,
                        Tab::Search => app.search.editing = true,
                        Tab::Notifications => app.notif_list.filtering = true,
                    },
                    KeyCode::Char(']') if app.tab == Tab::Repos => {
                        app.repo_list.select_next_source();
                    }
                    KeyCode::Char('[') if app.tab == Tab::Repos => {
                        app.repo_list.select_previous_source();
                    }
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
                                if is_list {
                                    d.move_down();
                                } else {
                                    d.scroll_down(1);
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if is_list {
                                    d.move_up();
                                } else {
                                    d.scroll_up(1);
                                }
                            }
                            KeyCode::Char('d') if !is_list => {
                                d.scroll_down(10);
                            }
                            KeyCode::Char('u') if !is_list => {
                                d.scroll_up(10);
                            }
                            KeyCode::Char('g') | KeyCode::Home => {
                                if is_list {
                                    d.move_to_first();
                                } else {
                                    d.scroll = 0;
                                }
                            }
                            KeyCode::Char('G') | KeyCode::End => {
                                if is_list {
                                    d.move_to_last();
                                } else {
                                    d.scroll_down(d.lines_count as u16);
                                }
                            }
                            KeyCode::PageDown => {
                                if is_list {
                                    d.page_down_list(page_size);
                                } else {
                                    d.scroll_down(page_size as u16);
                                }
                            }
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if is_list {
                                    d.page_down_list(page_size);
                                } else {
                                    d.scroll_down(page_size as u16);
                                }
                            }
                            KeyCode::PageUp => {
                                if is_list {
                                    d.page_up_list(page_size);
                                } else {
                                    d.scroll_up(page_size as u16);
                                }
                            }
                            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if is_list {
                                    d.page_up_list(page_size);
                                } else {
                                    d.scroll_up(page_size as u16);
                                }
                            }
                            KeyCode::Tab => d.next_tab(),
                            KeyCode::BackTab => d.prev_tab(),
                            KeyCode::Char('/') if is_list => d.filtering = true,
                            KeyCode::Enter => app.on_enter(),
                            _ => {}
                        }
                    }
                }
                Screen::ItemDetail => {
                    if let Some(ref mut detail) = app.item_detail {
                        match key.code {
                            KeyCode::Char('j') | KeyCode::Down => detail.scroll_down(1),
                            KeyCode::Char('k') | KeyCode::Up => detail.scroll_up(1),
                            KeyCode::Char('d') => detail.scroll_down(10),
                            KeyCode::Char('u') => detail.scroll_up(10),
                            KeyCode::Char('g') | KeyCode::Home => detail.scroll_to_top(),
                            KeyCode::Char('G') | KeyCode::End => detail.scroll_to_bottom(),
                            KeyCode::PageDown => detail.scroll_down(page_size as u16),
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                detail.scroll_down(page_size as u16);
                            }
                            KeyCode::PageUp => detail.scroll_up(page_size as u16),
                            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                detail.scroll_up(page_size as u16);
                            }
                            KeyCode::Char('r') => detail.toggle_reader(),
                            KeyCode::Tab => detail.next_tab(),
                            KeyCode::BackTab => detail.prev_tab(),
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

    // Layout: breadcrumb, tabs, divider, content, divider, contextual actions
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    draw_breadcrumb(f, app, chunks[0]);
    draw_tabs(f, app, chunks[1]);
    let divider = "─".repeat(chunks[2].width as usize);
    f.render_widget(
        Line::from(Span::styled(divider, Style::default().fg(border()))),
        chunks[2],
    );
    draw_content(f, app, chunks[3]);
    let divider = "─".repeat(chunks[4].width as usize);
    f.render_widget(
        Line::from(Span::styled(divider, Style::default().fg(border()))),
        chunks[4],
    );
    draw_footer(f, app, chunks[5]);

    if app.show_help {
        draw_help(f, app, area);
    }

    if app.show_actions {
        draw_actions(f, app, area);
    }

    if app.show_theme_picker {
        draw_theme_picker(f, app, area);
    }

    if app.account_picker.visible {
        app.account_picker.render(f, area, app.tick);
    }

    if app.source_picker.visible {
        app.source_picker.render(f, &app.repo_list, area);
    }
}

fn status_prefix() -> Vec<Span<'static>> {
    vec![
        Span::styled(" ghx", style_bold().fg(accent())),
        Span::styled(" │ ", style_dim()),
    ]
}

fn draw_breadcrumb(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = status_prefix();
    match app.screen {
        Screen::ItemDetail => {
            if let Some(detail) = app.item_detail.as_ref() {
                spans.push(Span::styled(detail.repo_name.clone(), style_normal()));
                spans.push(Span::styled(" › ", style_dim()));
                spans.push(Span::styled(
                    format!("{} #{}", detail.kind_label(), detail.number),
                    style_bold().fg(heading()),
                ));
            }
        }
        Screen::RepoDetail => {
            if let Some(detail) = app.repo_detail.as_ref() {
                spans.push(Span::styled(detail.repo_name.clone(), style_normal()));
            }
        }
        Screen::Home => {
            let (section, context) = match app.tab {
                Tab::Repos => ("Repositories", Some(app.repo_list.active_source_label())),
                Tab::Lists => (
                    "Lists",
                    app.lists_view.current_list_name().map(String::from),
                ),
                Tab::Search => ("Search", None),
                Tab::Notifications => ("Notifications", None),
            };
            spans.push(Span::styled(section, style_normal()));
            if let Some(context) = context {
                spans.push(Span::styled(" › ", style_dim()));
                spans.push(Span::styled(context, style_bold().fg(heading())));
            }
        }
    }
    f.render_widget(Line::from(spans), area);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let (titles, active): (Vec<String>, usize) = match app.screen {
        Screen::ItemDetail => {
            let Some(detail) = app.item_detail.as_ref() else {
                return;
            };
            let mut titles = vec![detail.conversation_label().into()];
            if detail.has_diff() {
                titles.push("Diff".into());
            }
            let active = usize::from(detail.tab == ui::item_detail::ItemTab::Diff);
            (titles, active)
        }
        Screen::RepoDetail => {
            let Some(detail) = app.repo_detail.as_ref() else {
                return;
            };
            let titles = ui::repo_detail::RepoTab::ALL
                .iter()
                .map(|tab| tab.label().to_string())
                .collect();
            let active = ui::repo_detail::RepoTab::ALL
                .iter()
                .position(|tab| *tab == detail.tab)
                .unwrap_or(0);
            (titles, active)
        }
        Screen::Home => (
            vec![
                "Repositories".into(),
                "Lists".into(),
                "Search".into(),
                "Notifications".into(),
            ],
            match app.tab {
                Tab::Repos => 0,
                Tab::Lists => 1,
                Tab::Search => 2,
                Tab::Notifications => 3,
            },
        ),
    };

    let mut spans = vec![Span::raw(" ")];
    for (index, title) in titles.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("    "));
        }
        if index == active {
            spans.push(Span::styled(format!("[{title}]"), style_selected()));
        } else {
            spans.push(Span::styled(title.clone(), style_dim()));
        }
    }
    f.render_widget(Line::from(spans), area);
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let tick = app.tick;

    if app.screen == Screen::ItemDetail {
        if let Some(ref mut detail) = app.item_detail {
            detail.render(f, area, tick);
        }
        return;
    }

    if app.screen == Screen::RepoDetail {
        if let Some(ref mut detail) = app.repo_detail {
            detail.render(f, area, tick);
        }
        return;
    }

    match app.tab {
        Tab::Repos => {
            if app.repo_list.filtering {
                let chunks =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);

                let filter_line = Line::from(vec![
                    Span::styled(" / ", style_key()),
                    Span::styled(format!("{}\u{2588}", app.repo_list.filter), style_normal()),
                ]);
                f.render_widget(filter_line, chunks[0]);
                app.repo_list.render(f, chunks[1], tick);
            } else if !app.repo_list.filter.is_empty() {
                let chunks =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);

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
        Tab::Lists => {
            app.lists_view.ensure_loaded();
            app.lists_view.render(f, area, tick);
        }
        Tab::Search => {
            app.search.render(f, area, tick);
        }
        Tab::Notifications => {
            app.notif_list.ensure_loaded();
            let content_area = if app.notif_list.filtering || !app.notif_list.filter.is_empty() {
                let chunks =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
                let line = if app.notif_list.filtering {
                    Line::from(vec![
                        Span::styled(" / ", style_key()),
                        Span::styled(format!("{}\u{2588}", app.notif_list.filter), style_normal()),
                    ])
                } else {
                    Line::from(Span::styled(
                        format!(
                            " filter: {} ({}/{})",
                            app.notif_list.filter,
                            app.notif_list.filtered_indices.len(),
                            app.notif_list.notifs.len()
                        ),
                        style_dim(),
                    ))
                };
                f.render_widget(line, chunks[0]);
                chunks[1]
            } else {
                area
            };
            app.notif_list.render(f, content_area, tick);
        }
    }
}

fn is_filtering(app: &App) -> bool {
    match app.screen {
        Screen::Home => match app.tab {
            Tab::Repos => app.repo_list.filtering,
            Tab::Lists => app.lists_view.filtering,
            Tab::Search => app.search.editing,
            Tab::Notifications => app.notif_list.filtering,
        },
        Screen::RepoDetail => app
            .repo_detail
            .as_ref()
            .is_some_and(|detail| detail.filtering),
        Screen::ItemDetail => false,
    }
}

fn contextual_actions(app: &App) -> Vec<(&'static str, &'static str)> {
    if is_filtering(app) {
        return vec![
            ("Type", "Filter"),
            ("Enter", "Apply"),
            ("Backspace", "Delete"),
            ("Esc", "Clear"),
        ];
    }

    if app.screen == Screen::Home && app.tab == Tab::Lists && app.lists_view.has_error() {
        return vec![
            ("r", "Retry"),
            ("Tab", "Next tab"),
            ("A", "Account"),
            ("a", "Actions"),
            ("?", "Help"),
            ("q", "Quit"),
        ];
    }

    let mut actions = match app.screen {
        Screen::Home => match app.tab {
            Tab::Repos => vec![
                ("Enter", "Details"),
                ("/", "Filter"),
                ("s", "Sources"),
                ("[ / ]", "Previous/next source"),
                ("o", "Browser"),
                ("Tab", "Next tab"),
            ],
            Tab::Lists if app.lists_view.is_browsing_repos() => vec![
                ("Enter", "Details"),
                ("/", "Filter"),
                ("o", "Browser"),
                ("Esc", "Lists"),
            ],
            Tab::Lists => vec![("Enter", "Browse"), ("/", "Filter"), ("Tab", "Next tab")],
            Tab::Search => vec![
                ("/", "Search"),
                ("Enter", "Details"),
                ("o", "Browser"),
                ("Tab", "Next tab"),
            ],
            Tab::Notifications => vec![
                ("Enter", "Details"),
                ("m", "Mark read"),
                ("/", "Filter"),
                ("o", "Browser"),
            ],
        },
        Screen::RepoDetail => {
            let is_list = app.repo_detail.as_ref().is_some_and(|detail| {
                matches!(
                    detail.tab,
                    ui::repo_detail::RepoTab::Issues | ui::repo_detail::RepoTab::PullRequests
                )
            });
            if is_list {
                vec![
                    ("Enter", "Details"),
                    ("/", "Filter"),
                    ("o", "Browser"),
                    ("Tab", "Next tab"),
                ]
            } else {
                vec![("j/k", "Scroll"), ("o", "Browser"), ("Tab", "Next tab")]
            }
        }
        Screen::ItemDetail => {
            let mut item_actions = vec![("j/k", "Scroll"), ("o", "Browser")];
            if app
                .item_detail
                .as_ref()
                .is_some_and(|detail| detail.tab == ui::item_detail::ItemTab::Conversation)
            {
                item_actions.push(("r", "Reader"));
            }
            if app
                .item_detail
                .as_ref()
                .is_some_and(|detail| detail.has_diff())
            {
                item_actions.push(("Tab", "Conversation/Diff"));
            }
            item_actions
        }
    };
    if app.screen == Screen::Home && !actions.iter().any(|(key, _)| *key == "s") {
        actions.push(("s", "Sources"));
    }
    actions.push(("A", "Account"));
    actions.push(("a", "Actions"));
    actions.push(("?", "Help"));
    actions.push((
        "q",
        if app.screen == Screen::Home {
            "Quit"
        } else {
            "Back"
        },
    ));
    actions
}

fn navigation_help(app: &App) -> Vec<(&'static str, &'static str)> {
    match app.screen {
        Screen::Home => vec![
            ("j/k, ↑/↓", "Move selection"),
            ("g/G, Home/End", "First/last item"),
            ("PgDn/PgUp", "Page down/up"),
            ("Tab/S-Tab", "Next/previous section"),
            ("s", "Choose repository source"),
            ("[ / ]", "Previous/next repository source"),
        ],
        Screen::RepoDetail => vec![
            ("j/k, ↑/↓", "Move or scroll"),
            ("g/G, Home/End", "First/last or top/bottom"),
            ("PgDn/PgUp", "Page down/up"),
            ("Tab/S-Tab", "Next/previous repository tab"),
        ],
        Screen::ItemDetail => vec![
            ("j/k, ↑/↓", "Scroll one line"),
            ("d/u", "Scroll ten lines"),
            ("g/G, Home/End", "Top/bottom"),
            ("PgDn/PgUp", "Page down/up"),
        ],
    }
}

fn footer_actions(app: &App) -> Vec<(&'static str, &'static str)> {
    if is_filtering(app) {
        return contextual_actions(app);
    }
    contextual_actions(app)
        .into_iter()
        .filter(|(key, _)| !matches!(*key, "o" | "?" | "Tab"))
        .map(|(key, label)| match key {
            "[ / ]" => ("[/]", "Cycle"),
            "a" => (key, "More"),
            _ => (key, label),
        })
        .collect()
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (index, (key, label)) in footer_actions(app).iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", style_dim()));
        }
        spans.push(Span::styled(*key, style_key()));
        spans.push(Span::styled(format!(" {label}"), style_dim()));
    }
    f.render_widget(Line::from(spans), area);
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let help_lines = vec![
        ("Navigation", navigation_help(app)),
        ("Available here", contextual_actions(app)),
        (
            "Global",
            vec![
                ("A", "Switch GitHub account"),
                ("t", "Switch theme"),
                ("C-c", "Quit immediately"),
            ],
        ),
    ];

    // Calculate popup size
    let width = 54u16;
    let height = help_lines
        .iter()
        .map(|(_, items)| items.len() + 2)
        .sum::<usize>() as u16
        + 3;

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
            style_bold().fg(heading()),
        )));
        for (key, desc) in items {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<14}"), style_key()),
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

fn draw_actions(f: &mut Frame, app: &App, area: Rect) {
    let mut actions = contextual_actions(app);
    let global_index = actions.len().saturating_sub(4);
    actions.insert(global_index, ("t", "Switch theme"));
    if let Some(action) = actions.iter_mut().find(|(key, _)| *key == "a") {
        action.1 = "Close menu";
    }

    let width = 44u16;
    let height = (actions.len() as u16 + 4).min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width.min(area.width), height.min(area.height));

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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent()))
        .title(Span::styled(" Actions ", style_bold().fg(accent())))
        .style(Style::default().bg(bg()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    for (index, (key, description)) in actions.iter().enumerate() {
        if index as u16 >= inner.height.saturating_sub(1) {
            break;
        }
        let line = Line::from(vec![
            Span::styled(format!(" {key:<12}"), style_key()),
            Span::styled(*description, style_normal()),
        ]);
        f.render_widget(
            line,
            Rect::new(inner.x, inner.y + index as u16, inner.width, 1),
        );
    }
    let hint_y = inner.y + inner.height.saturating_sub(1);
    f.render_widget(
        Line::from(Span::styled(" Press a shortcut to run it", style_dim())),
        Rect::new(inner.x, hint_y, inner.width, 1),
    );
}

fn draw_theme_picker(f: &mut Frame, app: &App, area: Rect) {
    let count = app.themes.len();
    let width = 40u16;
    let height = (count as u16 + 4).min(area.height.saturating_sub(4));

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
        .title(Span::styled(" Theme ", style_bold().fg(accent())))
        .style(Style::default().bg(bg()));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    // Visible window for scrolling
    let visible = inner.height.saturating_sub(2) as usize; // reserve 2 lines for status
    let scroll_offset = if app.theme_index >= visible {
        app.theme_index - visible + 1
    } else {
        0
    };

    // Render theme list
    for (i, (name, _)) in app.themes.iter().enumerate().skip(scroll_offset) {
        let row = i - scroll_offset;
        if row >= visible {
            break;
        }
        let ly = inner.y + row as u16;

        let display = name.replace('-', " ");
        let line = if i == app.theme_index {
            Line::from(vec![
                Span::styled("  › ", style_selected()),
                Span::styled(
                    format!(
                        "{display:<width$}",
                        width = (inner.width as usize).saturating_sub(5)
                    ),
                    style_selected().bg(border()),
                ),
            ])
        } else {
            Line::from(vec![
                Span::raw("    "),
                Span::styled(display, style_normal()),
            ])
        };
        f.render_widget(line, Rect::new(inner.x, ly, inner.width, 1));
    }

    // Status line at bottom of popup
    let status_y = inner.y + inner.height - 1;
    let status = Line::from(vec![
        Span::styled(" j/k", style_key()),
        Span::styled(":select  ", style_dim()),
        Span::styled("enter", style_key()),
        Span::styled(":ok  ", style_dim()),
        Span::styled("esc", style_key()),
        Span::styled(":cancel", style_dim()),
    ]);
    f.render_widget(status, Rect::new(inner.x, status_y, inner.width, 1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width).fold(String::new(), |mut row, x| {
            row.push_str(buffer[(x, y)].symbol());
            row
        })
    }

    #[test]
    fn repository_source_stays_in_breadcrumb_not_primary_tabs() {
        let mut app = App::new(None);
        app.repo_list.orgs = vec!["Synthetic Organization".into()];
        app.repo_list.set_source_by_index(2);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let breadcrumb = row_text(buffer, 1);
        let tabs = row_text(buffer, 2);
        assert!(breadcrumb.contains("Repositories › Synthetic Organization"));
        assert!(tabs.contains("[Repositories]    Lists    Search    Notifications"));
        assert!(!tabs.contains("Synthetic Organization"));
        let theme = theme::current();
        assert_eq!(buffer[(23, 1)].fg, theme.heading);
        assert_eq!(buffer[(2, 2)].fg, theme.selection);
        assert_eq!(buffer[(2, 22)].fg, theme.key);
    }

    #[test]
    fn default_footer_keeps_account_more_and_quit_visible_at_eighty_columns() {
        let app = App::new(None);
        let actions = footer_actions(&app);
        let keys: Vec<_> = actions.iter().map(|(key, _)| *key).collect();
        let rendered_width = 1
            + actions
                .iter()
                .map(|(key, label)| key.len() + label.len() + 1)
                .sum::<usize>()
            + actions.len().saturating_sub(1) * 2;

        assert!(keys.contains(&"A"));
        assert!(keys.contains(&"a"));
        assert!(keys.contains(&"q"));
        assert!(!keys.contains(&"?"));
        assert!(rendered_width <= 78);
    }
}
