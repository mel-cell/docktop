use ratatui::{
    layout::Rect,
    style::Style,
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
        .title(" LOGS ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let logs: Vec<Line> = app.logs
        .iter()
        .map(|log| Line::from(Span::raw(log)))
        .collect();

    let p = Paragraph::new(logs)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme.foreground));
        //.scroll((app.scroll_offset as u16, 0));
    
    f.render_widget(p, inner);
}
