use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame, Terminal,
};
use std::{error::Error, fs, io, path::PathBuf};
use tui_textarea::TextArea;

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

struct App<'a> {
    file_tree: Vec<FileNode>,
    visible_items: Vec<VisibleItem>,
    selected_file_idx: usize,
    editor: TextArea<'a>,
    chat_input: TextArea<'a>,
    chat_history: Vec<String>,
    terminal_logs: Vec<String>,
    terminal_input: String,
    active_panel: ActivePanel,
    should_quit: bool,
}

impl<'a> App<'a> {
    fn new() -> Self {
        let mut editor = TextArea::default();
        editor.set_block(Block::default().borders(Borders::ALL).title(" Editor "));
        
        let mut chat_input = TextArea::default();
        chat_input.set_block(Block::default().borders(Borders::ALL).title(" Chat Input "));

        let mut app = Self {
            file_tree: Vec::new(),
            visible_items: Vec::new(),
            selected_file_idx: 0,
            editor,
            chat_input,
            chat_history: vec!["Hello! I'm your AI assistant. Press Tab to switch panels.".to_string()],
            terminal_logs: vec!["nterm initialized.".to_string()],
            terminal_input: String::new(),
            active_panel: ActivePanel::FileTree,
            should_quit: false,
        };
        
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
        if let Some(item) = self.visible_items.get(self.selected_file_idx) {
            if !item.is_dir {
                if let Ok(content) = fs::read_to_string(&item.path) {
                    let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                    self.editor = TextArea::from(lines);
                    self.editor.set_block(Block::default().borders(Borders::ALL).title(format!(" Editor - {} ", item.name)));
                }
            }
        }
    }
    
    fn execute_command(&mut self) {
        let input = self.terminal_input.trim();
        if input.is_empty() {
            return;
        }
        
        self.terminal_logs.push(format!("$ {}", input));
        
        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts.as_slice() {
            ["exit"] => {
                self.should_quit = true;
            }
            ["cd", path] => {
                if std::env::set_current_dir(path).is_ok() {
                    self.terminal_logs.push(format!("Changed directory to {}", path));
                    self.refresh_file_tree();
                    self.selected_file_idx = 0;
                } else {
                    self.terminal_logs.push(format!("Failed to change directory to {}", path));
                }
            }
            ["clear"] => {
                self.terminal_logs.clear();
            }
            _ => {
                use std::process::Command;
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(input)
                    .output();
                    
                match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        
                        for line in stdout.lines() {
                            self.terminal_logs.push(line.to_string());
                        }
                        for line in stderr.lines() {
                            self.terminal_logs.push(line.to_string());
                        }
                    }
                    Err(e) => {
                        self.terminal_logs.push(format!("Error: {}", e));
                    }
                }
            }
        }
        self.terminal_input.clear();
    }
}

// Standalone functions to avoid borrow checker issues

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        if app.should_quit {
            return Ok(());
        }
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                KeyCode::Tab => {
                    app.active_panel = match app.active_panel {
                        ActivePanel::FileTree => ActivePanel::Editor,
                        ActivePanel::Editor => ActivePanel::Chat,
                        ActivePanel::Chat => ActivePanel::Terminal,
                        ActivePanel::Terminal => ActivePanel::FileTree,
                    };
                }
                _ => {
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
                            } else {
                                app.chat_input.input(key);
                            }
                        }
                        ActivePanel::FileTree => {
                            match key.code {
                                KeyCode::Up => {
                                    if app.selected_file_idx > 0 {
                                        app.selected_file_idx -= 1;
                                        app.load_selected_file();
                                    }
                                }
                                KeyCode::Down => {
                                    if app.selected_file_idx < app.visible_items.len().saturating_sub(1) {
                                        app.selected_file_idx += 1;
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
                                _ => {}
                            }
                        }
                        ActivePanel::Terminal => {
                             match key.code {
                                KeyCode::Enter => {
                                    app.execute_command();
                                }
                                KeyCode::Backspace => {
                                    app.terminal_input.pop();
                                }
                                KeyCode::Char(c) => {
                                    app.terminal_input.push(c);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(f.area());

    // File Tree
    let items: Vec<ListItem> = app.visible_items.iter().enumerate().map(|(i, item)| {
        let style = if i == app.selected_file_idx {
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
    f.render_widget(List::new(items).block(file_tree_block), chunks[0]);

    // Editor & Terminal
    let middle_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
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

    // Terminal
    let mut terminal_items: Vec<ListItem> = app.terminal_logs.iter().rev().take(15).rev().map(|log| ListItem::new(log.as_str())).collect();
    terminal_items.push(ListItem::new(format!("> {}", app.terminal_input)).style(Style::default().fg(Color::Cyan)));

    let terminal_block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::Terminal { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(List::new(terminal_items).block(terminal_block), middle_chunks[1]);

    // Chat
    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(chunks[2]);

    let chat_history_items: Vec<ListItem> = app.chat_history.iter().map(|msg| ListItem::new(msg.as_str())).collect();
    let chat_history_block = Block::default()
        .title(" AI Chat ")
        .borders(Borders::ALL)
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(List::new(chat_history_items).block(chat_history_block), chat_chunks[0]);

    let mut chat_input = app.chat_input.clone();
    chat_input.set_block(Block::default()
        .borders(Borders::ALL)
        .title(" Chat Input ")
        .border_style(if app.active_panel == ActivePanel::Chat { Style::default().fg(Color::Yellow) } else { Style::default() }));
    f.render_widget(&chat_input, chat_chunks[1]);
}
