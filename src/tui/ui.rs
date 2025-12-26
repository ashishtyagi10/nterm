use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
    Frame,
};
use tui_term::widget::PseudoTerminal;

use super::app::{App, ActivePanel};
use super::editor::EditorWidget;
use super::theme::Theme;

pub struct AppLayout {
    pub menu: Rect,
    pub file_tree: Rect,
    pub editor: Rect,
    pub terminal: Rect,
    pub chat_history: Rect,
    pub chat_input: Rect,
}

pub fn get_layout_chunks(area: Rect, active_panel: &ActivePanel) -> AppLayout {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let menu = main_chunks[0];

    // Chat panel expands to 35% when focused, otherwise 20%
    let (file_tree_percent, middle_percent, chat_percent) = if *active_panel == ActivePanel::Chat {
        (20, 45, 35)
    } else {
        (20, 60, 20)
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(file_tree_percent),
            Constraint::Percentage(middle_percent),
            Constraint::Percentage(chat_percent),
        ])
        .split(main_chunks[1]);

    let file_tree = chunks[0];

    let (editor_percent, terminal_percent) = if *active_panel == ActivePanel::Terminal {
        (40, 60)
    } else {
        (60, 40)
    };

    let middle_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(editor_percent), Constraint::Percentage(terminal_percent)])
        .split(chunks[1]);
        
    let editor = middle_chunks[0];
    let terminal = middle_chunks[1];

    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(chunks[2]);
        
    let chat_history = chat_chunks[0];
    let chat_input = chat_chunks[1];

    AppLayout {
        menu,
        file_tree,
        editor,
        terminal,
        chat_history,
        chat_input,
    }
}

