pub mod monitor;
pub mod containers;
pub mod charts;
pub mod logs;
pub mod footer;
pub mod tools;
pub mod util;

pub use util::calculate_cpu_usage;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    style::{Style, Modifier},
    text::{Line, Span},
    Frame,
};
use crate::app::App;
use crate::config::Theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = &app.config.theme_data;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Monitor (CPU, Mem, Net)
            Constraint::Min(10),    // Main Content (Containers + Tools)
            Constraint::Length(10), // Bottom (Charts + Logs)
            Constraint::Length(3),  // Footer (Management + Shortcuts)
        ])
        .split(f.size());

    // 1. Top Monitor Panel
    monitor::draw(f, app, chunks[0], theme);

    // 2. Main Content (Containers + Tools)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Containers
            Constraint::Percentage(40), // Tools
        ])
        .split(chunks[1]);

    containers::draw(f, app, main_chunks[0], theme);
    tools::draw(f, app, main_chunks[1], theme);

    // 3. Bottom Content (Charts + Logs)
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Charts (Net Traffic, Storage IO)
            Constraint::Percentage(60), // Logs
        ])
        .split(chunks[2]);

    charts::draw(f, app, bottom_chunks[0], theme);
    logs::draw(f, app, bottom_chunks[1], theme);
    footer::draw(f, app, chunks[3], theme);

    // 5. Wizard Overlay (Focus Mode)
    if let Some(wizard) = &app.wizard {
        let area = centered_rect(80, 80, f.size());
        f.render_widget(ratatui::widgets::Clear, area); 
        draw_wizard(f, wizard, area, theme);
    }

    // 6. Toast Notifications (Top-Right)
    if let Some((msg, time)) = &app.action_status {
        if time.elapsed().as_secs() < 5 {
            let toast_width = 40;
            let toast_height = 3;
            let size = f.size();
            let area = Rect::new(
                size.width.saturating_sub(toast_width + 2), // Top Right with padding
                1, 
                toast_width, 
                toast_height
            );
            
            f.render_widget(ratatui::widgets::Clear, area);
            
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" Notification ")
                .border_style(Style::default().fg(theme.running).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(theme.background));
            
            let text = Paragraph::new(msg.as_str())
                .block(block)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(theme.foreground).add_modifier(Modifier::BOLD));
            
            f.render_widget(text, area);
        }
    }
}

// Helper to center rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}



