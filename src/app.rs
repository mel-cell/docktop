use crossterm::event::KeyCode;
use crate::docker::{Container, ContainerStats, ContainerInspection};
use crate::config::Config;
use std::collections::VecDeque;
use std::fs;
use sysinfo::System;
use ratatui::widgets::ListState;

#[derive(Clone)]
pub struct WizardState {
    pub step: WizardStep,
}

#[derive(Clone, Debug)]
pub struct JanitorItem {
    pub id: String,
    pub name: String,
    pub kind: JanitorItemKind,
    pub size: u64,
    pub age: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum JanitorItemKind {
    Image,
    Volume,
    Container,
}

#[derive(Clone, PartialEq)]
pub enum PortStatus {
    None,
    Available,
    Occupied(String), // Process info
    Invalid,
}

#[derive(Clone, PartialEq)]
pub enum FileBrowserMode {
    Build,
    Compose,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Framework {
    Laravel,
    NextJs,
    NuxtJs,
    Go,
    Django,
    Rails,
    Rust,
    Manual,
}

impl Framework {
    pub fn display_name(&self) -> &str {
        match self {
            Framework::Laravel => "Laravel (PHP)",
            Framework::NextJs => "Next.js (Node)",
            Framework::NuxtJs => "Nuxt.js (Node)",
            Framework::Go => "Go (Golang)",
            Framework::Django => "Django (Python)",
            Framework::Rails => "Ruby on Rails",
            Framework::Rust => "Rust",
            Framework::Manual => "Manual / Custom",
        }
    }

