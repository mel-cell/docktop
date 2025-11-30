use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use crate::app::App;
use crate::docker::ContainerStats;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.size());

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    draw_container_list(f, app, top_chunks[0]);
    draw_stats(f, app, top_chunks[1]);
    draw_inspector(f, app, bottom_chunks[0]);
    draw_logs(f, app, bottom_chunks[1]);
}

fn draw_container_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .containers
        .iter()
        .map(|c| {
            let status_color = if c.state == "running" {
                Color::Green
            } else {
                Color::Red
            };
            let status_icon = if c.state == "running" { "🟢" } else { "🔴" };
            
            let content = Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::raw(c.names.first().map(|s| s.trim_start_matches('/')).unwrap_or("unknown")),
                Span::styled(format!(" [{}]", c.state), Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let _list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("List Containers"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.selected_index));

    f.render_stateful_widget(_list, area, &mut state);
}

fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Statistik");
    
    let text = if app.is_loading_details {
        vec![Line::from(Span::styled("Loading stats...", Style::default().fg(Color::Yellow)))]
    } else if let Some(stats) = &app.current_stats {
        let cpu_percent = calculate_cpu_usage(stats);
        let mem_usage = stats.memory_stats.usage.unwrap_or(0) as f64 / 1024.0 / 1024.0; // MB
        let mem_limit = stats.memory_stats.limit.unwrap_or(0) as f64 / 1024.0 / 1024.0; // MB
        let mem_percent = if mem_limit > 0.0 { (mem_usage / mem_limit) * 100.0 } else { 0.0 };

        vec![
            Line::from(vec![
                Span::styled("CPU Usage: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{:.2}%", cpu_percent)),
            ]),
            Line::from(vec![
                Span::styled("Memory Usage: ", Style::default().fg(Color::Magenta)),
                Span::raw(format!("{:.2} MB / {:.2} MB ({:.2}%)", mem_usage, mem_limit, mem_percent)),
            ]),
        ]
    } else {
        vec![Line::from("Select a running container to view stats.")]
    };

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_inspector(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Info Inspector");

    let text = if app.is_loading_details {
        vec![Line::from(Span::styled("Loading details...", Style::default().fg(Color::Yellow)))]
    } else if let Some(inspect) = &app.current_inspection {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::Yellow)),
                Span::raw(&inspect.id[..12]),
            ]),
        ];
        
        if let Some(config) = &inspect.config {
             lines.push(Line::from(vec![
                Span::styled("Image: ", Style::default().fg(Color::Yellow)),
                Span::raw(&config.image),
            ]));
            
            if let Some(cmd) = &config.cmd {
                 lines.push(Line::from(vec![
                    Span::styled("Command: ", Style::default().fg(Color::Yellow)),
                    Span::raw(cmd.join(" ")),
                ]));
            }
        }

        if let Some(net) = &inspect.network_settings {
            if let Some(ip) = &net.ip_address {
                 lines.push(Line::from(vec![
                    Span::styled("IP Address: ", Style::default().fg(Color::Yellow)),
                    Span::raw(ip),
                ]));
            }
            
            if let Some(ports) = &net.ports {
                lines.push(Line::from(Span::styled("Ports:", Style::default().fg(Color::Yellow))));
                for (port, bindings) in ports {
                    if let Some(bindings) = bindings {
                        for binding in bindings {
                            lines.push(Line::from(format!("  {} -> {}:{}", port, binding.host_ip, binding.host_port)));
                        }
                    }
                }
            }
        }

        lines
    } else {
        vec![Line::from("Select a container to inspect.")]
    };

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Logs (Live Stream)");
    
    let log_text = if app.is_loading_details {
        "Loading logs...".to_string()
    } else {
        app.logs.iter().cloned().collect::<Vec<String>>().join("\n")
    };

    let paragraph = Paragraph::new(log_text)
        .block(block)
        .wrap(Wrap { trim: false });
    
    f.render_widget(paragraph, area);
}

fn calculate_cpu_usage(stats: &ContainerStats) -> f64 {
    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64 - stats.precpu_stats.cpu_usage.total_usage as f64;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64 - stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
    let num_cpus = stats.cpu_stats.cpu_usage.percpu_usage.as_ref().map(|v| v.len()).unwrap_or(1) as f64;

    if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * num_cpus * 100.0
    } else {
        0.0
    }
}
