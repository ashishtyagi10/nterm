use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame, Terminal,
};
use std::{
    error::Error,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    sync::{Arc, RwLock, mpsc},
    thread,
    time::Duration,
};
use tui_textarea::{TextArea, CursorMove};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tui_term::widget::PseudoTerminal;

#[derive(PartialEq)]
enum ActivePanel {
    FileTree,
    Editor,
    Terminal,
    Chat,
}

#[derive(Clone, Debug)]
struct FileNode {
    path: PathBuf,
    name: String,
    is_dir: bool,
    expanded: bool,
    children: Vec<FileNode>,
    depth: usize,
}

impl FileNode {
    fn from_path(path: PathBuf, depth: usize) -> Self {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let is_dir = path.is_dir();
        Self {
            path,
            name,
            is_dir,
            expanded: false,
            children: Vec::new(),
            depth,
        }
    }

    fn toggle_expand(&mut self) {
        if self.is_dir {
            if self.expanded {
                self.expanded = false;
                self.children.clear();
            } else {
                self.expanded = true;
                self.load_children();
            }
        }
    }

    fn load_children(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.path) {
            let mut files: Vec<FileNode> = entries
                .filter_map(|res| res.ok())
                .map(|e| FileNode::from_path(e.path(), self.depth + 1))
                .filter(|node| !node.name.starts_with('.'))
                .collect();
            
            files.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
            
            self.children = files;
        }
    }
}

struct VisibleItem {
    name: String,
    path: PathBuf,
    is_dir: bool,
    depth: usize,
    expanded: bool,
}

enum AppEvent {
    Input(Event),
    PtyData,
    Tick,
}

struct App<'a> {
    file_tree: Vec<FileNode>,
    visible_items: Vec<VisibleItem>,
    selected_file_idx: usize,
    file_tree_state: ListState,
    file_tree_scroll_offset: usize,
    file_tree_scroll_state: ScrollbarState,
    
    editor: TextArea<'a>,
    editor_scroll_state: ScrollbarState,
    
    chat_input: TextArea<'a>,
    chat_history: Vec<String>,
    chat_scroll: u16,
    chat_scroll_state: ScrollbarState,
    
    active_panel: ActivePanel,
    should_quit: bool,
    
    // Terminal State
    pty_writer: Box<dyn Write + Send>,
    terminal_screen: Arc<RwLock<tui_term::vt100::Parser>>,
    terminal_scroll_state: ScrollbarState,
    history_buffer: Arc<RwLock<Vec<u8>>>,
    event_rx: mpsc::Receiver<AppEvent>,
}

