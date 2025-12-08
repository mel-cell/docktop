use crossterm::event::KeyCode;
use crate::docker::{Container, ContainerStats, ContainerInspection};
use crate::config::Config;
use std::collections::VecDeque;
use std::fs;
use sysinfo::System;
use ratatui::widgets::ListState;
use crate::wizard::models::*;



#[derive(Clone)]
pub struct Fish {
    pub x: f64,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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





    // For Scaffolding (Creating new project from scratch)


    // For Existing Projects (The Merge Strategy)








    pub fn wizard_handle_key(&mut self, key_event: crossterm::event::KeyEvent) -> Option<WizardAction> {
        let key = key_event.code;
        let modifiers = key_event.modifiers;
        let mut next_step = None;
        let mut action_msg = None;
        let mut wizard_action = None;

        if let Some(wizard) = &mut self.wizard {
            if key == KeyCode::Char('w') {
                return Some(WizardAction::Close);
            }
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
                                    port_status: crate::wizard::models::PortStatus::None,
                                    profile: crate::wizard::models::ResourceProfile::Custom,
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
                WizardStep::QuickRunInput { image, name, ports, env, cpu, memory, restart, show_advanced, focused_field, editing_id, port_status, profile } => {
                    match key {
                        KeyCode::Down | KeyCode::Tab => {
                            let max_fields = if *show_advanced { 8 } else { 4 }; // 0-3 basic, 4-7 advanced
                            *focused_field = (*focused_field + 1) % max_fields;
                        }
                        KeyCode::Up | KeyCode::BackTab => {
                            let max_fields = if *show_advanced { 8 } else { 4 };
                            if *focused_field > 0 {
                                *focused_field -= 1;
                            } else {
                                *focused_field = max_fields - 1;
                            }
                        }
                        KeyCode::Char(' ') if *focused_field == 4 => {
                             // Cycle Profile
                             *profile = match profile {
                                 crate::wizard::models::ResourceProfile::Eco => crate::wizard::models::ResourceProfile::Standard,
                                 crate::wizard::models::ResourceProfile::Standard => crate::wizard::models::ResourceProfile::Performance,
                                 crate::wizard::models::ResourceProfile::Performance => crate::wizard::models::ResourceProfile::Custom,
                                 crate::wizard::models::ResourceProfile::Custom => crate::wizard::models::ResourceProfile::Eco,
                             };
                             let (new_cpu, new_mem) = profile.values();
                             if !new_cpu.is_empty() { *cpu = new_cpu; }
                             if !new_mem.is_empty() { *memory = new_mem; }
                        }
                        KeyCode::Char(' ') if *focused_field == 7 => {
                             // Cycle restart policies
                             *restart = match restart.as_str() {
                                 "no" => "always".to_string(),
                                 "always" => "unless-stopped".to_string(),
                                 "unless-stopped" => "on-failure".to_string(),
                                 _ => "no".to_string(),
                             };
                        }
                        KeyCode::Char('a') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                             *show_advanced = !*show_advanced;
                             if !*show_advanced && *focused_field >= 4 {
                                 *focused_field = 0;
                             }
                        }
                        KeyCode::Char(c) => {
                            if *focused_field == 5 || *focused_field == 6 {
                                *profile = crate::wizard::models::ResourceProfile::Custom;
                            }
                            
                            if *focused_field == 0 { image.push(c); }
                            else if *focused_field == 1 { name.push(c); }
                            else if *focused_field == 2 { ports.push(c); }
                            else if *focused_field == 3 { env.push(c); }
                            else if *focused_field == 5 { cpu.push(c); }
                            else if *focused_field == 6 { memory.push(c); }
                            
                            if *focused_field == 2 {
                                *port_status = crate::wizard::logic::check_port(&ports);
                            }
                        }
                        KeyCode::Backspace => {
                            if *focused_field == 5 || *focused_field == 6 {
                                *profile = crate::wizard::models::ResourceProfile::Custom;
                            }
                            
                            if *focused_field == 0 { image.pop(); }
                            else if *focused_field == 1 { name.pop(); }
                            else if *focused_field == 2 { ports.pop(); }
                            else if *focused_field == 3 { env.pop(); }
                            else if *focused_field == 5 { cpu.pop(); }
                            else if *focused_field == 6 { memory.pop(); }
                            
                            if *focused_field == 2 {
                                *port_status = crate::wizard::logic::check_port(&ports);
                            }
                        }
                        KeyCode::Enter => {
                            let action = if let Some(old_id) = editing_id {
                                WizardAction::Replace {
                                    old_id: old_id.clone(),
                                    image: image.clone(),
                                    name: name.clone(),
                                    ports: ports.clone(),
                                    env: env.clone(),
                                    cpu: cpu.clone(),
                                    memory: memory.clone(),
                                    restart: restart.clone(),
                                }
                            } else {
                                WizardAction::Create {
                                    image: image.clone(),
                                    name: name.clone(),
                                    ports: ports.clone(),
                                    env: env.clone(),
                                    cpu: cpu.clone(),
                                    memory: memory.clone(),
                                    restart: restart.clone(),
                                }
                            };

                            let mut cmd = format!("docker run -d --name {}", name);
                            if !ports.is_empty() { cmd.push_str(&format!(" -p {}", ports)); }
                            if !env.is_empty() { cmd.push_str(&format!(" -e {}", env)); }
                            if !cpu.is_empty() { cmd.push_str(&format!(" --cpus {}", cpu)); }
                            if !memory.is_empty() { cmd.push_str(&format!(" --memory {}", memory)); }
                            if !restart.is_empty() && restart != "no" { cmd.push_str(&format!(" --restart {}", restart)); }
                            cmd.push_str(&format!(" {}", image));

                            let prev = crate::wizard::models::WizardStep::QuickRunInput {
                                image: image.clone(),
                                name: name.clone(),
                                ports: ports.clone(),
                                env: env.clone(),
                                cpu: cpu.clone(),
                                memory: memory.clone(),
                                restart: restart.clone(),
                                show_advanced: *show_advanced,
                                focused_field: *focused_field,
                                editing_id: editing_id.clone(),
                                port_status: port_status.clone(),
                                profile: profile.clone(),
                            };

                            next_step = Some(WizardStep::Preview {
                                title: "Preview Command".to_string(),
                                content: cmd,
                                action,
                                previous_step: Box::new(prev),
                            });
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
                                    let (framework, version) = crate::wizard::logic::detect_framework(&path); // Pass path directly
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
                                  *port_status = crate::wizard::logic::check_port(port);
                              },
                              KeyCode::Backspace => { 
                                  port.pop(); 
                                  *port_status = crate::wizard::logic::check_port(port);
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
                                             if let Ok(_) = crate::wizard::logic::write_dockerfile(path, detected_framework, detected_version, port) {
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
                                     if let Ok(_) = crate::wizard::logic::write_dockerfile(path, detected_framework, detected_version, port) {
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
                             if let Ok(_) = crate::wizard::logic::write_dockerfile(path, detected_framework, detected_version, port) {
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
                    KeyCode::Up => if *focused_field > 0 { *focused_field = 3 } else { *focused_field = 3 },
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
                            let (cpu, mem) = crate::wizard::logic::detect_resources();
                            next_step = Some(WizardStep::ResourceAllocation {
                                path: path.clone(),
                                services: selected_services.clone(),
                                all_services: all_services.clone(),
                                cpu_limit: String::new(),
                                mem_limit: String::new(),
                                focused_field: 0,
                                detected_cpu: cpu,
                                detected_mem: mem,
                                profile: crate::wizard::models::ResourceProfile::Standard,
                            });
                        }
                        KeyCode::Esc => {
                            next_step = Some(WizardStep::ComposeGenerator { path: path.clone() });
                        }
                        _ => {}
                    }
                }
                WizardStep::ResourceAllocation { path, services, all_services, cpu_limit, mem_limit, focused_field, detected_cpu, detected_mem, profile } => {
                     match key {
                        KeyCode::Up => if *focused_field > 0 { *focused_field -= 1 },
                        KeyCode::Down | KeyCode::Tab => if *focused_field < 3 { *focused_field += 1 },
                        KeyCode::Char(' ') if *focused_field == 0 => {
                             // Cycle Profile
                             *profile = match profile {
                                 crate::wizard::models::ResourceProfile::Eco => crate::wizard::models::ResourceProfile::Standard,
                                 crate::wizard::models::ResourceProfile::Standard => crate::wizard::models::ResourceProfile::Performance,
                                 crate::wizard::models::ResourceProfile::Performance => crate::wizard::models::ResourceProfile::Custom,
                                 crate::wizard::models::ResourceProfile::Custom => crate::wizard::models::ResourceProfile::Eco,
                             };
                             let (new_cpu, new_mem) = profile.values();
                             if !new_cpu.is_empty() { *cpu_limit = new_cpu; }
                             if !new_mem.is_empty() { *mem_limit = new_mem; }
                        }
                        KeyCode::Char('s') => {
                            let (auto_cpu, auto_mem) = crate::wizard::logic::calculate_auto_resources(*detected_mem, *detected_cpu);
                            *cpu_limit = auto_cpu;
                            *mem_limit = auto_mem;
                            *profile = crate::wizard::models::ResourceProfile::Custom;
                            
                            let res = if path.is_file() {
                                crate::wizard::logic::generate_override_file(path, services, cpu_limit, mem_limit).map(Some)
                                    .map_err(|_| "Failed to write override file".to_string())
                            } else {
                                crate::wizard::logic::generate_new_compose_file(path, services, cpu_limit, mem_limit)
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
                            if *focused_field == 3 {
                                let (content, override_path) = if path.is_file() {
                                    // Existing project
                                    let content = crate::wizard::logic::generate_override_content(services, cpu_limit, mem_limit);
                                    let p = path.parent().unwrap_or(path).join(".docktop-override.yml");
                                    (content, Some(p))
                                } else {
                                    // New project
                                    let content = crate::wizard::logic::generate_new_compose_content(services, cpu_limit, mem_limit);
                                    (content, None)
                                };

                                let action = WizardAction::ComposeUp {
                                    path: path.clone(),
                                    override_path: override_path.clone(),
                                };
                                
                                let prev = crate::wizard::models::WizardStep::ResourceAllocation {
                                    path: path.clone(),
                                    services: services.clone(),
                                    all_services: all_services.clone(),
                                    cpu_limit: cpu_limit.clone(),
                                    mem_limit: mem_limit.clone(),
                                    focused_field: *focused_field,
                                    detected_cpu: *detected_cpu,
                                    detected_mem: *detected_mem,
                                    profile: profile.clone(),
                                };

                                next_step = Some(WizardStep::Preview {
                                    title: "Preview Docker Compose".to_string(),
                                    content,
                                    action,
                                    previous_step: Box::new(prev),
                                });
                            } else {
                                *focused_field += 1;
                            }
                        }
                        KeyCode::Char(c) => {
                            if *focused_field == 1 || *focused_field == 2 {
                                *profile = crate::wizard::models::ResourceProfile::Custom;
                            }
                            if *focused_field == 1 {
                                cpu_limit.push(c);
                            } else if *focused_field == 2 {
                                mem_limit.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if *focused_field == 1 || *focused_field == 2 {
                                *profile = crate::wizard::models::ResourceProfile::Custom;
                            }
                            if *focused_field == 1 {
                                cpu_limit.pop();
                            } else if *focused_field == 2 {
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
                WizardStep::Preview { title: _, content, action, previous_step } => {
                    match key {
                        KeyCode::Enter => {
                            if let WizardAction::ComposeUp { path, override_path } = &action {
                                 let res = if let Some(p) = override_path {
                                     std::fs::write(p, content).map_err(|e| format!("Failed to write: {}", e))
                                 } else {
                                     std::fs::write(path.join("docker-compose.yml"), content).map_err(|e| format!("Failed to write: {}", e))
                                 };
                                 
                                 if let Err(msg) = res {
                                     next_step = Some(WizardStep::Error(msg));
                                 } else {
                                     next_step = Some(WizardStep::Processing {
                                         message: "Executing...".to_string(),
                                         spinner_frame: 0,
                                     });
                                     action_msg = Some("Executing action...".to_string());
                                     wizard_action = Some(action.clone());
                                 }
                            } else {
                                next_step = Some(WizardStep::Processing {
                                    message: "Executing...".to_string(),
                                    spinner_frame: 0,
                                });
                                action_msg = Some("Executing action...".to_string());
                                wizard_action = Some(action.clone());
                            }
                        }
                        KeyCode::Esc => {
                            next_step = Some(*previous_step.clone());
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
