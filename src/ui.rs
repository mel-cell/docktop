use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Paragraph, Wrap, Table, Row, Cell, Chart, Dataset, Axis, GraphType, List, ListItem, Clear, Sparkline, block::{Title, Position}},
    symbols,
    Frame,
};
use crate::app::App;
use crate::docker::ContainerStats;
use crate::config::Theme;
use crate::theme::icons::IconSet;

pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = app.config.theme_data.clone();
    let theme = &theme;
    
    // Main Layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Title Bar
            Constraint::Length(11), // Monitor (12 - 1)
            Constraint::Min(0),     // Container List
            Constraint::Length(10), // Bottom: Logs
            Constraint::Length(1),  // Footer
        ])
        .split(f.size());

    draw_title_bar(f, app, chunks[0]); // Pass app
    draw_monitor_section(f, app, chunks[1], theme);
    draw_container_section(f, app, chunks[2], theme);
    draw_logs_section(f, app, chunks[3], theme);
    draw_footer(f, app, chunks[4], theme);

    if app.show_details {
        draw_details_popup(f, app, f.size(), theme);
    }
}

fn draw_title_bar(f: &mut Frame, _app: &App, area: Rect) {
    let host_name = sysinfo::System::host_name().unwrap_or_else(|| "Unknown".to_string());
    let uptime = sysinfo::System::uptime();
    let uptime_str = format!("{:02}:{:02}:{:02}", uptime / 3600, (uptime % 3600) / 60, uptime % 60);
    
    let text = format!(" DockTop v0.1.0 | Host: {} | Uptime: {} ", host_name, uptime_str);
    
    let title = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(title, area);
}

fn draw_monitor_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(" MONITOR ", Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Split into 3 columns: CPU (40%), Memory (30%), Network (30%)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(inner_area);

    draw_cpu_section(f, app, chunks[0], theme);
    draw_memory_section(f, app, chunks[1], theme);
    draw_network_section(f, app, chunks[2], theme);
}

fn draw_cpu_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" CPU ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(stats) = &app.current_stats {
        let cpu = calculate_cpu_usage(stats, &app.previous_stats);
        let num_cpus = stats.cpu_stats.cpu_usage.percpu_usage.as_ref().map(|v| v.len()).unwrap_or(1);
        
        let label = if num_cpus > 1 {
            format!("Total ({} Cores): {:.1}%", num_cpus, cpu)
        } else {
            format!("Usage: {:.1}%", cpu)
        };

        let last_val = app.cpu_history.last().map(|(_, v)| *v).unwrap_or(0.0);
        let color = if last_val < 50.0 {
            theme.cpu_low
        } else if last_val < 80.0 {
            theme.cpu_mid
        } else {
            theme.cpu_high
        };

        let datasets = vec![
            Dataset::default()
                .name(label)
                .marker(if app.config.general.show_braille { symbols::Marker::Braille } else { symbols::Marker::Dot })
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(&app.cpu_history),
        ];

        let chart = Chart::new(datasets)
            .block(Block::default().borders(Borders::NONE))
            .x_axis(Axis::default().style(Style::default().fg(theme.foreground)).bounds(app.x_axis_bounds))
            .y_axis(Axis::default().style(Style::default().fg(theme.foreground)).bounds([0.0, 100.0]));
        
        f.render_widget(chart, inner);
    }
}

fn draw_memory_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" MEM ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(stats) = &app.current_stats {
        let mem_usage = stats.memory_stats.usage.unwrap_or(0) as f64;
        let mem_limit = stats.memory_stats.limit.unwrap_or(0) as f64;
        
        // Extract Cache and Swap
        let (cache, swap) = if let Some(details) = &stats.memory_stats.stats {
            let c = *details.get("cache").or(details.get("total_cache")).unwrap_or(&0) as f64;
            let s = *details.get("swap").unwrap_or(&0) as f64;
            (c, s)
        } else {
            (0.0, 0.0)
        };

        // Helper to format bytes
        let fmt_bytes = |b: f64| -> String {
            if b > 1024.0 * 1024.0 * 1024.0 {
                format!("{:.2} GiB", b / 1024.0 / 1024.0 / 1024.0)
            } else {
                format!("{:.0} MiB", b / 1024.0 / 1024.0)
            }
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Main Usage
                Constraint::Length(1), // Stacked Bar
                Constraint::Length(1), // Legend/Details
            ])
            .split(inner);

        // RAM Usage Text
        let label = Paragraph::new(format!("RAM: {} / {}", fmt_bytes(mem_usage), fmt_bytes(mem_limit)))
            .style(Style::default().fg(theme.foreground));
        f.render_widget(label, chunks[0]);

        // Stacked Bar
        if mem_limit > 0.0 {
            let width = chunks[1].width as f64;
            // Real used (without cache) usually? Docker stats usage includes cache sometimes depending on version.
            // But here we treat 'mem_usage' as total used reported by docker.
            // If we want to separate cache, we should subtract it if usage includes it.
            // For simplicity, let's assume mem_usage includes cache.
            let real_used = (mem_usage - cache).max(0.0);
            
            let used_ratio = real_used / mem_limit;
            let cache_ratio = cache / mem_limit;
            // Ensure they don't exceed 1.0
            let _total_ratio = (used_ratio + cache_ratio).min(1.0);

            let used_chars = (used_ratio * width).round() as usize;
            let cache_chars = (cache_ratio * width).round() as usize;
            let free_chars = (width as usize).saturating_sub(used_chars + cache_chars);

            let bar = Line::from(vec![
                Span::styled(" ".repeat(used_chars), Style::default().bg(theme.memory_chart)),
                Span::styled(" ".repeat(cache_chars), Style::default().bg(Color::Cyan)),
                Span::styled(" ".repeat(free_chars), Style::default().bg(Color::DarkGray)),
            ]);
            f.render_widget(Paragraph::new(bar), chunks[1]);
        }

        // Legend
        let legend = Line::from(vec![
            Span::styled(" Used ", Style::default().fg(Color::Green)),
            Span::styled(format!("({}) ", fmt_bytes(mem_usage - cache)), Style::default().fg(Color::Gray)),
            Span::styled(" Cache ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("({}) ", fmt_bytes(cache)), Style::default().fg(Color::Gray)),
            Span::styled(" Swap ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("({})", fmt_bytes(swap)), Style::default().fg(Color::Gray)),
        ]);
        f.render_widget(Paragraph::new(legend), chunks[2]);
    }
}