pub fn ui(f: &mut Frame, app: &mut App) {
    let layout = get_layout_chunks(f.area(), &app.active_panel);

    // Apply main background color
    f.render_widget(Block::default().style(Style::default().bg(app.current_theme.background)), f.area());


    // --- Menu Bar ---
    let menu_bar_area = layout.menu;
    let menu_titles_count = app.menu_titles.len();
    let menu_constraints = std::iter::repeat(Constraint::Length(10))
        .take(menu_titles_count)
        .chain(std::iter::once(Constraint::Min(0)))
        .collect::<Vec<_>>();
    
    let menu_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(menu_constraints)
        .split(menu_bar_area);
        
    for (i, title) in app.menu_titles.iter().enumerate() {
        let style = if app.menu_open_idx == Some(i) {
            Style::default().fg(app.current_theme.selection_fg).bg(app.current_theme.selection_bg)
        } else {
            Style::default().fg(app.current_theme.foreground)
        };
        f.render_widget(Paragraph::new(title.as_str()).style(style), menu_chunks[i]);
    }

    // File Tree
    let height = layout.file_tree.height as usize;
    if app.selected_file_idx < app.file_tree_scroll_offset {
        app.file_tree_scroll_offset = app.selected_file_idx;
    } else if app.selected_file_idx >= app.file_tree_scroll_offset + height {
        app.file_tree_scroll_offset = app.selected_file_idx - height + 1;
    }

    let items: Vec<ListItem> = app.visible_items.iter()
        .skip(app.file_tree_scroll_offset)
        .take(height)
        .enumerate()
        .map(|(i, item)| {
            let actual_idx = app.file_tree_scroll_offset + i;
            let style = if actual_idx == app.selected_file_idx {
                Style::default().bg(app.current_theme.selection_bg).fg(app.current_theme.selection_fg)
            } else {
                Style::default().fg(if item.is_dir { app.current_theme.directory } else { app.current_theme.file })
            };
            
            let prefix = if item.is_dir {
                if item.expanded { "v " } else { "+ " } 
            } else {
                "- "
            };
            
            let indent = "  ".repeat(item.depth);
            let content = format!("{}{}{}", indent, prefix, item.name);
            
            ListItem::new(content).style(style)
        }).collect();
    
    let file_tree_block = Block::default()
        .title(" File Tree ")
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::FileTree { Style::default().fg(app.current_theme.border_active) } else { Style::default().fg(app.current_theme.border) });
    
    app.file_tree_state.select(None);
    
    f.render_stateful_widget(List::new(items).block(file_tree_block), layout.file_tree, &mut app.file_tree_state);
    
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        layout.file_tree,
        &mut app.file_tree_scroll_state
    );

    // Editor (or Settings when show_settings is true)
    if app.show_settings {
        render_settings_panel(f, app, layout.editor);
    } else {
        let editor_title = app.editor_state.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| format!(" Editor - {} ", n.to_string_lossy()))
            .unwrap_or_else(|| " Editor ".to_string());

        let editor_widget = EditorWidget::new()
            .block(Block::default()
                .borders(Borders::ALL)
                .title(editor_title)
                .border_style(if app.active_panel == ActivePanel::Editor {
                    Style::default().fg(app.current_theme.border_active)
                } else {
                    Style::default().fg(app.current_theme.border)
                }))
            .line_number_style(Style::default().fg(app.current_theme.line_number))
            .cursor_style(Style::default().bg(app.current_theme.cursor_bg).fg(app.current_theme.cursor_fg))
            .focused(app.active_panel == ActivePanel::Editor);

        f.render_stateful_widget(editor_widget, layout.editor, &mut app.editor_state);

        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            layout.editor,
            &mut app.editor_scroll_state
        );
    }

    // Terminal
    let terminal_border_style = if app.active_panel == ActivePanel::Terminal {
        Style::default().fg(app.current_theme.border_active)
    } else {
        Style::default().fg(app.current_theme.border)
    };
    let terminal_block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(terminal_border_style)
        .style(Style::default().bg(app.current_theme.background).fg(app.current_theme.foreground));

    let screen = app.terminal_screen.read().unwrap();
    let pseudo_term = PseudoTerminal::new(screen.screen())
        .block(terminal_block.clone());

    f.render_widget(pseudo_term, layout.terminal);

    // Post-process: Replace Color::Reset backgrounds with theme background
    // tui-term uses Color::Reset for "default" terminal colors, which renders as black
    // We override these to match our theme (process entire terminal area including borders)
    use ratatui::style::Color;
    for y in layout.terminal.y..layout.terminal.y + layout.terminal.height {
        for x in layout.terminal.x..layout.terminal.x + layout.terminal.width {
            if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                if cell.bg == Color::Reset {
                    cell.set_bg(app.current_theme.background);
                }
                if cell.fg == Color::Reset {
                    cell.set_fg(app.current_theme.foreground);
                }
            }
        }
    }
    
    let terminal_scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"));
    
    let mut terminal_scroll_state = app.terminal_scroll_state
        .viewport_content_length(layout.terminal.height as usize);
        
    f.render_stateful_widget(
        terminal_scrollbar,
        layout.terminal,
        &mut terminal_scroll_state
    );

    // Chat
    let chat_text = app.chat_history.join("\n\n");
    let chat_history_block = Block::default()
        .title(format!(" AI Chat ({}) (Ctrl+M to Switch) ", app.get_selected_model_name()))
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(app.current_theme.border_active) } else { Style::default().fg(app.current_theme.border) })
        .style(Style::default().bg(app.current_theme.background));

    // Parse markdown for styled rendering
    let chat_lines = parse_markdown_to_lines(&chat_text, &app.current_theme);

    // Calculate wrapped line count for proper scroll limits
    let chat_inner_width = layout.chat_history.width.saturating_sub(2) as usize; // Subtract borders
    let chat_inner_height = layout.chat_history.height.saturating_sub(2) as usize;

    // Estimate wrapped lines (each line wraps based on width)
    let wrapped_lines: usize = chat_lines.iter()
        .map(|line| {
            let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
            if line_width == 0 {
                1
            } else {
                (line_width + chat_inner_width - 1) / chat_inner_width.max(1)
            }
        })
        .sum();

    let max_scroll = wrapped_lines.saturating_sub(chat_inner_height) as u16;
    app.chat_scroll = app.chat_scroll.min(max_scroll);

    // Create paragraph with styled lines
    // Note: Don't set a default style here as it would override span styles
    let chat_paragraph = Paragraph::new(chat_lines)
        .block(chat_history_block)
        .wrap(Wrap { trim: true })
        .scroll((app.chat_scroll, 0));

    f.render_widget(chat_paragraph, layout.chat_history);
    
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        layout.chat_history,
        &mut app.chat_scroll_state
    );

    let mut chat_input = app.chat_input.clone();
    chat_input.set_block(Block::default()
        .borders(Borders::ALL)
        .title(" Chat Input ")
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(app.current_theme.border_active) } else { Style::default().fg(app.current_theme.border) }));
    f.render_widget(&chat_input, layout.chat_input);

    // --- Menu Dropdown Overlay ---
    if let Some(idx) = app.menu_open_idx {
        let menu_x = (idx * 10) as u16;
        let raw_items = App::get_menu_items(idx);

        let menu_items: Vec<ListItem> = raw_items
            .iter()
            .enumerate()
            .map(|(i, (label, _action))| {
                let shortcut = match (idx, i) {
                    (0, 0) => " (Ctrl+S)",
                    (0, 1) => " (Ctrl+P)",
                    (0, 2) => " (Ctrl+Q)",
                    (1, 0) => " (Ctrl+C)",
                    (1, 1) => " (Ctrl+V)",
                    (2, 0) => " (Ctrl+R)",
                    (2, 1) => " (Ctrl+H)",
                    _ => "",
                };
                let text = format!(" {}{} ", label, shortcut);
                let style = if app.menu_hover_idx == Some(i) {
                    Style::default()
                        .bg(app.current_theme.selection_bg)
                        .fg(app.current_theme.selection_fg)
                } else {
                    Style::default()
                        .bg(app.current_theme.background)
                        .fg(app.current_theme.foreground)
                };
                ListItem::new(text).style(style)
            })
            .collect();

        let height = (menu_items.len() + 2) as u16;
        let width = 24;
        let area = Rect::new(menu_x, 1, width, height);
        f.render_widget(Clear, area);
        f.render_widget(
            List::new(menu_items)
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(app.current_theme.border)))
                .style(Style::default().bg(app.current_theme.background)),
            area
        );
    }

    // --- Search Modal ---
    if app.is_searching {
        let area = centered_rect(60, 50, f.area());
        f.render_widget(Clear, area);
        
        let block = Block::default()
            .title(" File Search (Esc to Close) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.current_theme.border))
            .style(Style::default().bg(app.current_theme.background).fg(app.current_theme.foreground));
        f.render_widget(block.clone(), area);
        
        let inner_area = block.inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(inner_area);
            
        f.render_widget(&app.search_input, chunks[0]);
        
        let items: Vec<ListItem> = app.search_results.iter()
            .map(|p| ListItem::new(p.to_string_lossy().into_owned()))
            .collect();
            
        let list = List::new(items)
            .block(Block::default().borders(Borders::TOP))
            .highlight_style(Style::default().bg(app.current_theme.selection_bg).fg(app.current_theme.selection_fg));
            
        f.render_stateful_widget(list, chunks[1], &mut app.search_state);
    }

}

