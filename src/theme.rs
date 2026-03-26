use ratatui::style::Color;
use std::collections::HashMap;
use std::sync::OnceLock;

static THEME: OnceLock<Theme> = OnceLock::new();

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

pub fn current() -> &'static Theme {
    THEME.get_or_init(|| {
        let home = std::env::var("HOME").unwrap_or_default();
        let themes_dir = format!("{home}/.config/ghx/themes");
        let name = read_config_theme(&home).unwrap_or_else(|| "tokyo-night-moon".into());
        load_theme_file(&themes_dir, &name).unwrap_or_else(|| fallback())
    })
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

fn load_theme_file(themes_dir: &str, name: &str) -> Option<Theme> {
    let path = format!("{themes_dir}/{name}.toml");
    let content = std::fs::read_to_string(path).ok()?;
    parse_theme(&content)
}

fn parse_theme(content: &str) -> Option<Theme> {
    // Parse [colors] section into name → hex color map
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

    // Resolve a ui key: ui maps name → color name, colors maps color name → Color
    let resolve = |ui_key: &str| -> Option<Color> {
        let color_name = ui.get(ui_key)?;
        colors.get(color_name).copied()
    };

    // Also need a direct color lookup for things not in [ui]
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