fn draw_network_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::default()
        .title(" NET ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let rx_data: Vec<u64> = app.net_rx_history.iter().map(|(_, v)| *v as u64).collect();
    let tx_data: Vec<u64> = app.net_tx_history.iter().map(|(_, v)| *v as u64).collect();

    let max_rx = rx_data.iter().max().copied().unwrap_or(0);
    let max_tx = tx_data.iter().max().copied().unwrap_or(0);

    // Helper to format bytes
    let fmt_bytes = |b: u64| -> String {
        let b = b as f64;
        if b > 1024.0 * 1024.0 {
            format!("{:.1} MB/s", b / 1024.0 / 1024.0)
        } else {
            format!("{:.1} KB/s", b / 1024.0)
        }
    };

    let sparkline_rx = Sparkline::default()
        .block(Block::default().title(format!("RX (Peak: {})", fmt_bytes(max_rx))).borders(Borders::NONE))
        .style(Style::default().fg(Color::Blue))
        .data(&rx_data);
    f.render_widget(sparkline_rx, chunks[0]);

    let sparkline_tx = Sparkline::default()
        .block(Block::default().title(format!("TX (Peak: {})", fmt_bytes(max_tx))).borders(Borders::NONE))
        .style(Style::default().fg(theme.network_tx)) // Orange-ish
        .data(&tx_data);
    f.render_widget(sparkline_tx, chunks[1]);

    // Aquarium Animation
    let width = 30; // Approximate width of the aquarium area in chars
    let height = 5; // Height of the aquarium area
    
    let mut aquarium_lines = vec![String::from("                              "); height];

    // Draw fishes
    for fish in &app.fishes {
        if fish.y > 0 && fish.y < height - 1 {
            let fish_char = if fish.direction > 0.0 { "><>" } else { "<><" };
            let x = fish.x as usize;
            if x < width - 3 {
                // Simple overlay
                let line = &mut aquarium_lines[fish.y];
                // Ensure we don't panic if string is short (though we init with spaces)
                if line.len() >= x + 3 {
                    line.replace_range(x..x+3, fish_char);
                }
            }
            
            // Bubble
            if (fish.x as usize) % 4 == 0 {
                 let bubble_y = fish.y.saturating_sub(1);
                 if bubble_y > 0 {
                     let line = &mut aquarium_lines[bubble_y];
                     let bx = x.saturating_sub(1);
                     if bx < width && bx > 0 {
                         line.replace_range(bx..bx+1, "o");
                     }
                 }
            }
        }
    }

    let aquarium_text: Vec<Line> = aquarium_lines.iter().enumerate().map(|(i, s)| {
        if i == 0 || i == 4 {
            Line::from(Span::styled(s, Style::default().fg(Color::Blue)))
        } else {
             // We need to color the fish differently than the background spaces
             // But for simplicity in this text widget, let's just color the whole line cyan for now
             // Or better, parse the string and colorize fish parts. 
             // Since we constructed a string, we lost the object info. 
             // Let's just print the string with the fish color, and maybe bubbles white?
             // To do it properly we'd need to build a Vec<Span>.
             
             let mut spans: Vec<Span> = vec![];
             
             for (_idx, c) in s.char_indices() {
                 if c == '<' || c == '>' {
                     spans.push(Span::styled(c.to_string(), Style::default().fg(theme.network_rx)));
                 } else if c == 'o' {
                     spans.push(Span::styled(c.to_string(), Style::default().fg(Color::White)));
                 } else {
                     spans.push(Span::raw(c.to_string()));
                 }
             }
             Line::from(spans)
        }
    }).collect();
    
    let aquarium = Paragraph::new(aquarium_text)
        .block(Block::default().borders(Borders::LEFT).border_style(Style::default().fg(theme.border)))
        .alignment(ratatui::layout::Alignment::Left); // Left align to match our grid
        
    f.render_widget(aquarium, chunks[1]);
}



