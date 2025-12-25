mod action;
mod file_tree;
mod app;
mod ui;
mod ai;
mod config;
mod editor;
mod theme;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind, MouseButton},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::error::Error;
use std::io;
use tui_textarea::TextArea;
use ratatui::widgets::{Block, Borders};

use crate::app::{App, AppEvent, ActivePanel};
use crate::action::Action;
use crate::ui::{ui, get_layout_chunks};
use ratatui::layout::Rect;
use std::env;
use std::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Check for --new-window flag
    let args: Vec<String> = env::args().collect();
    if !args.contains(&"--new-window".to_string()) {
        if cfg!(target_os = "macos") {
            let current_exe = env::current_exe()?;
            let exe_path = current_exe.to_str().ok_or("Failed to get executable path")?;
            
            // Spawn new terminal window running this executable with --new-window
            // Using osascript to tell Terminal to do this
            let script = format!(
                "tell application \"Terminal\" to do script \"{} --new-window\"",
                exe_path
            );
            
            Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output()?;
                
            return Ok(());
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend + std::io::Write>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        if app.should_quit {
            return Ok(());
        }
        
        // Update Scrollbar States
        app.file_tree_scroll_state = app.file_tree_scroll_state.content_length(app.visible_items.len()).position(app.file_tree_scroll_offset);
        app.editor_scroll_state = app.editor_scroll_state.content_length(app.editor_state.line_count()).position(app.editor_state.scroll_offset);
        let chat_lines = app.chat_history.join("\n").lines().count(); 
        app.chat_scroll_state = app.chat_scroll_state.content_length(chat_lines).position(app.chat_scroll as usize);
        
        terminal.draw(|f| ui(f, app))?;

        // Wait for at least one event
        let first_event = match app.event_rx.recv() {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };
        
        let mut events = vec![first_event];
        // Drain pending events (limit to 50)
        while let Ok(e) = app.event_rx.try_recv() {
            events.push(e);
            if events.len() > 50 { break; }
        }

        for event in events {
            match event {
                AppEvent::PtyData => {
                    if let Ok(screen) = app.terminal_screen.read() {
                         let scrollback = screen.screen().scrollback();
                         let height = screen.screen().size().0;
                         app.terminal_scroll_state = app.terminal_scroll_state
                            .content_length(scrollback as usize + height as usize)
                            .position(scrollback as usize);
                    }
                },
                AppEvent::AiResponse(response) => {
                    app.chat_history.push(format!("AI: {}", response));
                },
                AppEvent::Tick => {}, // No-op for tick events
                AppEvent::Input(input) => {
                    if let Event::Key(key) = input {
                        // Settings Mode Handling
                        if app.show_settings {
                            match key.code {
                                KeyCode::Esc => {
                                    app.show_settings = false;
                                    // Reset input to current config value on cancel
                                    app.settings_input = TextArea::default();
                                    app.settings_input.set_block(Block::default().borders(Borders::ALL).title(" Gemini API Key "));
                                    if let Some(key) = &app.config.gemini_api_key {
                                        app.settings_input.insert_str(key);
                                    }
                                },
                                KeyCode::Tab => {
                                    app.toggle_theme();
                                },
                                KeyCode::Enter => {
                                    let key = app.settings_input.lines()[0].trim().to_string();
                                    if !key.is_empty() {
                                        app.config.gemini_api_key = Some(key);
                                        let _ = app.config.save();
                                    }
                                    app.show_settings = false;
                                },
                                _ => {
                                    app.settings_input.input(key);
                                }
                            }
                            continue;
                        }

                        // Search Mode Handling
                        if app.is_searching {
                            match key.code {
                                KeyCode::Esc => app.is_searching = false,
                                KeyCode::Enter => {
                                    if let Some(idx) = app.search_state.selected() {
                                        if let Some(path) = app.search_results.get(idx).cloned() {
                                            app.load_file_path(path);
                                            app.active_panel = ActivePanel::Editor;
                                            app.is_searching = false;
                                        }
                                    }
                                },
                                KeyCode::Up => {
                                    let i = match app.search_state.selected() {
                                        Some(i) => if i == 0 { app.search_results.len().saturating_sub(1) } else { i - 1 },
                                        None => 0,
                                    };
                                    app.search_state.select(Some(i));
                                },
                                KeyCode::Down => {
                                    let i = match app.search_state.selected() {
                                        Some(i) => if i >= app.search_results.len().saturating_sub(1) { 0 } else { i + 1 },
                                        None => 0,
                                    };
                                    app.search_state.select(Some(i));
                                },
                                _ => {
                                    app.search_input.input(key);
                                    app.on_search_input();
                                }
                            }
                            continue;
                        }

                        // Check Global Actions
                        if let Some(action) = app.key_map.get(&(key.code, key.modifiers)) {
                            match action {
                                Action::Quit => app.should_quit = true,
                                Action::SwitchFocus => {
                                    app.active_panel = match app.active_panel {
                                        ActivePanel::FileTree => ActivePanel::Editor,
                                        ActivePanel::Editor => ActivePanel::Chat,
                                        ActivePanel::Chat => ActivePanel::Terminal,
                                        ActivePanel::Terminal => ActivePanel::FileTree,
                                    };
                                },
                                Action::ToggleMenu => {
                                    if app.menu_open_idx.is_some() {
                                        app.menu_open_idx = None;
                                    } else {
                                        // app.menu_open_idx = Some(0); 
                                    }
                                },
                                Action::ResetLayout => app.active_panel = ActivePanel::Editor,
                                Action::DumpHistory => {
                                    if let Ok(buffer) = app.history_buffer.read() {
                                        let clean_content = String::from_utf8_lossy(&buffer).to_string();
                                        let lines: Vec<String> = clean_content.lines().map(|s| s.to_string()).collect();
                                        app.editor_state.lines = if lines.is_empty() { vec![String::new()] } else { lines };
                                        app.editor_state.cursor_row = 0;
                                        app.editor_state.cursor_col = 0;
                                        app.editor_state.file_path = None;
                                        app.active_panel = ActivePanel::Editor;
                                    }
                                },
                                Action::FileSearch => {
                                    app.is_searching = !app.is_searching;
                                    if app.is_searching {
                                        // Focus search, maybe clear input?
                                        // app.search_input = TextArea::default(); // Optional: Clear on open
                                        app.on_search_input(); // Refresh
                                    }
                                },
                                Action::CycleModel => {
                                    app.cycle_model();
                                },
                                Action::OpenSettings => {
                                    app.show_settings = true;
                                },
                                Action::Copy => {
                                    if app.active_panel == ActivePanel::Editor {
                                        if let Some(text) = app.editor_state.copy() {
                                            if let Some(clipboard) = &app.clipboard {
                                                if let Ok(mut clipboard) = clipboard.lock() {
                                                    let _ = clipboard.set_text(text);
                                                }
                                            }
                                        }
                                    }
                                },
                                Action::Paste => {
                                     if app.active_panel == ActivePanel::Editor {
                                        if let Some(clipboard) = &app.clipboard {
                                            if let Ok(mut clipboard) = clipboard.lock() {
                                                if let Ok(text) = clipboard.get_text() {
                                                    app.editor_state.paste(&text);
                                                }
                                            }
                                        }
                                     } else if app.active_panel == ActivePanel::Terminal {
                                        // Handle paste in terminal via global key check fallback?
                                        // Or explicit handle here.
                                        // Terminal uses PTY writer.
                                        if let Some(clipboard) = &app.clipboard {
                                            if let Ok(mut clipboard) = clipboard.lock() {
                                                if let Ok(text) = clipboard.get_text() {
                                                    let _ = app.pty_writer.write_all(text.as_bytes());
                                                    let _ = app.pty_writer.flush();
                                                }
                                            }
                                        }
                                     }
                                },
                                Action::About => {
                                    app.chat_history.push("AI: nterm v0.1.0 - A terminal IDE built in Rust.".to_string());
                                    // Make sure chat is visible
                                    app.active_panel = ActivePanel::Chat;
                                },
                                _ => {}
                            }
                            continue;
                        }
                        
                        // Close menu on Esc if not handled by action
                        if key.code == KeyCode::Esc && app.menu_open_idx.is_some() {
                            app.menu_open_idx = None;
                            continue;
                        }
                    }
                    
                    // Menu Mouse Handling
                    if let Event::Mouse(mouse) = input {
                        // Handle hover when menu is open
                        if let Some(idx) = app.menu_open_idx {
                            let menu_x = (idx * 10) as u16;
                            let menu_items = App::get_menu_items(idx);
                            let menu_width = 24u16;
                            let menu_height = menu_items.len() as u16 + 2; // +2 for borders

                            // Check if mouse is within menu dropdown area
                            if mouse.column >= menu_x
                                && mouse.column < menu_x + menu_width
                                && mouse.row >= 1
                                && mouse.row < 1 + menu_height
                            {
                                // Calculate which item is hovered (row 1 is border, items start at row 2)
                                let item_row = mouse.row.saturating_sub(2);
                                if (item_row as usize) < menu_items.len() {
                                    app.menu_hover_idx = Some(item_row as usize);
                                } else {
                                    app.menu_hover_idx = None;
                                }
                            } else if mouse.row != 0 {
                                // Mouse outside menu and not on menu bar - could close on move
                                app.menu_hover_idx = None;
                            }
                        }

                        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                            if mouse.row == 0 {
                                // Click on menu bar
                                let idx = (mouse.column / 10) as usize;
                                if idx < app.menu_titles.len() {
                                    if app.menu_open_idx == Some(idx) {
                                        // Toggle off if clicking same menu
                                        app.menu_open_idx = None;
                                    } else {
                                        app.menu_open_idx = Some(idx);
                                    }
                                    app.menu_hover_idx = None;
                                } else {
                                    app.menu_open_idx = None;
                                    app.menu_hover_idx = None;
                                }
                                continue;
                            } else if let Some(idx) = app.menu_open_idx {
                                let menu_x = (idx * 10) as u16;
                                let menu_items = App::get_menu_items(idx);
                                let menu_width = 24u16;

                                if mouse.column >= menu_x
                                    && mouse.column < menu_x + menu_width
                                    && mouse.row >= 2
                                    && mouse.row < 2 + menu_items.len() as u16
                                {
                                    // Click on a menu item
                                    let item_idx = (mouse.row - 2) as usize;
                                    if let Some((_, action)) = menu_items.get(item_idx) {
                                        // Execute the action
                                        match action {
                                            Action::Quit => app.should_quit = true,
                                            Action::OpenSettings => app.show_settings = true,
                                            Action::FileSearch => {
                                                app.is_searching = true;
                                                app.on_search_input();
                                            }
                                            Action::Copy => {
                                                if app.active_panel == ActivePanel::Editor {
                                                    if let Some(text) = app.editor_state.copy() {
                                                        if let Some(clipboard) = &app.clipboard {
                                                            if let Ok(mut clipboard) = clipboard.lock() {
                                                                let _ = clipboard.set_text(text);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Action::Paste => {
                                                if app.active_panel == ActivePanel::Editor {
                                                    if let Some(clipboard) = &app.clipboard {
                                                        if let Ok(mut clipboard) = clipboard.lock() {
                                                            if let Ok(text) = clipboard.get_text() {
                                                                app.editor_state.paste(&text);
                                                            }
                                                        }
                                                    }
                                                } else if app.active_panel == ActivePanel::Terminal {
                                                    if let Some(clipboard) = &app.clipboard {
                                                        if let Ok(mut clipboard) = clipboard.lock() {
                                                            if let Ok(text) = clipboard.get_text() {
                                                                let _ = app.pty_writer.write_all(text.as_bytes());
                                                                let _ = app.pty_writer.flush();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Action::ResetLayout => app.active_panel = ActivePanel::Editor,
                                            Action::DumpHistory => {
                                                if let Ok(buffer) = app.history_buffer.read() {
                                                    let clean_content = String::from_utf8_lossy(&buffer).to_string();
                                                    let lines: Vec<String> = clean_content.lines().map(|s| s.to_string()).collect();
                                                    app.editor_state.lines = if lines.is_empty() { vec![String::new()] } else { lines };
                                                    app.editor_state.cursor_row = 0;
                                                    app.editor_state.cursor_col = 0;
                                                    app.editor_state.file_path = None;
                                                    app.active_panel = ActivePanel::Editor;
                                                }
                                            }
                                            Action::About => {
                                                app.chat_history.push("AI: nterm v0.1.0 - A terminal IDE built in Rust.".to_string());
                                                app.active_panel = ActivePanel::Chat;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                app.menu_open_idx = None;
                                app.menu_hover_idx = None;
                                continue;
                            } else {
                                // Click outside menu closes it
                                app.menu_open_idx = None;
                                app.menu_hover_idx = None;
                            }
                        }
                    }

                    // Global Focus Switching via Mouse
                    if let Event::Mouse(mouse) = input {
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                             if let Ok(size) = terminal.size() {
                                 let rect = Rect { x: 0, y: 0, width: size.width, height: size.height };
                                 let layout = get_layout_chunks(rect, &app.active_panel);
                                 let col = mouse.column;
                                 let row = mouse.row;
                                 
                                 if col >= layout.file_tree.x && col < layout.file_tree.x + layout.file_tree.width &&
                                    row >= layout.file_tree.y && row < layout.file_tree.y + layout.file_tree.height {
                                     app.active_panel = ActivePanel::FileTree;
                                 } else if col >= layout.editor.x && col < layout.editor.x + layout.editor.width &&
                                    row >= layout.editor.y && row < layout.editor.y + layout.editor.height {
                                     app.active_panel = ActivePanel::Editor;
                                 } else if col >= layout.terminal.x && col < layout.terminal.x + layout.terminal.width &&
                                    row >= layout.terminal.y && row < layout.terminal.y + layout.terminal.height {
                                     app.active_panel = ActivePanel::Terminal;
                                 } else if (col >= layout.chat_history.x && col < layout.chat_history.x + layout.chat_history.width &&
                                            row >= layout.chat_history.y && row < layout.chat_history.y + layout.chat_history.height) ||
                                           (col >= layout.chat_input.x && col < layout.chat_input.x + layout.chat_input.width &&
                                            row >= layout.chat_input.y && row < layout.chat_input.y + layout.chat_input.height) {
                                     app.active_panel = ActivePanel::Chat;
                                 }
                             }
                        }
                    }

                    match input {
                        Event::Mouse(mouse) => {
                            match app.active_panel {
                                ActivePanel::Terminal => {
                                     let input_bytes = match mouse.kind {
                                        MouseEventKind::ScrollDown => vec![27, 91, 66], 
                                        MouseEventKind::ScrollUp => vec![27, 91, 65],   
                                        _ => vec![],
                                    };
                                    if !input_bytes.is_empty() {
                                        let _ = app.pty_writer.write_all(&input_bytes);
                                        let _ = app.pty_writer.flush();
                                    }
                                },
                                ActivePanel::FileTree => {
                                    match mouse.kind {
                                        MouseEventKind::ScrollDown => {
                                            let max_scroll = app.visible_items.len().saturating_sub(1);
                                            app.file_tree_scroll_offset = (app.file_tree_scroll_offset + 3).min(max_scroll);
                                        },
                                        MouseEventKind::ScrollUp => {
                                            app.file_tree_scroll_offset = app.file_tree_scroll_offset.saturating_sub(3);
                                        },
                                        _ => {} // Other mouse events
                                    }
                                },
                                ActivePanel::Editor => {
                                     match mouse.kind {
                                        MouseEventKind::ScrollDown => {
                                            app.editor_state.scroll_down(3);
                                        },
                                        MouseEventKind::ScrollUp => {
                                            app.editor_state.scroll_up(3);
                                        },
                                        _ => {} // Other mouse events
                                    }
                                },
                                ActivePanel::Chat => {
                                    match mouse.kind {
                                        MouseEventKind::ScrollDown => {
                                            app.chat_scroll = app.chat_scroll.saturating_add(3);
                                        },
                                        MouseEventKind::ScrollUp => {
                                            app.chat_scroll = app.chat_scroll.saturating_sub(3);
                                        },
                                        _ => {} // Other mouse events
                                    }
                                }
                            }
                        },
                        Event::Key(key) => {
                            // Only process panel specific keys if NOT a global action (handled above)
                            // But wait, we need to pass input to terminal for Ctrl+C etc if it was NOT a global action map.
                            // Currently key_map has Ctrl+Q. Ctrl+C is NOT in map, so it falls through here.
                            // This is correct.
                            
                            // Check if menu is open, Esc handled in global key map (ToggleMenu)?
                            // We added Esc -> ToggleMenu.
                            // If menu is open, any key might close it?
                            if app.menu_open_idx.is_some() {
                                app.menu_open_idx = None;
                                // Don't consume key? Or consume? Usually consume.
                                // But if it was "Esc", action handled it.
                                // If it was "Enter", we might want to select menu item?
                                // Simplified: clicking only for now.
                                continue; // Close menu and don't process key further
                            }

                            match app.active_panel {
                                    ActivePanel::Editor => {
                                        match key.code {
                                            KeyCode::Char(c) => {
                                                app.editor_state.insert_char(c);
                                            }
                                            KeyCode::Backspace => {
                                                app.editor_state.backspace();
                                            }
                                            KeyCode::Delete => {
                                                app.editor_state.delete();
                                            }
                                            KeyCode::Enter => {
                                                app.editor_state.insert_newline();
                                            }
                                            KeyCode::Up => {
                                                app.editor_state.move_cursor_up();
                                            }
                                            KeyCode::Down => {
                                                app.editor_state.move_cursor_down();
                                            }
                                            KeyCode::Left => {
                                                app.editor_state.move_cursor_left();
                                            }
                                            KeyCode::Right => {
                                                app.editor_state.move_cursor_right();
                                            }
                                            KeyCode::Home => {
                                                app.editor_state.move_cursor_home();
                                            }
                                            KeyCode::End => {
                                                app.editor_state.move_cursor_end();
                                            }
                                            KeyCode::PageUp => {
                                                app.editor_state.page_up(20);
                                            }
                                            KeyCode::PageDown => {
                                                app.editor_state.page_down(20);
                                            }
                                            _ => {}
                                        }
                                    }
                                    ActivePanel::Chat => {
                                        match key.code {
                                            KeyCode::Enter => {
                                                let content = app.chat_input.lines()[0].clone();
                                                if !content.is_empty() {
                                                    app.send_chat_message(content);
                                                    app.chat_input = TextArea::default();
                                                    app.chat_input.set_block(Block::default().borders(Borders::ALL).title(" Chat Input "));
                                                    // Auto-scroll to bottom on new message
                                                    app.chat_scroll = u16::MAX;
                                                }
                                            }
                                            KeyCode::Up => {
                                                app.chat_scroll = app.chat_scroll.saturating_sub(1);
                                            }
                                            KeyCode::Down => {
                                                app.chat_scroll = app.chat_scroll.saturating_add(1);
                                            }
                                            KeyCode::PageUp => {
                                                app.chat_scroll = app.chat_scroll.saturating_sub(10);
                                            }
                                            KeyCode::PageDown => {
                                                app.chat_scroll = app.chat_scroll.saturating_add(10);
                                            }
                                            KeyCode::Home => {
                                                app.chat_scroll = 0;
                                            }
                                            KeyCode::End => {
                                                app.chat_scroll = u16::MAX; // Will be clamped in render
                                            }
                                            _ => {
                                                app.chat_input.input(key);
                                            }
                                        }
                                    }
                                    ActivePanel::FileTree => {
                                        match key.code {
                                            KeyCode::Up => {
                                                if app.selected_file_idx > 0 {
                                                    app.selected_file_idx -= 1;
                                                    app.file_tree_state.select(Some(app.selected_file_idx));
                                                    app.load_selected_file();
                                                }
                                            }
                                            KeyCode::Down => {
                                                if app.selected_file_idx < app.visible_items.len().saturating_sub(1) {
                                                    app.selected_file_idx += 1;
                                                    app.file_tree_state.select(Some(app.selected_file_idx));
                                                    app.load_selected_file();
                                                }
                                            }
                                            KeyCode::PageUp => {
                                                let jump = 10;
                                                app.selected_file_idx = app.selected_file_idx.saturating_sub(jump);
                                                app.file_tree_state.select(Some(app.selected_file_idx));
                                            }
                                            KeyCode::PageDown => {
                                                let jump = 10;
                                                let max_idx = app.visible_items.len().saturating_sub(1);
                                                app.selected_file_idx = (app.selected_file_idx + jump).min(max_idx);
                                                app.file_tree_state.select(Some(app.selected_file_idx));
                                            }
                                            KeyCode::Right => {
                                                if let Some(item) = app.visible_items.get(app.selected_file_idx) {
                                                    if item.is_dir && !item.expanded {
                                                        app.toggle_selected_dir();
                                                    }
                                                }
                                            }
                                            KeyCode::Left => {
                                                if let Some(item) = app.visible_items.get(app.selected_file_idx) {
                                                    if item.is_dir && item.expanded {
                                                        app.toggle_selected_dir();
                                                    }
                                                }
                                            }
                                            KeyCode::Enter => {
                                                if let Some(item) = app.visible_items.get(app.selected_file_idx) {
                                                    if item.is_dir {
                                                        app.toggle_selected_dir();
                                                    } else {
                                                        app.load_selected_file();
                                                        app.active_panel = ActivePanel::Editor;
                                                    }
                                                }
                                            }
                                            _ => {} // Ignore other key presses for now
                                        }
                                    }
                                    ActivePanel::Terminal => {
                                        let input_bytes = match key.code {
                                            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                                if let Some(clipboard) = &app.clipboard {
                                                    if let Ok(mut clipboard) = clipboard.lock() {
                                                        if let Ok(text) = clipboard.get_text() {
                                                            let _ = app.pty_writer.write_all(text.as_bytes());
                                                            let _ = app.pty_writer.flush();
                                                        }
                                                    }
                                                }
                                                vec![] // Don't send ^V to PTY
                                            },
                                            KeyCode::Char(c) => {
                                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                                    match c {
                                                        'c' => vec![3],
                                                        'd' => vec![4],
                                                        'z' => vec![26],
                                                        c => vec![(c as u8) & 0x1f],
                                                    }
                                                } else {
                                                     let mut b = [0; 4];
                                                     c.encode_utf8(&mut b).as_bytes().to_vec()
                                                }
                                            },
                                            KeyCode::Enter => vec![13],
                                            KeyCode::Backspace => vec![8],
                                            KeyCode::Left => vec![27, 91, 68],
                                            KeyCode::Right => vec![27, 91, 67],
                                            KeyCode::Up => vec![27, 91, 65],
                                            KeyCode::Down => vec![27, 91, 66],
                                            KeyCode::PageUp => vec![27, 91, 53, 126], // ESC [5~
                                            KeyCode::PageDown => vec![27, 91, 54, 126], // ESC [6~
                                            KeyCode::Home => vec![27, 91, 72], // ESC [H
                                            KeyCode::End => vec![27, 91, 70], // ESC [F
                                            KeyCode::Esc => vec![27],
                                            _ => vec![],
                                        };

                                        if !input_bytes.is_empty() {
                                            let _ = app.pty_writer.write_all(&input_bytes);
                                            let _ = app.pty_writer.flush();
                                        }
                                    }
                                }
                        }
                        _ => {} // Ignore other event types for now
                    }
                }
            }
        }
    }
}