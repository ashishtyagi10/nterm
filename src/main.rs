mod action;
mod file_tree;
mod app;
mod ui;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind, MouseButton},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::error::Error;
use std::io;
use tui_textarea::CursorMove;
use tui_textarea::TextArea;
use ratatui::widgets::{Block, Borders}; // Needed for DumpHistory

use crate::app::{App, AppEvent, ActivePanel};
use crate::action::Action;
use crate::ui::ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
        app.editor_scroll_state = app.editor_scroll_state.content_length(app.editor.lines().len()).position(app.editor.cursor().0);
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
                AppEvent::Tick => {}, // No-op for tick events
                AppEvent::Input(input) => {
                    if let Event::Key(key) = input {
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
                                        app.editor = TextArea::from(lines);
                                        app.editor.set_block(Block::default().borders(Borders::ALL).title(" Editor - Terminal History "));
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
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                            if mouse.row == 0 {
                                let idx = (mouse.column / 10) as usize;
                                if idx < app.menu_titles.len() {
                                    app.menu_open_idx = Some(idx);
                                } else {
                                    app.menu_open_idx = None;
                                }
                                continue;
                            } else if let Some(idx) = app.menu_open_idx {
                                let menu_x = (idx * 10) as u16;
                                if mouse.column >= menu_x && mouse.column < menu_x + 15 && mouse.row >= 1 && mouse.row < 6 {
                                     match idx {
                                         0 => { if mouse.row == 2 { app.should_quit = true; } }, // File -> Exit
                                         2 => { if mouse.row == 2 { app.active_panel = ActivePanel::Editor; } }, // View -> Reset
                                         _ => {} // Other menu items
                                     }
                                }
                                app.menu_open_idx = None;
                                continue;
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
                                            if app.file_tree_scroll_offset < app.visible_items.len().saturating_sub(1) {
                                                app.file_tree_scroll_offset += 1;
                                            }
                                        },
                                        MouseEventKind::ScrollUp => {
                                            if app.file_tree_scroll_offset > 0 {
                                                app.file_tree_scroll_offset -= 1;
                                            }
                                        },
                                        _ => {} // Other mouse events
                                    }
                                },
                                ActivePanel::Editor => {
                                     match mouse.kind {
                                        MouseEventKind::ScrollDown => {
                                            app.editor.move_cursor(CursorMove::Down);
                                        },
                                        MouseEventKind::ScrollUp => {
                                            app.editor.move_cursor(CursorMove::Up);
                                        },
                                        _ => {} // Other mouse events
                                    }
                                },
                                ActivePanel::Chat => {
                                    match mouse.kind {
                                        MouseEventKind::ScrollDown => {
                                            app.chat_scroll = app.chat_scroll.saturating_add(1);
                                        },
                                        MouseEventKind::ScrollUp => {
                                            app.chat_scroll = app.chat_scroll.saturating_sub(1);
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
                                        app.editor.input(key);
                                    }
                                    ActivePanel::Chat => {
                                        if key.code == KeyCode::Enter {
                                            let content = app.chat_input.lines()[0].clone();
                                            if !content.is_empty() {
                                                app.chat_history.push(format!("You: {}", content));
                                                app.chat_input = TextArea::default();
                                                app.chat_input.set_block(Block::default().borders(Borders::ALL).title(" Chat Input "));
                                                app.chat_history.push("AI: I see you're working on this project. How can I help with the code?".to_string());
                                            }
                                        } else if key.code == KeyCode::Up {
                                            app.chat_scroll = app.chat_scroll.saturating_sub(1);
                                        } else if key.code == KeyCode::Down {
                                            app.chat_scroll = app.chat_scroll.saturating_add(1);
                                        } else {
                                            app.chat_input.input(key);
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