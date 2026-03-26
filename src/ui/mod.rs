pub mod notif_list;
pub mod repo_detail;
pub mod repo_list;
pub mod search;

use crate::theme;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_line(tick: usize, msg: &str) -> Line<'static> {
    let frame = SPINNER_FRAMES[tick / 2 % SPINNER_FRAMES.len()];
    Line::from(vec![
        Span::styled(format!(" {frame} "), style_accent()),
        Span::styled(msg.to_string(), style_dim()),
    ])
}

pub fn bg() -> Color { theme::current().bg }
pub fn fg() -> Color { theme::current().fg }
pub fn dim() -> Color { theme::current().dim }
pub fn accent() -> Color { theme::current().accent }
pub fn border() -> Color { theme::current().border }
pub fn red() -> Color { theme::current().red }
pub fn green() -> Color { theme::current().green }
pub fn yellow() -> Color { theme::current().yellow }
pub fn purple() -> Color { theme::current().purple }
pub fn style_normal() -> Style {
    Style::default().fg(fg())
}

pub fn style_dim() -> Style {
    Style::default().fg(dim())
}

pub fn style_accent() -> Style {
    Style::default().fg(accent())
}

pub fn style_selected() -> Style {
    Style::default().fg(accent()).add_modifier(Modifier::BOLD)
}

pub fn style_bold() -> Style {
    Style::default().fg(fg()).add_modifier(Modifier::BOLD)
}

pub fn style_purple() -> Style {
    Style::default().fg(purple())
}

pub fn timeago(ts: &str) -> String {
    let Ok(then) = parse_rfc3339(ts) else {
        return String::new();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now.saturating_sub(then);
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 86400 * 30 {
        format!("{}d ago", secs / 86400)
    } else if secs < 86400 * 365 {
        format!("{}mo ago", secs / (86400 * 30))
    } else {
        format!("{}y ago", secs / (86400 * 365))
    }
}

fn parse_rfc3339(s: &str) -> Result<u64, ()> {
    if s.len() < 19 {
        return Err(());
    }
    let year: u64 = s[0..4].parse().map_err(|_| ())?;
    let month: u64 = s[5..7].parse().map_err(|_| ())?;
    let day: u64 = s[8..10].parse().map_err(|_| ())?;
    let hour: u64 = s[11..13].parse().map_err(|_| ())?;
    let min: u64 = s[14..16].parse().map_err(|_| ())?;
    let sec: u64 = s[17..19].parse().map_err(|_| ())?;

    let mut days = 0u64;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [31, 28 + if is_leap(year) { 1 } else { 0 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 0..(month - 1) as usize {
        days += month_days[m];
    }
    days += day - 1;
    Ok(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
