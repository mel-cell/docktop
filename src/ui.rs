use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph, Wrap, Gauge, Table, Row, Cell, Chart, Dataset, Axis, GraphType},
    symbols,
    Frame,
};
use crate::app::App;
use crate::docker::ContainerStats;
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = app.config.theme_data.clone();
    let theme = &theme;
    
    // Main Layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // Top: Monitor
            Constraint::Min(0),     // Middle: List
            Constraint::Length(10), // Bottom: Logs
            Constraint::Length(1),  // Footer
        ])
        .split(f.size());

    draw_monitor_section(f, app, chunks[0], theme);
    draw_container_section(f, app, chunks[1], theme);
    draw_logs_section(f, app, chunks[2], theme);
    draw_footer(f, app, chunks[3], theme);
}

fn draw_monitor_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(" MONITOR ", Style::default().fg(theme.title).add_modifier(Modifier::BOLD)));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Split into 3 columns: CPU (40%), Memory (30%), Network (30%)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(inner_area);

    draw_cpu_section(f, app, chunks[0], theme);
    draw_memory_section(f, app, chunks[1], theme);
    draw_network_section(f, app, chunks[2], theme);
}

fn draw_cpu_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" CPU ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.inactive_fg));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(stats) = &app.current_stats {
        let cpu = calculate_cpu_usage(stats, &app.previous_stats);
        let num_cpus = stats.cpu_stats.cpu_usage.percpu_usage.as_ref().map(|v| v.len()).unwrap_or(1);
        
        let label = if num_cpus > 1 {
            format!("Total ({} Cores): {:.1}%", num_cpus, cpu)
        } else {
            format!("Usage: {:.1}%", cpu)
        };

        let datasets = vec![
            Dataset::default()
                .name(label)
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(theme.graph_color))
                .data(&app.cpu_history),
        ];

        let chart = Chart::new(datasets)
            .block(Block::default().borders(Borders::NONE))
            .x_axis(Axis::default().style(Style::default().fg(theme.graph_text)).bounds(app.x_axis_bounds))
            .y_axis(Axis::default().style(Style::default().fg(theme.graph_text)).bounds([0.0, 100.0]));
        
        f.render_widget(chart, inner);
    }
}

fn draw_memory_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" MEM ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.inactive_fg));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(stats) = &app.current_stats {
        let mem_usage = stats.memory_stats.usage.unwrap_or(0) as f64;
        let mem_limit = stats.memory_stats.limit.unwrap_or(0) as f64;
        let mem_percent = if mem_limit > 0.0 { (mem_usage / mem_limit) * 100.0 } else { 0.0 };
        
        // Extract Cache and Swap
        let (cache, swap) = if let Some(details) = &stats.memory_stats.stats {
            let c = *details.get("cache").or(details.get("total_cache")).unwrap_or(&0) as f64;
            // Swap is usually 'swap' or 'total_swap' in stats, but sometimes it's not directly exposed as usage
            // Docker stats often calculates swap as (mem+swap usage) - mem usage?
            // Let's check for 'swap' key directly first
            let s = *details.get("swap").unwrap_or(&0) as f64;
            (c, s)
        } else {
            (0.0, 0.0)
        };

        // Helper to format bytes
        let fmt_bytes = |b: f64| -> String {
            if b > 1024.0 * 1024.0 * 1024.0 {
                format!("{:.2} GiB", b / 1024.0 / 1024.0 / 1024.0)
            } else {
                format!("{:.0} MiB", b / 1024.0 / 1024.0)
            }
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Main Usage
                Constraint::Length(1), // Gauge
                Constraint::Length(1), // Cache
                Constraint::Length(1), // Swap
            ])
            .split(inner);

        // RAM Usage
        let label = Paragraph::new(format!("RAM: {} / {}", fmt_bytes(mem_usage), fmt_bytes(mem_limit)))
            .style(Style::default().fg(theme.main_fg));
        f.render_widget(label, chunks[0]);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(theme.graph_color).bg(theme.selected_bg))
            .ratio(mem_percent / 100.0)
            .label(format!("{:.1}%", mem_percent));
        f.render_widget(gauge, chunks[1]);

        // Cache
        if cache > 0.0 {
             let cache_label = Paragraph::new(format!("Cache: {}", fmt_bytes(cache)))
                .style(Style::default().fg(theme.inactive_fg));
             f.render_widget(cache_label, chunks[2]);
        }

        // Swap
        // Note: If swap is 0, it might mean disabled or not used.
        let swap_label = Paragraph::new(format!("Swap: {}", fmt_bytes(swap)))
            .style(Style::default().fg(theme.inactive_fg));
        f.render_widget(swap_label, chunks[3]);
    }
}