fn draw_container_section(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let mut wizard_in_main = false;
    if let Some(wizard) = &app.wizard {
        if let crate::app::WizardStep::QuickRunInput { editing_id, .. } = &wizard.step {
            if editing_id.is_some() {
                wizard_in_main = true;
            }
        }
    }

    if wizard_in_main {
        if let Some(wizard) = &app.wizard {
            draw_wizard(f, wizard, chunks[0], theme);
        }
    } else {
        draw_container_table(f, app, chunks[0], theme);
    }
    
    draw_container_sidebar(f, app, chunks[1], theme);
}

fn draw_container_table(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let header_cells = ["State", "ID", "Name", "Image", "Status"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).add_modifier(Modifier::BOLD)));
    
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.background))
        .height(1)
        .bottom_margin(1);

    let rows = app.containers.iter().map(|c| {
        let (icon, color) = if c.state == "running" {
            ("●", theme.running)
        } else if c.state == "restarting" {
            ("●", theme.restarting)
        } else {
            ("○", theme.stopped)
        };
        let cells = vec![
            Cell::from(Span::styled(icon, Style::default().fg(color))),
            Cell::from(Span::styled(&c.id[..12], Style::default().fg(theme.foreground))),
            Cell::from(Span::styled(&c.names[0], Style::default().fg(theme.foreground).add_modifier(Modifier::BOLD))),
            Cell::from(Span::styled(&c.image, Style::default().fg(theme.foreground))),
            Cell::from(Span::styled(&c.status, Style::default().fg(theme.border))),
        ];
        Row::new(cells).height(1)
    });

    let widths = [
        Constraint::Length(5),
        Constraint::Length(12),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(30),
    ];

    let actions_line = Line::from(vec![
        Span::styled(" MANAGEMENT: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("[C] Create ", Style::default().fg(Color::Gray)),
        Span::styled("[E] Edit ", Style::default().fg(Color::Gray)),
        Span::styled("[e] Shell ", Style::default().fg(Color::Gray)),
        Span::styled("[B] Rebuild ", Style::default().fg(Color::Gray)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("ACTIONS: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("[R] Restart ", Style::default().fg(Color::Gray)),
        Span::styled("[S] Stop ", Style::default().fg(Color::Gray)),
        Span::styled("[U] Start ", Style::default().fg(Color::Gray)),
        Span::styled("[Del] Delete ", Style::default().fg(Color::Gray)),
        Span::styled("[Enter] Details ", Style::default().fg(Color::Gray)),
    ]);

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" CONTAINERS ")
            .title_bottom(actions_line)
            .border_style(Style::default().fg(Color::DarkGray))
        )
        .highlight_style(Style::default().bg(theme.selection_bg).fg(theme.selection_fg).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.selected_index));
    f.render_stateful_widget(table, area, &mut state);
}



fn draw_container_sidebar(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    if let Some(wizard) = &app.wizard {
        let is_edit = if let crate::app::WizardStep::QuickRunInput { editing_id, .. } = &wizard.step {
            editing_id.is_some()
        } else {
            false
        };

        if !is_edit {
            draw_wizard(f, wizard, area, theme);
            return;
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" TOOLS ")
        .title(Title::from(" [Tab] Tools ").alignment(Alignment::Right).position(Position::Bottom));
    
    f.render_widget(block.clone(), area);

    if !app.globe_frames.is_empty() {
        let inner = block.inner(area);
        
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
            
        let frame_index = (millis / 100) as usize % app.globe_frames.len();
        let globe = &app.globe_frames[frame_index];
        
        let globe_height = globe.len() as u16;
        let globe_width = if !globe.is_empty() { globe[0].len() as u16 } else { 0 };
        
        let v_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(globe_height),
                Constraint::Min(1),
            ])
            .split(inner);
            
        let h_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(globe_width),
                Constraint::Min(1),
            ])
            .split(v_layout[1]);
            
        let p = Paragraph::new(globe.join("\n"))
            .style(Style::default().fg(Color::White));
        f.render_widget(p, h_layout[1]);

    }
}