impl<'a> App<'a> {
    fn new() -> Self {
        let mut editor = TextArea::default();
        editor.set_block(Block::default().borders(Borders::ALL).title(" Editor "));
        
        let mut chat_input = TextArea::default();
        chat_input.set_block(Block::default().borders(Borders::ALL).title(" Chat Input "));

        // Initialize PTY
        let pty_system = NativePtySystem::default();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }).expect("Failed to create PTY");

        let cmd = CommandBuilder::new("bash");
        let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn shell");

        let mut reader = pair.master.try_clone_reader().expect("Failed to clone reader");
        let writer = pair.master.take_writer().expect("Failed to take writer");

        let parser = Arc::new(RwLock::new(tui_term::vt100::Parser::new(24, 80, 0)));
        let parser_clone = parser.clone();
        
        let history = Arc::new(RwLock::new(Vec::new()));
        let history_clone = history.clone();
        
        // Event Channel
        let (tx, rx) = mpsc::channel();
        let pty_tx = tx.clone();
        let tick_tx = tx.clone();

        // Spawn thread to read from PTY
        thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        let data = &buffer[..n];
                        if let Ok(mut p) = parser_clone.write() {
                            p.process(data);
                        }
                        if let Ok(mut h) = history_clone.write() {
                            h.extend_from_slice(data);
                        }
                        let _ = pty_tx.send(AppEvent::PtyData);
                    }
                    Ok(_) => break, 
                    Err(_) => break,
                }
            }
        });
        
        // Input Thread
        thread::spawn(move || {
            loop {
                if let Ok(event) = event::read() {
                    let _ = tx.send(AppEvent::Input(event));
                }
            }
        });
        
        // Tick Thread
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(250));
                let _ = tick_tx.send(AppEvent::Tick);
            }
        });

        let mut app = Self {
            file_tree: Vec::new(),
            visible_items: Vec::new(),
            selected_file_idx: 0,
            file_tree_state: ListState::default(),
            file_tree_scroll_offset: 0,
            file_tree_scroll_state: ScrollbarState::default(),
            
            editor,
            editor_scroll_state: ScrollbarState::default(),
            
            chat_input,
            chat_history: vec!["Hello! I'm your AI assistant. Press Tab to switch panels.".to_string()],
            chat_scroll: 0,
            chat_scroll_state: ScrollbarState::default(),
            
            active_panel: ActivePanel::FileTree,
            should_quit: false,
            pty_writer: writer,
            terminal_screen: parser,
            terminal_scroll_state: ScrollbarState::default(),
            history_buffer: history,
            event_rx: rx,
        };
        
        app.file_tree_state.select(Some(0));
        app.refresh_file_tree();
        app
    }

    fn refresh_file_tree(&mut self) {
        let root_path = PathBuf::from(".");
        if let Ok(entries) = fs::read_dir(&root_path) {
            let mut roots: Vec<FileNode> = entries
                .filter_map(|res| res.ok())
                .map(|e| FileNode::from_path(e.path(), 0))
                .filter(|node| !node.name.starts_with('.'))
                .collect();
                
             roots.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
            self.file_tree = roots;
            self.update_visible_items();
        }
    }

    fn update_visible_items(&mut self) {
        let mut new_items = Vec::new();
        for node in &self.file_tree {
            flatten_node(node, &mut new_items);
        }
        self.visible_items = new_items;
    }

    fn toggle_selected_dir(&mut self) {
        if let Some(item) = self.visible_items.get(self.selected_file_idx) {
            if item.is_dir {
                let path_to_toggle = item.path.clone();
                toggle_node_recursive(&mut self.file_tree, &path_to_toggle);
                self.update_visible_items();
            }
        }
    }

    fn load_selected_file(&mut self) {
        let path_to_load = if let Some(item) = self.visible_items.get(self.selected_file_idx) {
            if !item.is_dir {
                Some((item.path.clone(), item.name.clone()))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((path, name)) = path_to_load {
            if let Ok(content) = fs::read_to_string(&path) {
                let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                self.editor = TextArea::from(lines.clone());
                self.editor.set_block(Block::default().borders(Borders::ALL).title(format!(" Editor - {} ", name)));
                self.editor.move_cursor(CursorMove::Top);
                
                self.apply_simple_highlighting(&path);
            }
        }
    }
    
    fn apply_simple_highlighting(&mut self, path: &PathBuf) {
        let keywords = if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext {
                "rs" => "fn|let|mut|struct|impl|enum|match|if|else|loop|while|for|return|pub|use|mod|crate",
                "py" => "def|class|if|else|elif|while|for|return|import|from|try|except|with|as|pass|lambda",
                "js" | "ts" => "function|const|let|var|if|else|while|for|return|import|export|class|interface|type",
                "go" => "func|package|import|var|const|type|struct|interface|if|else|for|return|go|defer",
                "c" | "cpp" | "h" => "int|char|void|if|else|while|for|return|struct|class|public|private|protected",
                "html" => "div|span|html|body|head|script|style|link|meta|title|h1|h2|h3|p|a|img|ul|ol|li",
                "css" => "color|background|margin|padding|border|display|position|width|height|font|text",
                "json" => "true|false|null",
                "md" => "#|\\*|-", 
                "toml" => "\\[|\\]", 
                _ => "",
            }
        } else {
            ""
        };

        if !keywords.is_empty() {
            let pattern = if keywords.chars().any(|c| !c.is_alphanumeric() && c != '|') {
                keywords.to_string() 
            } else {
                 format!("\\b({})\\b", keywords)
            };
            
            self.editor.set_search_pattern(pattern).ok();
            self.editor.set_search_style(Style::default().fg(Color::Magenta));
        } else {
             self.editor.set_search_pattern("").ok();
        }
    }
}

