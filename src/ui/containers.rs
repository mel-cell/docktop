use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, BorderType, Cell, Row, Table, TableState},
    Frame,
};
use crate::app::App;
use crate::config::Theme;
use crate::theme::icons::IconSet;

pub fn draw(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" CONTAINERS ");
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let header_cells = ["State", "ID", "Name", "Image", "IP", "Status", "Ports"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1)
        .bottom_margin(1);

    let rows = app.containers.iter().map(|c| {
        let state_icon = if c.state == "running" {
            Span::styled(IconSet::CONTAINER, Style::default().fg(theme.running))
        } else {
            Span::styled(IconSet::CONTAINER, Style::default().fg(theme.stopped))
        };

        let cells = vec![
            Cell::from(state_icon),
            Cell::from(c.id.chars().take(12).collect::<String>()),
            Cell::from(c.names.join(", ")),
            Cell::from(c.image.clone()),
            Cell::from("127.0.0.1"), // Mock IP for now, actual IP needs inspection
            Cell::from(c.status.clone()),
            Cell::from(c.ports.iter().map(|p| format!("{}:{}", p.public_port.unwrap_or(0), p.private_port)).collect::<Vec<_>>().join(", ")),
        ];
        Row::new(cells).height(1).style(Style::default().fg(theme.foreground))
    });

    let t = Table::new(rows, [
        Constraint::Length(3),
        Constraint::Length(12),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Length(15),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
    ])
    .header(header)
    .highlight_style(Style::default().fg(theme.selection_fg).bg(theme.selection_bg).add_modifier(Modifier::BOLD));
    
    // We need a mutable reference to TableState, but app.state is immutable here.
    // In a real refactor we'd pass &mut TableState. For now, we clone the state or use what we have.
    // The main draw loop usually handles the state.
    // Let's assume we can't modify state here easily without changing signature.
    // We will construct a temporary state from app.selected_index
    let mut state = TableState::default();
    state.select(Some(app.selected_index));
    
    f.render_stateful_widget(t, inner, &mut state);
}
