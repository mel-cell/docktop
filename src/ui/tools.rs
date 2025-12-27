use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph, Wrap},
    Frame,
};
use crate::app::App;
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" TOOLS & FILTERS ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search Bar
            Constraint::Length(1), // Spacer
            Constraint::Min(1),    // Active Filters / Quick Help
        ])
        .split(inner);

    // 1. Search Bar
    let search_style = if app.is_typing_filter {
        Style::default().fg(theme.running).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.border)
    };
    
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Filter Containers (/) ")
        .border_style(search_style);

    let query_text = if app.filter_query.is_empty() {
        if app.is_typing_filter { "Type to filter..." } else { "Press '/' to search" }
    } else {
        &app.filter_query
    };
    
    f.render_widget(Paragraph::new(query_text).block(search_block).style(Style::default().fg(theme.foreground)), chunks[0]);

    // 2. Quick Actions Grid
    let help_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(chunks[2]);
        
    f.render_widget(Paragraph::new("Quick Actions").style(Style::default().fg(theme.header_fg).add_modifier(Modifier::UNDERLINED)), help_chunks[0]);
    
    let actions = vec![
        Line::from(vec![Span::styled(" [w] ", Style::default().fg(theme.selection_bg)), Span::raw("Wizard Mode")]),
        Line::from(vec![Span::styled(" [e] ", Style::default().fg(theme.selection_bg)), Span::raw("Shell Exec")]),
        Line::from(vec![Span::styled(" [^] ", Style::default().fg(theme.selection_bg)), Span::raw("Sort List")]),
        Line::from(vec![Span::styled(" [x] ", Style::default().fg(theme.selection_bg)), Span::raw("Delete")]),
        Line::from(vec![Span::styled(" [v] ", Style::default().fg(theme.selection_bg)), Span::raw("Start")]),
        Line::from(vec![Span::styled(" [s] ", Style::default().fg(theme.selection_bg)), Span::raw("Stop")]),
    ];
    
    f.render_widget(Paragraph::new(actions).wrap(Wrap { trim: true }), help_chunks[1]);
}