/// Render the settings panel in the editor area with two-column form layout
fn render_settings_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.current_theme.border_active))
        .style(Style::default().bg(app.current_theme.background).fg(app.current_theme.foreground));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Calculate layout: header, model list, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Header/instructions
            Constraint::Min(0),     // Model list
            Constraint::Length(1),  // Footer
        ])
        .split(inner_area);

    // Header with keyboard shortcuts
    let header = Line::from(vec![
        Span::styled("↑↓", Style::default().fg(app.current_theme.directory).add_modifier(Modifier::BOLD)),
        Span::styled(" Navigate  ", Style::default().fg(app.current_theme.line_number)),
        Span::styled("Enter", Style::default().fg(app.current_theme.directory).add_modifier(Modifier::BOLD)),
        Span::styled(" Edit  ", Style::default().fg(app.current_theme.line_number)),
        Span::styled("Space", Style::default().fg(app.current_theme.directory).add_modifier(Modifier::BOLD)),
        Span::styled(" Set Active  ", Style::default().fg(app.current_theme.line_number)),
        Span::styled("Tab", Style::default().fg(app.current_theme.directory).add_modifier(Modifier::BOLD)),
        Span::styled(" Theme  ", Style::default().fg(app.current_theme.line_number)),
        Span::styled("Esc", Style::default().fg(app.current_theme.directory).add_modifier(Modifier::BOLD)),
        Span::styled(" Close", Style::default().fg(app.current_theme.line_number)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // Model list area
    let list_area = chunks[1];
    let visible_height = list_area.height as usize;
    let total_models = app.config.models.len();

    // Each model card takes 4 lines (top border, content row 1, content row 2, bottom border)
    // But we can share borders between adjacent cards
    let lines_per_model = 3usize; // top border shared, 2 content lines, bottom becomes next top

    // Update scroll to keep selected model visible
    let selected_start_line = app.settings_model_idx * lines_per_model;
    if selected_start_line < app.settings_scroll_offset {
        app.settings_scroll_offset = selected_start_line;
    } else if selected_start_line + lines_per_model > app.settings_scroll_offset + visible_height {
        app.settings_scroll_offset = (selected_start_line + lines_per_model).saturating_sub(visible_height);
    }

    // Calculate column widths for the form layout
    let label_width = 12u16; // "API Key:" etc
    let total_width = list_area.width;

    // Build all lines for all models with box drawing
    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for (i, model) in app.config.models.iter().enumerate() {
        let is_selected = i == app.settings_model_idx;
        let is_active = i == app.config.selected_model_idx;

        // Determine border style based on selection
        let border_style = if is_selected {
            Style::default().fg(app.current_theme.border_active)
        } else {
            Style::default().fg(app.current_theme.border)
        };

        // Top border with model name
        let status_icon = if model.api_key.is_some() { "✓" } else { "✗" };
        let status_style = if model.api_key.is_some() {
            Style::default().fg(app.current_theme.directory)
        } else {
            Style::default().fg(app.current_theme.file)
        };

        // Calculate remaining space for border line after model name
        let name_section_len = model.name.len() + 5 + if is_active { 9 } else { 0 }; // " [✓]" + " ★ ACTIVE"
        let remaining = total_width.saturating_sub(name_section_len as u16 + 4) as usize;

        let mut top_spans = vec![
            Span::styled("┌ ", border_style),
        ];

        // Model name styling
        let name_style = if is_selected {
            Style::default().fg(app.current_theme.selection_fg).bg(app.current_theme.selection_bg).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(app.current_theme.border_active).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.current_theme.foreground).add_modifier(Modifier::BOLD)
        };

        top_spans.push(Span::styled(model.name.clone(), name_style));
        top_spans.push(Span::styled(" [", border_style));
        top_spans.push(Span::styled(status_icon, status_style));
        top_spans.push(Span::styled("]", border_style));
        if is_active {
            top_spans.push(Span::styled(" ★ ACTIVE", Style::default().fg(app.current_theme.border_active)));
        }
        top_spans.push(Span::styled(" ".to_string() + &"─".repeat(remaining.saturating_sub(if is_active { 9 } else { 0 })) + "┐", border_style));

        all_lines.push(Line::from(top_spans));

        // Row 1: Provider | Model ID
        let provider_label = "Provider:";
        let model_label = "Model:";
        let provider_value = format!("{}", model.provider);
        let model_value = model.model_id.clone();

        let row1 = Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(format!("{:<width$}", provider_label, width = label_width as usize), Style::default().fg(app.current_theme.line_number)),
            Span::styled(format!("{:<15}", provider_value), Style::default().fg(app.current_theme.foreground)),
            Span::styled(format!("{:<8}", model_label), Style::default().fg(app.current_theme.line_number)),
            Span::styled(model_value.clone(), Style::default().fg(app.current_theme.foreground)),
            Span::styled(format!("{:>width$}│", "", width = total_width.saturating_sub(label_width + 15 + 8 + model_value.len() as u16 + 4) as usize), border_style),
        ]);
        all_lines.push(row1);

        // Row 2: API Key (with editing support)
        let api_label = "API Key:";
        let key_display = if is_selected && app.settings_editing {
            "[editing...]".to_string()
        } else {
            match &model.api_key {
                Some(key) if !key.is_empty() => {
                    if key.len() > 4 {
                        format!("{}...{}", "*".repeat(8), &key[key.len()-4..])
                    } else {
                        "*".repeat(key.len())
                    }
                },
                _ => "(not set - press Enter to configure)".to_string(),
            }
        };

        let key_style = if is_selected && app.settings_editing {
            Style::default().fg(app.current_theme.cursor_bg)
        } else if model.api_key.is_some() {
            Style::default().fg(app.current_theme.directory)
        } else {
            Style::default().fg(app.current_theme.file).add_modifier(Modifier::ITALIC)
        };

        let key_display_len = key_display.len();
        let row2 = Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(format!("{:<width$}", api_label, width = label_width as usize), Style::default().fg(app.current_theme.line_number)),
            Span::styled(key_display, key_style),
            Span::styled(format!("{:>width$}│", "", width = total_width.saturating_sub(label_width + key_display_len as u16 + 4) as usize), border_style),
        ]);
        all_lines.push(row2);

        // Bottom border
        let bottom_border = format!("└{}┘", "─".repeat(total_width.saturating_sub(2) as usize));
        all_lines.push(Line::from(Span::styled(bottom_border, border_style)));
    }

    // Apply scroll offset and render visible lines
    let visible_lines: Vec<Line<'static>> = all_lines
        .into_iter()
        .skip(app.settings_scroll_offset)
        .take(visible_height)
        .collect();

    f.render_widget(Paragraph::new(visible_lines), list_area);

    // If editing, render the TextArea over the API key value
    if app.settings_editing {
        // Find the API key line position (each model has 4 lines: top, row1, row2, bottom)
        // API key is in row2, which is index 2 within each model's lines
        let lines_per_model_with_border = 4usize;
        let selected_api_line = app.settings_model_idx * lines_per_model_with_border + 2;
        let line_in_view = selected_api_line.saturating_sub(app.settings_scroll_offset);

        if line_in_view < visible_height {
            let input_y = list_area.y + line_in_view as u16;
            let input_x = list_area.x + 2 + label_width; // After "│ " and label
            let input_width = total_width.saturating_sub(label_width + 6);

            let input_area = Rect::new(input_x, input_y, input_width, 1);
            f.render_widget(Clear, input_area);

            let input_text = app.settings_input.lines().join("");
            let display_text = if input_text.is_empty() {
                "█".to_string()
            } else {
                format!("{}█", input_text)
            };

            f.render_widget(
                Paragraph::new(display_text)
                    .style(Style::default().fg(app.current_theme.foreground).bg(app.current_theme.background)),
                input_area
            );
        }
    }

    // Footer with theme info
    let footer = Line::from(vec![
        Span::styled(format!("Theme: {:?} │ Models: {}", app.config.theme, total_models), Style::default().fg(app.current_theme.line_number)),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[2]);

    // Scrollbar
    let total_lines = total_models * 4; // 4 lines per model with borders
    let mut scroll_state = ratatui::widgets::ScrollbarState::default()
        .content_length(total_lines)
        .position(app.settings_scroll_offset);

    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        list_area,
        &mut scroll_state
    );
}

