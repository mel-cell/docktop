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

mod app;
mod config;
mod docker;
mod ui;

use app::App;
use docker::{Container, ContainerStats, ContainerInspection, DockerClient};

#[derive(Debug)]
enum Action {
    Start(String),
    Stop(String),
    Restart(String),
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

    // Docker Client (Shared)
    let docker_client = std::sync::Arc::new(DockerClient::new());
    
    // Task 1: Container Lister (Fast Loop)
    let client_clone1 = docker_client.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(containers) = client_clone1.list_containers().await {
                if tx_containers.send(containers).await.is_err() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
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

    // Task 4: Action Executor
    let client_clone4 = docker_client.clone();
    tokio::spawn(async move {
        while let Some(action) = rx_action.recv().await {
            let res = match action {
                Action::Start(id) => {
                    match client_clone4.start_container(&id).await {
                        Ok(_) => format!("Started container {}", &id[..12]),
                        Err(e) => format!("Failed to start: {}", e),
                    }
                }
                Action::Stop(id) => {
                    match client_clone4.stop_container(&id).await {
                        Ok(_) => format!("Stopped container {}", &id[..12]),
                        Err(e) => format!("Failed to stop: {}", e),
                    }
                }
                Action::Restart(id) => {
                    match client_clone4.restart_container(&id).await {
                        Ok(_) => format!("Restarted container {}", &id[..12]),
                        Err(e) => format!("Failed to restart: {}", e),
                    }
                }
            };
            let _ = tx_action_result.send(res).await;
        }
    });

    // App State
    let mut app = App::new();
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = std::time::Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
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

        if last_tick.elapsed() >= tick_rate {
            // Update Containers
            while let Ok(containers) = rx_containers.try_recv() {
                app.containers = containers;
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
            while let Ok(msg) = rx_action_result.try_recv() {
                app.set_action_status(msg);
            }
            
            app.clear_action_status();
            app.update_fish();

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
