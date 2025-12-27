use std::{io, time::Duration};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, watch};
use tokio::io::AsyncReadExt;
// Removed unused imports

mod app;
mod config;
mod docker;
mod ui;
mod theme;
mod action;
pub mod wizard;
mod keys;

use action::Action;

use app::App;
use docker::{Container, ContainerStats, ContainerInspection, DockerClient};

fn update_docktop() -> Result<(), Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("mel-cell")
        .repo_name("docktop")
        .bin_name("docktop")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;
    
    println!("Update status: `{}`!", status.version());
    Ok(())
}

fn enter_container_shell(container_id: &str, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, cli_path: &str) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    println!("Entering container shell for {}...", container_id);
    
    // Try bash first
    let status = std::process::Command::new(cli_path)
        .arg("exec")
        .arg("-it")
        .arg(container_id)
        .arg("/bin/bash")
        .status();

    // If bash fails, try sh
    if status.is_err() || !status.unwrap().success() {
        println!("Bash failed, trying sh...");
        let _ = std::process::Command::new(cli_path)
            .arg("exec")
            .arg("-it")
            .arg(container_id)
            .arg("/bin/sh")
            .status();
    }

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(())
}

fn enter_database_cli(container_id: &str, image: &str, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, cli_path: &str) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    println!("Entering database CLI for {} ({}) ...", container_id, image);

    let mut cmd = std::process::Command::new(cli_path);
    cmd.arg("exec").arg("-it").arg(container_id);

    let image_lower = image.to_lowercase();
    if image_lower.contains("mysql") || image_lower.contains("mariadb") {
        cmd.arg("mysql").arg("-u").arg("root").arg("-p");
    } else if image_lower.contains("postgres") {
        cmd.arg("psql").arg("-U").arg("postgres");
    } else if image_lower.contains("redis") {
        cmd.arg("redis-cli");
    } else if image_lower.contains("mongo") {
        cmd.arg("mongosh");
    } else {
        println!("Unknown database type for image: {}", image);
        println!("Press any key to continue...");
        let _ = std::io::stdin().read_line(&mut String::new());
        
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        terminal.clear()?;
        return Ok(());
    }

    let status = cmd.status();

    if status.is_err() || !status.unwrap().success() {
         println!("Failed to start database CLI.");
         println!("Press any key to continue...");
         let _ = std::io::stdin().read_line(&mut String::new());
    }

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Check for update arg
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "update" {
        if let Err(e) = update_docktop() {
            eprintln!("Update failed: {}", e);
            std::process::exit(1);
        }
        println!("Update successful! Please restart docktop.");
        std::process::exit(0);
    }

    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channels
    let (tx_containers, mut rx_containers) = mpsc::channel::<Vec<Container>>(10);
    let (tx_details, mut rx_details) = mpsc::channel::<(Option<ContainerStats>, Option<ContainerInspection>)>(10);
    let (tx_logs, mut rx_logs) = mpsc::channel::<String>(100);
    let (tx_target, rx_target) = watch::channel::<Option<String>>(None);
    let (tx_action, rx_action) = mpsc::channel::<Action>(10);
    let (tx_action_result, mut rx_action_result) = mpsc::channel::<String>(10);
    let (tx_janitor_items, mut rx_janitor_items) = mpsc::channel::<Vec<crate::wizard::models::JanitorItem>>(10);
    let (tx_refresh, mut rx_refresh) = mpsc::channel::<()>(1);

    // Docker Client (Shared)
    let docker_client = std::sync::Arc::new(DockerClient::new());
    
    // Task 1: Container Lister (Event Driven + Slow Poll)
    let client_clone1 = docker_client.clone();
    tokio::spawn(async move {
        // Initial fetch
        if let Ok(containers) = client_clone1.list_containers().await {
             let _ = tx_containers.send(containers).await;
        }

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(10)) => {}, // Slow poll
                _ = rx_refresh.recv() => {}, // Event triggered
            }
            
            if let Ok(containers) = client_clone1.list_containers().await {
                if tx_containers.send(containers).await.is_err() {
                    break;
                }
            }
        }
    });

    // Task 2: Details Fetcher (On Demand + Slow Loop)
    let client_clone2 = docker_client.clone();
    let mut rx_target_details = rx_target.clone();
    tokio::spawn(async move {
        let mut last_fetch = std::time::Instant::now();
        loop {
            let target_changed = rx_target_details.has_changed().unwrap_or(false);
            let time_to_update = last_fetch.elapsed() >= Duration::from_secs(2);
            
            if target_changed || time_to_update {
                if target_changed {
                    let _ = rx_target_details.borrow_and_update();
                }
                
                let target_id = rx_target_details.borrow().clone();
                if let Some(id) = target_id {
                    let stats = client_clone2.get_stats(&id).await.ok();
                    let inspect = client_clone2.inspect_container(&id).await.ok();
                    
                    if tx_details.send((stats, inspect)).await.is_err() {
                        break;
                    }
                    last_fetch = std::time::Instant::now();
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Task 3: Log Streamer
    let client_clone3 = docker_client.clone();
    let mut rx_target_logger = rx_target.clone();

    let tx_logs_streamer = tx_logs.clone();
    tokio::spawn(async move {
        let mut current_log_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut last_id: Option<String> = None;

        loop {
            if rx_target_logger.changed().await.is_err() { break; }
            let new_id = rx_target_logger.borrow().clone();

            if new_id != last_id {
                if let Some(task) = current_log_task.take() {
                    task.abort();
                }
                
                if let Some(id) = new_id.clone() {
                    let client = client_clone3.clone();
                    let tx = tx_logs_streamer.clone();
                    
                    current_log_task = Some(tokio::spawn(async move {
                        if let Ok(mut stream) = client.get_logs_stream(&id).await {
                             let mut header = [0u8; 8];
                             if stream.read_exact(&mut header).await.is_err() { return; }
                             
                             let is_multiplexed = header[0] <= 2 && header[1] == 0 && header[2] == 0 && header[3] == 0;
                             
                             if is_multiplexed {
                                 let size = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
                                 if size < 10_000_000 {
                                     let mut payload = vec![0u8; size];
                                     if stream.read_exact(&mut payload).await.is_ok() {
                                         let line = String::from_utf8_lossy(&payload).to_string();
                                         for l in line.lines() { if tx.send(l.to_string()).await.is_err() { return; } }
                                     }
                                 }
                                 loop {
                                     if stream.read_exact(&mut header).await.is_err() { break; }
                                     let size = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
                                     if size > 10_000_000 { break; }
                                     let mut payload = vec![0u8; size];
                                     if stream.read_exact(&mut payload).await.is_err() { break; }
                                     let line = String::from_utf8_lossy(&payload).to_string();
                                     for l in line.lines() { if tx.send(l.to_string()).await.is_err() { return; } }
                                 }
                             } else {
                                 let chunk = String::from_utf8_lossy(&header).to_string();
                                 if tx.send(chunk).await.is_err() { return; }
                                 let mut buffer = [0u8; 1024];
                                 loop {
                                     match stream.read(&mut buffer).await {
                                         Ok(0) => break,
                                         Ok(n) => {
                                             let s = String::from_utf8_lossy(&buffer[..n]).to_string();
                                             for line in s.split_inclusive('\n') {
                                                 if tx.send(line.to_string()).await.is_err() { return; }
                                             }
                                         }
                                         Err(_) => break,
                                     }
                                 }
                             }
                        }
                    }));
                }
                last_id = new_id;
            }
        }
    });

    // Task 4: Docker Events Listener
    let client_clone4 = docker_client.clone();
    let tx_refresh_clone = tx_refresh.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(mut stream) = client_clone4.get_events_stream().await {
                let mut buffer = [0u8; 1024];
                loop {
                    match stream.read(&mut buffer).await {
                        Ok(0) => break, // Connection closed
                        Ok(_) => {
                            // Any data means an event occurred
                            let _ = tx_refresh_clone.send(()).await;
                        }
                        Err(_) => break,
                    }
                }
            }
            // Retry delay
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Task 5: Action Executor
    tokio::spawn(action::run_action_loop(rx_action, tx_action_result, tx_janitor_items, tx_refresh, tx_logs.clone()));

    // App State
    let mut app = App::new();
    let mut last_tick = std::time::Instant::now();
    let mut last_user_event = std::time::Instant::now();
    let idle_timeout = Duration::from_secs(5);
    let idle_tick_rate = Duration::from_secs(2);

    loop {
        let is_idle = last_user_event.elapsed() > idle_timeout;
        let tick_rate = if is_idle {
            idle_tick_rate
        } else {
            Duration::from_millis(app.config.general.refresh_rate_ms)
        };
        
        app.refresh_system_stats();
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            last_user_event = std::time::Instant::now();
            if let Event::Key(key) = event::read()? {
                // 0. Force Quit (Ctrl+C) - Always available for safety
                if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    break;
                }

                // 1. Wizard / Modal Mode - Prioritize Input
                if app.wizard.is_some() {
                    // Check for Help in Wizard?
                    // Usually we might want F1 help? But let's keep it simple: Wizard consumes all.
                    // Exception: Maybe we want to allow `toggle_wizard` to close it IF it's not a printable char?
                    // For now, let's rely on Esc handled by wizard logic.
                    
                    if let Some(wizard_action) = app.wizard_handle_key(key) {
                         match wizard_action {
                             crate::wizard::models::WizardAction::Close => {
                                 app.wizard = None;
                             },
                             crate::wizard::models::WizardAction::EditPreview => {
                                 // Handle external editor for Preview
                                 if let Some(wizard) = &mut app.wizard {
                                     if let crate::wizard::models::WizardStep::Preview { content, .. } = &mut wizard.step {
                                         let file_path = "/tmp/docktop_preview.yml";
                                         if std::fs::write(file_path, content.as_bytes()).is_ok() {
                                             let _ = crossterm::terminal::disable_raw_mode();
                                             let _ = execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
                                             
                                             let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                                             let _ = std::process::Command::new(editor).arg(file_path).status();
                                             
                                             let _ = crossterm::terminal::enable_raw_mode();
                                             let _ = execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture);
                                             let _ = terminal.clear(); // Redraw immediately
                                             
                                             if let Ok(new_content) = std::fs::read_to_string(file_path) {
                                                 *content = new_content;
                                             }
                                         }
                                     }
                                 }
                             },
                             wa => {
                                 // Map other actions to backend Action
                                 let action = match wa {
                                     crate::wizard::models::WizardAction::Create { image, name, ports, env, cpu, memory, restart } => Action::Create { image, name, ports, env, cpu, memory, restart },
                                     crate::wizard::models::WizardAction::Build { tag, path, mount } => Action::Build { tag, path, mount },
                                     crate::wizard::models::WizardAction::ComposeUp { path, override_path } => Action::ComposeUp { path, override_path },
                                     crate::wizard::models::WizardAction::Replace { old_id, image, name, ports, env, cpu, memory, restart } => Action::Replace { old_id, image, name, ports, env, cpu, memory, restart },
                                     crate::wizard::models::WizardAction::ScanJanitor => Action::ScanJanitor,
                                     crate::wizard::models::WizardAction::CleanJanitor(items) => Action::CleanJanitor(items),
                                     _ => Action::RefreshContainers, // Fallback/No-op
                                 };
                                 let _ = tx_action.send(action).await;
                             }
                         }
                    }

                }
                // 2. Global Hotkeys (Only when Wizard is CLOSED)
                else if keys::key_matches(key, &app.config.keys.quit) {
                    break;
                } else if keys::key_matches(key, &app.config.keys.refresh) {
                    let _ = tx_action.send(Action::RefreshContainers).await;
                } else if keys::key_matches(key, &app.config.keys.toggle_wizard) {
                    app.toggle_wizard();
                } else if keys::key_matches(key, "c") || keys::key_matches(key, "Tab") {
                     app.toggle_wizard();
                } else if keys::key_matches(key, "Esc") {
                    if app.is_typing_filter {
                        app.is_typing_filter = false;
                        app.filter_query.clear();
                    } else if app.show_help {
                        app.show_help = false;
                    }
                } else if app.is_typing_filter {
                    // Handle typing
                    match key.code {
                        KeyCode::Char(c) => {
                            app.filter_query.push(c);
                        }
                        KeyCode::Backspace => {
                            app.filter_query.pop();
                        }
                        KeyCode::Enter => {
                            app.is_typing_filter = false;
                        }
                        _ => {}
                    }
                } else if keys::key_matches(key, "/") {
                    app.is_typing_filter = true;
                    app.filter_query.clear();
                } else if keys::key_matches(key, &app.config.keys.toggle_help) {
                    app.show_help = !app.show_help;
                } else {
                    if app.show_help {
                        // Ignore other keys when help is shown
                    } else {
                        if keys::key_matches(key, &app.config.keys.enter) {
                            app.show_details = !app.show_details;
                            if let Some(c) = app.get_selected_container() {
                                let _ = tx_target.send(Some(c.id.clone()));
                            }
                        } else if keys::key_matches(key, &app.config.keys.delete) {
                            if let Some(c) = app.get_selected_container() {
                                let _ = tx_action.send(Action::Delete(c.id.clone())).await;
                            }
                        } else if keys::key_matches(key, &app.config.keys.down) {
                            app.next();
                            if let Some(c) = app.get_selected_container() {
                                let _ = tx_target.send(Some(c.id.clone()));
                            }
                        } else if keys::key_matches(key, &app.config.keys.up) {
                             app.previous();
                             if let Some(c) = app.get_selected_container() {
                                let _ = tx_target.send(Some(c.id.clone()));
                            }
                        } else if keys::key_matches(key, &app.config.keys.edit) {
                            if let Some(c) = app.get_selected_container() {
                                if let Some(inspect) = &app.current_inspection {
                                    let image = inspect.config.as_ref().map(|c| c.image.clone()).unwrap_or_default();
                                    let name = inspect.name.as_ref().map(|n| n.trim_start_matches('/').to_string()).unwrap_or_default();
                                    
                                    let mut ports = String::new();
                                    if let Some(network_settings) = &inspect.network_settings {
                                        if let Some(bindings) = &network_settings.ports {
                                            for (k, v) in bindings {
                                                if let Some(list) = v {
                                                    if let Some(binding) = list.first() {
                                                        let host_port = &binding.host_port;
                                                        let container_port = k.trim_end_matches("/tcp");
                                                        ports = format!("{}:{}", host_port, container_port);
                                                        break; 
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    let mut env = String::new();
                                    if let Some(config) = &inspect.config {
                                        if let Some(envs) = &config.env {
                                            if let Some(first_env) = envs.first() {
                                                env = first_env.clone();
                                            }
                                        }
                                    }

                                    let mut cpu = String::new();
                                    let mut memory = String::new();
                                    if let Some(host_config) = &inspect.host_config {
                                        if let Some(nano) = host_config.nano_cpus {
                                            if nano > 0 {
                                                cpu = format!("{}", nano as f64 / 1_000_000_000.0);
                                            }
                                        }
                                        if let Some(mem) = host_config.memory {
                                            if mem > 0 {
                                                if mem % (1024 * 1024 * 1024) == 0 {
                                                    memory = format!("{}g", mem / (1024 * 1024 * 1024));
                                                } else if mem % (1024 * 1024) == 0 {
                                                    memory = format!("{}m", mem / (1024 * 1024));
                                                } else if mem % 1024 == 0 {
                                                    memory = format!("{}k", mem / 1024);
                                                } else {
                                                    memory = format!("{}", mem);
                                                }
                                            }
                                        }
                                    }
                                    
                                    let mut restart = String::new();
                                    if let Some(host_config) = &inspect.host_config {
                                        if let Some(policy) = &host_config.restart_policy {
                                            restart = policy.name.clone();
                                        }
                                    }

                                    app.wizard = Some(crate::wizard::models::WizardState {
                                        step: crate::wizard::models::WizardStep::QuickRunInput {
                                            image,
                                            name,
                                            ports,
                                            env,
                                            cpu,
                                            memory,
                                            restart,
                                            show_advanced: true,
                                            focused_field: 0,
                                            editing_id: Some(c.id.clone()),
                                            port_status: crate::wizard::models::PortStatus::None,
                                            profile: crate::wizard::models::ResourceProfile::Custom,
                                        },
                                    });
                                }
                            }
                        } else if keys::key_matches(key, &app.config.keys.shell) {
                             if let Some(container) = app.get_selected_container() {
                                let id = container.id.clone();
                                let cli_path = app.config.general.docker_cli_path.clone();
                                let _ = enter_container_shell(&id, &mut terminal, &cli_path);
                                terminal.clear()?;
                            }
                        } else if keys::key_matches(key, &app.config.keys.db_cli) {
                             if let Some(container) = app.get_selected_container() {
                                let image = container.image.to_lowercase();
                                if image.contains("mysql") || image.contains("mariadb") || image.contains("postgres") || image.contains("redis") || image.contains("mongo") {
                                    let id = container.id.clone();
                                    let cli_path = app.config.general.docker_cli_path.clone();
                                    let _ = enter_database_cli(&id, &container.image, &mut terminal, &cli_path);
                                    terminal.clear()?;
                                }
                            }
                        } else if keys::key_matches(key, &app.config.keys.restart) {
                            if let Some(c) = app.get_selected_container() {
                                let id = c.id.clone();
                                app.set_action_status("Restarting...".to_string());
                                let _ = tx_action.send(Action::Restart(id)).await;
                            }
                        } else if keys::key_matches(key, &app.config.keys.stop) {
                            if let Some(c) = app.get_selected_container() {
                                let id = c.id.clone();
                                app.set_action_status("Stopping...".to_string());
                                let _ = tx_action.send(Action::Stop(id)).await;
                            }
                        } else if keys::key_matches(key, &app.config.keys.start) {
                            if let Some(c) = app.get_selected_container() {
                                let id = c.id.clone();
                                app.set_action_status("Starting...".to_string());
                                let _ = tx_action.send(Action::Start(id)).await;
                            }
                        } else if keys::key_matches(key, &app.config.keys.yaml) {
                             if let Some(c) = app.get_selected_container() {
                                if let Some(inspect) = &app.current_inspection {
                                    // Prepare YAML content
                                    let image = inspect.config.as_ref().map(|c| c.image.clone()).unwrap_or_default();
                                    let name = inspect.name.as_ref().map(|n| n.trim_start_matches('/').to_string()).unwrap_or_default();
                                    
                                    // Extract Ports
                                    let mut ports_vec = Vec::new();
                                    if let Some(network_settings) = &inspect.network_settings {
                                        if let Some(bindings) = &network_settings.ports {
                                            for (k, v) in bindings {
                                                if let Some(list) = v {
                                                    if let Some(binding) = list.first() {
                                                        let host_port = &binding.host_port;
                                                        let container_port = k.trim_end_matches("/tcp");
                                                        ports_vec.push(format!("{}:{}", host_port, container_port));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    let ports = ports_vec.join(",");

                                    // Extract Env
                                    let mut env_vec = Vec::new();
                                    if let Some(config) = &inspect.config {
                                        if let Some(envs) = &config.env {
                                            env_vec = envs.clone();
                                        }
                                    }
                                    
                                    // Extract Restart Policy
                                    let mut restart = "no".to_string();
                                    if let Some(host_config) = &inspect.host_config {
                                        if let Some(policy) = &host_config.restart_policy {
                                            restart = policy.name.clone();
                                        }
                                    }

                                    // Extract Resources
                                    let mut cpu = "".to_string();
                                    let mut memory = "".to_string();
                                    if let Some(host_config) = &inspect.host_config {
                                        if let Some(nano) = host_config.nano_cpus {
                                            if nano > 0 {
                                                cpu = format!("{}", nano as f64 / 1_000_000_000.0);
                                            }
                                        }
                                        if let Some(mem) = host_config.memory {
                                            if mem > 0 {
                                                if mem % (1024 * 1024 * 1024) == 0 {
                                                    memory = format!("{}g", mem / (1024 * 1024 * 1024));
                                                } else if mem % (1024 * 1024) == 0 {
                                                    memory = format!("{}m", mem / (1024 * 1024));
                                                } else {
                                                    memory = format!("{}", mem);
                                                }
                                            }
                                        }
                                    }

                                    #[derive(serde::Serialize, serde::Deserialize)]
                                    struct ContainerConfigYaml {
                                        image: String,
                                        name: String,
                                        ports: String,
                                        env: Vec<String>,
                                        restart: String,
                                        cpu: String,
                                        memory: String,
                                    }

                                    let yaml_struct = ContainerConfigYaml {
                                        image,
                                        name,
                                        ports,
                                        env: env_vec,
                                        restart,
                                        cpu,
                                        memory,
                                    };

                                    if let Ok(yaml_content) = serde_yaml::to_string(&yaml_struct) {
                                        let temp_file_path = format!("/tmp/docktop_edit_{}.yaml", c.id);
                                        if std::fs::write(&temp_file_path, yaml_content).is_ok() {
                                            disable_raw_mode()?;
                                            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
                                            
                                            let editor = std::env::var("EDITOR").unwrap_or("nano".to_string());
                                            let _ = std::process::Command::new(editor)
                                                .arg(&temp_file_path)
                                                .status();

                                            enable_raw_mode()?;
                                            execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                                            terminal.clear()?;

                                            if let Ok(new_content) = std::fs::read_to_string(&temp_file_path) {
                                                if let Ok(new_config) = serde_yaml::from_str::<ContainerConfigYaml>(&new_content) {
                                                    let action = Action::Replace {
                                                        old_id: c.id.clone(),
                                                        image: new_config.image,
                                                        name: new_config.name,
                                                        ports: new_config.ports,
                                                        env: new_config.env.join(" "),
                                                        cpu: new_config.cpu,
                                                        memory: new_config.memory,
                                                        restart: new_config.restart,
                                                    };
                                                    let _ = tx_action.send(action).await;
                                                    app.set_action_status("Applying YAML changes...".to_string());
                                                } else {
                                                    app.set_action_status("Invalid YAML format!".to_string());
                                                }
                                            }
                                            let _ = std::fs::remove_file(temp_file_path);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Update Containers
            while let Ok(containers) = rx_containers.try_recv() {
                app.update_containers(containers);
                // If selection out of bounds, reset
                if app.selected_index >= app.containers.len() && !app.containers.is_empty() {
                    app.selected_index = app.containers.len() - 1;
                }
                if app.containers.len() > 0 && rx_target.borrow().is_none() {
                     if let Some(c) = app.get_selected_container() {
                        let _ = tx_target.send(Some(c.id.clone()));
                    }
                }
            }

            // Update Details
            while let Ok((stats, inspect)) = rx_details.try_recv() {
                // Store current as previous before updating
                if let Some(curr) = app.current_stats.take() {
                    app.previous_stats = Some(curr);
                }
                app.current_stats = stats;
                app.current_inspection = inspect;
                app.is_loading_details = false;

                // Update CPU & Network History
                // We need to clone stats or extract values to avoid borrowing app twice
                let (cpu, rx, tx) = if let Some(stats) = &app.current_stats {
                    let cpu = ui::calculate_cpu_usage(stats, &app.previous_stats);
                    
                    let (rx, tx) = if let Some(nets) = &stats.networks {
                        let mut total_rx = 0.0;
                        let mut total_tx = 0.0;
                        for (_, net) in nets {
                            total_rx += net.rx_bytes as f64;
                            total_tx += net.tx_bytes as f64;
                        }
                        
                        if let Some(prev) = &app.previous_stats {
                            if let Some(prev_nets) = &prev.networks {
                                let mut prev_rx = 0.0;
                                let mut prev_tx = 0.0;
                                for (_, net) in prev_nets {
                                    prev_rx += net.rx_bytes as f64;
                                    prev_tx += net.tx_bytes as f64;
                                }
                                (total_rx - prev_rx, total_tx - prev_tx)
                            } else {
                                (0.0, 0.0)
                            }
                        } else {
                            (0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0)
                    };
                    
                    (Some(cpu), Some(rx), Some(tx))
                } else {
                    (None, None, None)
                };

                if let Some(c) = cpu {
                    app.update_cpu_history(c);
                }
                if let Some(r) = rx {
                    if let Some(t) = tx {
                        app.update_net_history(r, t);
                    }
                }
            }

            // Update Logs
            while let Ok(log) = rx_logs.try_recv() {
                app.add_log(log);
            }

            // Update Action Results
            if let Ok(msg) = rx_action_result.try_recv() {
                let is_scan_complete = msg == "Scan Complete";
                app.set_action_status(msg);
                // If we receive a result, it means the action is done.
                // We should close the wizard if it's open.
                if app.wizard.is_some() {
                    // Only close if not Janitor scanning (which keeps wizard open but updates state)
                    // Actually we want to close wizard after CleanJanitor but not ScanJanitor
                    // But ScanJanitor returns "Scan Complete" string.
                    if !is_scan_complete {
                         app.toggle_wizard();
                    }
                }
            }

            // Update Janitor Items
            while let Ok(items) = rx_janitor_items.try_recv() {
                if let Some(wizard) = &mut app.wizard {
                    if let crate::wizard::models::WizardStep::Janitor { items: ref mut current_items, loading, .. } = &mut wizard.step {
                        *current_items = items;
                        *loading = false;
                    }
                }
            }
            
            app.clear_action_status();
            app.update_fish();
            app.update_wizard_spinner();

            last_tick = std::time::Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
