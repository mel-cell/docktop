use std::{io, time::Duration};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, watch};

mod app;
mod docker;
mod ui;

use app::App;
use docker::DockerClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Channels
    let (tx_data, mut rx_data) = mpsc::channel(10);
    let (tx_target, rx_target) = watch::channel::<Option<String>>(None);

    // Docker Client Task
    let docker_client = std::sync::Arc::new(DockerClient::new());
    let client_clone = docker_client.clone();
    
    let mut rx_target_clone = rx_target.clone();
    
    tokio::spawn(async move {
        let mut rx_target = rx_target_clone;
        loop {
            let containers_res = client_clone.list_containers().await;
            let target_id = rx_target.borrow_and_update().clone();
            
            let stats = if let Some(id) = target_id {
                 client_clone.get_stats(&id).await.ok()
            } else {
                None
            };

            if let Ok(containers) = containers_res {
                if tx_data.send((containers, stats)).await.is_err() {
                    break;
                }
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // App State
    let mut app = App::new();
    let tick_rate = Duration::from_millis(250);
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
                    KeyCode::Down => {
                        app.next();
                        if let Some(c) = app.get_selected_container() {
                            let _ = tx_target.send(Some(c.id.clone()));
                        }
                    }
                    KeyCode::Up => {
                        app.previous();
                        if let Some(c) = app.get_selected_container() {
                            let _ = tx_target.send(Some(c.id.clone()));
                        }
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Check for data updates
            while let Ok((containers, stats)) = rx_data.try_recv() {
                app.containers = containers;
                app.current_stats = stats;
                
                // Ensure selected index is valid
                if app.selected_index >= app.containers.len() && !app.containers.is_empty() {
                    app.selected_index = app.containers.len() - 1;
                }
                
                // If we have no selection but we have containers, select the first one and notify backend
                if app.containers.len() > 0 && rx_target.borrow().is_none() {
                     if let Some(c) = app.get_selected_container() {
                        let _ = tx_target.send(Some(c.id.clone()));
                    }
                }
            }
            last_tick = std::time::Instant::now();
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
