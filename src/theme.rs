use ratatui::style::Color;
use std::collections::HashMap;
use std::sync::RwLock;

static THEME: RwLock<Option<Theme>> = RwLock::new(None);

#[derive(Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub border: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub purple: Color,
    pub heading: Color,
}

pub fn current() -> Theme {
    let guard = THEME.read().unwrap();
    guard.clone().unwrap_or_else(|| fallback())
}

pub fn set_theme(theme: Theme) {
    let mut guard = THEME.write().unwrap();
    *guard = Some(theme);
}

pub fn init() {
    let home = std::env::var("HOME").unwrap_or_default();
    let name = read_config_theme(&home).unwrap_or_else(|| "tokyo-night-moon".into());
    let themes = load_all_themes();
    let theme = themes
        .iter()
        .find(|(n, _)| n == &name)
        .map(|(_, t)| t.clone())
        .unwrap_or_else(fallback);
    set_theme(theme);
}

pub fn load_all_themes() -> Vec<(String, Theme)> {
    let mut themes = Vec::new();

    // Embedded themes from the themes/ directory at compile time
    let embedded: &[(&str, &str)] = &[
        ("classic", include_str!("../themes/classic.toml")),
        ("fire", include_str!("../themes/fire.toml")),
        ("matrix", include_str!("../themes/matrix.toml")),
        ("monochrome", include_str!("../themes/monochrome.toml")),
        ("ocean", include_str!("../themes/ocean.toml")),
        ("purple", include_str!("../themes/purple.toml")),
        ("sunset", include_str!("../themes/sunset.toml")),
        ("synthwave", include_str!("../themes/synthwave.toml")),
        ("tokyo-night-moon", include_str!("../themes/tokyo-night-moon.toml")),
    ];

    for (name, content) in embedded {
        if let Some(theme) = parse_theme(content) {
            themes.push((name.to_string(), theme));
        }
    }

    // Load user themes from ~/.config/ghx/themes/ (override embedded ones)
    let home = std::env::var("HOME").unwrap_or_default();
    let user_dir = format!("{home}/.config/ghx/themes");
    if let Ok(entries) = std::fs::read_dir(&user_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Some(theme) = parse_theme(&content) {
                            // Remove existing embedded theme with same name
                            themes.retain(|(n, _)| n != name);
                            themes.push((name.to_string(), theme));
                        }
                    }
                }
            }
        }
    }

    themes.sort_by(|(a, _), (b, _)| a.cmp(b));
    themes
}

pub fn configured_theme_name() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    read_config_theme(&home).unwrap_or_else(|| "tokyo-night-moon".into())
}

pub fn save_config_theme(name: &str) {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = format!("{home}/.config/ghx");
    let path = format!("{dir}/config.toml");

    // Read existing config, replace or add theme line
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;
    for line in content.lines() {
        if line.trim().starts_with("theme") {
            lines.push(format!("theme = \"{name}\""));
            found = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        lines.push(format!("theme = \"{name}\""));
    }

    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, lines.join("\n") + "\n");
}

fn fallback() -> Theme {
    Theme {
        bg: hex(0x22, 0x24, 0x36),
        fg: hex(0xc8, 0xd3, 0xf5),
        dim: hex(0x63, 0x6d, 0xa6),
        accent: hex(0xc0, 0x99, 0xff),
        border: hex(0x3b, 0x42, 0x61),
        red: hex(0xff, 0x75, 0x7f),
        green: hex(0xc3, 0xe8, 0x8d),
        yellow: hex(0xff, 0xc7, 0x77),
        purple: hex(0xfc, 0xa7, 0xea),
        heading: hex(0x82, 0xaa, 0xff),
    }
}

fn read_config_theme(home: &str) -> Option<String> {
    let path = format!("{home}/.config/ghx/config.toml");
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("theme") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let val = rest.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn parse_theme(content: &str) -> Option<Theme> {
    let mut colors: HashMap<&str, Color> = HashMap::new();
    let mut ui: HashMap<&str, &str> = HashMap::new();
    let mut section = "";

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            section = if line.contains("colors") {
                "colors"
            } else if line.contains("ui") {
                "ui"
            } else {
                ""
            };
            continue;
        }
        if let Some((key, val)) = parse_kv(line) {
            match section {
                "colors" => {
                    if let Some(c) = parse_hex_color(val) {
                        colors.insert(key, c);
                    }
                }
                "ui" => {
                    ui.insert(key, val);
                }
                _ => {}
            }
        }
    }

    let resolve = |ui_key: &str| -> Option<Color> {
        let color_name = ui.get(ui_key)?;
        colors.get(color_name).copied()
    };

    let color = |name: &str| -> Option<Color> { colors.get(name).copied() };

    Some(Theme {
        bg: color("bg").unwrap_or(hex(0x22, 0x24, 0x36)),
        fg: resolve("text").or_else(|| color("fg")).unwrap_or(hex(0xc8, 0xd3, 0xf5)),
        dim: resolve("text_dim").or_else(|| color("fg_dim")).unwrap_or(hex(0x63, 0x6d, 0xa6)),
        accent: resolve("accent").unwrap_or(hex(0xc0, 0x99, 0xff)),
        border: resolve("border").or_else(|| color("fg_muted")).unwrap_or(hex(0x3b, 0x42, 0x61)),
        red: color("red").unwrap_or(hex(0xff, 0x75, 0x7f)),
        green: color("green").unwrap_or(hex(0xc3, 0xe8, 0x8d)),
        yellow: color("yellow").unwrap_or(hex(0xff, 0xc7, 0x77)),
        purple: color("magenta").or_else(|| color("purple")).unwrap_or(hex(0xfc, 0xa7, 0xea)),
        heading: resolve("heading").unwrap_or(hex(0x82, 0xaa, 0xff)),
    })
}

fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim();
    let val = rest.trim().trim_matches('"').trim_matches('\'');
    Some((key, val))
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

const fn hex(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}