fn flatten_node(node: &FileNode, visible_items: &mut Vec<VisibleItem>) {
    visible_items.push(VisibleItem {
        name: node.name.clone(),
        path: node.path.clone(),
        is_dir: node.is_dir,
        depth: node.depth,
        expanded: node.expanded,
    });

    if node.expanded {
        for child in &node.children {
            flatten_node(child, visible_items);
        }
    }
}

fn toggle_node_recursive(nodes: &mut Vec<FileNode>, target: &PathBuf) -> bool {
    for node in nodes.iter_mut() {
        if &node.path == target {
            node.toggle_expand();
            return true;
        }
        if node.expanded {
            if toggle_node_recursive(&mut node.children, target) {
                return true;
            }
        }
    }
    false
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

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
        app.file_tree_scroll_state = app.file_tree_scroll_state.content_length(app.visible_items.len()).position(app.selected_file_idx);
        app.editor_scroll_state = app.editor_scroll_state.content_length(app.editor.lines().len()).position(app.editor.cursor().0);
        let chat_lines = app.chat_history.join("\n\n").lines().count(); 
        app.chat_scroll_state = app.chat_scroll_state.content_length(chat_lines).position(app.chat_scroll as usize);
        
        terminal.draw(|f| ui(f, app))?;

        // Wait for event
        if let Ok(event) = app.event_rx.recv() {
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
                    match input {
                        Event::Mouse(mouse) => {
                            match app.active_panel {
                                ActivePanel::Terminal => {
                                     let input_bytes = match mouse.kind {
                                        event::MouseEventKind::ScrollDown => vec![27, 91, 66], 
                                        event::MouseEventKind::ScrollUp => vec![27, 91, 65],   
                                        _ => vec![],
                                    };
                                    if !input_bytes.is_empty() {
                                        let _ = app.pty_writer.write_all(&input_bytes);
                                        let _ = app.pty_writer.flush();
                                    }
                                },
                                ActivePanel::FileTree => {
                                    match mouse.kind {
                                        event::MouseEventKind::ScrollDown => {
                                            if app.file_tree_scroll_offset < app.visible_items.len().saturating_sub(1) {
                                                app.file_tree_scroll_offset += 1;
                                            }
                                        },
                                        event::MouseEventKind::ScrollUp => {
                                            if app.file_tree_scroll_offset > 0 {
                                                app.file_tree_scroll_offset -= 1;
                                            }
                                        },
                                        _ => {}
                                    }
                                },
                                ActivePanel::Editor => {
                                     match mouse.kind {
                                        event::MouseEventKind::ScrollDown => {
                                            app.editor.move_cursor(CursorMove::Down);
                                        },
                                        event::MouseEventKind::ScrollUp => {
                                            app.editor.move_cursor(CursorMove::Up);
                                        },
                                        _ => {} // Ignore other mouse events for now
                                    }
                                },
                                ActivePanel::Chat => {
                                    match mouse.kind {
                                        event::MouseEventKind::ScrollDown => {
                                            app.chat_scroll = app.chat_scroll.saturating_add(1);
                                        },
                                        event::MouseEventKind::ScrollUp => {
                                            app.chat_scroll = app.chat_scroll.saturating_sub(1);
                                        },
                                        _ => {} // Ignore other mouse events for now
                                    }
                                }
                            }
                        },
                        Event::Key(key) => {
                            // Global Quit: Ctrl+Q
                            if let KeyCode::Char('q') = key.code {
                                 if key.modifiers.contains(KeyModifiers::CONTROL) {
                                     return Ok(());
                                 }
                            }
                            
                            // Panel Switch: Tab
                            if key.code == KeyCode::Tab {
                                app.active_panel = match app.active_panel {
                                    ActivePanel::FileTree => ActivePanel::Editor,
                                    ActivePanel::Editor => ActivePanel::Chat,
                                    ActivePanel::Chat => ActivePanel::Terminal,
                                    ActivePanel::Terminal => ActivePanel::FileTree,
                                };
                            } else {
                                // Panel Specific Input
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
                                            KeyCode::Char(c) => {
                                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                                    match c {
                                                        'h' => {
                                                            // Dump history to editor
                                                            if let Ok(buffer) = app.history_buffer.read() {
                                                                let clean_content = String::from_utf8_lossy(&buffer).to_string(); 
                                                                let lines: Vec<String> = clean_content.lines().map(|s| s.to_string()).collect();
                                                                app.editor = TextArea::from(lines);
                                                                app.editor.set_block(Block::default().borders(Borders::ALL).title(" Editor - Terminal History "));
                                                                app.active_panel = ActivePanel::Editor;
                                                            }
                                                            vec![]
                                                        }
                                                        'c' => vec![3], 
                                                        'd' => vec![4], 
                                                        'z' => vec![26], 
                                                        _ => vec![c as u8] 
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
                        }
                        _ => {} // Ignore other event types for now
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(f.area());

    // File Tree
    let height = chunks[0].height as usize;
    // Adjust scroll offset if selection moves out of view (keyboard nav)
    // This logic should ideally be in update loop, but here is okay for simple sync
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
    
    // We are manually handling selection style above, so we pass None to state to avoid double styling/confusion
    // or we can rely on manual styling entirely and just use state for nothing?
    // Ratatui List highlights item at state.selected().
    // If we manually styled it, we can set state.select(None).
    app.file_tree_state.select(None);
    
    f.render_stateful_widget(List::new(items).block(file_tree_block), chunks[0], &mut app.file_tree_state);
    
    app.file_tree_scroll_state = app.file_tree_scroll_state.content_length(app.visible_items.len()).position(app.file_tree_scroll_offset);
    
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        chunks[0],
        &mut app.file_tree_scroll_state
    );

    // Editor & Terminal
    let (editor_percent, terminal_percent) = if app.active_panel == ActivePanel::Terminal {
        (40, 60)
    } else {
        (60, 40)
    };

    let middle_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(editor_percent), Constraint::Percentage(terminal_percent)])
        .split(chunks[1]);

    // Editor
    let mut editor = app.editor.clone();
    let editor_title = if let Some(item) = app.visible_items.get(app.selected_file_idx) {
        if !item.is_dir {
             format!(" Editor - {} ", item.name)
        } else {
            " Editor ".to_string()
        }
    } else {
        " Editor ".to_string()
    };
    editor.set_block(Block::default()
        .borders(Borders::ALL)
        .title(editor_title)
        .border_style(if app.active_panel == ActivePanel::Editor { Style::default().fg(Color::Yellow) } else { Style::default() }));
    f.render_widget(&editor, middle_chunks[0]);
    
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        middle_chunks[0],
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
    f.render_widget(pseudo_term, middle_chunks[1]);
    
    let terminal_scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("▲"))
        .end_symbol(Some("▼"));
    
    let mut terminal_scroll_state = app.terminal_scroll_state
        .viewport_content_length(middle_chunks[1].height as usize);
        
    f.render_stateful_widget(
        terminal_scrollbar,
        middle_chunks[1],
        &mut terminal_scroll_state
    );

    // Chat
    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(chunks[2]);

    let chat_text = app.chat_history.join("\n\n");
    let chat_history_block = Block::default()
        .title(" AI Chat ")
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(Color::Yellow) } else { Style::default() });
    
    let chat_paragraph = Paragraph::new(chat_text)
        .block(chat_history_block)
        .wrap(Wrap { trim: true })
        .scroll((app.chat_scroll, 0));
        
    f.render_widget(chat_paragraph, chat_chunks[0]);
    
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼")),
        chat_chunks[0],
        &mut app.chat_scroll_state
    );

    let mut chat_input = app.chat_input.clone();
    chat_input.set_block(Block::default()
        .borders(Borders::ALL)
        .title(" Chat Input ")
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(Color::Yellow) } else { Style::default() }));
    f.render_widget(&chat_input, chat_chunks[1]);
}
