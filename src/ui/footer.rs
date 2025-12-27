use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph},
    Frame,
};
use crate::app::App;
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_type(BorderType::Plain);
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let k = &app.config.keys;

    // Helper to format key
    let fmt = |k: &str, d: &str| format!("[{}] {}", k.to_uppercase(), d);

    let management = format!("{} {} {} {} | ", 
        fmt(&k.toggle_wizard, "Wizard"),
        fmt(&k.edit, "Edit"),
        fmt(&k.shell, "Shell"),
        fmt(&k.db_cli, "DB CLI")
    );

    let actions = format!("{} {} {} {} {} {}", 
        fmt(&k.restart, "Restart"),
        fmt(&k.stop, "Stop"),
        fmt(&k.start, "Start"),
        fmt(&k.yaml, "YAML"),
        fmt(&k.delete, "Delete"),
        fmt(&k.enter, "Details")
    );

    let general = format!("{} {} {} {} {}", 
        fmt("Tab", "Tools"),
        fmt("/", "Filter"), // Added Filter shortcut
        fmt(&k.refresh, "Refresh"),
        fmt(&k.toggle_help, "Help"),
        fmt(&k.quit, "Quit")
    );

    let text = vec![
        Line::from(vec![
            Span::styled("MANAGEMENT: ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::raw(management),
            Span::styled("ACTIONS: ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::raw(actions),
        ]),
        Line::from(vec![
            Span::styled("GENERAL: ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)),
            Span::raw(general),
        ]),
    ];

    let p = Paragraph::new(text).style(Style::default().fg(theme.foreground));
    f.render_widget(p, inner);
}
