use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph, Sparkline},
    Frame,
};
use crate::app::App;
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_network_traffic(f, app, chunks[0], theme);
    draw_storage_io(f, app, chunks[1], theme);
}

fn draw_network_traffic(f: &mut Frame, _app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" NETWORK TRAFFIC ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Mock data for visualization
    let data = [2, 4, 6, 8, 5, 3, 2, 5, 8, 9, 4, 2, 1, 4, 6, 3, 2];
    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(theme.chart_mid))
        .data(&data)
        .bar_set(ratatui::symbols::bar::NINE_LEVELS);
    
    f.render_widget(sparkline, inner);
}

fn draw_storage_io(f: &mut Frame, _app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" STORAGE IO ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(vec![Span::raw("Read:  "), Span::styled("0 MiB", Style::default().fg(theme.foreground))]),
        Line::from(vec![Span::raw("Write: "), Span::styled("2.0 KiB/s", Style::default().fg(theme.foreground))]),
        Line::from(""),
        Line::from(vec![Span::raw("Cache: "), Span::styled("8 MiB", Style::default().fg(theme.foreground))]),
    ];
    let p = Paragraph::new(text).style(Style::default().fg(theme.foreground));
    f.render_widget(p, inner);
}
