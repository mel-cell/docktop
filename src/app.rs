use crossterm::event::KeyCode;
use crate::docker::{Container, ContainerStats, ContainerInspection};
use crate::config::Config;
use std::collections::VecDeque;
use std::fs;
use sysinfo::System;

#[derive(Clone)]
pub struct WizardState {
    pub step: WizardStep,
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
        focused_field: usize,
        editing_id: Option<String>,
    },
    FileBrowser {
        current_path: std::path::PathBuf,
        selected_file_index: usize,
        entries: Vec<std::path::PathBuf>,
        mode: FileBrowserMode,
    },
    DockerfileGenerator {
        path: std::path::PathBuf,
        detected_framework: Framework,
        manual_selection_open: bool,
        manual_selected_index: usize,
        port: String,
        editing_port: bool,
        focused_option: usize,
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
    },
    ResourceAllocation {
        path: std::path::PathBuf,
        services: Vec<String>,
        cpu_limit: String,
        mem_limit: String,
        focused_field: usize,
        detected_cpu: usize,
        detected_mem: u64,
    },
    Error(String),
}

#[derive(Clone)]
pub enum WizardAction {
    Create { image: String, name: String, ports: String, env: String, cpu: String, memory: String },
    Build { tag: String, path: std::path::PathBuf, mount: bool },
    ComposeUp { path: std::path::PathBuf },
    Replace { old_id: String, image: String, name: String, ports: String, env: String, cpu: String, memory: String },
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
    pub net_axis_bounds: [f64; 2],
    pub config: Config,
    pub fishes: Vec<Fish>,
    pub wizard: Option<WizardState>,
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
            net_axis_bounds: [0.0, 100.0],
            config: Config::load(),
            fishes,
            globe_frames,
            wizard: None,
            _system: System::new_all(),
        }
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
        
        if self.cpu_history.len() > 100 {
            self.cpu_history.remove(0);
        }

        if x > 100.0 {
            self.x_axis_bounds = [x - 100.0, x];
        } else {
            self.x_axis_bounds = [0.0, 100.0];
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

        if self.net_rx_history.len() > 100 {
            self.net_rx_history.remove(0);
            self.net_tx_history.remove(0);
        }
        

            


        if x > 100.0 {
            self.net_axis_bounds = [x - 100.0, x];
        } else {
            self.net_axis_bounds = [0.0, 100.0];
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

    fn load_directory_entries(path: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut entries = Vec::new();
        // Add parent directory option if not at root
        if let Some(parent) = path.parent() {
            entries.push(parent.to_path_buf());
        }
        
        if let Ok(read_dir) = std::fs::read_dir(path) {
            for entry in read_dir.flatten() {
                entries.push(entry.path());
            }
        }
        entries.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir && !b_is_dir {
                std::cmp::Ordering::Less
            } else if !a_is_dir && b_is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.file_name().cmp(&b.file_name())
            }
        });
        entries
    }

    pub fn detect_framework(path: &std::path::Path) -> Framework {
        if let Ok(content) = fs::read_to_string(path.join("composer.json")) {
            if content.contains("laravel/framework") {
                return Framework::Laravel;
            }
        }
        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            if content.contains("\"next\"") {
                return Framework::NextJs;
            }
            if content.contains("\"nuxt\"") {
                return Framework::NuxtJs;
            }
        }
        if path.join("go.mod").exists() {
            return Framework::Go;
        }
        if let Ok(content) = fs::read_to_string(path.join("requirements.txt")) {
            if content.contains("django") {
                return Framework::Django;
            }
        }
        if let Ok(content) = fs::read_to_string(path.join("Gemfile")) {
            if content.contains("rails") {
                return Framework::Rails;
            }
        }
        if path.join("Cargo.toml").exists() {
            return Framework::Rust;
        }
        
        Framework::Manual
    }

    fn write_advanced_compose_file(path: &std::path::Path, services: &[String], cpu: &str, mem: &str) -> std::io::Result<()> {
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
                    // Auto-limit for Redis if auto-mode was used (heuristic: if app has limits, redis gets smaller ones)
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

    fn write_dockerfile(path: &std::path::Path, framework: &Framework, port: &str) -> std::io::Result<()> {
        let content = match framework {
            Framework::Laravel => format!(r#"# Generated by DockTop for Laravel
FROM php:8.2-fpm

RUN apt-get update && apt-get install -y git curl libpng-dev libonig-dev libxml2-dev zip unzip
RUN docker-php-ext-install pdo_mysql mbstring exif pcntl bcmath gd
COPY --from=composer:latest /usr/bin/composer /usr/bin/composer

WORKDIR /var/www
COPY . .
RUN composer install

CMD php artisan serve --host=0.0.0.0 --port={}
EXPOSE {}
"#, port, port),
            Framework::NextJs => format!(r#"# Generated by DockTop for Next.js
FROM node:18-alpine AS base

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
"#, port),
            Framework::Go => format!(r#"# Generated by DockTop for Go
FROM golang:1.21-alpine

WORKDIR /app
COPY go.mod ./
COPY go.sum ./
RUN go mod download

COPY . .
RUN go build -o /main

EXPOSE {}
CMD ["/main"]
"#, port),
            Framework::Rust => format!(r#"# Generated by DockTop for Rust
FROM rust:1.75-alpine as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

FROM alpine:latest
COPY --from=builder /usr/local/cargo/bin/app /usr/local/bin/app
EXPOSE {}
CMD ["app"]
"#, port),
            _ => format!("FROM alpine\nWORKDIR /app\nCOPY . .\nEXPOSE {}\nCMD [\"/app/main\"]", port),
        };
        
        fs::write(path.join("Dockerfile"), content)?;
        Ok(())
    }

    pub fn wizard_handle_key(&mut self, key: KeyCode) -> Option<WizardAction> {
        let mut next_step = None;
        let mut action_msg = None;
        let mut wizard_action = None;

        if let Some(wizard) = &mut self.wizard {
            match &mut wizard.step {
                WizardStep::ModeSelection { selected_index } => {
                    match key {
                        KeyCode::Up => if *selected_index > 0 { *selected_index -= 1 } else { *selected_index = 2 },
                        KeyCode::Down => *selected_index = (*selected_index + 1) % 3,
                        KeyCode::Enter => {
                            if *selected_index == 0 {
                                next_step = Some(WizardStep::QuickRunInput {
                                    image: String::new(),
                                    name: String::new(),
                                    ports: String::new(),
                                    env: String::new(),
                                    cpu: String::new(),
                                    memory: String::new(),
                                    focused_field: 0,
                                    editing_id: None,
                                });
                            } else {
                                let current_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                                let entries = Self::load_directory_entries(&current_path);
                                let mode = if *selected_index == 1 { FileBrowserMode::Build } else { FileBrowserMode::Compose };
                                next_step = Some(WizardStep::FileBrowser {
                                    current_path,
                                    selected_file_index: 0,
                                    entries,
                                    mode,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                WizardStep::QuickRunInput { image, name, ports, env, cpu, memory, focused_field, editing_id } => {
                    match key {
                        KeyCode::Down | KeyCode::Tab => {
                            *focused_field = (*focused_field + 1) % 6;
                        }
                        KeyCode::Up | KeyCode::BackTab => {
                            if *focused_field > 0 {
                                *focused_field -= 1;
                            } else {
                                *focused_field = 5;
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
                                _ => image,
                            };
                            target.push(c);
                        }
                        KeyCode::Backspace => {
                            let target = match *focused_field {
                                0 => image,
                                1 => name,
                                2 => ports,
                                3 => env,
                                4 => cpu,
                                5 => memory,
                                _ => image,
                            };
                            target.pop();
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
                                });
                            }
                        }
                        _ => {}
                    }
                }
                WizardStep::FileBrowser { current_path, selected_file_index, entries, mode } => {
                    match key {
                        KeyCode::Up => {
                            if *selected_file_index > 0 {
                                *selected_file_index -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if *selected_file_index + 1 < entries.len() {
                                *selected_file_index += 1;
                            }
                        }
                        KeyCode::Char(' ') => {
                            if *mode == FileBrowserMode::Build {
                                let framework = Self::detect_framework(current_path);
                                next_step = Some(WizardStep::DockerfileGenerator {
                                    path: current_path.clone(),
                                    detected_framework: framework.clone(),
                                    manual_selection_open: false,
                                    manual_selected_index: 0,
                                    port: framework.default_port().to_string(),
                                    editing_port: false,
                                    focused_option: 0,
                                });
                            } else if *mode == FileBrowserMode::Compose {
                                let has_compose = current_path.join("docker-compose.yml").exists() || current_path.join("docker-compose.yaml").exists();
                                if has_compose {
                                    next_step = Some(WizardStep::Processing {
                                        message: "Running Docker Compose...".to_string(),
                                        spinner_frame: 0,
                                    });
                                    action_msg = Some("Running docker compose up".to_string());
                                    wizard_action = Some(WizardAction::ComposeUp {
                                        path: current_path.clone(),
                                    });
                                } else {
                                    let target_path = if current_path.ends_with("docker") {
                                        current_path.clone()
                                    } else {
                                        current_path.join("docker")
                                    };
                                    next_step = Some(WizardStep::ComposeGenerator {
                                        path: target_path,
                                    });
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if !entries.is_empty() {
                                let selected_path = &entries[*selected_file_index];
                                if selected_path.is_dir() {
                                    *current_path = selected_path.clone();
                                    *entries = Self::load_directory_entries(current_path);
                                    *selected_file_index = 0;
                                } else {
                                    // File selected
                                    if let Some(name) = selected_path.file_name() {
                                        let name_str = name.to_string_lossy();
                                        if *mode == FileBrowserMode::Build && name_str == "Dockerfile" {
                                            next_step = Some(WizardStep::BuildConf {
                                                tag: "my-app:latest".to_string(),
                                                mount_volume: false,
                                                focused_field: 0,
                                                path: current_path.clone(),
                                            });
                                        } else if *mode == FileBrowserMode::Compose && (name_str == "docker-compose.yml" || name_str == "docker-compose.yaml") {
                                             next_step = Some(WizardStep::Processing {
                                                message: "Running Docker Compose...".to_string(),
                                                spinner_frame: 0,
                                            });
                                            action_msg = Some("Running docker compose up".to_string());
                                            wizard_action = Some(WizardAction::ComposeUp {
                                                path: current_path.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if let Some(parent) = current_path.parent() {
                                *current_path = parent.to_path_buf();
                                *entries = Self::load_directory_entries(current_path);
                                *selected_file_index = 0;
                            }
                        }
                        _ => {}
                    }
                }
                WizardStep::DockerfileGenerator { path, detected_framework, manual_selection_open, manual_selected_index, port, editing_port, focused_option } => {
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
                             KeyCode::Char(c) => port.push(c),
                             KeyCode::Backspace => { port.pop(); },
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
                                         if let Ok(_) = Self::write_dockerfile(path, detected_framework, port) {
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
                                 if let Ok(_) = Self::write_dockerfile(path, detected_framework, port) {
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
                WizardStep::ComposeGenerator { path } => {
                    match key {
                        KeyCode::Char('g') | KeyCode::Enter => {
                            next_step = Some(WizardStep::ComposeServiceSelection {
                                path: path.clone(),
                                selected_services: vec![],
                                focused_index: 0,
                            });
                        }
                        KeyCode::Char('c') | KeyCode::Esc => {
                             next_step = Some(WizardStep::FileBrowser {
                                current_path: path.clone(),
                                selected_file_index: 0,
                                entries: Self::load_directory_entries(path),
                                mode: FileBrowserMode::Compose,
                            });
                        }
                        _ => {}
                    }
                }
                WizardStep::ComposeServiceSelection { path, selected_services, focused_index } => {
                    let services = ["MySQL", "PostgreSQL", "Redis", "Nginx"];
                    match key {
                        KeyCode::Up => if *focused_index > 0 { *focused_index -= 1 },
                        KeyCode::Down => if *focused_index < services.len() { *focused_index += 1 },
                        KeyCode::Char(' ') => {
                            if *focused_index < services.len() {
                                let svc = services[*focused_index].to_string();
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
                WizardStep::ResourceAllocation { path, services, cpu_limit, mem_limit, focused_field, detected_cpu, detected_mem } => {
                     match key {
                        KeyCode::Up => if *focused_field > 0 { *focused_field -= 1 },
                        KeyCode::Down | KeyCode::Tab => if *focused_field < 2 { *focused_field += 1 },
                        KeyCode::Char('s') => {
                            let (auto_cpu, auto_mem) = Self::calculate_auto_resources(*detected_mem, *detected_cpu);
                            if let Ok(_) = Self::write_advanced_compose_file(path, services, &auto_cpu, &auto_mem) {
                                next_step = Some(WizardStep::Processing {
                                    message: "Running Docker Compose...".to_string(),
                                    spinner_frame: 0,
                                });
                                action_msg = Some("Running docker compose up".to_string());
                                wizard_action = Some(WizardAction::ComposeUp {
                                    path: path.clone(),
                                });
                            } else {
                                next_step = Some(WizardStep::Error("Failed to write docker-compose.yml".to_string()));
                            }
                        }
                        KeyCode::Enter => {
                            if *focused_field == 2 {
                                if let Ok(_) = Self::write_advanced_compose_file(path, services, cpu_limit, mem_limit) {
                                    next_step = Some(WizardStep::Processing {
                                        message: "Running Docker Compose...".to_string(),
                                        spinner_frame: 0,
                                    });
                                    action_msg = Some("Running docker compose up".to_string());
                                    wizard_action = Some(WizardAction::ComposeUp {
                                        path: path.clone(),
                                    });
                                } else {
                                    next_step = Some(WizardStep::Error("Failed to write docker-compose.yml".to_string()));
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