/// Parse markdown text and return styled Lines for rendering
fn parse_markdown_to_lines(text: &str, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();

    for line in text.lines() {
        // Check for code block start/end - handle even if line has prefix like "AI: ```"
        let trimmed_for_code = line.trim_start_matches("AI: ").trim_start_matches("You: ");

        if trimmed_for_code.starts_with("```") {
            if in_code_block {
                // End of code block - render accumulated code
                for code_line in &code_block_lines {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("│ {}", code_line),
                            Style::default()
                                .fg(theme.directory)
                                .bg(theme.selection_bg),
                        ),
                    ]));
                }
                code_block_lines.clear();
                in_code_block = false;
            } else {
                // Start of code block
                // If line starts with AI: or You:, show that prefix first
                if line.starts_with("AI:") {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "AI: ".to_string(),
                            Style::default()
                                .fg(theme.selection_fg)
                                .bg(theme.border_active)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                } else if line.starts_with("You:") {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "You: ".to_string(),
                            Style::default()
                                .fg(theme.cursor_fg)
                                .bg(theme.cursor_bg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_block_lines.push(line.to_string());
            continue;
        }

        // Handle headers
        if line.starts_with("### ") {
            lines.push(Line::from(vec![
                Span::styled(
                    line[4..].to_string(),
                    Style::default()
                        .fg(theme.border_active)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        } else if line.starts_with("## ") {
            lines.push(Line::from(vec![
                Span::styled(
                    line[3..].to_string(),
                    Style::default()
                        .fg(theme.border_active)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ]));
        } else if line.starts_with("# ") {
            lines.push(Line::from(vec![
                Span::styled(
                    line[2..].to_string(),
                    Style::default()
                        .fg(theme.selection_fg)
                        .bg(theme.selection_bg)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        // Handle bullet lists
        else if line.starts_with("- ") || line.starts_with("* ") {
            lines.push(Line::from(vec![
                Span::styled("  • ".to_string(), Style::default().fg(theme.border_active)),
                Span::styled(line[2..].to_string(), Style::default().fg(theme.foreground)),
            ]));
        }
        // Handle numbered lists
        else if line.len() > 2 && line.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            && (line.contains(". ") || line.contains(") ")) {
            let split_pos = line.find(". ").or_else(|| line.find(") "));
            if let Some(pos) = split_pos {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", &line[..=pos]),
                        Style::default().fg(theme.border_active),
                    ),
                    Span::styled(line[pos + 2..].to_string(), Style::default().fg(theme.foreground)),
                ]));
            } else {
                lines.push(Line::from(Span::raw(line.to_string())));
            }
        }
        // Handle "You:" prefix (user messages)
        else if line.starts_with("You:") {
            let rest = if line.len() > 4 { &line[4..] } else { "" };
            // Check if the rest of the message contains inline markdown
            let rest_spans = if rest.contains('`') || rest.contains("**") {
                parse_inline_markdown(rest, theme)
            } else {
                vec![Span::styled(rest.to_string(), Style::default().fg(theme.foreground))]
            };
            let mut spans = vec![
                Span::styled(
                    "You: ".to_string(),
                    Style::default()
                        .fg(theme.cursor_fg)
                        .bg(theme.cursor_bg)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(rest_spans);
            lines.push(Line::from(spans));
        }
        // Handle "AI:" prefix (AI messages)
        else if line.starts_with("AI:") {
            let rest = if line.len() > 3 { &line[3..] } else { "" };
            // Check if the rest of the message contains inline markdown
            let rest_spans = if rest.contains('`') || rest.contains("**") {
                parse_inline_markdown(rest, theme)
            } else {
                vec![Span::styled(rest.to_string(), Style::default().fg(theme.foreground))]
            };
            let mut spans = vec![
                Span::styled(
                    "AI: ".to_string(),
                    Style::default()
                        .fg(theme.selection_fg)
                        .bg(theme.border_active)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            spans.extend(rest_spans);
            lines.push(Line::from(spans));
        }
        // Handle inline code and bold
        else if line.contains('`') || line.contains("**") {
            let styled_spans = parse_inline_markdown(line, theme);
            lines.push(Line::from(styled_spans));
        }
        // Regular text
        else {
            lines.push(Line::from(Span::styled(line.to_string(), Style::default().fg(theme.foreground))));
        }
    }

    // Handle unclosed code block
    if in_code_block {
        for code_line in &code_block_lines {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("│ {}", code_line),
                    Style::default()
                        .fg(theme.directory)
                        .bg(theme.selection_bg),
                ),
            ]));
        }
    }

    lines
}

/// Parse inline markdown (backticks for code, ** for bold)
fn parse_inline_markdown(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_pos = 0;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    while current_pos < len {
        // Check for inline code (backtick)
        if chars[current_pos] == '`' && current_pos + 1 < len {
            // Find closing backtick
            if let Some(end_pos) = chars[current_pos + 1..].iter().position(|&c| c == '`') {
                let end_pos = current_pos + 1 + end_pos;
                let code_text: String = chars[current_pos + 1..end_pos].iter().collect();
                spans.push(Span::styled(
                    format!(" {} ", code_text),
                    Style::default()
                        .fg(theme.directory)
                        .bg(theme.selection_bg),
                ));
                current_pos = end_pos + 1;
                continue;
            }
        }

        // Check for bold (**)
        if current_pos + 1 < len && chars[current_pos] == '*' && chars[current_pos + 1] == '*' {
            // Find closing **
            let search_start = current_pos + 2;
            let mut found_end = None;
            for i in search_start..len.saturating_sub(1) {
                if chars[i] == '*' && chars[i + 1] == '*' {
                    found_end = Some(i);
                    break;
                }
            }
            if let Some(end_pos) = found_end {
                let bold_text: String = chars[current_pos + 2..end_pos].iter().collect();
                spans.push(Span::styled(
                    bold_text,
                    Style::default()
                        .fg(theme.foreground)
                        .add_modifier(Modifier::BOLD),
                ));
                current_pos = end_pos + 2;
                continue;
            }
        }

        // Regular character - accumulate until special char
        let start = current_pos;
        while current_pos < len && chars[current_pos] != '`' && !(current_pos + 1 < len && chars[current_pos] == '*' && chars[current_pos + 1] == '*') {
            current_pos += 1;
        }
        if start < current_pos {
            let regular_text: String = chars[start..current_pos].iter().collect();
            spans.push(Span::styled(regular_text, Style::default().fg(theme.foreground)));
        }

        // Prevent infinite loop
        if current_pos == start {
            let ch: String = chars[current_pos..current_pos + 1].iter().collect();
            spans.push(Span::styled(ch, Style::default().fg(theme.foreground)));
            current_pos += 1;
        }
    }

    spans
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_get_layout_chunks() {
        let area = Rect::new(0, 0, 100, 100);
        let layout = get_layout_chunks(area, &ActivePanel::Editor);

        // Check if areas are contained within main area
        assert!(layout.menu.area() > 0);
        assert!(layout.file_tree.area() > 0);
        assert!(layout.editor.area() > 0);
        assert!(layout.terminal.area() > 0);
        assert!(layout.chat_history.area() > 0);
        assert!(layout.chat_input.area() > 0);

        // Basic split checks
        assert_eq!(layout.menu.y, 0);
        assert_eq!(layout.menu.height, 1);
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 100);
        let center = centered_rect(50, 50, area);

        assert_eq!(center.width, 50);
        assert_eq!(center.height, 50);
        assert_eq!(center.x, 25);
        assert_eq!(center.y, 25);
    }

    #[test]
    fn test_parse_markdown_to_lines() {
        let theme = Theme::new(crate::theme::ThemeMode::Dark);

        // Test "You:" prefix
        let lines = parse_markdown_to_lines("You: Hello world", &theme);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.len() >= 2);
        assert_eq!(lines[0].spans[0].content.as_ref(), "You: ");

        // Test "AI:" prefix
        let lines = parse_markdown_to_lines("AI: Here is my response", &theme);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.len() >= 2);
        assert_eq!(lines[0].spans[0].content.as_ref(), "AI: ");

        // Test headers
        let lines = parse_markdown_to_lines("# Header 1", &theme);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].content.as_ref(), "Header 1");

        // Test bullet list
        let lines = parse_markdown_to_lines("- List item", &theme);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 2);
        assert_eq!(lines[0].spans[0].content.as_ref(), "  • ");

        // Test code block
        let code_text = "```\nlet x = 1;\n```";
        let lines = parse_markdown_to_lines(code_text, &theme);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans[0].content.contains("let x = 1;"));

        // Test code block with AI: prefix
        let ai_code = "AI: ```python\ndef foo():\n    pass\n```";
        let lines = parse_markdown_to_lines(ai_code, &theme);
        assert!(lines.len() >= 2); // AI: prefix line + code lines
        assert_eq!(lines[0].spans[0].content.as_ref(), "AI: ");

        // Test inline code
        let lines = parse_markdown_to_lines("Use `code` here", &theme);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.len() >= 2); // "Use ", " code ", " here"
    }
}