fn draw_wizard(f: &mut Frame, wizard: &crate::app::WizardState, area: Rect, _theme: &Theme) {
    let title = if matches!(wizard.step, crate::app::WizardStep::ModeSelection { .. }) {
        " TOOLS - WIZARD "
    } else {
        " WIZARD "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::White))
        .title(title);
    
    let inner = block.inner(area);
    f.render_widget(block, area);

    match &wizard.step {
        crate::app::WizardStep::ModeSelection { selected_index } => {
            let options = [
                (">_ Quick Pull & Run", "Pull from registry and run immediately"),
                ("./ Build from Source", "Build Dockerfile from local directory"),
                ("{} Docker Compose", "Run docker-compose.yml project"),
                (" Janitor", "Clean up unused resources"),
                ("⚙ Settings", "Configure application"),
            ];
            
            let items: Vec<ListItem> = options
                .iter()
                .enumerate()
                .map(|(i, (title, desc))| {
                    let (title_style, desc_style) = if i == *selected_index {
                        (Style::default().fg(Color::White).add_modifier(Modifier::BOLD), Style::default().fg(Color::Gray))
                    } else {
                        (Style::default().fg(Color::DarkGray), Style::default().fg(Color::DarkGray))
                    };
                    
                    let content = vec![
                        Line::from(Span::styled(*title, title_style)),
                        Line::from(Span::styled(format!("   {}", desc), desc_style)),
                        Line::from(""),
                    ];
                    ListItem::new(content)
                })
                .collect();
            
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Select Operation ").border_style(Style::default().fg(Color::Gray)));
            f.render_widget(list, inner);
            
            let help_area = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(1), Constraint::Length(1)]).split(inner);
            let help = Paragraph::new("UP/DOWN: Navigate | ENTER: Select | ESC: Cancel").style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, help_area[1]);
        },
        crate::app::WizardStep::QuickRunInput { image, name, ports, env, cpu, memory, focused_field, editing_id, port_status } => {
            let title = if editing_id.is_some() { "Edit Container" } else { "Quick Pull & Run" };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(inner);

            let title_p = Paragraph::new(title).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            let fields = [
                ("Image Name", image),
                ("Container Name", name),
                ("Ports (host:container)", ports),
                ("Env Vars (KEY=VAL)", env),
                ("CPU Limit (e.g. 0.5)", cpu),
                ("Memory Limit (e.g. 512m)", memory),
            ];

            for (i, (label, value)) in fields.iter().enumerate() {
                let style = if *focused_field == i {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let mut title_text = label.to_string();
                if i == 2 { // Ports field
                    match port_status {
                        crate::app::PortStatus::Available => title_text.push_str(" [OK]"),
                        crate::app::PortStatus::Occupied(who) => title_text.push_str(&format!(" [BUSY: {}]", who)),
                        crate::app::PortStatus::Invalid => title_text.push_str(" [INVALID]"),
                        _ => {}
                    }
                }

                let p = Paragraph::new(value.as_str())
                    .block(Block::default().borders(Borders::ALL).title(title_text).border_style(style));
                f.render_widget(p, chunks[i+1]);
            }
            
            let help = Paragraph::new("ENTER: Create/Update | TAB: Next Field").style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, chunks[7]);
        },
        crate::app::WizardStep::FileBrowser { current_path, list_state, items, mode } => {
             let title = match mode {
                 crate::app::FileBrowserMode::Build => "Select Project (Dockerfile)",
                 crate::app::FileBrowserMode::Compose => "Select Compose File",
             };
             
             let list_items: Vec<ListItem> = items.iter().enumerate().map(|(i, item)| {
                 let name = item.path.file_name().unwrap_or_default().to_string_lossy();
                 
                 // Tree Indentation Logic
                 let mut prefix = String::new();
                 for _ in 0..item.depth {
                     prefix.push_str("│   ");
                 }
                 
                 let branch = if item.is_last { "└── " } else { "├── " };
                 // Only add branch if depth > 0, otherwise it's root level
                 // Actually, even at root level 0, we want branches if it's a list.
                 // But typically root items in a file list don't have branches to the left unless they are children of something.
                 // In our case, `items` contains children of `current_path`.
                 // So they are all depth 0 relative to the view?
                 // No, `load_directory_tree` starts recursion at depth 0.
                 // So top level items have depth 0.
                 
                 // Let's make it look like the user requested:
                 // Root
                 // ├── Child 1
                 // └── Child 2
                 
                 let tree_prefix = if item.depth > 0 {
                     format!("{}{}", prefix, branch)
                 } else {
                     // Top level items
                     if item.is_last { "└── ".to_string() } else { "├── ".to_string() }
                 };

                 let icon = if item.is_dir { 
                     if item.expanded { IconSet::FOLDER_OPEN } else { IconSet::FOLDER_CLOSED }
                 } else { 
                     IconSet::get_file_icon(&name) 
                 };
                 
                 let display_name = format!("{} {} {}", tree_prefix, icon, name);
                 
                 let style = if Some(i) == list_state.selected() {
                     Style::default().fg(Color::White).bg(Color::DarkGray)
                 } else {
                     Style::default().fg(Color::Gray)
                 };
                 
                 // Highlight Dockerfile
                 let final_style = if name == "Dockerfile" || name == "docker-compose.yml" {
                      if Some(i) == list_state.selected() {
                          style.add_modifier(Modifier::BOLD).fg(Color::Yellow)
                      } else {
                          style.add_modifier(Modifier::BOLD).fg(Color::Yellow)
                      }
                 } else {
                     style
                 };

                 ListItem::new(display_name).style(final_style)
             }).collect();

             let instructions = match mode {
                 crate::app::FileBrowserMode::Build => " ENTER: Expand/Select | SPACE: Detect | BACKSPACE: Go Up ",
                 crate::app::FileBrowserMode::Compose => " ENTER: Expand/Select | SPACE: Generate | BACKSPACE: Go Up ",
             };

             let list = List::new(list_items)
                 .block(Block::default()
                    .borders(Borders::ALL)
                    .title(format!("{} - {}", title, current_path.display()))
                    .title_bottom(instructions)
                    .border_style(Style::default().fg(Color::Gray)));
             
             let mut state = list_state.clone();
             f.render_stateful_widget(list, inner, &mut state);
        },
        crate::app::WizardStep::DockerfileGenerator { path, detected_framework, detected_version, manual_selection_open, manual_selected_index, port, editing_port, focused_option, port_status } => {
             let title = " Dockerfile Generator ";
             let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Length(3), // Path
                    Constraint::Length(3), // Detected Framework
                    Constraint::Length(3), // Port
                    Constraint::Min(1),    // Options
                ])
                .split(inner);

            let title_p = Paragraph::new(title).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            let path_p = Paragraph::new(path.to_string_lossy())
                .block(Block::default().borders(Borders::ALL).title("Target Directory").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(path_p, chunks[1]);

            let framework_style = if *focused_option == 0 {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else if *detected_framework == crate::app::Framework::Manual {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            };
            let framework_p = Paragraph::new(format!("{} ({})", detected_framework.display_name(), detected_version))
                .block(Block::default().borders(Borders::ALL).title("Detected Framework").border_style(if *focused_option == 0 { Style::default().fg(Color::White) } else { Style::default().fg(Color::DarkGray) }))
                .style(framework_style);
            f.render_widget(framework_p, chunks[2]);

            let port_style = if *editing_port || *focused_option == 1 { Style::default().fg(Color::White) } else { Style::default().fg(Color::DarkGray) };
            let mut port_title = "Port (Press 'p' to edit)".to_string();
            match port_status {
                crate::app::PortStatus::Available => port_title.push_str(" [OK]"),
                crate::app::PortStatus::Occupied(who) => port_title.push_str(&format!(" [BUSY: {}]", who)),
                crate::app::PortStatus::Invalid => port_title.push_str(" [INVALID]"),
                _ => {}
            }

            let port_p = Paragraph::new(port.as_str())
                .block(Block::default().borders(Borders::ALL).title(port_title).border_style(port_style));
            f.render_widget(port_p, chunks[3]);

            let options = vec![
                "[ Generate Dockerfile ]",
                "[ Skip Generation ]",
            ];
            
            let options_items: Vec<ListItem> = options.iter().enumerate().map(|(i, op)| {
                // Map button index 0 -> focused_option 2, index 1 -> focused_option 3
                let style = if (i == 0 && *focused_option == 2) || (i == 1 && *focused_option == 3) {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(*op).style(style)
            }).collect();

            let options_list = List::new(options_items)
                .block(Block::default().borders(Borders::ALL).title("Actions").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(options_list, chunks[4]);

            if *manual_selection_open {
                let area = centered_rect(60, 50, f.size());
                f.render_widget(Clear, area);
                let block = Block::default().title("Select Framework").borders(Borders::ALL).border_style(Style::default().fg(Color::White));
                f.render_widget(block.clone(), area);
                
                let frameworks = [
                    crate::app::Framework::Laravel,
                    crate::app::Framework::NextJs,
                    crate::app::Framework::NuxtJs,
                    crate::app::Framework::Go,
                    crate::app::Framework::Django,
                    crate::app::Framework::Rails,
                    crate::app::Framework::Rust,
                    crate::app::Framework::Manual,
                ];
                
                let list_items: Vec<ListItem> = frameworks.iter().enumerate().map(|(i, fw)| {
                    let style = if i == *manual_selected_index {
                        Style::default().fg(Color::Black).bg(Color::White)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ListItem::new(fw.display_name()).style(style)
                }).collect();
                
                let list = List::new(list_items).block(Block::default().borders(Borders::NONE));
                let inner_area = block.inner(area);
                f.render_widget(list, inner_area);
            }
        },
        crate::app::WizardStep::ComposeGenerator { path } => {
            let title = " Compose Generator ";
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Length(3), // Path
                    Constraint::Min(1),    // Options
                    Constraint::Length(1), // Help
                ])
                .split(inner);

            let title_p = Paragraph::new(title).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            let path_p = Paragraph::new(path.to_string_lossy())
                .block(Block::default().borders(Borders::ALL).title("Target Directory").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(path_p, chunks[1]);

            let options = vec![
                "[G] Generate Default docker-compose.yml",
                "[C] Cancel",
            ];
            let options_text = options.join("\n");
            let options_p = Paragraph::new(options_text)
                .block(Block::default().borders(Borders::ALL).title("Options").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(options_p, chunks[2]);

            let help = Paragraph::new("G/ENTER: Generate | C/ESC: Cancel")
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
            f.render_widget(help, chunks[3]);
        },
        crate::app::WizardStep::ComposeServiceSelection { path, selected_services, focused_index, all_services } => {
            let title = " Review Services ";
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Length(3), // Path
                    Constraint::Min(1),    // Services
                    Constraint::Length(1), // Help
                ])
                .split(inner);

            let title_p = Paragraph::new(title).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            let path_p = Paragraph::new(path.to_string_lossy())
                .block(Block::default().borders(Borders::ALL).title("Target Directory").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(path_p, chunks[1]);

            let items: Vec<ListItem> = all_services.iter().enumerate().map(|(i, svc)| {
                let is_selected = selected_services.contains(svc);
                let check = if is_selected { "[x]" } else { "[ ]" };
                let style = if i == *focused_index {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(format!("{} {}", check, svc)).style(style)
            }).collect();
            
            // Add "Next" button at the end
            let next_style = if *focused_index == all_services.len() {
                Style::default().fg(Color::Black).bg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };
            let mut all_items = items;
            all_items.push(ListItem::new("[ Next > ]").style(next_style));

            let list = List::new(all_items)
                .block(Block::default().borders(Borders::ALL).title("Services Found").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(list, chunks[2]);

            let help = Paragraph::new("SPACE: Toggle | UP/DOWN: Navigate | ENTER: Next | ESC: Back")
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
            f.render_widget(help, chunks[3]);
        },
        crate::app::WizardStep::ResourceAllocation { path: _, services: _, cpu_limit, mem_limit, focused_field, detected_cpu, detected_mem, all_services: _ } => {
             let title = " Resource Allocation ";
             let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Length(3), // CPU
                    Constraint::Length(3), // Mem
                    Constraint::Min(1),    // Info
                    Constraint::Length(1), // Help
                ])
                .split(inner);

            let title_p = Paragraph::new(title).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            let cpu_style = if *focused_field == 0 { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
            let cpu_p = Paragraph::new(cpu_limit.as_str())
                .block(Block::default().borders(Borders::ALL).title(format!("CPU Limit (Cores) - Detected: {}", detected_cpu)).border_style(cpu_style));
            f.render_widget(cpu_p, chunks[1]);

            let mem_gb = *detected_mem as f64 / (1024.0 * 1024.0 * 1024.0);
            let mem_style = if *focused_field == 1 { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
            let mem_p = Paragraph::new(mem_limit.as_str())
                .block(Block::default().borders(Borders::ALL).title(format!("Memory Limit - Detected: {:.1} GB", mem_gb)).border_style(mem_style));
            f.render_widget(mem_p, chunks[2]);

            let info_text = vec![
                Line::from(""),
                Line::from(Span::styled("PRO TIP:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                Line::from("Leave empty or press [s] to allow DockTop to calculate"),
                Line::from("optimal resources automatically based on your hardware."),
                Line::from(""),
                Line::from(if *focused_field == 2 {
                    Span::styled("[ Generate docker-compose.yml ]", Style::default().fg(Color::Black).bg(Color::Green))
                } else {
                    Span::styled("[ Generate docker-compose.yml ]", Style::default().fg(Color::Gray))
                }),
            ];
            let info_p = Paragraph::new(info_text)
                .block(Block::default().borders(Borders::ALL).title("Info").border_style(Style::default().fg(Color::DarkGray)));
            f.render_widget(info_p, chunks[3]);

            let help = Paragraph::new("UP/DOWN: Navigate | S: Auto-Calculate | ENTER: Next/Generate | ESC: Back")
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
            f.render_widget(help, chunks[4]);
        },
        crate::app::WizardStep::OverwriteConfirm { path, detected_framework: _, detected_version: _, port: _ } => {
             let block = Block::default()
                .title(" ⚠️  WARNING: File Exists ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .style(Style::default().bg(Color::Black));
            
            let area = centered_rect(50, 30, inner);
            f.render_widget(Clear, area);
            f.render_widget(block.clone(), area);
            
            let inner = block.inner(area);
            
            let text = vec![
                Line::from(Span::styled("Dockerfile already exists!", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(format!("Target: {}/Dockerfile", path.display())),
                Line::from(""),
                Line::from("Do you want to backup the existing file and overwrite it?"),
                Line::from("The old file will be renamed to Dockerfile.bak"),
                Line::from(""),
                Line::from(vec![
                    Span::styled("[Y] Backup & Overwrite", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled("[N] Cancel", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                ]),
            ];
            
            let p = Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .wrap(Wrap { trim: true });
            
            f.render_widget(p, inner);
        },
        crate::app::WizardStep::Janitor { items, list_state, loading } => {
             let title = " Janitor - Cleanup ";
             let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Title
                    Constraint::Min(1),    // List
                    Constraint::Length(3), // Summary
                    Constraint::Length(1), // Help
                ])
                .split(inner);

            let title_p = Paragraph::new(title).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, chunks[0]);

            if *loading {
                let p = Paragraph::new("Scanning system for unused resources...").alignment(ratatui::layout::Alignment::Center);
                f.render_widget(p, chunks[1]);
            } else {
                let list_items: Vec<ListItem> = items.iter().enumerate().map(|(i, item)| {
                    let check = if item.selected { "[x]" } else { "[ ]" };
                    let kind_str = match item.kind {
                        crate::app::JanitorItemKind::Image => IconSet::IMAGE,
                        crate::app::JanitorItemKind::Volume => IconSet::VOLUME,
                        crate::app::JanitorItemKind::Container => IconSet::CONTAINER,
                    };
                    
                    let size_str = if item.size > 0 {
                        let s = item.size as f64;
                        if s > 1024.0 * 1024.0 * 1024.0 {
                            format!("{:.2} GB", s / 1024.0 / 1024.0 / 1024.0)
                        } else {
                            format!("{:.1} MB", s / 1024.0 / 1024.0)
                        }
                    } else {
                        "-".to_string()
                    };

                    let content = format!("{} {} | {:<3} | {:<10} | {:<15} | {}", check, item.id.chars().take(12).collect::<String>(), kind_str, size_str, item.age, item.name);
                    
                    let style = if Some(i) == list_state.selected() {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    ListItem::new(content).style(style)
                }).collect();

                let mut state = list_state.clone();
                let list = List::new(list_items)
                    .block(Block::default().borders(Borders::ALL).title("Junk Items").border_style(Style::default().fg(Color::DarkGray)));
                f.render_stateful_widget(list, chunks[1], &mut state);

                // Summary
                let total_size: u64 = items.iter().filter(|i| i.selected).map(|i| i.size).sum();
                let total_str = if total_size > 1024 * 1024 * 1024 {
                    format!("{:.2} GB", total_size as f64 / 1024.0 / 1024.0 / 1024.0)
                } else {
                    format!("{:.1} MB", total_size as f64 / 1024.0 / 1024.0)
                };

                let summary = Paragraph::new(format!("Potential Space Reclaimed: {}", total_str))
                    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Green)))
                    .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
                f.render_widget(summary, chunks[2]);
            }

            let help = Paragraph::new("SPACE: Toggle | UP/DOWN: Navigate | ENTER: Clean Selected | ESC: Cancel")
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC));
            f.render_widget(help, chunks[3]);
        },
        crate::app::WizardStep::BuildConf { tag, mount_volume, focused_field, .. } => {
             let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(1)])
                .split(inner);
            
            let tag_style = if *focused_field == 0 { Style::default().fg(Color::White) } else { Style::default().fg(Color::DarkGray) };
            let p = Paragraph::new(tag.as_str()).block(Block::default().borders(Borders::ALL).title("Image Tag").border_style(tag_style));
            f.render_widget(p, layout[0]);

            let border_style = if *focused_field == 1 { Style::default().fg(Color::White) } else { Style::default().fg(Color::DarkGray) };
            let check = if *mount_volume { "[x]" } else { "[ ]" };
            let block = Block::default().borders(Borders::ALL).border_style(border_style).title("Options");
            let p = Paragraph::new(format!("{} Mount current folder for live-reload?", check)).block(block);
            f.render_widget(p, layout[1]);

            let help = Paragraph::new("ENTER: Build | SPACE: Toggle | ESC: Cancel")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, layout[2]);
        },
        crate::app::WizardStep::Processing { message, .. } => {
            let text = vec![
                Line::from(""),
                Line::from(Span::styled(message, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(Span::styled("Please wait...", Style::default().fg(Color::Gray))),
            ];
            let p = Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .wrap(Wrap { trim: true });
            f.render_widget(p, inner);
        },
        crate::app::WizardStep::Error(msg) => {
             let text = vec![
                Line::from(Span::styled("Error:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(msg, Style::default().fg(Color::White))),
                Line::from(""),
                Line::from(Span::styled("Press Esc to close", Style::default().fg(Color::Gray))),
            ];
            let p = Paragraph::new(text).wrap(Wrap { trim: true });
            f.render_widget(p, inner);
        },
        crate::app::WizardStep::Settings { focused_field, temp_config } => {
             let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Theme
                    Constraint::Length(3), // Braille
                    Constraint::Length(3), // Refresh
                    Constraint::Length(3), // Confirm
                    Constraint::Min(1),    // Help
                ])
                .split(inner);

             let style = |idx| if *focused_field == idx { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Gray) };
             
             // Theme Selection
             let p = Paragraph::new(format!("< {} > (Left/Right)", temp_config.general.theme))
                .block(Block::default().borders(Borders::ALL).title("Theme").border_style(style(0)));
             f.render_widget(p, layout[0]);

             // Braille
             let check = if temp_config.general.show_braille { "[x]" } else { "[ ]" };
             let p = Paragraph::new(format!("{} Enable Braille Graphs", check))
                .block(Block::default().borders(Borders::ALL).title("Appearance").border_style(style(1)));
             f.render_widget(p, layout[1]);

             // Refresh Rate
             let p = Paragraph::new(format!("< {} ms > (Left/Right)", temp_config.general.refresh_rate_ms))
                .block(Block::default().borders(Borders::ALL).title("Refresh Rate").border_style(style(2)));
             f.render_widget(p, layout[2]);

             // Confirm Delete
             let check = if temp_config.general.confirm_on_delete { "[x]" } else { "[ ]" };
             let p = Paragraph::new(format!("{} Confirm on Delete", check))
                .block(Block::default().borders(Borders::ALL).title("Safety").border_style(style(3)));
             f.render_widget(p, layout[3]);

             // Help
             let help = Paragraph::new("[S] Save & Apply | [R] Reset | [Esc] Cancel")
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center);
             f.render_widget(help, layout[4]);
        },

    }
}

