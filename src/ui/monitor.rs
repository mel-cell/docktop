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
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // CPU
            Constraint::Percentage(30), // Memory & Storage
            Constraint::Percentage(30), // Net
        ])
        .split(area);

    draw_cpu(f, app, chunks[0], theme);
    draw_memory_storage(f, app, chunks[1], theme);
    draw_net(f, app, chunks[2], theme);
}

fn draw_cpu(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" MONITOR - CPU ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner);

    // CPU Sparkline
    let cpu_data: Vec<u64> = app.cpu_history.iter().map(|&(x, _)| x as u64).collect();
    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(theme.chart_low))
        .data(&cpu_data)
        .bar_set(ratatui::symbols::bar::NINE_LEVELS);
    f.render_widget(sparkline, chunks[0]);

    // CPU Stats
    let cpu_usage = app.cpu_history.last().map(|&(x, _)| x).unwrap_or(0.0);
    let text = vec![
        Line::from(vec![Span::raw("CPU:    "), Span::styled(format!("{:.1}%", cpu_usage), Style::default().fg(if cpu_usage > 80.0 { theme.chart_high } else { theme.foreground }))]),
        Line::from(vec![Span::raw("Cores:  "), Span::raw(format!("{}", app._system.cpus().len()))]),
        Line::from(vec![Span::raw("Load:   "), Span::raw(format!("{:.2}", System::load_average().one))]),
    ];
    let p = Paragraph::new(text).style(Style::default().fg(theme.foreground));
    f.render_widget(p, chunks[1]);
}

fn draw_memory_storage(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Memory
    let mem_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" MEMORY ");
    let mem_inner = mem_block.inner(chunks[0]);
    f.render_widget(mem_block, chunks[0]);

    let total_mem = app._system.total_memory();
    let used_mem = app._system.used_memory();
    let mem_ratio = used_mem as f64 / total_mem as f64;
    
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(theme.chart_mid).bg(theme.background))
        .ratio(mem_ratio)
        .label(format!("{:.1} / {:.1} GB", used_mem as f64 / 1024.0/1024.0/1024.0, total_mem as f64 / 1024.0/1024.0/1024.0));
    f.render_widget(gauge, mem_inner);

    // Storage
    let store_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" STORAGE ");
    let store_inner = store_block.inner(chunks[1]);
    f.render_widget(store_block, chunks[1]);

    // Mock storage data for now as sysinfo disk usage can be heavy to query constantly
    let gauge_store = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(theme.chart_low).bg(theme.background))
        .ratio(0.45)
        .label("56 / 125 GB");
    f.render_widget(gauge_store, store_inner);
}

fn draw_net(f: &mut Frame, _app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" NET ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(vec![Span::raw("RX: "), Span::styled("0.0 kB/s", Style::default().fg(theme.foreground))]),
        Line::from(vec![Span::raw("TX: "), Span::styled("0.0 kB/s", Style::default().fg(theme.foreground))]),
        Line::from(""),
        Line::from(Span::styled("No Activity", Style::default().fg(theme.stopped).add_modifier(Modifier::ITALIC))),
    ];
    let p = Paragraph::new(text).style(Style::default().fg(theme.foreground));
    f.render_widget(p, inner);
}
