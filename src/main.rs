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
use bollard::Docker;
use bollard::query_parameters::{StartContainerOptions, CreateImageOptions, CreateContainerOptions, BuildImageOptions, StopContainerOptions, RestartContainerOptions, RemoveContainerOptions, ListImagesOptions, ListVolumesOptions, ListContainersOptions, RemoveImageOptions, RemoveVolumeOptions};
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use futures_util::stream::StreamExt;
use http_body_util::Full;
use http_body_util::Either;
use bytes::Bytes;

mod app;
mod config;
mod docker;
mod ui;
mod theme;

use app::App;
use docker::{Container, ContainerStats, ContainerInspection, DockerClient};

#[derive(Debug)]
enum Action {
    Start(String),
    Stop(String),
    Restart(String),
    Create { image: String, name: String, ports: String, env: String, cpu: String, memory: String },
    Build { tag: String, path: std::path::PathBuf, mount: bool },
    ComposeUp { path: std::path::PathBuf, override_path: Option<std::path::PathBuf> },
    Replace { old_id: String, image: String, name: String, ports: String, env: String, cpu: String, memory: String },
    ScanJanitor,
    CleanJanitor(Vec<app::JanitorItem>),
    Delete(String),
    RefreshContainers,
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

#[tokio::main]
async fn main() -> Result<()> {
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
    let (tx_action, mut rx_action) = mpsc::channel::<Action>(10);
    let (tx_action_result, mut rx_action_result) = mpsc::channel::<String>(10);
    let (tx_janitor_items, mut rx_janitor_items) = mpsc::channel::<Vec<app::JanitorItem>>(10);
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
                    let tx = tx_logs.clone();
                    
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
    let tx_refresh_action = tx_refresh.clone();
    tokio::spawn(async move {
        let docker = Docker::connect_with_local_defaults().unwrap();
        while let Some(action) = rx_action.recv().await {
            let res = match action {
                Action::RefreshContainers => {
                    let _ = tx_refresh_action.send(()).await;
                    "Refreshed containers".to_string()
                },
                Action::ScanJanitor => {
                    let _ = tx_action_result.send("Scanning for junk...".to_string()).await;
                    let mut items = Vec::new();
                    
                    // 1. Dangling Images
                    let mut filters = std::collections::HashMap::new();
                    filters.insert("dangling".to_string(), vec!["true".to_string()]);
                    
                    if let Ok(images) = docker.list_images(Some(ListImagesOptions {
                        filters: Some(filters),
                        ..Default::default()
                    })).await {
                        for img in images {
                            items.push(app::JanitorItem {
                                id: img.id.clone(),
                                name: "<none>".to_string(),
                                kind: app::JanitorItemKind::Image,
                                size: img.size as u64,
                                age: "Unknown".to_string(),
                                selected: true,
                            });
                        }
                    }

                    // 2. Unused Volumes
                    let mut filters = std::collections::HashMap::new();
                    filters.insert("dangling".to_string(), vec!["true".to_string()]);

                    if let Ok(volumes) = docker.list_volumes(Some(ListVolumesOptions {
                        filters: Some(filters),
                    })).await {
                        if let Some(vols) = volumes.volumes {
                            for vol in vols {
                                items.push(app::JanitorItem {
                                    id: vol.name.clone(),
                                    name: vol.name.clone(),
                                    kind: app::JanitorItemKind::Volume,
                                    size: 0,
                                    age: "-".to_string(),
                                    selected: false,
                                });
                            }
                        }
                    }

                    // 3. Stopped Containers
                    let mut filters = std::collections::HashMap::new();
                    filters.insert("status".to_string(), vec!["exited".to_string(), "dead".to_string()]);

                    if let Ok(containers) = docker.list_containers(Some(ListContainersOptions {
                        all: true,
                        filters: Some(filters),
                        ..Default::default()
                    })).await {
                        for c in containers {
                            items.push(app::JanitorItem {
                                id: c.id.unwrap_or_default(),
                                name: c.names.unwrap_or_default().first().cloned().unwrap_or_default(),
                                kind: app::JanitorItemKind::Container,
                                size: 0, // Container size requires size=true in list which is slow
                                age: c.status.unwrap_or_default(),
                                selected: true,
                            });
                        }
                    }
                    
                    let _ = tx_janitor_items.send(items).await;
                    "Scan Complete".to_string()
                }
                Action::CleanJanitor(items) => {
                    let mut count = 0;
                    for item in items {
                        match item.kind {
                            app::JanitorItemKind::Image => {
                                let _ = docker.remove_image(&item.id, None::<RemoveImageOptions>, None).await;
                            },
                            app::JanitorItemKind::Volume => {
                                let _ = docker.remove_volume(&item.id, None::<RemoveVolumeOptions>).await;
                            },
                            app::JanitorItemKind::Container => {
                                let _ = docker.remove_container(&item.id, None::<RemoveContainerOptions>).await;
                            },
                        }
                        count += 1;
                        if count % 5 == 0 {
                             let _ = tx_action_result.send(format!("Cleaned {} items...", count)).await;
                        }
                    }
                    format!("Janitor finished. Removed {} items.", count)
                }
                Action::Start(id) => {
                    match docker.start_container(&id, None::<StartContainerOptions>).await {
                        Ok(_) => format!("Started container {}", &id[..12]),
                        Err(e) => format!("Failed to start: {}", e),
                    }
                }
                Action::Stop(id) => {
                    match docker.stop_container(&id, None::<StopContainerOptions>).await {
                        Ok(_) => format!("Stopped container {}", &id[..12]),
                        Err(e) => format!("Failed to stop: {}", e),
                    }
                }
                Action::Restart(id) => {
                    match docker.restart_container(&id, None::<RestartContainerOptions>).await {
                        Ok(_) => format!("Restarted container {}", &id[..12]),
                        Err(e) => format!("Failed to restart: {}", e),
                    }
                }
                Action::Create { image, name, ports, env, cpu, memory } => {
                    let _ = tx_action_result.send(format!("Pulling {}...", image)).await;
                    let mut stream = docker.create_image(
                        Some(CreateImageOptions { from_image: Some(image.clone()), ..Default::default() }),
                        None,
                        None
                    );
                    while let Some(_) = stream.next().await {}

                    let _ = tx_action_result.send(format!("Creating {}...", name)).await;
                    
                    let mut port_bindings = std::collections::HashMap::new();
                    let mut exposed_ports = std::collections::HashMap::new();
                    if !ports.is_empty() {
                         let parts: Vec<&str> = ports.split(':').collect();
                         if parts.len() == 2 {
                             let container_port = format!("{}/tcp", parts[1]);
                             exposed_ports.insert(container_port.clone(), std::collections::HashMap::new());
                             port_bindings.insert(container_port, Some(vec![PortBinding {
                                 host_ip: Some("0.0.0.0".to_string()),
                                 host_port: Some(parts[0].to_string()),
                             }]));
                         }
                    }

                    let nano_cpus = if !cpu.is_empty() {
                        cpu.parse::<f64>().ok().map(|v| (v * 1_000_000_000.0) as i64)
                    } else { None };

                    let memory_bytes = if !memory.is_empty() {
                        let lower = memory.to_lowercase();
                        if let Some(val) = lower.strip_suffix('m') {
                            val.parse::<i64>().ok().map(|v| v * 1024 * 1024)
                        } else if let Some(val) = lower.strip_suffix('g') {
                            val.parse::<i64>().ok().map(|v| v * 1024 * 1024 * 1024)
                        } else if let Some(val) = lower.strip_suffix('k') {
                            val.parse::<i64>().ok().map(|v| v * 1024)
                        } else {
                            lower.parse::<i64>().ok()
                        }
                    } else { None };

                    let envs = if !env.is_empty() { Some(vec![env]) } else { None };
                    
                    let config = ContainerCreateBody {
                        image: Some(image.clone()),
                        exposed_ports: Some(exposed_ports),
                        host_config: Some(HostConfig {
                            port_bindings: Some(port_bindings),
                            nano_cpus,
                            memory: memory_bytes,
                            ..Default::default()
                        }),
                        env: envs,
                        ..Default::default()
                    };

                    let options = if !name.is_empty() {
                        Some(CreateContainerOptions { name: Some(name.clone()), ..Default::default() })
                    } else { None };

                    match docker.create_container(options, config).await {
                        Ok(res) => {
                            let _ = tx_action_result.send(format!("Starting {}...", res.id)).await;
                            match docker.start_container(&res.id, None::<StartContainerOptions>).await {
                                Ok(_) => format!("Started new container {}", &res.id[..12]),
                                Err(e) => format!("Failed to start: {}", e),
                            }
                        },
                        Err(e) => format!("Failed to create: {}", e),
                    }
                }
                Action::Build { tag, path, mount } => {
                     let _ = tx_action_result.send(format!("Building {}...", tag)).await;
                     
                     let mut tar = tar::Builder::new(Vec::new());
                     if let Err(e) = tar.append_dir_all(".", &path) {
                         format!("Failed to pack context: {}", e)
                     } else {
                         let tar_content = tar.into_inner().unwrap();
                         let build_options = BuildImageOptions {
                             t: Some(tag.clone()),
                             rm: true,
                             ..Default::default()
                         };
                         
                         let body = Full::new(Bytes::from(tar_content));
                         let mut stream = docker.build_image(build_options, None, Some(Either::Left(body)));
                         while let Some(_) = stream.next().await {}
                         
                         // Run
                         let _ = tx_action_result.send(format!("Running {}...", tag)).await;
                         let mut host_config = HostConfig::default();
                         if mount {
                             if let Ok(abs_path) = std::fs::canonicalize(&path) {
                                 host_config.binds = Some(vec![format!("{}:/app", abs_path.to_string_lossy())]);
                             }
                         }

                         let config = ContainerCreateBody {
                             image: Some(tag.clone()),
                             host_config: Some(host_config),
                             ..Default::default()
                         };
                         
                         match docker.create_container(None::<CreateContainerOptions>, config).await {
                             Ok(res) => {
                                 match docker.start_container(&res.id, None::<StartContainerOptions>).await {
                                     Ok(_) => format!("Built and started {}", &res.id[..12]),
                                     Err(e) => format!("Failed to start: {}", e),
                                 }
                             },
                             Err(e) => format!("Failed to create: {}", e),
                         }
                     }
                }
                Action::ComposeUp { path, override_path } => {
                    let _ = tx_action_result.send("Running docker compose up...".to_string()).await;
                    
                    let (work_dir, main_file) = if path.is_file() {
                        (path.parent().unwrap_or(&path).to_path_buf(), path.file_name().unwrap().to_string_lossy().to_string())
                    } else {
                        (path.clone(), "docker-compose.yml".to_string())
                    };

                    let mut cmd = std::process::Command::new("docker");
                    cmd.arg("compose")
                       .arg("-f")
                       .arg(&main_file);
                    
                    if let Some(ref ovr) = override_path {
                        if let Some(ovr_name) = ovr.file_name() {
                            cmd.arg("-f").arg(ovr_name);
                        }
                    }

                    cmd.arg("up")
                       .arg("-d")
                       .current_dir(&work_dir);

                    let output = cmd.output();
                        
                    match output {
                        Ok(o) => {
                            // Cleanup override file
                            if let Some(ovr) = override_path {
                                let _ = std::fs::remove_file(ovr);
                            }

                            if o.status.success() {
                                "Compose Up Successful".to_string()
                            } else {
                                format!("Compose Failed: {}", String::from_utf8_lossy(&o.stderr))
                            }
                        },
                        Err(e) => {
                             // Cleanup override file
                            if let Some(ovr) = override_path {
                                let _ = std::fs::remove_file(ovr);
                            }
                            format!("Failed to run compose: {}", e)
                        },
                    }
                }
                Action::Replace { old_id, image, name, ports, env, cpu, memory } => {
                     let _ = tx_action_result.send(format!("Stopping {}...", old_id)).await;
                     let _ = docker.stop_container(&old_id, None::<StopContainerOptions>).await;
                     let _ = tx_action_result.send(format!("Removing {}...", old_id)).await;
                     let _ = docker.remove_container(&old_id, None::<RemoveContainerOptions>).await;
                     
                    let _ = tx_action_result.send(format!("Pulling {}...", image)).await;
                    let mut stream = docker.create_image(
                        Some(CreateImageOptions { from_image: Some(image.clone()), ..Default::default() }),
                        None,
                        None
                    );
                    while let Some(_) = stream.next().await {}

                    let _ = tx_action_result.send(format!("Creating {}...", name)).await;
                    
                    let mut port_bindings = std::collections::HashMap::new();
                    let mut exposed_ports = std::collections::HashMap::new();
                    if !ports.is_empty() {
                         let parts: Vec<&str> = ports.split(':').collect();
                         if parts.len() == 2 {
                             let container_port = format!("{}/tcp", parts[1]);
                             exposed_ports.insert(container_port.clone(), std::collections::HashMap::new());
                             port_bindings.insert(container_port, Some(vec![PortBinding {
                                 host_ip: Some("0.0.0.0".to_string()),
                                 host_port: Some(parts[0].to_string()),
                             }]));
                         }
                    }

                    let nano_cpus = if !cpu.is_empty() {
                        cpu.parse::<f64>().ok().map(|v| (v * 1_000_000_000.0) as i64)
                    } else { None };

                    let memory_bytes = if !memory.is_empty() {
                        let lower = memory.to_lowercase();
                        if let Some(val) = lower.strip_suffix('m') {
                            val.parse::<i64>().ok().map(|v| v * 1024 * 1024)
                        } else if let Some(val) = lower.strip_suffix('g') {
                            val.parse::<i64>().ok().map(|v| v * 1024 * 1024 * 1024)
                        } else if let Some(val) = lower.strip_suffix('k') {
                            val.parse::<i64>().ok().map(|v| v * 1024)
                        } else {
                            lower.parse::<i64>().ok()
                        }
                    } else { None };

                    let envs = if !env.is_empty() { Some(vec![env]) } else { None };
                    
                    let config = ContainerCreateBody {
                        image: Some(image.clone()),
                        exposed_ports: Some(exposed_ports),
                        host_config: Some(HostConfig {
                            port_bindings: Some(port_bindings),
                            nano_cpus,
                            memory: memory_bytes,
                            ..Default::default()
                        }),
                        env: envs,
                        ..Default::default()
                    };

                    let options = if !name.is_empty() {
                        Some(CreateContainerOptions { name: Some(name.clone()), ..Default::default() })
                    } else { None };

                    match docker.create_container(options, config).await {
                        Ok(res) => {
                            let _ = tx_action_result.send(format!("Starting {}...", res.id)).await;
                            match docker.start_container(&res.id, None::<StartContainerOptions>).await {
                                Ok(_) => format!("Replaced container {}", &res.id[..12]),
                                Err(e) => format!("Failed to start: {}", e),
                            }
                        },
                        Err(e) => format!("Failed to create: {}", e),
                    }
                }
                Action::Delete(id) => {
                    let _ = tx_action_result.send(format!("Removing {}...", id)).await;
                    match docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await {
                        Ok(_) => format!("Removed container {}", &id[..12]),
                        Err(e) => format!("Failed to remove: {}", e),
                    }
                }
            };
            let _ = tx_action_result.send(res).await;
        }
    });

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
        
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            last_user_event = std::time::Instant::now();
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                    if app.wizard.is_some() {
                        app.toggle_wizard();
                    } else {
                        break;
                    }
                }
                KeyCode::F(5) => {
                    let _ = tx_action.send(Action::RefreshContainers).await;
                }
                KeyCode::Char('c') | KeyCode::Tab => {
                    if app.wizard.is_none() {
                        app.toggle_wizard();
                    }
                }
                KeyCode::Esc => {
                    if app.wizard.is_some() {
                        app.toggle_wizard();
                    }
                }
                    _ => {
                        if app.wizard.is_some() {
                                if let Some(wizard_action) = app.wizard_handle_key(key.code) {
                                    if let app::WizardAction::Close = wizard_action {
                                        app.wizard = None;
                                    } else {
                                        let action = match wizard_action {
                                            app::WizardAction::Create { image, name, ports, env, cpu, memory } => Action::Create { image, name, ports, env, cpu, memory },
                                            app::WizardAction::Build { tag, path, mount } => Action::Build { tag, path, mount },
                                            app::WizardAction::ComposeUp { path, override_path } => Action::ComposeUp { path, override_path },
                                            app::WizardAction::Replace { old_id, image, name, ports, env, cpu, memory } => Action::Replace { old_id, image, name, ports, env, cpu, memory },
                                            app::WizardAction::ScanJanitor => Action::ScanJanitor,
                                            app::WizardAction::CleanJanitor(items) => Action::CleanJanitor(items),
                                            app::WizardAction::Close => unreachable!(),
                                        };
                                        let _ = tx_action.send(action).await;
                                    }
                                }
                        } else {
                            match key.code {
                                KeyCode::Enter => {
                                    app.show_details = !app.show_details;
                                }
                                KeyCode::Delete | KeyCode::Char('x') => {
                                    if let Some(c) = app.get_selected_container() {
                                        let _ = tx_action.send(Action::Delete(c.id.clone())).await;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    app.next();
                                    if let Some(c) = app.get_selected_container() {
                                        let _ = tx_target.send(Some(c.id.clone()));
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    app.previous();
                                    if let Some(c) = app.get_selected_container() {
                                        let _ = tx_target.send(Some(c.id.clone()));
                                    }
                                }
                                KeyCode::Char('E') => {
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
                                                                break; // Just take first one for now
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

                                            app.wizard = Some(app::WizardState {
                                                step: app::WizardStep::QuickRunInput {
                                                    image,
                                                    name,
                                                    ports,
                                                    env,
                                                    cpu,
                                                    memory,
                                                    focused_field: 0,
                                                    editing_id: Some(c.id.clone()),
                                                    port_status: app::PortStatus::None,
                                                },
                                            });
                                        }
                                    }
                                }
                                KeyCode::Char('e') => {
                    if let Some(container) = app.get_selected_container() {
                        let id = container.id.clone();
                        let cli_path = app.config.general.docker_cli_path.clone();
                        let _ = enter_container_shell(&id, &mut terminal, &cli_path);
                        terminal.clear()?;
                    }
                }
                                KeyCode::Char('b') => {
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
                                            
                                            let action = Action::Replace {
                                                old_id: c.id.clone(),
                                                image,
                                                name,
                                                ports,
                                                env,
                                                cpu,
                                                memory,
                                            };
                                            let _ = tx_action.send(action).await;
                                            app.set_action_status("Rebuilding container...".to_string());
                                        }
                                    }
                                }
                                KeyCode::Char('r') => {
                                    if let Some(c) = app.get_selected_container() {
                                        let id = c.id.clone();
                                        app.set_action_status("Restarting...".to_string());
                                        let _ = tx_action.send(Action::Restart(id)).await;
                                    }
                                }
                                KeyCode::Char('s') => {
                                    if let Some(c) = app.get_selected_container() {
                                        let id = c.id.clone();
                                        app.set_action_status("Stopping...".to_string());
                                        let _ = tx_action.send(Action::Stop(id)).await;
                                    }
                                }
                                KeyCode::Char('u') => { // 'u' for Up/Start
                                    if let Some(c) = app.get_selected_container() {
                                        let id = c.id.clone();
                                        app.set_action_status("Starting...".to_string());
                                        let _ = tx_action.send(Action::Start(id)).await;
                                    }
                                }
                                _ => {}
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
                    if let app::WizardStep::Janitor { items: ref mut current_items, loading, .. } = &mut wizard.step {
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
