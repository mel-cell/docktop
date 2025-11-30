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
    // Main Layout: Header (Top), Body (Middle), Footer (Bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body
            Constraint::Length(3), // Footer
        ])
        .split(f.size());

    draw_header(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let running_count = app.containers.iter().filter(|c| c.state == "running").count();
    let stopped_count = app.containers.len() - running_count;

    let title = Span::styled(" DOCKTOP PRO ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD));
    let stats = Span::styled(
        format!(" Running: {} | Stopped: {} ", running_count, stopped_count),
        Style::default().fg(Color::White),
    );

    let header_text = Line::from(vec![title, Span::raw(" "), stats]);
    
    let block = Block::default().borders(Borders::BOTTOM);
    let paragraph = Paragraph::new(header_text).block(block);
    f.render_widget(paragraph, area);
}

fn draw_body(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_container_list(f, app, chunks[0]);
    draw_details_panel(f, app, chunks[1]);
}

fn draw_container_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .containers
        .iter()
        .map(|c| {
            let (status_icon, color) = if c.state == "running" {
                ("●", Color::Green)
            } else {
                ("○", Color::Red)
            };
            
            let name = c.names.first().map(|s| s.trim_start_matches('/')).unwrap_or("unknown");
            
            let content = Line::from(vec![
                Span::styled(format!(" {} ", status_icon), Style::default().fg(color)),
                Span::styled(name, Style::default().fg(Color::White)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
        .highlight_symbol("▎");

    let mut state = ListState::default();
    state.select(Some(app.selected_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_details_panel(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(area);

    draw_info_section(f, app, chunks[0]);
    draw_logs_section(f, app, chunks[1]);
}

fn draw_info_section(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(" DETAILS", Style::default().add_modifier(Modifier::BOLD)))];

    if app.is_loading_details {
        lines.push(Line::from(Span::styled(" Loading details...", Style::default().fg(Color::Yellow))));
    } else if let Some(inspect) = &app.current_inspection {
        lines.push(Line::from(vec![
            Span::styled(" ID      : ", Style::default().fg(Color::Cyan)),
            Span::raw(&inspect.id[..12]),
        ]));
        
        if let Some(config) = &inspect.config {
            lines.push(Line::from(vec![
                Span::styled(" Image   : ", Style::default().fg(Color::Cyan)),
                Span::raw(&config.image),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled(" State   : ", Style::default().fg(Color::Cyan)),
            Span::raw(app.get_selected_container().map(|c| c.state.as_str()).unwrap_or("?")),
        ]));

        if let Some(net) = &inspect.network_settings {
             if let Some(ports) = &net.ports {
                let mut port_str = String::new();
                for (port, bindings) in ports {
                    if let Some(bindings) = bindings {
                        for binding in bindings {
                            if !port_str.is_empty() { port_str.push_str(", "); }
                            port_str.push_str(&format!("{}->{}", binding.host_port, port));
                        }
                    }
                }
                if !port_str.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled(" Ports   : ", Style::default().fg(Color::Cyan)),
                        Span::raw(port_str),
                    ]));
                }
            }
        }

        if let Some(stats) = &app.current_stats {
             let cpu = calculate_cpu_usage(stats, &app.previous_stats);
             let mem = stats.memory_stats.usage.unwrap_or(0) as f64 / 1024.0 / 1024.0;
             lines.push(Line::from(vec![
                Span::styled(" CPU/Mem : ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{:.1}% / {:.0}MB", cpu, mem)),
            ]));
        }
    } else {
        lines.push(Line::from(" Select a container..."));
    }

    let paragraph = Paragraph::new(lines).block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(paragraph, area);
}

fn draw_logs_section(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(" LATEST LOGS", Style::default().add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(" ─────────────────────────────────", Style::default().fg(Color::DarkGray))),
    ];

    if app.is_loading_details {
        lines.push(Line::from(Span::styled(" Loading logs...", Style::default().fg(Color::Yellow))));
    } else {
        for log in &app.logs {
            lines.push(Line::from(Span::raw(log)));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let status_text = if let Some((msg, _)) = &app.action_status {
        Span::styled(format!(" {} ", msg), Style::default().bg(Color::Yellow).fg(Color::Black))
    } else {
        Span::raw("")
    };

    let keys = Span::styled(
        " j/k: Nav • r: Restart • s: Stop • u: Start • q: Quit",
        Style::default().fg(Color::DarkGray),
    );

    let line = Line::from(vec![status_text, Span::raw(" "), keys]);
    let block = Block::default().borders(Borders::TOP);
    let paragraph = Paragraph::new(line).block(block);
    f.render_widget(paragraph, area);
}

fn calculate_cpu_usage(stats: &ContainerStats, prev_stats: &Option<ContainerStats>) -> f64 {
    let (prev_cpu, prev_sys) = if let Some(prev) = prev_stats {
        (prev.cpu_stats.cpu_usage.total_usage, prev.cpu_stats.system_cpu_usage.unwrap_or(0))
    } else {
        (stats.precpu_stats.cpu_usage.total_usage, stats.precpu_stats.system_cpu_usage.unwrap_or(0))
    };

    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64 - prev_cpu as f64;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64 - prev_sys as f64;
    let num_cpus = stats.cpu_stats.cpu_usage.percpu_usage.as_ref().map(|v| v.len()).unwrap_or(1) as f64;

    if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * num_cpus * 100.0
    } else {
        0.0
    }
}
