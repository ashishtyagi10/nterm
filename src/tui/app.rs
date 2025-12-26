use ratatui::{
    crossterm::event::{Event, KeyCode, KeyModifiers},
    widgets::{Block, Borders, ListState, ScrollbarState},
};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, RwLock, mpsc, Mutex},
    thread,
    time::Duration,
};
use tui_textarea::TextArea;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use walkdir::WalkDir;
use arboard::Clipboard;

use super::action::Action;
use super::editor::EditorState;
use super::theme::Theme;
use crate::shared::{FileNode, VisibleItem, flatten_node, toggle_node_recursive};
use crate::shared::send_message;
use crate::shared::Config;

#[derive(PartialEq)]

pub enum ActivePanel {

    FileTree,

    Editor,

    Terminal,

    Chat,

}



pub enum AppEvent {

    Input(Event),

    PtyData,

    Tick,

    AiResponse(String),

}



pub struct App<'a> {

    pub file_tree: Vec<FileNode>,

    pub visible_items: Vec<VisibleItem>,

    pub selected_file_idx: usize,

    pub file_tree_state: ListState,

    pub file_tree_scroll_offset: usize,

    pub file_tree_scroll_state: ScrollbarState,

    

    pub editor_state: EditorState,

    pub editor_scroll_state: ScrollbarState,

    

    pub chat_input: TextArea<'a>,

    pub chat_history: Vec<String>,

    pub chat_scroll: u16,

    pub chat_scroll_state: ScrollbarState,

    

    pub is_searching: bool,

    pub search_input: TextArea<'a>,

    pub search_results: Vec<PathBuf>,

    pub search_state: ListState,

    

    pub show_settings: bool,

    pub settings_input: TextArea<'a>,

    pub settings_model_idx: usize,

    pub settings_editing: bool,  // Whether currently editing an API key

    pub settings_scroll_offset: usize,  // Scroll offset for settings list

    pub config: Config,



    pub active_panel: ActivePanel,

    pub should_quit: bool,

    

    // Terminal State

    pub pty_writer: Box<dyn Write + Send>,

    pub terminal_screen: Arc<RwLock<tui_term::vt100::Parser>>,

    pub terminal_scroll_state: ScrollbarState,

    pub history_buffer: Arc<RwLock<Vec<u8>>>,

    pub event_rx: mpsc::Receiver<AppEvent>,

    pub event_tx: mpsc::Sender<AppEvent>,

    

    // Clipboard

    pub clipboard: Option<Arc<Mutex<Clipboard>>>,

    

    // Menus & Keys

    pub menu_titles: Vec<String>,

    pub menu_open_idx: Option<usize>,

    pub menu_hover_idx: Option<usize>,

    pub key_map: HashMap<(KeyCode, KeyModifiers), Action>,

    pub current_theme: Theme,
}



impl<'a> App<'a> {

