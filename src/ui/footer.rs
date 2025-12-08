use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph},
    Frame,
};
use crate::app::App;
use crate::config::Theme;

pub fn draw(f: &mut Frame, _app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain);
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = vec![
        Line::from(vec![
            Span::styled("MANAGEMENT: ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::raw("[C] Create [E] Edit [6] Shell [S] Rebuild [O] DB CLI  |  "),
            Span::styled("ACTIONS: ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::raw("[R] Restart [S] Stop [V] Start [y] YAML [Del] Delete [Enter] Details"),
        ]),
        Line::from(vec![
            Span::styled("ADVANCED COMMANDS SHORTCUTS: ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::raw("[Tab] Tools [?] Help [q] Quit"),
        ]),
    ];

    let p = Paragraph::new(text).style(Style::default().fg(theme.foreground));
    f.render_widget(p, inner);
}