fn draw_network_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" NET ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.inactive_fg));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner);

    // Graph
    let datasets = vec![
        Dataset::default()
            .name("RX")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&app.net_rx_history),
        Dataset::default()
            .name("TX")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Red))
            .data(&app.net_tx_history),
    ];

    let max_val = app.net_rx_history.iter().chain(app.net_tx_history.iter())
        .map(|(_, v)| *v)
        .fold(0.0, f64::max);
    let y_max = if max_val > 100.0 { max_val * 1.1 } else { 100.0 };

    let chart = Chart::new(datasets)
        .block(Block::default().borders(Borders::NONE))
        .x_axis(Axis::default().style(Style::default().fg(theme.graph_text)).bounds(app.net_axis_bounds))
        .y_axis(Axis::default().style(Style::default().fg(theme.graph_text)).bounds([0.0, y_max]));
    
    f.render_widget(chart, chunks[0]);

    // Aquarium Animation
    let width = 30; // Approximate width of the aquarium area in chars
    let height = 5; // Height of the aquarium area
    
    let mut aquarium_lines = vec![String::from("                              "); height];

    // Draw fishes
    for fish in &app.fishes {
        if fish.y > 0 && fish.y < height - 1 {
            let fish_char = if fish.direction > 0.0 { "><>" } else { "<><" };
            let x = fish.x as usize;
            if x < width - 3 {
                // Simple overlay
                let line = &mut aquarium_lines[fish.y];
                // Ensure we don't panic if string is short (though we init with spaces)
                if line.len() >= x + 3 {
                    line.replace_range(x..x+3, fish_char);
                }
            }
            
            // Bubble
            if (fish.x as usize) % 4 == 0 {
                 let bubble_y = fish.y.saturating_sub(1);
                 if bubble_y > 0 {
                     let line = &mut aquarium_lines[bubble_y];
                     let bx = x.saturating_sub(1);
                     if bx < width && bx > 0 {
                         line.replace_range(bx..bx+1, "o");
                     }
                 }
            }
        }
    }

    let aquarium_text: Vec<Line> = aquarium_lines.iter().enumerate().map(|(i, s)| {
        if i == 0 || i == 4 {
            Line::from(Span::styled(s, Style::default().fg(Color::Blue)))
        } else {
             // We need to color the fish differently than the background spaces
             // But for simplicity in this text widget, let's just color the whole line cyan for now
             // Or better, parse the string and colorize fish parts. 
             // Since we constructed a string, we lost the object info. 
             // Let's just print the string with the fish color, and maybe bubbles white?
             // To do it properly we'd need to build a Vec<Span>.
             
             let mut spans: Vec<Span> = vec![];
             
             for (_idx, c) in s.char_indices() {
                 if c == '<' || c == '>' {
                     spans.push(Span::styled(c.to_string(), Style::default().fg(theme.fish_color)));
                 } else if c == 'o' {
                     spans.push(Span::styled(c.to_string(), Style::default().fg(Color::White)));
                 } else {
                     spans.push(Span::raw(c.to_string()));
                 }
             }
             Line::from(spans)
        }
    }).collect();
    
    let aquarium = Paragraph::new(aquarium_text)
        .block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(theme.inactive_fg)))
        .alignment(ratatui::layout::Alignment::Left); // Left align to match our grid
        
    f.render_widget(aquarium, chunks[1]);
}



fn draw_container_section(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(area);

    draw_container_table(f, app, chunks[0], theme);
    draw_container_sidebar(f, app, chunks[1], theme);
}

fn draw_container_table(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let header_cells = ["State", "ID", "Name", "Image", "Status"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.title).add_modifier(Modifier::BOLD)));
    
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.main_bg))
        .height(1)
        .bottom_margin(1);

    let rows = app.containers.iter().map(|c| {
        let (icon, color) = if c.state == "running" {
            ("●", theme.graph_color)
        } else {
            ("○", theme.inactive_fg)
        };

        let cells = vec![
            Cell::from(Span::styled(format!(" {} ", icon), Style::default().fg(color))),
            Cell::from(Span::styled(&c.id[..12], Style::default().fg(theme.inactive_fg))),
            Cell::from(Span::styled(c.names.first().map(|s| s.trim_start_matches('/')).unwrap_or(""), Style::default().fg(theme.main_fg).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled(&c.image, Style::default().fg(theme.main_fg))),
            Cell::from(Span::styled(&c.status, Style::default().fg(theme.inactive_fg))),
        ];
        Row::new(cells).height(1)
    });

    let table = Table::new(rows, [
        Constraint::Length(4),
        Constraint::Length(14),
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(theme.border)).title(" CONTAINERS "))
    .highlight_style(Style::default().bg(theme.selected_bg).fg(theme.selected_fg).add_modifier(Modifier::BOLD))
    .highlight_symbol("▎");

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.selected_index));

    f.render_stateful_widget(table, area, &mut state);
}

fn draw_container_sidebar(f: &mut Frame, _app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .title(" INFO ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    let text = vec![
        Line::from(Span::styled("Menu", Style::default().fg(theme.title))),
        Line::from(""),
        Line::from(Span::styled("r: Restart", Style::default().fg(theme.inactive_fg))),
        Line::from(Span::styled("s: Stop", Style::default().fg(theme.inactive_fg))),
        Line::from(Span::styled("u: Start", Style::default().fg(theme.inactive_fg))),
        Line::from(""),
        Line::from(Span::styled("q: Quit", Style::default().fg(theme.inactive_fg))),
    ];
    let p = Paragraph::new(text).wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}

fn draw_logs_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" LOGS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![];
    if app.is_loading_details {
        lines.push(Line::from(Span::styled(" Loading logs...", Style::default().fg(theme.inactive_fg))));
    } else {
        for log in &app.logs {
            let style = if log.contains("ERROR") || log.contains("ERR") {
                Style::default().fg(Color::Red)
            } else if log.contains("WARN") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(theme.graph_text)
            };
            lines.push(Line::from(Span::styled(log, style)));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner_area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let status_text = if let Some((msg, _)) = &app.action_status {
        Span::styled(format!(" {} ", msg), Style::default().bg(theme.title).fg(Color::Black))
    } else {
        Span::raw("")
    };

    let keys = Span::styled(
        " j/k: Nav • r: Restart • s: Stop • u: Start • q: Quit",
        Style::default().fg(theme.inactive_fg),
    );

    let line = Line::from(vec![status_text, Span::raw(" "), keys]);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

pub fn calculate_cpu_usage(stats: &ContainerStats, prev_stats: &Option<ContainerStats>) -> f64 {
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
