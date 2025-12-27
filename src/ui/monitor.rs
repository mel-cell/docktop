use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Gauge, Paragraph, Sparkline},
    Frame,
};
use crate::app::App;
use crate::config::Theme;
use sysinfo::System;

pub fn draw(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(theme.header_fg))
        .title(Span::styled(" SYSTEM DASHBOARD ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33), // CPU & Load
            Constraint::Percentage(33), // Memory & Storage
            Constraint::Percentage(34), // Network & General
        ])
        .split(inner);

    draw_cpu_panel(f, app, chunks[0], theme);
    draw_memory_panel(f, app, chunks[1], theme);
    draw_network_panel(f, app, chunks[2], theme);
}

fn draw_cpu_panel(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(2)])
        .split(area);

    f.render_widget(Paragraph::new("CPU Usage History").style(Style::default().fg(theme.chart_mid).add_modifier(Modifier::BOLD)), chunks[0]);

    let cpu_data: Vec<u64> = app.cpu_history.iter().map(|&(x, _)| x as u64).collect();
    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(theme.chart_low))
        .data(&cpu_data)
        .bar_set(ratatui::symbols::bar::NINE_LEVELS);
    f.render_widget(sparkline, chunks[1]);
    
    let cpu_usage = app.cpu_history.last().map(|&(x, _)| x).unwrap_or(0.0);
    let stats = format!("Core Load: {:.1}% | Threads: {} | Load Avg: {:.2}", cpu_usage, app._system.cpus().len(), System::load_average().one);
    f.render_widget(Paragraph::new(stats).style(Style::default().fg(theme.foreground)), chunks[2]);
}

fn draw_memory_panel(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), 
            Constraint::Length(1), // Gauge
            Constraint::Length(1), // Spacer
            Constraint::Length(1), 
            Constraint::Length(1), // Gauge
        ])
        .split(area);

    // RAM
    let total_mem = app._system.total_memory();
    let used_mem = app._system.used_memory();
    let mem_ratio = used_mem as f64 / total_mem as f64;

    f.render_widget(Paragraph::new("Memory Usage").style(Style::default().fg(theme.memory_chart).add_modifier(Modifier::BOLD)), chunks[0]);
    let ram_gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(theme.memory_chart).bg(theme.header_bg))
        .ratio(mem_ratio)
        .label(format!("{:.1}GB / {:.1}GB", used_mem as f64 / 1024.0_f64.powi(3), total_mem as f64 / 1024.0_f64.powi(3)));
    f.render_widget(ram_gauge, chunks[1]);

    // Swap / Storage (Mocked relative to swap for visual balance)
    let total_swap = app._system.total_swap();
    let used_swap = app._system.used_swap();
    let swap_ratio = if total_swap > 0 { used_swap as f64 / total_swap as f64 } else { 0.0 };

    f.render_widget(Paragraph::new("Swap Usage").style(Style::default().fg(theme.chart_mid).add_modifier(Modifier::BOLD)), chunks[3]);
    let swap_gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(theme.chart_mid).bg(theme.header_bg))
        .ratio(swap_ratio)
        .label(format!("{:.1}GB / {:.1}GB", used_swap as f64 / 1024.0_f64.powi(3), total_swap as f64 / 1024.0_f64.powi(3)));
    f.render_widget(swap_gauge, chunks[4]);
}

fn draw_network_panel(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let rx = app.net_rx_history.last().map(|&(_, y)| y).unwrap_or(0.0);
    let tx = app.net_tx_history.last().map(|&(_, y)| y).unwrap_or(0.0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    
    f.render_widget(Paragraph::new("Network & IO").style(Style::default().fg(theme.network_rx).add_modifier(Modifier::BOLD)), chunks[0]);

    let text = vec![
        Line::from(vec![Span::styled("⬇ RX Stream: ", Style::default().fg(theme.network_rx)), Span::raw(format!("{:.1} kB/s", rx))]),
        Line::from(vec![Span::styled("⬆ TX Stream: ", Style::default().fg(theme.network_tx)), Span::raw(format!("{:.1} kB/s", tx))]),
        Line::from(""),
        Line::from(vec![Span::raw("Uptime: "), Span::styled(format!("{} s", System::uptime()), Style::default().fg(theme.foreground))]),
    ];
    
    f.render_widget(Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: true }), chunks[1]);
}
