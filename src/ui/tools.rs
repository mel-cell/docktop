use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph},
    Frame,
};
use crate::app::App;
use crate::config::Theme;

pub fn draw(f: &mut Frame, _app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" TOOLS ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // ASCII Art
            Constraint::Length(3), // Search
            Constraint::Length(6), // Filter Help
        ])
        .split(inner);

    // ASCII Art (Abstract)
    let art = vec![
        "      .---.      ",
        "     /     \\     ",
        "    | () () |    ",
        "     \\  ^  /     ",
        "      |||||      ",
        "      |||||      ",
    ];
    let p_art = Paragraph::new(art.join("\n"))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(theme.chart_high));
    f.render_widget(p_art, chunks[0]);

    // Search Box
    let search_block = Block::default()
        .borders(Borders::ALL)
        .title(" Search ")
        .border_style(Style::default().fg(theme.border));
    let search_p = Paragraph::new("Filter: ...")
        .block(search_block)
        .style(Style::default().fg(theme.foreground));
    f.render_widget(search_p, chunks[1]);

    // Filter Help
    let help_text = vec![
        Line::from(Span::styled("[^] next/match", Style::default().fg(theme.border))),
        Line::from(Span::styled("[/] Listening", Style::default().fg(theme.border))),
        Line::from(Span::styled("[+] minutes map", Style::default().fg(theme.border))),
        Line::from(Span::styled("[-] Match defaults", Style::default().fg(theme.border))),
    ];
    let help_p = Paragraph::new(help_text).style(Style::default().fg(theme.border));
    f.render_widget(help_p, chunks[2]);
}
