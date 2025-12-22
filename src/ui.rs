use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
    Frame,
};
use tui_term::widget::PseudoTerminal;

use crate::app::{App, ActivePanel};
use crate::editor::EditorWidget;

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

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
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
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
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
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default()
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
        .border_style(if app.active_panel == ActivePanel::FileTree { Style::default().fg(Color::Yellow) } else { Style::default() });
    
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

    // Editor
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
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            }))
        .line_number_style(Style::default().fg(Color::DarkGray))
        .cursor_style(Style::default().bg(Color::White).fg(Color::Black))
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

    // Terminal
    let terminal_block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::Terminal { Style::default().fg(Color::Yellow) } else { Style::default() });

    let screen = app.terminal_screen.read().unwrap();
    let pseudo_term = PseudoTerminal::new(screen.screen())
        .block(terminal_block);
    f.render_widget(pseudo_term, layout.terminal);
    
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
        .title(format!(" AI Chat ({}) (Ctrl+M to Switch) ", app.selected_model))
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(Color::Yellow) } else { Style::default() });

    // Calculate max scroll based on content height
    let chat_inner_height = layout.chat_history.height.saturating_sub(2) as usize; // Subtract borders
    let total_lines = chat_text.lines().count();
    let max_scroll = total_lines.saturating_sub(chat_inner_height) as u16;
    app.chat_scroll = app.chat_scroll.min(max_scroll);

    let chat_paragraph = Paragraph::new(chat_text)
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
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(Color::Yellow) } else { Style::default() }));
    f.render_widget(&chat_input, layout.chat_input);

    // --- Menu Dropdown Overlay ---
    if let Some(idx) = app.menu_open_idx {
        let menu_x = (idx * 10) as u16;
        let menu_items = match idx {
            0 => vec![ListItem::new(" Exit (Ctrl+Q) ")],
            1 => vec![ListItem::new(" Copy (Ctrl+C) "), ListItem::new(" Paste (Ctrl+V) ")],
            2 => vec![ListItem::new(" Reset Layout (Ctrl+R) ")],
            3 => vec![ListItem::new(" About ")],
            _ => vec![],
        };
        
        let height = (menu_items.len() + 2) as u16;
        let area = Rect::new(menu_x, 1, 20, height);
        f.render_widget(Clear, area);
        f.render_widget(
            List::new(menu_items)
                .block(Block::default().borders(Borders::ALL)),
            area
        );
    }

    // --- Search Modal ---
    if app.is_searching {
        let area = centered_rect(60, 50, f.area());
        f.render_widget(Clear, area);
        
        let block = Block::default()
            .title(" File Search (Esc to Close) ")
            .borders(Borders::ALL);
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
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
            
        f.render_stateful_widget(list, chunks[1], &mut app.search_state);
    }

    // --- Settings Modal ---
    if app.show_settings {
        let area = centered_rect(60, 20, f.area());
        f.render_widget(Clear, area);
        
        let block = Block::default()
            .title(" Settings - Gemini API Key (Enter to Save, Esc to Cancel) ")
            .borders(Borders::ALL);
            
        let inner_area = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(&app.settings_input, inner_area);
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