    pub fn default_port(&self) -> &str {
        match self {
            Framework::Laravel => "8000",
            Framework::NextJs => "3000",
            Framework::NuxtJs => "3000",
            Framework::Go => "8080",
            Framework::Django => "8000",
            Framework::Rails => "3000",
            Framework::Rust => "8080",
            Framework::Manual => "80",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TreeItem {
    pub path: std::path::PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
    pub is_last: bool, // To draw └── vs ├──
}

#[derive(Debug, serde::Deserialize)]
pub struct ComposeFile {
    pub services: std::collections::HashMap<String, ServiceConfig>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ServiceConfig {
    #[allow(dead_code)]
    pub image: Option<String>,
    #[allow(dead_code)]
    pub build: Option<serde_yaml::Value>,
}

#[derive(Clone)]
pub enum WizardStep {
    ModeSelection { selected_index: usize },
    QuickRunInput {
        image: String,
        name: String,
        ports: String,
        env: String,
        cpu: String,
        memory: String,
        restart: String,
        show_advanced: bool,
        focused_field: usize,
        editing_id: Option<String>,
        port_status: PortStatus,
    },
    FileBrowser {
        current_path: std::path::PathBuf, // Root of the tree view
        list_state: ListState,
        items: Vec<TreeItem>, // Flattened tree items
        mode: FileBrowserMode,
    },
    DockerfileGenerator {
        path: std::path::PathBuf,
        detected_framework: Framework,
        detected_version: String,
        manual_selection_open: bool,
        manual_selected_index: usize,
        port: String,
        editing_port: bool,
        focused_option: usize,
        port_status: PortStatus,
    },
    BuildConf {
        tag: String,
        mount_volume: bool,
        focused_field: usize,
        path: std::path::PathBuf,
    },
    Processing {
        message: String,
        spinner_frame: usize,
    },
    ComposeGenerator {
        path: std::path::PathBuf,
    },
    ComposeServiceSelection {
        path: std::path::PathBuf,
        selected_services: Vec<String>,
        focused_index: usize,
        all_services: Vec<String>,
    },
    ResourceAllocation {
        path: std::path::PathBuf,
        services: Vec<String>,
        all_services: Vec<String>, // Added to support going back
        cpu_limit: String,
        mem_limit: String,
        focused_field: usize,
        detected_cpu: usize,
        detected_mem: u64,
    },
    Janitor {
        items: Vec<JanitorItem>,
        list_state: ListState,
        loading: bool,
    },
    OverwriteConfirm {
        path: std::path::PathBuf,
        detected_framework: Framework,
        detected_version: String,
        port: String,
    },
    Settings {
        focused_field: usize,
        temp_config: Config,
    },
    Error(String),
}

#[derive(Clone)]
pub enum WizardAction {
    Create { image: String, name: String, ports: String, env: String, cpu: String, memory: String, restart: String },
    Build { tag: String, path: std::path::PathBuf, mount: bool },
    ComposeUp { path: std::path::PathBuf, override_path: Option<std::path::PathBuf> },
    Replace { old_id: String, image: String, name: String, ports: String, env: String, cpu: String, memory: String, restart: String },
    ScanJanitor,
    CleanJanitor(Vec<JanitorItem>),
    Close,
}

#[derive(Clone)]
pub struct Fish {
    pub x: f64,
    pub y: usize, // Vertical lane (0-4)
    pub direction: f64,
    pub speed: f64,
}

pub struct App {
    pub containers: Vec<Container>,
    pub selected_index: usize,
    pub current_stats: Option<ContainerStats>,
    pub previous_stats: Option<ContainerStats>,
    pub current_inspection: Option<ContainerInspection>,
    pub logs: VecDeque<String>,
    pub is_loading_details: bool,
    pub action_status: Option<(String, std::time::Instant)>,
    pub cpu_history: Vec<(f64, f64)>,
    pub net_rx_history: Vec<(f64, f64)>,
    pub net_tx_history: Vec<(f64, f64)>,
    pub x_axis_bounds: [f64; 2],
    pub show_details: bool,
    pub net_axis_bounds: [f64; 2],
    pub config: Config,
    pub fishes: Vec<Fish>,
    pub wizard: Option<WizardState>,
    pub show_help: bool,
    pub globe_frames: Vec<Vec<String>>,
    pub _system: System,
}

impl App {
    pub fn new() -> App {
        let mut fishes = Vec::new();
        for i in 0..10 {
             fishes.push(Fish {
                x: (i * 5) as f64,
                y: i % 5,
                direction: if i % 2 == 0 { 1.0 } else { -1.0 },
                speed: 0.2 + (i as f64 * 0.1),
            });
        }

        let mut globe_frames = Vec::new();
        let content = include_str!("../assets/ earthAnimation.bat");
        
        let chunks: Vec<&str> = content.split("cls").collect();
        for chunk in chunks.iter().skip(1) {
            let mut frame = Vec::new();
            for line in chunk.lines() {
                let trimmed_start = line.trim_start();
                if trimmed_start.to_lowercase().starts_with("echo") {
                    if let Some(idx) = line.to_lowercase().find("echo") {
                        let content = &line[idx+4..];
                        let mut frame_line = content.to_string();
                        if frame_line.starts_with(' ') || frame_line.starts_with('.') {
                            frame_line.remove(0);
                        }
                        frame.push(frame_line);
                    }
                }
            }
            if !frame.is_empty() {
                globe_frames.push(frame);
            }
        }
        
        if globe_frames.is_empty() {
             globe_frames.push(vec!["Animation not found".to_string()]);
        }

        App {
            containers: vec![],
            selected_index: 0,
            current_stats: None,
            previous_stats: None,
            current_inspection: None,
            logs: VecDeque::with_capacity(100),
            is_loading_details: false,
            action_status: None,
            cpu_history: vec![],
            net_rx_history: vec![],
            net_tx_history: vec![],
            x_axis_bounds: [0.0, 100.0],
            show_details: false,
            net_axis_bounds: [0.0, 100.0],
            config: Config::load(),
            fishes,
            globe_frames,
            wizard: None,
            show_help: false,
            _system: System::new_all(),
        }
    }

    pub fn update_containers(&mut self, mut containers: Vec<crate::docker::Container>) {
        // Filter
        if !self.config.general.show_all_containers {
            containers.retain(|c| c.state == "running");
        }

        // Sort
        match self.config.general.default_sort.as_str() {
            "name" => containers.sort_by(|a, b| a.names.first().unwrap_or(&String::new()).cmp(b.names.first().unwrap_or(&String::new()))),
            "status" => containers.sort_by(|a, b| a.state.cmp(&b.state)),
            _ => {}
        }
        
        self.containers = containers;
    }

    pub fn next(&mut self) {
        if !self.containers.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.containers.len();
            self.set_loading();
        }
    }

    pub fn previous(&mut self) {
        if !self.containers.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.containers.len() - 1;
            }
            self.set_loading();
        }
    }

    fn set_loading(&mut self) {
        self.current_stats = None;
        self.previous_stats = None;
        self.current_inspection = None;
        self.logs.clear();
        self.cpu_history.clear();
        self.net_rx_history.clear();
        self.net_tx_history.clear();
        self.x_axis_bounds = [0.0, 100.0];
        self.net_axis_bounds = [0.0, 100.0];
        self.is_loading_details = true;
    }

    pub fn get_selected_container(&self) -> Option<&Container> {
        self.containers.get(self.selected_index)
    }

    pub fn add_log(&mut self, log: String) {
        if self.logs.len() >= 100 {
            self.logs.pop_front();
        }
        self.logs.push_back(log);
    }

    pub fn set_action_status(&mut self, msg: String) {
        self.action_status = Some((msg, std::time::Instant::now()));
    }

    pub fn clear_action_status(&mut self) {
        if let Some((_, time)) = self.action_status {
            if time.elapsed() > std::time::Duration::from_secs(3) {
                self.action_status = None;
            }
        }
    }

    pub fn update_cpu_history(&mut self, cpu_usage: f64) {
        let x = if let Some(last) = self.cpu_history.last() {
            last.0 + 1.0
        } else {
            0.0
        };
        
        self.cpu_history.push((x, cpu_usage));
        
        let limit = self.config.general.graphs_history_size;
        while self.cpu_history.len() > limit {
            self.cpu_history.remove(0);
        }

        if x > limit as f64 {
            self.x_axis_bounds = [x - limit as f64, x];
        } else {
            self.x_axis_bounds = [0.0, limit as f64];
        }
    }

    pub fn update_net_history(&mut self, rx: f64, tx: f64) {
        let x = if let Some(last) = self.net_rx_history.last() {
            last.0 + 1.0
        } else {
            0.0
        };

        self.net_rx_history.push((x, rx));
        self.net_tx_history.push((x, tx));

        let limit = self.config.general.graphs_history_size;
        while self.net_rx_history.len() > limit {
            self.net_rx_history.remove(0);
            self.net_tx_history.remove(0);
        }
        
        if x > limit as f64 {
            self.net_axis_bounds = [x - limit as f64, x];
        } else {
            self.net_axis_bounds = [0.0, limit as f64];
        }
    }

    pub fn update_fish(&mut self) {
        for fish in &mut self.fishes {
            fish.x += fish.direction * fish.speed;
            if fish.x > 25.0 {
                fish.direction = -1.0;
            } else if fish.x < 0.0 {
                fish.direction = 1.0;
            }
        }
    }

    pub fn update_wizard_spinner(&mut self) {
        if let Some(wizard) = &mut self.wizard {
            if let WizardStep::Processing { spinner_frame, .. } = &mut wizard.step {
                *spinner_frame = (*spinner_frame + 1) % 4;
            }
        }
    }

    pub fn toggle_wizard(&mut self) {
        if self.wizard.is_some() {
            self.wizard = None;
        } else {
            self.wizard = Some(WizardState {
                step: WizardStep::ModeSelection { selected_index: 0 },
            });
        }
    }

    fn load_directory_tree(root: &std::path::Path, expanded_paths: &std::collections::HashSet<std::path::PathBuf>) -> Vec<TreeItem> {
        let mut items = Vec::new();
        Self::build_tree_recursive(root, 0, expanded_paths, &mut items);
        // Remove the root itself from the list if we only want to show contents, 
        // OR keep it. Usually file pickers show contents. 
        // But for a tree view, showing the root as top level is nice.
        // Let's actually just show contents of the current_path to start with.
        // Wait, the user wants a tree.
        // Let's make the list start with the contents of `root`.
        items
    }

    fn build_tree_recursive(path: &std::path::Path, depth: usize, expanded_paths: &std::collections::HashSet<std::path::PathBuf>, result: &mut Vec<TreeItem>) {
        if let Ok(entries) = fs::read_dir(path) {
            let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| {
                let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                (!is_dir, e.file_name()) // Dirs first
            });

            let count = entries.len();
            for (i, entry) in entries.iter().enumerate() {
                let path = entry.path();
                let is_dir = path.is_dir();
                let is_last = i == count - 1;
                
                let expanded = expanded_paths.contains(&path);
                
                result.push(TreeItem {
                    path: path.clone(),
                    depth,
                    is_dir,
                    expanded,
                    is_last,
                });

                if is_dir && expanded {
                    Self::build_tree_recursive(&path, depth + 1, expanded_paths, result);
                }
            }
        }
    }