    pub fn new() -> Self {

        let editor_state = EditorState::new();



        let mut chat_input = TextArea::default();

        chat_input.set_block(Block::default().borders(Borders::ALL).title(" Chat Input "));

        

        let mut search_input = TextArea::default();

        search_input.set_block(Block::default().borders(Borders::ALL).title(" Search Files "));

        

        let config = Config::load();

        let mut settings_input = TextArea::default();
        settings_input.set_block(Block::default().borders(Borders::ALL).title(" API Key "));
        // Load API key from first model if available
        if let Some(model) = config.models.first() {
            if let Some(key) = &model.api_key {
                settings_input.insert_str(key);
            }
        }



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

        let input_tx = tx.clone();

        thread::spawn(move || {

            loop {

                if let Ok(event) = ratatui::crossterm::event::read() {

                    let _ = input_tx.send(AppEvent::Input(event));

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

        

        // Clipboard

        let clipboard = Clipboard::new().ok().map(|c| Arc::new(Mutex::new(c)));

        

        // Key Binding Init

        let mut key_map = HashMap::new();

        key_map.insert((KeyCode::Char('q'), KeyModifiers::CONTROL), Action::Quit);

        key_map.insert((KeyCode::Tab, KeyModifiers::NONE), Action::SwitchFocus);

        key_map.insert((KeyCode::Esc, KeyModifiers::NONE), Action::ToggleMenu); 

        key_map.insert((KeyCode::F(1), KeyModifiers::NONE), Action::ToggleMenu);

        key_map.insert((KeyCode::Char('r'), KeyModifiers::CONTROL), Action::ResetLayout);

        key_map.insert((KeyCode::Char('h'), KeyModifiers::CONTROL), Action::DumpHistory);

        key_map.insert((KeyCode::Char('p'), KeyModifiers::CONTROL), Action::FileSearch);

        key_map.insert((KeyCode::Char('m'), KeyModifiers::CONTROL), Action::CycleModel);

        key_map.insert((KeyCode::Char('s'), KeyModifiers::CONTROL), Action::OpenSettings);
        key_map.insert((KeyCode::Char('c'), KeyModifiers::CONTROL), Action::Copy);
        key_map.insert((KeyCode::Char('v'), KeyModifiers::CONTROL), Action::Paste);



        let theme_mode = config.theme;

        let mut app = Self {

            file_tree: Vec::new(),

            visible_items: Vec::new(),

            selected_file_idx: 0,
            
            file_tree_state: ListState::default(),

            file_tree_scroll_offset: 0,

            file_tree_scroll_state: ScrollbarState::default(),

            

            editor_state,

            editor_scroll_state: ScrollbarState::default(),

            

            chat_input,

            chat_history: vec!["Hello! I'm your AI assistant. Press Tab to switch panels.".to_string()],

            chat_scroll: 0,

            chat_scroll_state: ScrollbarState::default(),



            is_searching: false,

            search_input,

            search_results: Vec::new(),

            search_state: ListState::default(),

            

            show_settings: false,

            settings_input,

            settings_model_idx: 0,

            settings_editing: false,

            settings_scroll_offset: 0,

            config,

            

            active_panel: ActivePanel::FileTree,

            should_quit: false,

            pty_writer: writer,

            terminal_screen: parser,

            terminal_scroll_state: ScrollbarState::default(),

            history_buffer: history,

            event_rx: rx,

            event_tx: tx,

            

            clipboard,

            

            menu_titles: vec![" File ".to_string(), " Edit ".to_string(), " View ".to_string(), " Help ".to_string()],

            menu_open_idx: None,

            menu_hover_idx: None,

            key_map,

            current_theme: Theme::new(theme_mode),

        };

        

        app.file_tree_state.select(Some(0));

        app.refresh_file_tree();

        app

    }

    pub fn refresh_file_tree(&mut self) {
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

    pub fn update_visible_items(&mut self) {
        let mut new_items = Vec::new();
        for node in &self.file_tree {
            flatten_node(node, &mut new_items);
        }
        self.visible_items = new_items;
    }

    pub fn toggle_selected_dir(&mut self) {
        if let Some(item) = self.visible_items.get(self.selected_file_idx) {
            if item.is_dir {
                let path_to_toggle = item.path.clone();
                toggle_node_recursive(&mut self.file_tree, &path_to_toggle);
                self.update_visible_items();
            }
        }
    }

    pub fn load_selected_file(&mut self) {
        if let Some(item) = self.visible_items.get(self.selected_file_idx) {
            if !item.is_dir {
                let _ = self.editor_state.load_file(item.path.clone());
            }
        }
    }

    pub fn load_file_path(&mut self, path: PathBuf) {
        let _ = self.editor_state.load_file(path);
    }

    pub fn on_search_input(&mut self) {
        let query = self.search_input.lines().join(" ");
        if query.trim().is_empty() {
            self.search_results.clear();
            return;
        }
        
        let query_lower = query.to_lowercase();
        self.search_results = WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| !e.path().to_string_lossy().starts_with("./.git"))
            .filter(|e| !e.path().to_string_lossy().contains("/target/"))
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            })
            .take(20)
            .map(|e| e.path().to_path_buf())
            .collect();
            
        self.search_state.select(Some(0));
    }
    
    pub fn cycle_model(&mut self) {
        self.config.cycle_model();
        let _ = self.config.save();
    }

    pub fn get_selected_model_name(&self) -> String {
        self.config.get_selected_model().display_name()
    }
    
    pub fn toggle_theme(&mut self) {
        use crate::shared::ThemeMode;
        self.config.theme = match self.config.theme {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        };
        self.current_theme = Theme::new(self.config.theme);
        let _ = self.config.save();

        // Reset the vt100 parser to apply new default colors
        if let Ok(mut parser) = self.terminal_screen.write() {
            let (rows, cols) = {
                let size = parser.screen().size();
                (size.0, size.1)
            };
            // Create new parser with same dimensions - this resets colors
            *parser = tui_term::vt100::Parser::new(rows, cols, 0);
        }

        // Send escape codes to reset terminal and re-run prompt
        // \x1b[0m - Reset all attributes
        // \x1b[2J - Clear screen
        // \x1b[H - Move cursor to home
        let reset_seq = "\x1b[0m\x1b[2J\x1b[H";
        let _ = self.pty_writer.write_all(reset_seq.as_bytes());
        let _ = self.pty_writer.flush();
    }

    pub fn send_chat_message(&mut self, content: String) {
        self.chat_history.push(format!("You: {}", content));

        let tx = self.event_tx.clone();
        let model_config = self.config.get_selected_model().clone();
        let history = self.chat_history.clone();

        tokio::spawn(async move {
            let response = match send_message(&model_config, &history, &content).await {
                Ok(resp) => resp,
                Err(e) => format!("Error: {}", e),
            };

            let _ = tx.send(AppEvent::AiResponse(response));
        });
    }

    /// Returns the menu items for a given menu index
    pub fn get_menu_items(idx: usize) -> Vec<(&'static str, Action)> {
        match idx {
            0 => vec![
                ("Settings", Action::OpenSettings),
                ("File Search", Action::FileSearch),
                ("Exit", Action::Quit),
            ],
            1 => vec![
                ("Copy", Action::Copy),
                ("Paste", Action::Paste),
            ],
            2 => vec![
                ("Reset Layout", Action::ResetLayout),
                ("Dump History", Action::DumpHistory),
            ],
            3 => vec![
                ("About", Action::About),
            ],
            _ => vec![],
        }
    }

    /// Open settings and load current model's API key
    pub fn open_settings(&mut self) {
        self.show_settings = true;
        self.settings_model_idx = 0;
        self.settings_editing = false;
        self.settings_scroll_offset = 0;
        self.load_settings_for_model(0);
    }

    /// Navigate to next model in settings
    pub fn settings_select_next(&mut self) {
        if !self.config.models.is_empty() {
            // Save current if editing
            if self.settings_editing {
                self.save_current_model_key();
            }
            self.settings_model_idx = (self.settings_model_idx + 1) % self.config.models.len();
            self.settings_editing = false;
            self.load_settings_for_model(self.settings_model_idx);
        }
    }

    /// Navigate to previous model in settings
    pub fn settings_select_prev(&mut self) {
        if !self.config.models.is_empty() {
            // Save current if editing
            if self.settings_editing {
                self.save_current_model_key();
            }
            self.settings_model_idx = if self.settings_model_idx == 0 {
                self.config.models.len() - 1
            } else {
                self.settings_model_idx - 1
            };
            self.settings_editing = false;
            self.load_settings_for_model(self.settings_model_idx);
        }
    }

    /// Start editing the current model's API key
    pub fn settings_start_edit(&mut self) {
        self.settings_editing = true;
    }

    /// Stop editing and save
    pub fn settings_stop_edit(&mut self) {
        if self.settings_editing {
            self.save_current_model_key();
            self.settings_editing = false;
        }
    }

    /// Save just the current model's API key (without closing settings)
    fn save_current_model_key(&mut self) {
        let api_key = self.settings_input.lines().join("");
        let api_key = if api_key.trim().is_empty() {
            None
        } else {
            Some(api_key.trim().to_string())
        };

        if let Some(model) = self.config.models.get_mut(self.settings_model_idx) {
            model.api_key = api_key;
        }
        let _ = self.config.save();
    }

    /// Set the selected model as the active model for chat
    pub fn settings_set_active_model(&mut self) {
        self.config.selected_model_idx = self.settings_model_idx;
        let _ = self.config.save();
    }

    /// Load the API key for the specified model into the settings input
    pub fn load_settings_for_model(&mut self, idx: usize) {
        self.settings_model_idx = idx;
        // Clear and reload
        while self.settings_input.delete_char() {}
        while self.settings_input.delete_newline() {}
        // Select all and delete to clear
        self.settings_input.select_all();
        self.settings_input.cut();

        if let Some(model) = self.config.models.get(idx) {
            if let Some(key) = &model.api_key {
                self.settings_input.insert_str(key);
            }
        }
    }

}