// Re-implement draw_wizard here or move it to a separate module if it gets too large
// For now, keeping it here as it was in the original ui.rs, but updated to use the new style
fn draw_wizard(f: &mut Frame, wizard: &crate::wizard::models::WizardState, area: Rect, theme: &Theme) {
    // Use the provided area (Tools panel) directly
    // let area = centered_rect(90, 90, area); // Removed overlay logic
    
    // Clear the area behind the modal
    // f.render_widget(Clear, area); // Not needed as it's a panel now

    let title = if matches!(wizard.step, crate::wizard::models::WizardStep::ModeSelection { .. }) {
        " WIZARD - SELECT MODE "
    } else {
        " WIZARD "
    };

    // Main Block with distinct style
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Thick)
        .border_style(Style::default().fg(theme.selection_bg))
        .style(Style::default().bg(theme.background))
        .title(Span::styled(title, Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    match &wizard.step {
        crate::wizard::models::WizardStep::ModeSelection { selected_index } => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(inner);

            let options = [
                (">_ Quick Pull & Run", "Pull from registry and run immediately", "Launch a container directly from a Docker Hub image.\n\nIdeal for:\n- Redis/MySQL instances\n- Testing tools\n- Quick experiments"),
                ("./ Build from Source", "Build Dockerfile from local directory", "Detects your project type (Node, Python, Go, etc.) and generates a Dockerfile automatically.\n\nSupports:\n- Smart Framework Detection\n- Auto-Port Mapping\n- Multi-stage builds"),
                ("{} Docker Compose", "Run docker-compose.yml project", "Manage multi-container applications defined in docker-compose.yml.\n\nFeatures:\n- Service Selection\n- Resource Limits Override\n- Environment Variable Management"),
                (" Janitor", "Clean up unused resources", "Scan and remove unused images, stopped containers, and dangling volumes to free up disk space."),
                ("⚙ Settings", "Configure application", "Adjust DockTop preferences, themes, and update settings."),
            ];
            
            let items: Vec<ListItem> = options
                .iter()
                .enumerate()
                .map(|(i, (title, desc, _))| {
                    let style = if i == *selected_index {
                        Style::default().fg(theme.selection_fg).bg(theme.selection_bg).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.foreground)
                    };
                    
                    let content = vec![
                        Line::from(Span::styled(*title, style)),
                        Line::from(Span::styled(format!("   {}", desc), if i == *selected_index { style } else { Style::default().fg(theme.border) })),
                        Line::from(""),
                    ];
                    ListItem::new(content)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(theme.border)))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            
            f.render_widget(list, chunks[0]);

            // Right side details
            let (_, _, detail_text) = options[*selected_index];
            let details = Paragraph::new(detail_text)
                .block(Block::default().borders(Borders::NONE).padding(ratatui::widgets::Padding::new(2, 2, 1, 1)))
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(theme.foreground));
            
            f.render_widget(details, chunks[1]);
        },
        crate::wizard::models::WizardStep::QuickRunInput { image, name, ports, env, cpu, memory, restart, show_advanced, focused_field, editing_id, port_status, profile } => {
            let title = if editing_id.is_some() { "Edit Container" } else { "Quick Pull & Run" };
            
            // Split into Left (Form) and Right (Preview/Help)
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(inner);

            // Left Side: Form
            let mut constraints = vec![Constraint::Length(1)]; // Title
            constraints.push(Constraint::Length(3)); // Image
            constraints.push(Constraint::Length(3)); // Name
            constraints.push(Constraint::Length(3)); // Ports
            constraints.push(Constraint::Length(6)); // Env (Bigger!)
            
            if *show_advanced {
                constraints.extend(vec![Constraint::Length(3); 4]); // Advanced fields
            }
            constraints.push(Constraint::Min(1)); // Spacer

            let left_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(main_chunks[0]);

            let title_p = Paragraph::new(title).style(Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, left_chunks[0]);

            let mut fields = vec![
                ("Image Name", image),
                ("Container Name", name),
                ("Ports (host:container)", ports),
                ("Env Vars (KEY=VAL, Space separated)", env),
            ];

            let profile_str = profile.display_name().to_string();

            if *show_advanced {
                fields.push(("Resource Profile (Space to cycle)", &profile_str));
                fields.push(("CPU Limit (e.g. 0.5)", cpu));
                fields.push(("Memory Limit (e.g. 512m)", memory));
                fields.push(("Restart Policy (Space to cycle)", restart));
            }

            for (i, (label, value)) in fields.iter().enumerate() {
                let style = if *focused_field == i {
                    Style::default().fg(theme.selection_fg).bg(theme.selection_bg)
                } else {
                    Style::default().fg(theme.border)
                };
                let mut title_text = label.to_string();
                if i == 2 { // Ports field
                    match port_status {
                        crate::wizard::models::PortStatus::Available => title_text.push_str(" [OK]"),
                        crate::wizard::models::PortStatus::Occupied(who) => title_text.push_str(&format!(" [BUSY: {}]", who)),
                        crate::wizard::models::PortStatus::Invalid => title_text.push_str(" [INVALID]"),
                        _ => {}
                    }
                }

                let p = Paragraph::new(value.as_str())
                    .block(Block::default().borders(Borders::ALL).title(title_text).border_style(style))
                    .wrap(Wrap { trim: true }); // Enable wrapping
                f.render_widget(p, left_chunks[i+1]);
            }

            // Right Side: Live Preview & Help
            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(1),    // Command Preview
                    Constraint::Length(3), // Help
                ])
                .split(main_chunks[1]);

            // Construct command for preview
            let mut cmd = format!("docker run -d --name {}", if name.is_empty() { "<name>" } else { name });
            if !ports.is_empty() { cmd.push_str(&format!(" -p {}", ports)); }
            if !env.is_empty() { cmd.push_str(&format!(" -e {}", env)); }
            if !cpu.is_empty() { cmd.push_str(&format!(" --cpus {}", cpu)); }
            if !memory.is_empty() { cmd.push_str(&format!(" --memory {}", memory)); }
            if !restart.is_empty() && restart != "no" { cmd.push_str(&format!(" --restart {}", restart)); }
            cmd.push_str(&format!(" {}", if image.is_empty() { "<image>" } else { image }));

            let preview_block = Block::default()
                .borders(Borders::ALL)
                .title(" Live Command Preview ")
                .border_style(Style::default().fg(theme.border));
            
            let preview_text = Paragraph::new(cmd)
                .block(preview_block)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(theme.foreground));
            
            f.render_widget(preview_text, right_chunks[1]);

            let mut help_text = if *show_advanced {
                "ENTER: Create | TAB: Next Field | SPACE: Cycle Options".to_string()
            } else {
                "ENTER: Create | TAB: Next Field | Ctrl+A: Advanced Options".to_string()
            };

            // Smart Hints for Databases
            let img_lower = image.to_lowercase();
            if img_lower.contains("mysql") || img_lower.contains("mariadb") {
                help_text.push_str("\n\n[!] Hint: Set MYSQL_ROOT_PASSWORD=... in Env");
            } else if img_lower.contains("postgres") {
                help_text.push_str("\n\n[!] Hint: Set POSTGRES_PASSWORD=... in Env");
            } else if img_lower.contains("mongo") {
                help_text.push_str("\n\n[!] Hint: Set MONGO_INITDB_ROOT_USERNAME=... in Env");
            }

            let help = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title(" Help "))
                .style(Style::default().fg(theme.border));
            f.render_widget(help, right_chunks[2]);

        },
        crate::wizard::models::WizardStep::Preview { title, content, action: _, previous_step: _ } => {
             let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Min(1),    // Content
                    Constraint::Length(1), // Help
                ])
                .split(inner);

            let title_p = Paragraph::new(title.as_str()).style(Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            let content_p = Paragraph::new(content.as_str())
                .block(Block::default().borders(Borders::ALL).title(" Preview ").border_style(Style::default().fg(theme.border)))
                .wrap(Wrap { trim: false });
            f.render_widget(content_p, chunks[1]);

            let help = Paragraph::new("ENTER: Confirm & Execute | ESC: Back")
                .style(Style::default().fg(theme.border).add_modifier(Modifier::ITALIC));
            f.render_widget(help, chunks[2]);
        },
        _ => {
            // Fallback for other steps (FileBrowser, etc.) - simplified for now
            let p = Paragraph::new("This step is not yet fully redesigned. Press ESC to go back.")
                .style(Style::default().fg(theme.foreground));
            f.render_widget(p, inner);
        }
    }
}
