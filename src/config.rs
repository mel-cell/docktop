use serde::{Deserialize, Serialize};
use ratatui::style::Color;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub docker: DockerConfig,
    #[serde(default)]
    pub keys: KeyConfig,
    
    #[serde(skip)]
    pub theme_data: Theme,
    
    #[serde(skip)]
    pub config_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KeyConfig {
    pub quit: String,
    pub refresh: String,
    pub toggle_wizard: String,
    pub toggle_help: String,
    pub up: String,
    pub down: String,
    pub enter: String,
    pub delete: String,
    pub details: String,
    pub edit: String,
    pub shell: String,
    pub db_cli: String,
    pub restart: String,
    pub stop: String,
    pub start: String,
    pub yaml: String,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            quit: "q".to_string(),
            refresh: "F5".to_string(),
            toggle_wizard: "w".to_string(),
            toggle_help: "?".to_string(),
            up: "k".to_string(),
            down: "j".to_string(),
            enter: "Enter".to_string(),
            delete: "x".to_string(),
            details: "Enter".to_string(),
            edit: "E".to_string(),
            shell: "e".to_string(),
            db_cli: "d".to_string(),
            restart: "r".to_string(),
            stop: "s".to_string(),
            start: "v".to_string(),
            yaml: "y".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GeneralConfig {
    pub theme: String,
    pub refresh_rate_ms: u64,
    pub mouse_support: bool,
    pub show_braille: bool,
    pub confirm_on_delete: bool,
    pub confirm_on_restart: bool,
    pub log_tail_lines: usize,
    pub default_sort: String,
    pub show_all_containers: bool,
    pub docker_cli_path: String,
    pub graphs_history_size: usize,
    pub enable_notifications: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "monochrome".to_string(),
            refresh_rate_ms: 1000,
            mouse_support: true,
            show_braille: true,
            confirm_on_delete: true,
            confirm_on_restart: false,
            log_tail_lines: 100,
            default_sort: "status".to_string(),
            show_all_containers: true,
            docker_cli_path: "/usr/bin/docker".to_string(),
            graphs_history_size: 60,
            enable_notifications: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DockerConfig {
    pub socket_path: String,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            socket_path: "unix:///var/run/docker.sock".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ThemeDefinition {
    pub name: String,
    pub background: String,
    pub foreground: String,
    pub border: String,
    pub running: String,
    pub stopped: String,
    pub restarting: String,
    pub selection_bg: String,
    pub selection_fg: String,
    pub header_fg: String,
    pub cpu_low: String,
    pub cpu_mid: String,
    pub cpu_high: String,
    pub memory_chart: String,
    pub network_rx: String,
    pub network_tx: String,
    pub chart_low: String,
    pub chart_mid: String,
    pub chart_high: String,
    pub header_bg: String,
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self {
            name: "Dracula (Default)".to_string(),
            background: "#282a36".to_string(),
            foreground: "#f8f8f2".to_string(),
            border: "#6272a4".to_string(),
            running: "#50fa7b".to_string(),
            stopped: "#ff5555".to_string(),
            restarting: "#ffb86c".to_string(),
            selection_bg: "#bd93f9".to_string(),
            selection_fg: "#282a36".to_string(),
            header_fg: "#8be9fd".to_string(),
            cpu_low: "#50fa7b".to_string(),
            cpu_mid: "#ffb86c".to_string(),
            cpu_high: "#ff5555".to_string(),
            memory_chart: "#bd93f9".to_string(),
            network_rx: "#8be9fd".to_string(),
            network_tx: "#ff79c6".to_string(),
            chart_low: "#50fa7b".to_string(),
            chart_mid: "#ffb86c".to_string(),
            chart_high: "#ff5555".to_string(),
            header_bg: "#44475a".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub running: Color,
    pub stopped: Color,
    pub restarting: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub header_fg: Color,
    pub cpu_low: Color,
    pub cpu_mid: Color,
    pub cpu_high: Color,
    pub memory_chart: Color,
    pub network_rx: Color,
    pub network_tx: Color,
    pub chart_low: Color,
    pub chart_mid: Color,
    pub chart_high: Color,
    pub header_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_definition(&ThemeDefinition::default())
    }
}

impl Theme {
    pub fn from_definition(def: &ThemeDefinition) -> Self {
        Self {
            background: parse_hex_color(&def.background),
            foreground: parse_hex_color(&def.foreground),
            border: parse_hex_color(&def.border),
            running: parse_hex_color(&def.running),
            stopped: parse_hex_color(&def.stopped),
            restarting: parse_hex_color(&def.restarting),
            selection_bg: parse_hex_color(&def.selection_bg),
            selection_fg: parse_hex_color(&def.selection_fg),
            header_fg: parse_hex_color(&def.header_fg),
            cpu_low: parse_hex_color(&def.cpu_low),
            cpu_mid: parse_hex_color(&def.cpu_mid),
            cpu_high: parse_hex_color(&def.cpu_high),
            memory_chart: parse_hex_color(&def.memory_chart),
            network_rx: parse_hex_color(&def.network_rx),
            network_tx: parse_hex_color(&def.network_tx),
            chart_low: parse_hex_color(&def.chart_low),
            chart_mid: parse_hex_color(&def.chart_mid),
            chart_high: parse_hex_color(&def.chart_high),
            header_bg: parse_hex_color(&def.header_bg),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut content = String::new();
        let mut path = None;

        if let Ok(home) = std::env::var("HOME") {
            let config_path = Path::new(&home).join(".config/docktop/config.toml");
            if config_path.exists() {
                if let Ok(c) = fs::read_to_string(&config_path) {
                    content = c;
                    path = Some(config_path.to_string_lossy().to_string());
                }
            }
        }
        
        if content.is_empty() {
             if let Ok(c) = fs::read_to_string("config.toml") {
                 content = c;
                 path = Some("config.toml".to_string());
             }
        }

        let mut config: Config = toml::from_str(&content).unwrap_or_else(|_| Config {
            general: GeneralConfig::default(),
            docker: DockerConfig::default(),
            keys: KeyConfig::default(),
            theme_data: Theme::default(),
            config_path: None,
        });
        
        config.config_path = path;
        config.theme_data = load_theme(&config.general.theme);
        config
    }

    pub fn save(&self) {
        let path = self.config_path.as_deref().unwrap_or("config.toml");
        if let Ok(content) = toml::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }
}

pub fn parse_hex_color(hex: &str) -> Color {
    if hex.len() == 7 && hex.starts_with('#') {
        let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(0);
        Color::Rgb(r, g, b)
    } else {
        Color::White
    }
}

pub fn load_theme(name: &str) -> Theme {
    if let Ok(home) = std::env::var("HOME") {
        let path = Path::new(&home).join(format!(".config/docktop/themes/{}.toml", name));
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(def) = toml::from_str::<ThemeDefinition>(&content) {
                    return Theme::from_definition(&def);
                }
            }
        }
    }

    let local_path = format!("themes/{}.toml", name);
    if Path::new(&local_path).exists() {
        if let Ok(content) = fs::read_to_string(local_path) {
             if let Ok(def) = toml::from_str::<ThemeDefinition>(&content) {
                 return Theme::from_definition(&def);
             }
        }
    }

    Theme::from_definition(&get_preset_theme_def(name))
}

pub fn get_preset_theme_def(name: &str) -> ThemeDefinition {
    match name.to_lowercase().as_str() {
        "monochrome" => ThemeDefinition {
            name: "Monochrome".to_string(),
            background: "#000000".to_string(),
            foreground: "#ffffff".to_string(),
            border: "#808080".to_string(),
            running: "#ffffff".to_string(),
            stopped: "#555555".to_string(),
            restarting: "#aaaaaa".to_string(),
            selection_bg: "#ffffff".to_string(),
            selection_fg: "#000000".to_string(),
            header_fg: "#ffffff".to_string(),
            cpu_low: "#555555".to_string(),
            cpu_mid: "#aaaaaa".to_string(),
            cpu_high: "#ffffff".to_string(),
            memory_chart: "#aaaaaa".to_string(),
            network_rx: "#ffffff".to_string(),
            network_tx: "#aaaaaa".to_string(),
            chart_low: "#555555".to_string(),
            chart_mid: "#aaaaaa".to_string(),
            chart_high: "#ffffff".to_string(),
            header_bg: "#333333".to_string(),
        },
        "gruvbox" | "gruvbox dark" => ThemeDefinition {
            name: "Gruvbox Dark".to_string(),
            background: "#282828".to_string(),
            foreground: "#ebdbb2".to_string(),
            border: "#928374".to_string(),
            running: "#b8bb26".to_string(),
            stopped: "#fb4934".to_string(),
            restarting: "#fabd2f".to_string(),
            selection_bg: "#d65d0e".to_string(),
            selection_fg: "#282828".to_string(),
            header_fg: "#83a598".to_string(),
            cpu_low: "#b8bb26".to_string(),
            cpu_mid: "#fabd2f".to_string(),
            cpu_high: "#fb4934".to_string(),
            memory_chart: "#d3869b".to_string(),
            network_rx: "#83a598".to_string(),
            network_tx: "#fe8019".to_string(),
            chart_low: "#b8bb26".to_string(),
            chart_mid: "#fabd2f".to_string(),
            chart_high: "#fb4934".to_string(),
            header_bg: "#3c3836".to_string(),
        },
        "cyberpunk" | "cyberpunk neon" => ThemeDefinition {
            name: "Cyberpunk Neon".to_string(),
            background: "#0d0e15".to_string(), // Deep dark slate/blue
            foreground: "#a9b1d6".to_string(), // Soft white/blue
            border: "#00f3ff".to_string(),     // Neon Cyan
            running: "#00ff94".to_string(),    // Neon Green
            stopped: "#ff0055".to_string(),    // Neon Red
            restarting: "#ffe600".to_string(), // Neon Yellow
            selection_bg: "#ff00ff".to_string(), // Neon Magenta (Selection)
            selection_fg: "#ffffff".to_string(), // White text on Magenta
            header_fg: "#00f3ff".to_string(),    // Cyan Headers
            cpu_low: "#00ff94".to_string(),
            cpu_mid: "#00f3ff".to_string(),
            cpu_high: "#ff0055".to_string(),
            memory_chart: "#bd00ff".to_string(), // Electric Purple
            network_rx: "#00f3ff".to_string(),
            network_tx: "#ff0055".to_string(),
            chart_low: "#00ff94".to_string(),
            chart_mid: "#00f3ff".to_string(),
            chart_high: "#ff0055".to_string(),
            header_bg: "#1a1b26".to_string(),    // Slightly lighter background
        },
        _ => ThemeDefinition::default(),
    }
}