fn draw_logs_section(f: &mut Frame, app: &App, area: Rect, _theme: &Theme) {
    let block = Block::default()
        .title(" LOGS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![];
    if app.is_loading_details {
        lines.push(Line::from(Span::styled(" Loading logs...", Style::default().fg(Color::Gray))));
    } else {
        for log in &app.logs {
            let style = if log.contains("ERROR") || log.contains("ERR") {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else if log.contains("WARN") {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(Span::styled(log, style)));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner_area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect, _theme: &Theme) {
    let status_text = if let Some((msg, _)) = &app.action_status {
        Span::styled(format!(" {} ", msg), Style::default().bg(Color::White).fg(Color::Black))
    } else {
        Span::raw("")
    };

    let keys = Span::styled(
        " j/k: Nav • q: Quit",
        Style::default().fg(Color::DarkGray),
    );

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    f.render_widget(Paragraph::new(status_text), layout[0]);
    f.render_widget(Paragraph::new(keys).alignment(ratatui::layout::Alignment::Right), layout[1]);
}

pub fn calculate_cpu_usage(stats: &ContainerStats, prev_stats: &Option<ContainerStats>) -> f64 {
    let (prev_cpu, prev_sys) = if let Some(prev) = prev_stats {
        (prev.cpu_stats.cpu_usage.total_usage, prev.cpu_stats.system_cpu_usage.unwrap_or(0))
    } else {
        (stats.precpu_stats.cpu_usage.total_usage, stats.precpu_stats.system_cpu_usage.unwrap_or(0))
    };

    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64 - prev_cpu as f64;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64 - prev_sys as f64;
    let num_cpus = stats.cpu_stats.cpu_usage.percpu_usage.as_ref().map(|v| v.len()).unwrap_or(1) as f64;

    if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * num_cpus * 100.0
    } else {
        0.0
    }
}

fn draw_details_popup(f: &mut Frame, app: &App, area: Rect, _theme: &Theme) {
    let block = Block::default()
        .title(" Container Details ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::Black));
    
    let area = centered_rect(60, 60, area);
    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    if let Some(inspect) = &app.current_inspection {
        let mut lines = Vec::new();
        
        lines.push(Line::from(vec![Span::styled("ID: ", Style::default().fg(Color::Cyan)), Span::raw(inspect.id.as_str())]));
        lines.push(Line::from(vec![Span::styled("Name: ", Style::default().fg(Color::Cyan)), Span::raw(inspect.name.as_deref().unwrap_or("?"))]));
        lines.push(Line::from(vec![Span::styled("Image: ", Style::default().fg(Color::Cyan)), Span::raw(inspect.config.as_ref().map(|c| c.image.as_str()).unwrap_or("?"))]));
        lines.push(Line::from(""));

        // Network
        lines.push(Line::from(Span::styled("Network:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
        if let Some(net) = &inspect.network_settings {
             if let Some(networks) = &net.networks {
                 for (name, settings) in networks {
                     lines.push(Line::from(format!("  {}: {}", name, settings.ip_address.as_deref().unwrap_or(""))));
                 }
             }
             // Ports
             if let Some(ports) = &net.ports {
                 lines.push(Line::from("  Ports:"));
                 for (port, bindings) in ports {
                     if let Some(binds) = bindings {
                         for b in binds {
                             lines.push(Line::from(format!("    {} -> {}:{}", port, b.host_ip, b.host_port)));
                         }
                     }
                 }
             }
        }
        lines.push(Line::from(""));

        // Env Vars
        lines.push(Line::from(Span::styled("Environment Variables:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
        if let Some(config) = &inspect.config {
            if let Some(env) = &config.env {
                for e in env {
                    lines.push(Line::from(format!("  {}", e)));
                }
            }
        }

        let p = Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .scroll((0, 0)); 
        
        f.render_widget(p, inner);
    } else {
        let p = Paragraph::new("Loading details...").alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, inner);
    }
}

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
