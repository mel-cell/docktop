use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::app::App;
use crate::docker::ContainerStats;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.size());

    draw_container_list(f, app, chunks[0]);
    draw_stats(f, app, chunks[1]);
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

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("List Containers"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.selected_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_stats(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Statistik");
    
    let text = if let Some(stats) = &app.current_stats {
        let cpu_percent = calculate_cpu_usage(stats);
        let mem_usage = stats.memory_stats.usage as f64 / 1024.0 / 1024.0; // MB
        let mem_limit = stats.memory_stats.limit as f64 / 1024.0 / 1024.0; // MB
        let mem_percent = (mem_usage / mem_limit) * 100.0;

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