    fn toggle_tree_expand(items: &Vec<TreeItem>, index: usize, expanded_paths: &mut std::collections::HashSet<std::path::PathBuf>) -> bool {
        if let Some(item) = items.get(index) {
            if item.is_dir {
                if expanded_paths.contains(&item.path) {
                    expanded_paths.remove(&item.path);
                } else {
                    expanded_paths.insert(item.path.clone());
                }
                return true;
            }
        }
        false
    }

    pub fn detect_framework(path: &std::path::Path) -> (Framework, String) {
        if let Ok(content) = fs::read_to_string(path.join("composer.json")) {
            if content.contains("laravel/framework") {
                let version = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    v["require"]["php"].as_str()
                        .map(|s| s.chars().skip_while(|c| !c.is_numeric()).take_while(|c| c.is_numeric() || *c == '.').collect::<String>())
                        .unwrap_or("8.2".to_string())
                } else { "8.2".to_string() };
                // If empty (e.g. *), fallback
                let version = if version.is_empty() { "8.2".to_string() } else { version };
                return (Framework::Laravel, version);
            }
        }
        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            let json: serde_json::Value = serde_json::from_str(&content).unwrap_or(serde_json::Value::Null);
            let node_version = json["engines"]["node"].as_str()
                .map(|s| s.chars().skip_while(|c| !c.is_numeric()).take_while(|c| c.is_numeric()).collect::<String>())
                .unwrap_or("18".to_string());
             let node_version = if node_version.is_empty() { "18".to_string() } else { node_version };

            if content.contains("\"next\"") {
                return (Framework::NextJs, node_version);
            }
            if content.contains("\"nuxt\"") {
                return (Framework::NuxtJs, node_version);
            }
        }
        if path.join("go.mod").exists() {
            // Parse go version?
             if let Ok(content) = fs::read_to_string(path.join("go.mod")) {
                 for line in content.lines() {
                     if line.starts_with("go ") {
                         let v = line.trim_start_matches("go ").trim().to_string();
                         return (Framework::Go, v);
                     }
                 }
             }
            return (Framework::Go, "1.21".to_string());
        }
        if let Ok(content) = fs::read_to_string(path.join("requirements.txt")) {
            if content.contains("django") {
                return (Framework::Django, "3.11".to_string()); // Python version default
            }
        }
        if let Ok(content) = fs::read_to_string(path.join("Gemfile")) {
            if content.contains("rails") {
                return (Framework::Rails, "3.2".to_string()); // Ruby version default
            }
        }
        if path.join("Cargo.toml").exists() {
            return (Framework::Rust, "latest".to_string());
        }
        
        (Framework::Manual, "latest".to_string())
    }

    pub fn check_port(port_input: &str) -> PortStatus {
        if port_input.is_empty() { return PortStatus::None; }
        
        let port_part = if let Some(idx) = port_input.find(':') {
            &port_input[..idx]
        } else {
            port_input
        };

        if let Ok(port) = port_part.parse::<u16>() {
            match std::net::TcpListener::bind(("0.0.0.0", port)) {
                Ok(_) => PortStatus::Available,
                Err(_) => {
                    // Port is taken. Try to find who has it.
                    // Try lsof first
                    let output = std::process::Command::new("lsof")
                        .arg("-i")
                        .arg(&format!(":{}", port))
                        .arg("-t") // Terse mode, just PIDs
                        .output();
                    
                    if let Ok(o) = output {
                        if !o.stdout.is_empty() {
                            let pid_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            // If multiple lines, take first
                            let pid_str = pid_str.lines().next().unwrap_or("");
                            if let Ok(pid) = pid_str.parse::<i32>() {
                                // We can use sysinfo here if we had access to self.system, but this is a static/helper method?
                                // Actually we can just return the PID and let UI resolve it or just return PID.
                                // Or we can instantiate a temporary System to look it up, but that's heavy.
                                // Let's just return "PID: <pid>"
                                // Better: run `ps -p <pid> -o comm=`
                                let ps_out = std::process::Command::new("ps")
                                    .arg("-p")
                                    .arg(pid_str)
                                    .arg("-o")
                                    .arg("comm=")
                                    .output();
                                if let Ok(ps_o) = ps_out {
                                    let name = String::from_utf8_lossy(&ps_o.stdout).trim().to_string();
                                    return PortStatus::Occupied(format!("{} (PID: {})", name, pid));
                                }
                                return PortStatus::Occupied(format!("PID: {}", pid));
                            }
                        }
                    }
                    PortStatus::Occupied("Unknown Process".to_string())
                }
            }
        } else {
            PortStatus::Invalid
        }
    }

    // For Scaffolding (Creating new project from scratch)
    fn generate_new_compose_file(path: &std::path::Path, services: &[String], cpu: &str, mem: &str) -> std::io::Result<()> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        let mut content = String::from("version: '3.8'\nservices:\n  app:\n    build: .\n    ports:\n      - \"80:80\"\n    restart: always\n");
        
        // Add resource limits to app
        if !cpu.is_empty() || !mem.is_empty() {
            content.push_str("    deploy:\n      resources:\n        limits:\n");
            if !cpu.is_empty() {
                content.push_str(&format!("          cpus: '{}'\n", cpu));
            }
            if !mem.is_empty() {
                content.push_str(&format!("          memory: {}\n", mem));
            }
        }

