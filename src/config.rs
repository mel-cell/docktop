use serde::Deserialize;
use ratatui::style::Color;
use std::fs;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(skip)]
    pub theme_data: Theme,
}

fn default_theme_name() -> String {
    "monochrome".to_string()
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub main_bg: Color,
    pub main_fg: Color,
    pub title: Color,
    pub hi_fg: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub inactive_fg: Color,
    pub graph_text: Color,
    pub border: Color,
    pub graph_color: Color,
    pub fish_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            main_bg: Color::Reset,
            main_fg: Color::White,
            title: Color::White,
            hi_fg: Color::White,
            selected_bg: Color::DarkGray,
            selected_fg: Color::White,
            inactive_fg: Color::Gray,
            graph_text: Color::Gray,
            border: Color::DarkGray,
            graph_color: Color::Green,
            fish_color: Color::Cyan,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let content = fs::read_to_string("config.toml").unwrap_or_default();
        let mut config: Config = toml::from_str(&content).unwrap_or_else(|_| Config {
            theme: default_theme_name(),
            theme_data: Theme::default(),
        });

        config.theme_data = load_theme(&config.theme);
        config
    }
}

fn load_theme(theme_name: &str) -> Theme {
    let path = format!("themes/{}.theme", theme_name);
    let content = fs::read_to_string(&path).unwrap_or_default();
    parse_theme(&content)
}

fn parse_theme(content: &str) -> Theme {
    let mut map = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("theme[") && line.contains("]=") {
            let parts: Vec<&str> = line.split("]=").collect();
            if parts.len() == 2 {
                let key = parts[0].trim_start_matches("theme[").trim();
                let value = parts[1].trim_matches('"').trim();
                map.insert(key.to_string(), value.to_string());
            }
        }
    }

    Theme {
        main_bg: parse_color_str(map.get("main_bg").map(|s| s.as_str()).unwrap_or("")),
        main_fg: parse_color_str(map.get("main_fg").map(|s| s.as_str()).unwrap_or("#FFFFFF")),
        title: parse_color_str(map.get("title").map(|s| s.as_str()).unwrap_or("#EEEEEE")),
        hi_fg: parse_color_str(map.get("hi_fg").map(|s| s.as_str()).unwrap_or("#FFFFFF")),
        selected_bg: parse_color_str(map.get("selected_bg").map(|s| s.as_str()).unwrap_or("#404040")),
        selected_fg: parse_color_str(map.get("selected_fg").map(|s| s.as_str()).unwrap_or("#FFFFFF")),
        inactive_fg: parse_color_str(map.get("inactive_fg").map(|s| s.as_str()).unwrap_or("#666666")),
        graph_text: parse_color_str(map.get("graph_text").map(|s| s.as_str()).unwrap_or("#888888")),
        border: parse_color_str(map.get("div_line").map(|s| s.as_str()).unwrap_or("#444444")),
        graph_color: parse_color_str(map.get("cpu_mid").map(|s| s.as_str()).unwrap_or("#888888")),
        fish_color: parse_color_str(map.get("proc_misc").map(|s| s.as_str()).unwrap_or("#00FFFF")),
    }
}

pub fn parse_color_str(color: &str) -> Color {
    if color.is_empty() {
        return Color::Reset;
    }
    if color.starts_with('#') {
        let hex = color.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            return Color::Rgb(r, g, b);
        } else if hex.len() == 2 {
             // Handle short hex like #BW (Black/White) - approximation
             // For now just fallback to white if unknown
             return Color::White;
        }
    }
    // Fallback for named colors if needed, though btop uses hex
    Color::White
}
