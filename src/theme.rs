use ratatui::style::Color;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

static THEME: RwLock<Option<Theme>> = RwLock::new(None);

#[derive(Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub selection: Color,
    pub key: Color,
    pub border: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub purple: Color,
    pub heading: Color,
}

pub fn current() -> Theme {
    let guard = THEME.read().unwrap();
    guard.clone().unwrap_or_else(fallback)
}

pub fn set_theme(theme: Theme) {
    let mut guard = THEME.write().unwrap();
    *guard = Some(theme);
}

pub fn init() {
    let home = std::env::var("HOME").unwrap_or_default();
    let theme = read_config_value(&home, "theme")
        .and_then(|path| load_theme_path(&expand_home(&home, &path)))
        .unwrap_or_else(fallback);
    set_theme(theme);
}

pub fn load_all_themes() -> Vec<(String, Theme)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut themes = read_config_value(&home, "theme_catalog")
        .map(|path| load_theme_catalog(&expand_home(&home, &path), &home))
        .unwrap_or_default();
    if themes.is_empty() {
        themes.push(("tokyo night moon".to_string(), fallback()));
    }
    themes
}

fn load_theme_catalog(catalog_path: &Path, home: &str) -> Vec<(String, Theme)> {
    let Ok(content) = std::fs::read_to_string(catalog_path) else {
        return Vec::new();
    };
    let Ok(catalog) = content.parse::<toml::Value>() else {
        return Vec::new();
    };
    catalog
        .get("themes")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .filter_map(|configured_path| {
            let path = expand_home(home, configured_path);
            let theme = load_theme_path(&path)?;
            Some((theme_name(&path), theme))
        })
        .collect()
}

fn load_theme_path(path: &Path) -> Option<Theme> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_theme(&content)
}

fn expand_home(home: &str, configured_path: &str) -> PathBuf {
    configured_path
        .strip_prefix("~/")
        .map(|rest| Path::new(home).join(rest))
        .unwrap_or_else(|| PathBuf::from(configured_path))
}

fn theme_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("theme")
        .replace('-', " ")
}

pub fn configured_theme_name() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    read_config_value(&home, "theme")
        .map(|path| theme_name(&expand_home(&home, &path)))
        .unwrap_or_else(|| "tokyo night moon".into())
}

fn fallback() -> Theme {
    Theme {
        bg: hex(0x22, 0x24, 0x36),
        fg: hex(0xc8, 0xd3, 0xf5),
        dim: hex(0x63, 0x6d, 0xa6),
        accent: hex(0xc0, 0x99, 0xff),
        selection: hex(0x82, 0xaa, 0xff),
        key: hex(0x86, 0xe1, 0xfc),
        border: hex(0x3b, 0x42, 0x61),
        red: hex(0xff, 0x75, 0x7f),
        green: hex(0xc3, 0xe8, 0x8d),
        yellow: hex(0xff, 0xc7, 0x77),
        purple: hex(0xfc, 0xa7, 0xea),
        heading: hex(0x82, 0xaa, 0xff),
    }
}

fn read_config_value(home: &str, requested_key: &str) -> Option<String> {
    let path = format!("{home}/.config/ghx/config.toml");
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = parse_kv(line)
            && key == requested_key
            && !value.is_empty()
        {
            return Some(value.to_string());
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

    let accent = resolve("accent").unwrap_or(hex(0xc0, 0x99, 0xff));

    Some(Theme {
        bg: resolve("background")
            .or_else(|| color("bg"))
            .unwrap_or(hex(0x22, 0x24, 0x36)),
        fg: resolve("text")
            .or_else(|| color("fg"))
            .unwrap_or(hex(0xc8, 0xd3, 0xf5)),
        dim: resolve("text_dim")
            .or_else(|| color("fg_dim"))
            .unwrap_or(hex(0x63, 0x6d, 0xa6)),
        accent,
        selection: resolve("selection")
            .or_else(|| resolve("heading"))
            .unwrap_or(accent),
        key: resolve("key").or_else(|| color("cyan")).unwrap_or(accent),
        border: resolve("border")
            .or_else(|| color("fg_muted"))
            .unwrap_or(hex(0x3b, 0x42, 0x61)),
        red: color("red").unwrap_or(hex(0xff, 0x75, 0x7f)),
        green: color("green").unwrap_or(hex(0xc3, 0xe8, 0x8d)),
        yellow: color("yellow").unwrap_or(hex(0xff, 0xc7, 0x77)),
        purple: color("purple")
            .or_else(|| color("magenta"))
            .or_else(|| color("mauve"))
            .unwrap_or(hex(0xfc, 0xa7, 0xea)),
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    const SHARED_THEME: &str = r##"
[colors]
bg = "#222436"
fg = "#c8d3f5"
fg_dim = "#636da6"
fg_muted = "#3b4261"
magenta = "#c099ff"
blue = "#82aaff"
cyan = "#86e1fc"
green = "#c3e88d"
yellow = "#ffc777"
purple = "#fca7ea"
red = "#ff757f"

[ui]
background = "bg"
text = "fg"
text_dim = "fg_dim"
accent = "magenta"
selection = "blue"
key = "cyan"
border = "fg_muted"
heading = "blue"
"##;

    #[test]
    fn tokyo_night_moon_assigns_distinct_semantic_colors() {
        let theme = parse_theme(SHARED_THEME).unwrap();

        assert_eq!(theme.accent, hex(0xc0, 0x99, 0xff));
        assert_eq!(theme.selection, hex(0x82, 0xaa, 0xff));
        assert_eq!(theme.key, hex(0x86, 0xe1, 0xfc));
        assert_eq!(theme.green, hex(0xc3, 0xe8, 0x8d));
        assert_eq!(theme.purple, hex(0xfc, 0xa7, 0xea));
        assert_ne!(theme.accent, theme.selection);
        assert_ne!(theme.selection, theme.key);
    }

    #[test]
    fn catalog_loads_only_explicit_theme_paths() {
        let root = test_root();
        let themes_dir = root.join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        std::fs::write(themes_dir.join("synthetic-theme.toml"), SHARED_THEME).unwrap();
        std::fs::write(themes_dir.join("unlisted.toml"), SHARED_THEME).unwrap();
        let catalog = root.join("catalog.toml");
        std::fs::write(
            &catalog,
            format!(
                "themes = [\"{}\"]\n",
                themes_dir.join("synthetic-theme.toml").display()
            ),
        )
        .unwrap();

        let themes = load_theme_catalog(&catalog, root.to_str().unwrap());
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].0, "synthetic theme");
        assert_eq!(themes[0].1.accent, hex(0xc0, 0x99, 0xff));

        std::fs::remove_dir_all(root).unwrap();
    }

    fn test_root() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "ghx-theme-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