        for svc in services {
            match svc.as_str() {
                "MySQL" => {
                    content.push_str("\n  mysql:\n    image: mysql:8.0\n    environment:\n      MYSQL_ROOT_PASSWORD: root\n      MYSQL_DATABASE: app_db\n    ports:\n      - \"3306:3306\"\n");
                },
                "PostgreSQL" => {
                    content.push_str("\n  postgres:\n    image: postgres:15\n    environment:\n      POSTGRES_USER: user\n      POSTGRES_PASSWORD: password\n      POSTGRES_DB: app_db\n    ports:\n      - \"5432:5432\"\n");
                },
                "Redis" => {
                    content.push_str("\n  redis:\n    image: redis:alpine\n    ports:\n      - \"6379:6379\"\n");
                    if !cpu.is_empty() {
                         content.push_str("    deploy:\n      resources:\n        limits:\n          cpus: '0.5'\n          memory: 512M\n");
                    }
                },
                "Nginx" => {
                    content.push_str("\n  nginx:\n    image: nginx:latest\n    ports:\n      - \"8080:80\"\n    depends_on:\n      - app\n");
                },
                _ => {}
            }
        }

        std::fs::write(path.join("docker-compose.yml"), content)
    }

    // For Existing Projects (The Merge Strategy)
    fn generate_override_file(path: &std::path::Path, services: &[String], cpu: &str, mem: &str) -> std::io::Result<std::path::PathBuf> {
        // Create a minimal struct for override
        // We construct YAML manually string for simplicity and control
        let mut content = String::from("version: '3.8'\nservices:\n");
        
        for svc in services {
            content.push_str(&format!("  {}:\n", svc));
            content.push_str("    deploy:\n      resources:\n        limits:\n");
            
            if !cpu.is_empty() {
                content.push_str(&format!("          cpus: '{}'\n", cpu));
            }
            if !mem.is_empty() {
                content.push_str(&format!("          memory: {}\n", mem));
            }
        }
        
        let override_path = path.parent().unwrap_or(path).join(".docktop-override.yml");
        std::fs::write(&override_path, content)?;
        Ok(override_path)
    }

    fn detect_resources() -> (usize, u64) {
        use sysinfo::System;
        let mut sys = System::new_all();
        sys.refresh_all();
        (sys.cpus().len(), sys.total_memory())
    }

    fn calculate_auto_resources(total_mem: u64, total_cpus: usize) -> (String, String) {
        // 20% overhead
        let available_mem = (total_mem as f64 * 0.8) as u64;
        let app_mem = (available_mem as f64 * 0.4) as u64;
        
        // Convert to human readable
        let mem_str = if app_mem > 1024 * 1024 * 1024 {
            format!("{}G", app_mem / (1024 * 1024 * 1024))
        } else {
            format!("{}M", app_mem / (1024 * 1024))
        };

        let cpu_str = format!("{:.1}", (total_cpus as f64 * 0.25).max(0.5)); // Give 25% of cores or at least 0.5

        (cpu_str, mem_str)
    }

    fn write_dockerfile(path: &std::path::Path, framework: &Framework, version: &str, port: &str) -> std::io::Result<()> {
        let content = match framework {
            Framework::Laravel => format!(r#"# Generated by DockTop for Laravel (PHP {})
FROM php:{}-fpm

RUN apt-get update && apt-get install -y git curl libpng-dev libonig-dev libxml2-dev zip unzip
RUN docker-php-ext-install pdo_mysql mbstring exif pcntl bcmath gd
COPY --from=composer:latest /usr/bin/composer /usr/bin/composer

WORKDIR /var/www
COPY . .
RUN composer install

CMD php artisan serve --host=0.0.0.0 --port={}
EXPOSE {}
"#, version, version, port, port),
            Framework::NextJs => format!(r#"# Generated by DockTop for Next.js (Node {})
FROM node:{}-alpine AS base

FROM base AS deps
WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci

FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npm run build

FROM base AS runner
WORKDIR /app
ENV NODE_ENV production
COPY --from=builder /app/public ./public
COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static

EXPOSE {}
CMD ["node", "server.js"]
"#, version, version, port),
            Framework::Go => format!(r#"# Generated by DockTop for Go (Go {})
FROM golang:{}-alpine

WORKDIR /app
COPY go.mod ./
COPY go.sum ./
RUN go mod download

COPY . .
RUN go build -o /main

EXPOSE {}
CMD ["/main"]
"#, version, version, port),
            Framework::Rust => format!(r#"# Generated by DockTop for Rust
FROM rust:{}-alpine as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

FROM alpine:latest
COPY --from=builder /usr/local/cargo/bin/app /usr/local/bin/app
EXPOSE {}
CMD ["app"]
"#, version, port),
            _ => format!("FROM alpine\nWORKDIR /app\nCOPY . .\nEXPOSE {}\nCMD [\"/app/main\"]", port),
        };
        
        fs::write(path.join("Dockerfile"), content)?;
        Ok(())
    }

    pub fn wizard_handle_key(&mut self, key_event: crossterm::event::KeyEvent) -> Option<WizardAction> {
        let key = key_event.code;
        let modifiers = key_event.modifiers;
        let mut next_step = None;
        let mut action_msg = None;
        let mut wizard_action = None;

        if let Some(wizard) = &mut self.wizard {
            match &mut wizard.step {
                WizardStep::ModeSelection { selected_index } => {
                    match key {
                        KeyCode::Up => if *selected_index > 0 { *selected_index -= 1 } else { *selected_index = 4 },
                        KeyCode::Down => *selected_index = (*selected_index + 1) % 5,
                        KeyCode::Enter => {
                            if *selected_index == 0 {
                                next_step = Some(WizardStep::QuickRunInput {
                                    image: String::new(),
                                    name: String::new(),
                                    ports: String::new(),
                                    env: String::new(),
                                    cpu: String::new(),
                                    memory: String::new(),
                                    restart: "no".to_string(),
                                    show_advanced: false,
                                    focused_field: 0,
                                    editing_id: None,
                                    port_status: PortStatus::None,
                                });
                            } else if *selected_index == 3 {
                                let mut state = ListState::default();
                                state.select(Some(0));
                                next_step = Some(WizardStep::Janitor {
                                    items: Vec::new(),
                                    list_state: state,
                                    loading: true,
                                });
                                wizard_action = Some(WizardAction::ScanJanitor);
                            } else if *selected_index == 4 {
                                next_step = Some(WizardStep::Settings {
                                    focused_field: 0,
                                    temp_config: self.config.clone(),
                                });
                            } else {
                                let current_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                                let expanded_paths = std::collections::HashSet::new(); // Start with nothing expanded
                                let items = Self::load_directory_tree(&current_path, &expanded_paths);
                                
                                let mut state = ListState::default();
                                state.select(Some(0));
                                
                                next_step = Some(WizardStep::FileBrowser {
                                    current_path,
                                    list_state: state,
                                    items,
                                    mode: if *selected_index == 1 { FileBrowserMode::Build } else { FileBrowserMode::Compose },
                                });
                            }
                        }
                        KeyCode::Esc => {
                            // Do nothing or go back
                        }
                        _ => {}
                    }
                }
                WizardStep::QuickRunInput { image, name, ports, env, cpu, memory, restart, show_advanced, focused_field, editing_id, port_status } => {
                    match key {
                        KeyCode::Down | KeyCode::Tab => {
                            let max_fields = if *show_advanced { 7 } else { 4 }; // 0-3 are basic, 4-6 are advanced
                            *focused_field = (*focused_field + 1) % max_fields;
                        }
                        KeyCode::Up | KeyCode::BackTab => {
                            let max_fields = if *show_advanced { 7 } else { 4 };
                            if *focused_field > 0 {
                                *focused_field -= 1;
                            } else {
                                *focused_field = max_fields - 1;
                            }
                        }
                        KeyCode::Char(' ') if *focused_field == 6 => {
                             // Cycle restart policies
                             *restart = match restart.as_str() {
                                 "no" => "always".to_string(),
                                 "always" => "unless-stopped".to_string(),
                                 "unless-stopped" => "on-failure".to_string(),
                                 _ => "no".to_string(),
                             };
                        }
                        KeyCode::Char('a') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) || *focused_field > 100 => { // Hacky way to detect advanced toggle? No, let's use a specific key or field.
                             // Actually, let's just use a hotkey 'A' to toggle advanced?
                             // Or maybe just 'Tab' eventually reaches a "Show Advanced" button?
                             // Let's make 'Ctrl+a' toggle advanced
                             *show_advanced = !*show_advanced;
                             if !*show_advanced && *focused_field >= 4 {
                                 *focused_field = 0;
                             }
                        }
                        KeyCode::Char(c) => {
                            let target = match *focused_field {
                                0 => image,
                                1 => name,
                                2 => ports,
                                3 => env,
                                4 => cpu,
                                5 => memory,
                                // 6 is restart, handled by Space
                                _ => image,
                            };
                            if *focused_field != 6 {
                                target.push(c);
                            }
                            if *focused_field == 2 {
                                *port_status = Self::check_port(ports);
                            }
                        }
                        KeyCode::Backspace => {
                            let target = match *focused_field {
                                0 => image,
                                1 => name,
                                2 => ports,
                                3 => env,
                                4 => cpu,
                                5 => memory,
                                // 6 is restart
                                _ => image,
                            };
                            if *focused_field != 6 {
                                target.pop();
                            }
                            if *focused_field == 2 {
                                *port_status = Self::check_port(ports);
                            }
                        }
                        KeyCode::Enter => {
                            next_step = Some(WizardStep::Processing {
                                message: if editing_id.is_some() { "Replacing container..." } else { "Creating container..." }.to_string(),
                                spinner_frame: 0,
                            });
                            if let Some(old_id) = editing_id {
                                action_msg = Some(format!("Replacing container {}", old_id));
                                wizard_action = Some(WizardAction::Replace {
                                    old_id: old_id.clone(),
                                    image: image.clone(),
                                    name: name.clone(),
                                    ports: ports.clone(),
                                    env: env.clone(),
                                    cpu: cpu.clone(),
                                    memory: memory.clone(),
                                    restart: restart.clone(),
                                });
                            } else {
                                action_msg = Some(format!("Creating container from {}", image));
                                wizard_action = Some(WizardAction::Create {
                                    image: image.clone(),
                                    name: name.clone(),
                                    ports: ports.clone(),
                                    env: env.clone(),
                                    cpu: cpu.clone(),
                                    memory: memory.clone(),
                                    restart: restart.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                WizardStep::FileBrowser { current_path, list_state, items, mode } => {
                    match key {
                        KeyCode::Up => {
                            let i = match list_state.selected() {
                                Some(i) => if i == 0 { 0 } else { i - 1 },
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                        KeyCode::Down => {
                            let i = match list_state.selected() {
                                Some(i) => if i >= items.len() - 1 { items.len() - 1 } else { i + 1 },
                                None => 0,
                            };
                            list_state.select(Some(i));
                        }
                        KeyCode::Char(' ') => {
                            // Select logic for Space
                            if let Some(selected_index) = list_state.selected() {
                                let item = &items[selected_index];
                                let path = item.path.clone();
                                
                                if *mode == FileBrowserMode::Build {
                                    let (framework, version) = Self::detect_framework(&path); // Pass path directly
                                    next_step = Some(WizardStep::DockerfileGenerator {
                                        path: path.clone(),
                                        detected_framework: framework.clone(),
                                        detected_version: version,
                                        manual_selection_open: false,
                                        manual_selected_index: 0,
                                        port: framework.default_port().to_string(),
                                        editing_port: false,
                                        focused_option: 0,
                                        port_status: PortStatus::None,
                                    });
                                } else if *mode == FileBrowserMode::Compose {
                                    // Logic for Compose selection...
                                    // For now, let's keep it simple: Space selects the file/folder
                                    // But wait, Space in tree view usually doesn't enter.
                                    // Let's make ENTER toggle folders or select files.
                                    // And SPACE can be "Quick Action" like before?
                                    // The user asked for "Select Project".
                                    
                                    // Let's stick to:
                                    // ENTER on Dir: Toggle Expand/Collapse
                                    // ENTER on File: Select it
                                    // SPACE: (Maybe same as Enter for file?)
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if !items.is_empty() {
                                if let Some(selected_index) = list_state.selected() {
                                    let item = &items[selected_index];
                                    
                                    if item.is_dir {
                                        // Reconstruct expanded_paths from current items
                                        let mut expanded_paths: std::collections::HashSet<std::path::PathBuf> = items.iter()
                                            .filter(|i| i.expanded)
                                            .map(|i| i.path.clone())
                                            .collect();
                                            
                                        if Self::toggle_tree_expand(&items, selected_index, &mut expanded_paths) {
                                            // Rebuild tree
                                            *items = Self::load_directory_tree(current_path, &expanded_paths);
                                            
                                            // Try to keep selection valid
                                            let new_len = items.len();
                                            if selected_index >= new_len {
                                                list_state.select(Some(new_len.saturating_sub(1)));
                                            }
                                        }
                                    } else {
                                        // File selected
                                        let selected_path = &item.path;
                                        if let Some(name) = selected_path.file_name() {
                                            let name_str = name.to_string_lossy();
                                            if *mode == FileBrowserMode::Build && name_str == "Dockerfile" {
                                                next_step = Some(WizardStep::BuildConf {
                                                    tag: "my-app:latest".to_string(),
                                                    mount_volume: false,
                                                    focused_field: 0,
                                                    path: current_path.clone(), // Use current_path (root) or selected_path? Usually root context.
                                                });
                                            } else if *mode == FileBrowserMode::Compose && (name_str == "docker-compose.yml" || name_str == "docker-compose.yaml") {
                                                 // PARSE YAML FIRST
                                                 if let Ok(content) = std::fs::read_to_string(selected_path) {
                                                     if let Ok(compose) = serde_yaml::from_str::<ComposeFile>(&content) {
                                                         let mut services: Vec<String> = compose.services.keys().cloned().collect();
                                                         services.sort();
                                                         
                                                         next_step = Some(WizardStep::ComposeServiceSelection {
                                                             path: selected_path.clone(),
                                                             selected_services: services.clone(), // Select all by default
                                                             focused_index: 0,
                                                             all_services: services, // Need to store all available to know what to render
                                                         });
                                                     } else {
                                                         // Parsing failed, maybe show error? For now fallback to old behavior or error state
                                                         next_step = Some(WizardStep::Error(format!("Failed to parse {}", name_str)));
                                                     }
                                                 } else {
                                                     next_step = Some(WizardStep::Error(format!("Failed to read {}", name_str)));
                                                 }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if let Some(parent) = current_path.parent() {
                                *current_path = parent.to_path_buf();
                                // Reset expanded state when going up? Or keep it?
                                // Reset is cleaner.
                                let expanded_paths = std::collections::HashSet::new();
                                *items = Self::load_directory_tree(current_path, &expanded_paths);
                                list_state.select(Some(0));
                            }
                        }
                        _ => {}
                    }
                }
                WizardStep::DockerfileGenerator { path, detected_framework, detected_version, manual_selection_open, manual_selected_index, port, editing_port, focused_option, port_status } => {
                     if *manual_selection_open {
                         match key {
                             KeyCode::Up => if *manual_selected_index > 0 { *manual_selected_index -= 1 },
                             KeyCode::Down => if *manual_selected_index < 7 { *manual_selected_index += 1 },
                             KeyCode::Enter => {
                                 let frameworks = [Framework::Laravel, Framework::NextJs, Framework::NuxtJs, Framework::Go, Framework::Django, Framework::Rails, Framework::Rust, Framework::Manual];
                                 *detected_framework = frameworks[*manual_selected_index].clone();
                                 *port = detected_framework.default_port().to_string();
                                 *manual_selection_open = false;
                             }
                             KeyCode::Esc => *manual_selection_open = false,
                             _ => {}
                         }
                     } else if *editing_port {
                          match key {
                              KeyCode::Char(c) => {
                                  port.push(c);
                                  *port_status = Self::check_port(port);
                              },
                              KeyCode::Backspace => { 
                                  port.pop(); 
                                  *port_status = Self::check_port(port);
                              },
                             KeyCode::Enter | KeyCode::Esc => *editing_port = false,
                             _ => {}
                         }
                     } else {
                         match key {
                             KeyCode::Up => *focused_option = (*focused_option + 3) % 4,
                             KeyCode::Down => *focused_option = (*focused_option + 1) % 4,
                             KeyCode::Enter => {
                                 match focused_option {
                                     0 => *manual_selection_open = true,
                                     1 => {
                                         *editing_port = true;
                                         port.clear();
                                     },
                                     2 => {
                                         if path.join("Dockerfile").exists() {
                                             next_step = Some(WizardStep::OverwriteConfirm {
                                                 path: path.clone(),
                                                 detected_framework: detected_framework.clone(),
                                                 detected_version: detected_version.clone(),
                                                 port: port.clone(),
                                             });
                                         } else {
                                             if let Ok(_) = Self::write_dockerfile(path, detected_framework, detected_version, port) {
                                                 next_step = Some(WizardStep::BuildConf {
                                                     tag: "my-app:latest".to_string(),
                                                     mount_volume: false,
                                                     focused_field: 0,
                                                     path: path.clone(),
                                                 });
                                             } else {
                                                 next_step = Some(WizardStep::Error("Failed to write Dockerfile".to_string()));
                                             }
                                         }
                                     },
                                     3 => {
                                         next_step = Some(WizardStep::BuildConf {
                                             tag: "my-app:latest".to_string(),
                                             mount_volume: false,
                                             focused_field: 0,
                                             path: path.clone(),
                                         });
                                     },
                                     _ => {}
                                 }
                             }
                             KeyCode::Char('y') => {
                                 if path.join("Dockerfile").exists() {
                                     next_step = Some(WizardStep::OverwriteConfirm {
                                         path: path.clone(),
                                         detected_framework: detected_framework.clone(),
                                         detected_version: detected_version.clone(),
                                         port: port.clone(),
                                     });
                                 } else {
                                     if let Ok(_) = Self::write_dockerfile(path, detected_framework, detected_version, port) {
                                         next_step = Some(WizardStep::BuildConf {
                                             tag: "my-app:latest".to_string(),
                                             mount_volume: false,
                                             focused_field: 0,
                                             path: path.clone(),
                                         });
                                     } else {
                                         next_step = Some(WizardStep::Error("Failed to write Dockerfile".to_string()));
                                     }
                                 }
                             }
                             KeyCode::Char('n') => {
                                 next_step = Some(WizardStep::BuildConf {
                                     tag: "my-app:latest".to_string(),
                                     mount_volume: false,
                                     focused_field: 0,
                                     path: path.clone(),
                                 });
                             }
                             KeyCode::Char('m') => {
                                 *manual_selection_open = true;
                                 *focused_option = 0;
                             },
                             KeyCode::Char('p') => {
                                 *editing_port = true;
                                 port.clear();
                                 *focused_option = 1;
                             },
                             _ => {}
                         }
                     }
                }
                WizardStep::OverwriteConfirm { path, detected_framework, detected_version, port } => {
                    match key {
                        KeyCode::Enter | KeyCode::Char('y') => {
                             // Backup
                             let _ = std::fs::rename(path.join("Dockerfile"), path.join("Dockerfile.bak"));
                             if let Ok(_) = Self::write_dockerfile(path, detected_framework, detected_version, port) {
                                  next_step = Some(WizardStep::BuildConf {
                                      tag: "my-app:latest".to_string(),
                                      mount_volume: false,
                                      focused_field: 0,
                                      path: path.clone(),
                                  });
                             } else {
                                  next_step = Some(WizardStep::Error("Failed to write Dockerfile".to_string()));
                             }
                        },
                        KeyCode::Esc | KeyCode::Char('n') => {
                             next_step = Some(WizardStep::DockerfileGenerator {
                                 path: path.clone(),
                                 detected_framework: detected_framework.clone(),
                                 detected_version: detected_version.clone(),
                                 manual_selection_open: false,
                                 manual_selected_index: 0,
                                 port: port.clone(),
                                 editing_port: false,
                                 focused_option: 0,
                                 port_status: PortStatus::None,
                             });
                        },
                        _ => {}
                    }
                }
                WizardStep::Settings { focused_field, temp_config } => {
                match key {
                    KeyCode::Up => if *focused_field > 0 { *focused_field -= 1 } else { *focused_field = 3 },
                    KeyCode::Down => *focused_field = (*focused_field + 1) % 4,
                    KeyCode::Left | KeyCode::Right => {
                        if *focused_field == 0 {
                            let themes = vec!["monochrome", "dracula", "gruvbox", "cyberpunk"];
                            let current_idx = themes.iter().position(|&t| t == temp_config.general.theme).unwrap_or(0);
                            let next_idx = if key == KeyCode::Right {
                                (current_idx + 1) % themes.len()
                            } else {
                                if current_idx == 0 { themes.len() - 1 } else { current_idx - 1 }
                            };
                            temp_config.general.theme = themes[next_idx].to_string();
                            temp_config.theme_data = crate::config::load_theme(&temp_config.general.theme);
                            self.config.theme_data = temp_config.theme_data.clone();
                        } else if *focused_field == 2 { // Refresh rate
                             let rates = [250, 500, 1000, 2000, 5000];
                             let current = temp_config.general.refresh_rate_ms;
                             let idx = rates.iter().position(|&r| r == current).unwrap_or(2);
                             let next_idx = if key == KeyCode::Right { (idx + 1) % rates.len() } else { if idx == 0 { rates.len() - 1 } else { idx - 1 } };
                             temp_config.general.refresh_rate_ms = rates[next_idx];
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                         match *focused_field {
                             1 => temp_config.general.show_braille = !temp_config.general.show_braille,
                             3 => temp_config.general.confirm_on_delete = !temp_config.general.confirm_on_delete,
                             _ => {}
                         }
                    }
                    KeyCode::Char('s') => { // Save
                        self.config = temp_config.clone();
                        self.config.theme_data = crate::config::load_theme(&self.config.general.theme);
                        self.config.save();
                        wizard_action = Some(WizardAction::Close);
                    }
                    KeyCode::Char('r') => { // Reset
                         *temp_config = Config::load();
                         self.config.theme_data = temp_config.theme_data.clone();
                    }
                    KeyCode::Esc => { // Cancel
                        self.config = Config::load(); // Revert any temporary theme changes
                        wizard_action = Some(WizardAction::Close);
                    }
                    _ => {}
                }
            }
                WizardStep::Janitor { items, list_state, loading } => {
                    if !*loading {
                        match key {
                            KeyCode::Up => {
                                let i = match list_state.selected() {
                                    Some(i) => if i == 0 { 0 } else { i - 1 },
                                    None => 0,
                                };
                                list_state.select(Some(i));
                            }
                            KeyCode::Down => {
                                let i = match list_state.selected() {
                                    Some(i) => if i >= items.len() - 1 { items.len() - 1 } else { i + 1 },
                                    None => 0,
                                };
                                list_state.select(Some(i));
                            }
                            KeyCode::Char(' ') => {
                                if let Some(i) = list_state.selected() {
                                    if let Some(item) = items.get_mut(i) {
                                        item.selected = !item.selected;
                                    }
                                }
                            },
                            KeyCode::Enter => {
                                let to_clean: Vec<JanitorItem> = items.iter().filter(|i| i.selected).cloned().collect();
                                if !to_clean.is_empty() {
                                    next_step = Some(WizardStep::Processing {
                                        message: "Cleaning up...".to_string(),
                                        spinner_frame: 0,
                                    });
                                    action_msg = Some("Running Janitor cleanup...".to_string());
                                    wizard_action = Some(WizardAction::CleanJanitor(to_clean));
                                }
                            },
                            _ => {}
                        }
                    }
                }
                WizardStep::ComposeGenerator { path } => {
                    match key {
                        KeyCode::Char('g') | KeyCode::Enter => {
                            next_step = Some(WizardStep::ComposeServiceSelection {
                                path: path.clone(),
                                selected_services: vec![],
                                focused_index: 0,
                                all_services: vec!["MySQL".to_string(), "PostgreSQL".to_string(), "Redis".to_string(), "Nginx".to_string()],
                            });
                        }
                        KeyCode::Char('c') | KeyCode::Esc => {
                             let mut state = ListState::default();
                             state.select(Some(0));
                             let expanded_paths = std::collections::HashSet::new();
                             next_step = Some(WizardStep::FileBrowser {
                                current_path: path.clone(),
                                list_state: state,
                                items: Self::load_directory_tree(path, &expanded_paths),
                                mode: FileBrowserMode::Compose,
                            });
                        }
                        _ => {}
                    }
                }
                WizardStep::ComposeServiceSelection { path, selected_services, focused_index, all_services } => {
                    match key {
                        KeyCode::Up => if *focused_index > 0 { *focused_index -= 1 },
                        KeyCode::Down => if *focused_index < all_services.len() { *focused_index += 1 },
                        KeyCode::Char(' ') => {
                            if *focused_index < all_services.len() {
                                let svc = all_services[*focused_index].clone();
                                if let Some(pos) = selected_services.iter().position(|x| *x == svc) {
                                    selected_services.remove(pos);
                                } else {
                                    selected_services.push(svc);
                                }
                            }
                        }
                        KeyCode::Enter => {
                            let (cpu, mem) = Self::detect_resources();
                            next_step = Some(WizardStep::ResourceAllocation {
                                path: path.clone(),
                                services: selected_services.clone(),
                                all_services: all_services.clone(),
                                cpu_limit: String::new(),
                                mem_limit: String::new(),
                                focused_field: 0,
                                detected_cpu: cpu,
                                detected_mem: mem,
                            });
                        }
                        KeyCode::Esc => {
                            next_step = Some(WizardStep::ComposeGenerator { path: path.clone() });
                        }
                        _ => {}
                    }
                }
                WizardStep::ResourceAllocation { path, services, all_services, cpu_limit, mem_limit, focused_field, detected_cpu, detected_mem } => {
                     match key {
                        KeyCode::Up => if *focused_field > 0 { *focused_field -= 1 },
                        KeyCode::Down | KeyCode::Tab => if *focused_field < 2 { *focused_field += 1 },
                        KeyCode::Char('s') => {
                            let (auto_cpu, auto_mem) = Self::calculate_auto_resources(*detected_mem, *detected_cpu);
                            
                            let res = if path.is_file() {
                                // Existing project: Generate override
                                Self::generate_override_file(path, services, &auto_cpu, &auto_mem).map(Some)
                                    .map_err(|_| "Failed to write override file".to_string())
                            } else {
                                // New project: Generate full file
                                Self::generate_new_compose_file(path, services, &auto_cpu, &auto_mem)
                                    .map(|_| None)
                                    .map_err(|_| "Failed to write docker-compose.yml".to_string())
                            };

                            match res {
                                Ok(override_path) => {
                                    next_step = Some(WizardStep::Processing {
                                        message: "Running Docker Compose...".to_string(),
                                        spinner_frame: 0,
                                    });
                                    action_msg = Some("Running docker compose up".to_string());
                                    wizard_action = Some(WizardAction::ComposeUp {
                                        path: path.clone(),
                                        override_path,
                                    });
                                }
                                Err(msg) => {
                                    next_step = Some(WizardStep::Error(msg));
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if *focused_field == 2 {
                                let res = if path.is_file() {
                                    // Existing project
                                    Self::generate_override_file(path, services, cpu_limit, mem_limit).map(Some)
                                        .map_err(|_| "Failed to write override file".to_string())
                                } else {
                                    // New project
                                    Self::generate_new_compose_file(path, services, cpu_limit, mem_limit)
                                        .map(|_| None)
                                        .map_err(|_| "Failed to write docker-compose.yml".to_string())
                                };

                                match res {
                                    Ok(override_path) => {
                                        next_step = Some(WizardStep::Processing {
                                            message: "Running Docker Compose...".to_string(),
                                            spinner_frame: 0,
                                        });
                                        action_msg = Some("Running docker compose up".to_string());
                                        wizard_action = Some(WizardAction::ComposeUp {
                                            path: path.clone(),
                                            override_path,
                                        });
                                    }
                                    Err(msg) => {
                                        next_step = Some(WizardStep::Error(msg));
                                    }
                                }
                            } else {
                                *focused_field += 1;
                            }
                        }
                        KeyCode::Char(c) => {
                            if *focused_field == 0 {
                                cpu_limit.push(c);
                            } else if *focused_field == 1 {
                                mem_limit.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if *focused_field == 0 {
                                cpu_limit.pop();
                            } else if *focused_field == 1 {
                                mem_limit.pop();
                            }
                        }
                        KeyCode::Esc => {
                             next_step = Some(WizardStep::ComposeServiceSelection {
                                path: path.clone(),
                                selected_services: services.clone(),
                                focused_index: 0,
                                all_services: all_services.clone(),
                            });
                        }
                        _ => {}
                     }
                }
                WizardStep::BuildConf { tag, mount_volume, focused_field, path } => {
                    match key {
                        KeyCode::Down | KeyCode::Tab => {
                            *focused_field = (*focused_field + 1) % 2;
                        }
                        KeyCode::Up | KeyCode::BackTab => {
                            if *focused_field > 0 {
                                *focused_field -= 1;
                            } else {
                                *focused_field = 1;
                            }
                        }
                        KeyCode::Char(c) => {
                            if *focused_field == 0 {
                                tag.push(c);
                            } else if *focused_field == 1 && c == ' ' {
                                *mount_volume = !*mount_volume;
                            }
                        }
                        KeyCode::Backspace => {
                            if *focused_field == 0 {
                                tag.pop();
                            }
                        }
                        KeyCode::Enter => {
                            if !tag.is_empty() {
                                next_step = Some(WizardStep::Processing {
                                    message: format!("Building {}...", tag),
                                    spinner_frame: 0,
                                });
                                action_msg = Some(format!("Building image {}", tag));
                                wizard_action = Some(WizardAction::Build {
                                    tag: tag.clone(),
                                    path: path.clone(),
                                    mount: *mount_volume,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            
            if let Some(step) = next_step {
                wizard.step = step;
            }
        }
        
        if let Some(msg) = action_msg {
            self.set_action_status(msg);
        }
        
        wizard_action
    }
}
