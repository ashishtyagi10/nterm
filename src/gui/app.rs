// iced GUI application for nterm - Terminal-style IDE

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, text, text_input, Column, Row, Space,
};
use iced::{Color, Element, Font, Length, Subscription, Task, Theme};
use iced::keyboard::{self, Key};
use iced::mouse;

use crate::shared::{Config, flatten_node, FileNode, VisibleItem, ThemeMode};

use super::message::{Divider, Message, Panel};
use super::syntax::SyntaxHighlighter;
use super::theme::{get_iced_theme, panel_style, TerminalColors};
use super::terminal_widget::TerminalView;

const FONT_SIZE: u16 = 13;
const HEADER_SIZE: u16 = 12;
const MENU_SIZE: u16 = 12;
const DIVIDER_WIDTH: f32 = 4.0;

// Text input IDs for focus management
const CHAT_INPUT_ID: &str = "chat_input";

/// Panel layout sizes (as fractions 0.0 to 1.0)
#[derive(Debug, Clone, Copy)]
pub struct PanelSizes {
    /// File tree width fraction (of total width)
    pub file_tree_width: f32,
    /// Chat width fraction (of total width)
    pub chat_width: f32,
    /// Editor height fraction (of middle section height)
    pub editor_height: f32,
}

impl Default for PanelSizes {
    fn default() -> Self {
        Self {
            file_tree_width: 0.20,  // 20%
            chat_width: 0.20,       // 20%
            editor_height: 0.60,    // 60% of middle
        }
    }
}

impl PanelSizes {
    /// Clamp all values to valid ranges
    pub fn clamp(&mut self) {
        self.file_tree_width = self.file_tree_width.clamp(0.10, 0.40);
        self.chat_width = self.chat_width.clamp(0.10, 0.40);
        self.editor_height = self.editor_height.clamp(0.20, 0.80);
    }
}

pub struct NtermGui {
    // Core state (reused from TUI)
    config: Config,
    file_tree: Vec<FileNode>,
    visible_items: Vec<VisibleItem>,
    selected_idx: usize,

    // Editor state
    editor_content: String,
    editor_file_path: Option<PathBuf>,
    editor_scroll: usize,

    // Terminal state
    terminal_view: TerminalView,

    // Chat state
    chat_messages: Vec<(String, String)>, // (role, content)
    chat_input: String,

    // UI state
    theme_mode: ThemeMode,
    active_panel: Panel,
    colors: TerminalColors,

    // Panel sizing
    panel_sizes: PanelSizes,
    dragging_divider: Option<Divider>,
    window_size: (f32, f32),

    // Menu state
    menu_open_idx: Option<usize>,

    // Current workspace
    workspace_path: PathBuf,

    // Syntax highlighting
    syntax_highlighter: SyntaxHighlighter,
}

impl NtermGui {
    pub fn new() -> (Self, Task<Message>) {
        let config = Config::load();
        let theme_mode = config.theme;
        let colors = TerminalColors::from_mode(theme_mode);

        // Use current directory as workspace
        let workspace_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let mut app = Self {
            config,
            file_tree: Vec::new(),
            visible_items: Vec::new(),
            selected_idx: 0,
            editor_content: String::from("// Welcome to nterm GUI\n// Select a file from the file tree to edit\n// \n// Keyboard shortcuts:\n//   Tab        - Cycle panels\n//   Ctrl+T     - Toggle theme\n//   Ctrl+Q     - Quit\n//   Arrow keys - Navigate\n//   Drag dividers to resize panels"),
            editor_file_path: None,
            editor_scroll: 0,
            terminal_view: TerminalView::new(),
            chat_messages: vec![
                ("System".to_string(), "Welcome to nterm AI Chat".to_string()),
            ],
            chat_input: String::new(),
            theme_mode,
            active_panel: Panel::FileTree,
            colors,
            panel_sizes: PanelSizes::default(),
            dragging_divider: None,
            window_size: (1200.0, 800.0),
            menu_open_idx: None,
            workspace_path,
            syntax_highlighter: SyntaxHighlighter::new(),
        };

        app.refresh_file_tree();

        (app, Task::none())
    }

    fn refresh_file_tree(&mut self) {
        self.file_tree.clear();

        if let Ok(entries) = fs::read_dir(&self.workspace_path) {
            let mut nodes: Vec<FileNode> = entries
                .filter_map(|e| e.ok())
                .map(|e| FileNode::from_path(e.path(), 0))
                .filter(|node| !node.name.starts_with('.'))
                .collect();

            nodes.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            });

            self.file_tree = nodes;
        }

        self.update_visible_items();
    }

    fn update_visible_items(&mut self) {
        self.visible_items.clear();
        for node in &self.file_tree {
            flatten_node(node, &mut self.visible_items);
        }
    }

    fn toggle_node(&mut self, idx: usize) {
        if idx >= self.visible_items.len() {
            return;
        }

        let target_path = self.visible_items[idx].path.clone();

        fn toggle_recursive(nodes: &mut Vec<FileNode>, target: &PathBuf) -> bool {
            for node in nodes.iter_mut() {
                if &node.path == target {
                    node.toggle_expand();
                    return true;
                }
                if node.expanded && toggle_recursive(&mut node.children, target) {
                    return true;
                }
            }
            false
        }

        toggle_recursive(&mut self.file_tree, &target_path);
        self.update_visible_items();
    }

    /// Preview a file in the editor without changing panel focus
    fn preview_file(&mut self, idx: usize) {
        if idx >= self.visible_items.len() {
            return;
        }

        let item = &self.visible_items[idx];
        if item.is_dir {
            return; // Don't preview directories
        }

        let path = item.path.clone();

        // Skip if already viewing this file
        if self.editor_file_path.as_ref() == Some(&path) {
            return;
        }

        // Check file size first to avoid blocking on large files
        const MAX_PREVIEW_SIZE: u64 = 512 * 1024; // 512KB limit for preview
        match fs::metadata(&path) {
            Ok(metadata) => {
                if metadata.len() > MAX_PREVIEW_SIZE {
                    self.editor_content = format!(
                        "// File too large to preview ({:.1} MB)\n// Press Enter to open anyway",
                        metadata.len() as f64 / (1024.0 * 1024.0)
                    );
                    self.editor_file_path = Some(path);
                    self.editor_scroll = 0;
                    return;
                }
            }
            Err(_) => {
                // Can't read metadata, try to read anyway
            }
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                self.editor_content = content;
                self.editor_file_path = Some(path);
                self.editor_scroll = 0;
            }
            Err(e) => {
                // Could be binary file or permission error
                self.editor_content = format!("// Cannot preview: {}", e);
                self.editor_file_path = Some(path);
                self.editor_scroll = 0;
            }
        }
    }

    /// Load a file and switch focus to editor (used for Enter key and mouse click)
    fn load_file(&mut self, idx: usize) {
        if idx >= self.visible_items.len() {
            return;
        }

        let item = &self.visible_items[idx];
        if item.is_dir {
            self.toggle_node(idx);
            return;
        }

        self.preview_file(idx);
        // Switch focus to editor when explicitly opening a file
        self.active_panel = Panel::Editor;
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FileTreeSelect(idx) => {
                self.selected_idx = idx;
                self.load_file(idx);
            }
            Message::FileTreeToggle(idx) => {
                self.toggle_node(idx);
            }
            Message::FileTreeUp => {
                if self.selected_idx > 0 {
                    self.selected_idx -= 1;
                }
            }
            Message::FileTreeDown => {
                if self.selected_idx + 1 < self.visible_items.len() {
                    self.selected_idx += 1;
                }
            }
            Message::TerminalStart => {
                if !self.terminal_view.is_running() {
                    if let Err(e) = self.terminal_view.start() {
                        self.chat_messages.push((
                            "System".to_string(),
                            format!("Failed to start terminal: {}", e),
                        ));
                    }
                }
            }
            Message::TerminalInput(input) => {
                if self.terminal_view.is_running() {
                    let _ = self.terminal_view.input(&input);
                }
            }
            Message::TerminalTick => {
                self.terminal_view.tick();
            }
            Message::ChatInputChanged(value) => {
                self.chat_input = value;
            }
            Message::ChatSend => {
                if !self.chat_input.trim().is_empty() {
                    let user_msg = self.chat_input.clone();
                    self.chat_messages.push(("You".to_string(), user_msg.clone()));
                    self.chat_input.clear();
                    // For now, echo back - TODO: integrate with AI
                    self.chat_messages.push(("AI".to_string(), format!("Echo: {}", user_msg)));
                }
            }
            Message::ToggleTheme => {
                self.theme_mode = match self.theme_mode {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                self.colors = TerminalColors::from_mode(self.theme_mode);
                self.config.theme = self.theme_mode;
                let _ = self.config.save();
            }
            Message::FocusPanel(panel) => {
                self.active_panel = panel;
                if panel == Panel::Chat {
                    return text_input::focus(text_input::Id::new(CHAT_INPUT_ID));
                }
            }
            Message::CyclePanel => {
                self.active_panel = self.active_panel.next();
                if self.active_panel == Panel::Chat {
                    return text_input::focus(text_input::Id::new(CHAT_INPUT_ID));
                }
            }
            Message::KeyPressed(key, modifiers) => {
                return self.handle_key(key, modifiers);
            }
            // Menu dropdown
            Message::MenuToggle(idx) => {
                if self.menu_open_idx == Some(idx) {
                    self.menu_open_idx = None;
                } else {
                    self.menu_open_idx = Some(idx);
                }
            }
            Message::MenuClose => {
                self.menu_open_idx = None;
            }
            // File menu actions
            Message::MenuSettings => {
                self.menu_open_idx = None;
                // TODO: show settings modal
                self.chat_messages.push((
                    "System".to_string(),
                    "Settings not yet implemented in GUI".to_string(),
                ));
            }
            Message::MenuFileSearch => {
                self.menu_open_idx = None;
                // TODO: show file search modal
                self.chat_messages.push((
                    "System".to_string(),
                    "File search not yet implemented in GUI".to_string(),
                ));
            }
            Message::MenuExit => {
                std::process::exit(0);
            }
            // Edit menu actions
            Message::MenuCopy => {
                self.menu_open_idx = None;
                // TODO: implement copy
            }
            Message::MenuPaste => {
                self.menu_open_idx = None;
                // TODO: implement paste
            }
            // View menu actions
            Message::MenuResetLayout => {
                self.menu_open_idx = None;
                self.panel_sizes = PanelSizes::default();
                self.active_panel = Panel::Editor;
            }
            Message::MenuToggleTheme => {
                self.menu_open_idx = None;
                self.theme_mode = match self.theme_mode {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                self.colors = TerminalColors::from_mode(self.theme_mode);
                self.config.theme = self.theme_mode;
                let _ = self.config.save();
            }
            // Help menu actions
            Message::MenuAbout => {
                self.menu_open_idx = None;
                self.chat_messages.push((
                    "System".to_string(),
                    "nterm v0.1.0 - A terminal-based IDE".to_string(),
                ));
            }
            Message::Quit => {
                std::process::exit(0);
            }
            Message::EditorScroll(_) => {}
            Message::WindowResized(w, h) => {
                self.window_size = (w as f32, h as f32);
            }
            Message::DividerDragStart(divider) => {
                self.dragging_divider = Some(divider);
            }
            Message::DividerDrag(x, y) => {
                if let Some(divider) = self.dragging_divider {
                    let (width, height) = self.window_size;
                    match divider {
                        Divider::FileTreeRight => {
                            self.panel_sizes.file_tree_width = x / width;
                        }
                        Divider::ChatLeft => {
                            self.panel_sizes.chat_width = 1.0 - (x / width);
                        }
                        Divider::EditorBottom => {
                            // Calculate relative to middle section
                            let menu_height = 30.0;
                            let status_height = 25.0;
                            let content_height = height - menu_height - status_height;
                            let relative_y = y - menu_height;
                            self.panel_sizes.editor_height = relative_y / content_height;
                        }
                    }
                    self.panel_sizes.clamp();
                }
            }
            Message::DividerDragEnd => {
                self.dragging_divider = None;
            }
        }

        Task::none()
    }

    fn handle_key(&mut self, key: Key, modifiers: keyboard::Modifiers) -> Task<Message> {
        // Global shortcuts first
        match key.as_ref() {
            Key::Named(keyboard::key::Named::Tab) => {
                if !modifiers.control() {
                    self.active_panel = self.active_panel.next();
                    // Focus chat input when switching to Chat panel
                    if self.active_panel == Panel::Chat {
                        return text_input::focus(text_input::Id::new(CHAT_INPUT_ID));
                    }
                    return Task::none();
                }
            }
            Key::Character("t") if modifiers.control() => {
                self.theme_mode = match self.theme_mode {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                self.colors = TerminalColors::from_mode(self.theme_mode);
                self.config.theme = self.theme_mode;
                let _ = self.config.save();
                return Task::none();
            }
            Key::Character("q") if modifiers.control() => {
                std::process::exit(0);
            }
            Key::Named(keyboard::key::Named::Escape) => {
                // Close menu if open
                if self.menu_open_idx.is_some() {
                    self.menu_open_idx = None;
                    return Task::none();
                }
            }
            _ => {}
        }

        // Close menu on any other key press
        self.menu_open_idx = None;

        // Panel-specific handling
        match self.active_panel {
            Panel::FileTree => {
                match key.as_ref() {
                    Key::Named(keyboard::key::Named::ArrowUp) => {
                        if self.selected_idx > 0 {
                            self.selected_idx -= 1;
                            // Preview file on keyboard navigation (keeps focus in file tree)
                            self.preview_file(self.selected_idx);
                        }
                    }
                    Key::Named(keyboard::key::Named::ArrowDown) => {
                        if self.selected_idx + 1 < self.visible_items.len() {
                            self.selected_idx += 1;
                            // Preview file on keyboard navigation (keeps focus in file tree)
                            self.preview_file(self.selected_idx);
                        }
                    }
                    Key::Named(keyboard::key::Named::Enter) => {
                        self.load_file(self.selected_idx);
                    }
                    Key::Named(keyboard::key::Named::ArrowRight) => {
                        if let Some(item) = self.visible_items.get(self.selected_idx) {
                            if item.is_dir && !item.expanded {
                                self.toggle_node(self.selected_idx);
                            }
                        }
                    }
                    Key::Named(keyboard::key::Named::ArrowLeft) => {
                        if let Some(item) = self.visible_items.get(self.selected_idx) {
                            if item.is_dir && item.expanded {
                                self.toggle_node(self.selected_idx);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Panel::Terminal => {
                // Handle terminal input
                if !self.terminal_view.is_running() {
                    // Start terminal on Enter
                    if matches!(key.as_ref(), Key::Named(keyboard::key::Named::Enter)) {
                        let _ = self.terminal_view.start();
                    }
                } else {
                    // Forward keys to terminal
                    match key.as_ref() {
                        Key::Character(c) if modifiers.control() => {
                            // Handle Ctrl+C, Ctrl+D, Ctrl+Z
                            match c {
                                "c" => { let _ = self.terminal_view.send_interrupt(); }
                                "d" => { let _ = self.terminal_view.send_eof(); }
                                "z" => { let _ = self.terminal_view.input_bytes(&[0x1A]); }
                                _ => {}
                            }
                        }
                        Key::Character(c) => {
                            let _ = self.terminal_view.input(c);
                        }
                        Key::Named(keyboard::key::Named::Enter) => {
                            let _ = self.terminal_view.input("\r");
                        }
                        Key::Named(keyboard::key::Named::Backspace) => {
                            let _ = self.terminal_view.input_bytes(&[0x7F]);
                        }
                        Key::Named(keyboard::key::Named::Escape) => {
                            let _ = self.terminal_view.input_bytes(&[0x1B]);
                        }
                        Key::Named(keyboard::key::Named::ArrowUp) => {
                            let _ = self.terminal_view.input_bytes(&[0x1B, b'[', b'A']);
                        }
                        Key::Named(keyboard::key::Named::ArrowDown) => {
                            let _ = self.terminal_view.input_bytes(&[0x1B, b'[', b'B']);
                        }
                        Key::Named(keyboard::key::Named::ArrowRight) => {
                            let _ = self.terminal_view.input_bytes(&[0x1B, b'[', b'C']);
                        }
                        Key::Named(keyboard::key::Named::ArrowLeft) => {
                            let _ = self.terminal_view.input_bytes(&[0x1B, b'[', b'D']);
                        }
                        Key::Named(keyboard::key::Named::Space) => {
                            let _ = self.terminal_view.input(" ");
                        }
                        _ => {}
                    }
                }
            }
            Panel::Editor | Panel::Chat => {}
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyPressed(key, modifiers))
        });

        // Timer subscription for terminal updates (poll every 50ms)
        let terminal_sub = if self.terminal_view.is_running() {
            iced::time::every(Duration::from_millis(50))
                .map(|_| Message::TerminalTick)
        } else {
            Subscription::none()
        };

        // Mouse tracking for divider dragging
        let mouse_sub = if self.dragging_divider.is_some() {
            iced::event::listen_with(|event, _status, _id| {
                match event {
                    iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                        Some(Message::DividerDrag(position.x, position.y))
                    }
                    iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                        Some(Message::DividerDragEnd)
                    }
                    _ => None,
                }
            })
        } else {
            Subscription::none()
        };

        Subscription::batch([keyboard_sub, terminal_sub, mouse_sub])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let menu_bar = self.view_menu_bar();
        let file_tree_panel = self.view_file_tree();
        let editor_panel = self.view_editor();
        let terminal_panel = self.view_terminal();
        let chat_panel = self.view_chat();
        let colors = self.colors;

        // Calculate FillPortion values - adjust based on active panel (like TUI)
        // Chat expands to 35% when focused (normally 20%)
        // Terminal expands to 55% height when focused (normally 40%)
        let (file_tree_portion, chat_portion) = if self.active_panel == Panel::Chat {
            (20u16, 35u16) // Chat focused: expand chat to 35%
        } else {
            (
                (self.panel_sizes.file_tree_width * 100.0) as u16,
                (self.panel_sizes.chat_width * 100.0) as u16,
            )
        };
        let middle_portion = 100 - file_tree_portion - chat_portion;

        let (editor_portion, terminal_portion) = if self.active_panel == Panel::Terminal {
            (45u16, 55u16) // Terminal focused: expand terminal to 55%
        } else {
            let editor = (self.panel_sizes.editor_height * 100.0) as u16;
            (editor, 100 - editor)
        };

        // Vertical divider style (between horizontal panels) - transparent, only visible when dragging
        let v_divider = |divider: Divider| -> Element<'_, Message> {
            let is_dragging = self.dragging_divider == Some(divider);
            mouse_area(
                container(Space::new(DIVIDER_WIDTH, Length::Fill))
                    .style(move |_theme| container::Style {
                        background: if is_dragging {
                            Some(colors.border_active.into())
                        } else {
                            None // Transparent when not dragging
                        },
                        ..Default::default()
                    })
            )
            .on_press(Message::DividerDragStart(divider))
            .into()
        };

        // Horizontal divider style (between vertical panels) - transparent, only visible when dragging
        let h_divider = |divider: Divider| -> Element<'_, Message> {
            let is_dragging = self.dragging_divider == Some(divider);
            mouse_area(
                container(Space::new(Length::Fill, DIVIDER_WIDTH))
                    .style(move |_theme| container::Style {
                        background: if is_dragging {
                            Some(colors.border_active.into())
                        } else {
                            None // Transparent when not dragging
                        },
                        ..Default::default()
                    })
            )
            .on_press(Message::DividerDragStart(divider))
            .into()
        };

        // Middle section: Editor on top, divider, Terminal on bottom
        let middle_section = column![
            container(editor_panel)
                .width(Length::Fill)
                .height(Length::FillPortion(editor_portion)),
            h_divider(Divider::EditorBottom),
            container(terminal_panel)
                .width(Length::Fill)
                .height(Length::FillPortion(terminal_portion)),
        ];

        // Main content with dividers
        let main_content = row![
            container(file_tree_panel)
                .width(Length::FillPortion(file_tree_portion))
                .height(Length::Fill),
            v_divider(Divider::FileTreeRight),
            container(middle_section)
                .width(Length::FillPortion(middle_portion))
                .height(Length::Fill),
            v_divider(Divider::ChatLeft),
            container(chat_panel)
                .width(Length::FillPortion(chat_portion))
                .height(Length::Fill),
        ]
        .height(Length::Fill);

        // Status bar at bottom
        let status_bar = self.view_status_bar();

        let content = column![
            menu_bar,
            main_content,
            status_bar,
        ];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(self.colors.background.into()),
                ..Default::default()
            })
            .into()
    }

    fn view_file_tree(&self) -> Element<'_, Message> {
        let is_active = self.active_panel == Panel::FileTree;
        let colors = self.colors;

        // Header
        let header = container(
            text(" File Tree")
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.foreground)
        )
        .padding([2, 5])
        .width(Length::Fill);

        // File items
        let items: Vec<Element<'_, Message>> = self
            .visible_items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let indent = "  ".repeat(item.depth);
                let icon = if item.is_dir {
                    if item.expanded { "v " } else { "+ " }
                } else {
                    "- "
                };

                let is_selected = idx == self.selected_idx;
                let item_color = if item.is_dir { colors.directory } else { colors.file };

                let label_text = text(format!("{}{}{}", indent, icon, item.name))
                    .size(FONT_SIZE)
                    .font(Font::MONOSPACE)
                    .color(if is_selected { colors.selection_fg } else { item_color });

                let btn = button(label_text)
                    .on_press(Message::FileTreeSelect(idx))
                    .width(Length::Fill)
                    .padding([1, 5])
                    .style(move |_theme, status| {
                        let bg = if is_selected {
                            Some(colors.selection_bg.into())
                        } else if matches!(status, button::Status::Hovered) {
                            Some(Color { a: 0.3, ..colors.selection_bg }.into())
                        } else {
                            None
                        };
                        button::Style {
                            background: bg,
                            text_color: if is_selected { colors.selection_fg } else { item_color },
                            ..Default::default()
                        }
                    });

                btn.into()
            })
            .collect();

        let file_list = scrollable(
            Column::with_children(items).spacing(0)
        )
        .height(Length::Fill)
        .width(Length::Fill);

        let content = column![
            header,
            file_list,
        ];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(2)
            .style(move |_theme| panel_style(&colors, is_active))
            .into()
    }

    fn view_menu_bar(&self) -> Element<'_, Message> {
        let colors = self.colors;
        let menu_open = self.menu_open_idx;

        // Menu button style
        let menu_btn_style = move |idx: usize| {
            move |_theme: &Theme, status: button::Status| {
                let is_open = menu_open == Some(idx);
                let bg = if is_open || matches!(status, button::Status::Hovered) {
                    Some(colors.selection_bg.into())
                } else {
                    None
                };
                button::Style {
                    background: bg,
                    text_color: colors.foreground,
                    ..Default::default()
                }
            }
        };

        // Menu item style
        let item_style = move |_theme: &Theme, status: button::Status| {
            let bg = if matches!(status, button::Status::Hovered) {
                Some(colors.selection_bg.into())
            } else {
                None
            };
            button::Style {
                background: bg,
                text_color: colors.foreground,
                ..Default::default()
            }
        };

        // Menu titles (matching TUI exactly)
        let file_btn = button(text(" File ").size(MENU_SIZE).font(Font::MONOSPACE))
            .on_press(Message::MenuToggle(0))
            .padding([4, 10])
            .style(menu_btn_style(0));

        let edit_btn = button(text(" Edit ").size(MENU_SIZE).font(Font::MONOSPACE))
            .on_press(Message::MenuToggle(1))
            .padding([4, 10])
            .style(menu_btn_style(1));

        let view_btn = button(text(" View ").size(MENU_SIZE).font(Font::MONOSPACE))
            .on_press(Message::MenuToggle(2))
            .padding([4, 10])
            .style(menu_btn_style(2));

        let help_btn = button(text(" Help ").size(MENU_SIZE).font(Font::MONOSPACE))
            .on_press(Message::MenuToggle(3))
            .padding([4, 10])
            .style(menu_btn_style(3));

        let menu_buttons = row![
            file_btn,
            edit_btn,
            view_btn,
            help_btn,
            Space::with_width(Length::Fill),
        ]
        .spacing(2)
        .padding([2, 5]);

        // Build dropdown if a menu is open
        let dropdown: Element<'_, Message> = if let Some(idx) = self.menu_open_idx {
            let items: Vec<(&str, Message)> = match idx {
                0 => vec![
                    ("Settings", Message::MenuSettings),
                    ("File Search", Message::MenuFileSearch),
                    ("Exit", Message::MenuExit),
                ],
                1 => vec![
                    ("Copy", Message::MenuCopy),
                    ("Paste", Message::MenuPaste),
                ],
                2 => vec![
                    ("Reset Layout", Message::MenuResetLayout),
                    ("Toggle Theme", Message::MenuToggleTheme),
                ],
                3 => vec![
                    ("About", Message::MenuAbout),
                ],
                _ => vec![],
            };

            let menu_items: Vec<Element<'_, Message>> = items
                .into_iter()
                .map(|(label, msg)| {
                    button(
                        text(format!("  {}  ", label))
                            .size(MENU_SIZE)
                            .font(Font::MONOSPACE)
                    )
                    .on_press(msg)
                    .width(Length::Fill)
                    .padding([4, 8])
                    .style(item_style)
                    .into()
                })
                .collect();

            let dropdown_content = Column::with_children(menu_items)
                .spacing(0)
                .width(Length::Shrink);

            // Position dropdown below the menu button
            let offset_x = (idx * 60) as u16; // Approximate button width

            row![
                Space::with_width(offset_x),
                container(dropdown_content)
                    .style(move |_theme| container::Style {
                        background: Some(colors.background.into()),
                        border: iced::Border {
                            color: colors.border,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .padding(2),
                Space::with_width(Length::Fill),
            ]
            .into()
        } else {
            Space::new(0, 0).into()
        };

        // Stack menu bar and dropdown
        column![
            menu_buttons,
            dropdown,
        ]
        .into()
    }

    fn view_editor(&self) -> Element<'_, Message> {
        let is_active = self.active_panel == Panel::Editor;
        let colors = self.colors;

        let file_name = self
            .editor_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        // Get file extension for syntax highlighting
        let extension = self
            .editor_file_path
            .as_ref()
            .and_then(|p| SyntaxHighlighter::extension_from_path(p));

        // Header
        let header = container(
            text(format!(" Editor - {}", file_name))
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.foreground)
        )
        .padding([2, 5])
        .width(Length::Fill);

        // Editor content with syntax-highlighted line numbers
        let lines: Vec<Element<'_, Message>> = self
            .editor_content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let line_num = text(format!("{:>4} ", i + 1))
                    .size(FONT_SIZE)
                    .font(Font::MONOSPACE)
                    .color(colors.line_number);

                // Get syntax-highlighted spans for this line
                let highlighted = self.syntax_highlighter.highlight_line(line, extension.as_deref());

                // Build row of highlighted text spans
                let spans: Vec<Element<'_, Message>> = highlighted
                    .into_iter()
                    .map(|span| {
                        text(span.text)
                            .size(FONT_SIZE)
                            .font(Font::MONOSPACE)
                            .color(span.color)
                            .into()
                    })
                    .collect();

                let line_content = Row::with_children(spans).spacing(0);

                row![line_num, line_content]
                    .spacing(2)
                    .into()
            })
            .collect();

        let editor_scroll = scrollable(
            Column::with_children(lines).spacing(0).padding([0, 5])
        )
        .height(Length::Fill)
        .width(Length::Fill);

        let content = column![
            header,
            editor_scroll,
        ];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(2)
            .style(move |_theme| panel_style(&colors, is_active))
            .into()
    }

    fn view_terminal(&self) -> Element<'_, Message> {
        let is_active = self.active_panel == Panel::Terminal;
        let colors = self.colors;

        // Header with start button
        let header_content: Element<'_, Message> = if !self.terminal_view.is_running() {
            let start_btn = button(
                text("Start Terminal")
                    .size(HEADER_SIZE)
                    .font(Font::MONOSPACE)
            )
            .on_press(Message::TerminalStart)
            .padding([2, 8])
            .style(move |_theme, status| {
                let bg = if matches!(status, button::Status::Hovered) {
                    Some(colors.selection_bg.into())
                } else {
                    Some(Color::from_rgb(
                        colors.background.r * 0.7,
                        colors.background.g * 0.7,
                        colors.background.b * 0.7,
                    ).into())
                };
                button::Style {
                    background: bg,
                    text_color: colors.foreground,
                    ..Default::default()
                }
            });

            row![
                text(" Terminal ")
                    .size(HEADER_SIZE)
                    .font(Font::MONOSPACE)
                    .color(colors.foreground),
                Space::with_width(Length::Fill),
                start_btn,
            ]
            .into()
        } else {
            text(" Terminal (running)")
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.foreground)
                .into()
        };

        let header = container(header_content)
            .padding([2, 5])
            .width(Length::Fill);

        // Terminal content
        let terminal_content = self.terminal_view.view(&colors);

        let content = column![
            header,
            terminal_content,
        ];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(2)
            .style(move |_theme| panel_style(&colors, is_active))
            .into()
    }

    fn view_chat(&self) -> Element<'_, Message> {
        let is_active = self.active_panel == Panel::Chat;
        let colors = self.colors;

        // Header
        let model_name = self.config.get_selected_model().name.clone();
        let header = container(
            text(format!(" AI Chat ({})", model_name))
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.foreground)
        )
        .padding([2, 5])
        .width(Length::Fill);

        // Chat messages
        let messages: Vec<Element<'_, Message>> = self
            .chat_messages
            .iter()
            .map(|(role, content)| {
                let role_color = if role == "You" {
                    colors.foreground
                } else if role == "AI" {
                    colors.directory
                } else {
                    colors.line_number
                };

                let role_text = text(format!("{}: ", role))
                    .size(FONT_SIZE)
                    .font(Font::MONOSPACE)
                    .color(role_color);

                let content_text = text(content)
                    .size(FONT_SIZE)
                    .font(Font::MONOSPACE)
                    .color(colors.foreground);

                column![
                    row![role_text, content_text],
                ]
                .spacing(2)
                .into()
            })
            .collect();

        let chat_scroll = scrollable(
            Column::with_children(messages).spacing(5).padding(5)
        )
        .height(Length::Fill)
        .width(Length::Fill);

        // Chat input field
        let input_field = text_input("Type a message...", &self.chat_input)
            .id(text_input::Id::new(CHAT_INPUT_ID))
            .on_input(Message::ChatInputChanged)
            .on_submit(Message::ChatSend)
            .padding(8)
            .size(FONT_SIZE)
            .font(Font::MONOSPACE)
            .width(Length::Fill)
            .style(move |_theme, _status| {
                text_input::Style {
                    background: colors.background.into(),
                    border: iced::Border {
                        color: colors.line_number,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    icon: colors.foreground,
                    placeholder: colors.line_number,
                    value: colors.foreground,
                    selection: colors.selection_bg,
                }
            });

        let send_button = button(
            text("Send")
                .size(FONT_SIZE)
                .font(Font::MONOSPACE)
        )
        .on_press(Message::ChatSend)
        .padding([8, 16])
        .style(move |_theme, status| {
            let bg = if matches!(status, button::Status::Hovered) {
                Some(colors.selection_bg.into())
            } else {
                Some(Color::from_rgb(
                    colors.background.r * 0.7,
                    colors.background.g * 0.7,
                    colors.background.b * 0.7,
                ).into())
            };
            button::Style {
                background: bg,
                text_color: colors.foreground,
                border: iced::Border {
                    color: colors.line_number,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        });

        let input_row = container(
            row![input_field, send_button].spacing(5).padding(5)
        )
        .width(Length::Fill);

        let content = column![
            header,
            chat_scroll,
            input_row,
        ];

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(2)
            .style(move |_theme| panel_style(&colors, is_active))
            .into()
    }

    fn view_status_bar(&self) -> Element<'_, Message> {
        let colors = self.colors;

        let theme_text = if self.theme_mode == ThemeMode::Dark { "Dark" } else { "Light" };

        let status = row![
            text(format!(" {} ", self.active_panel.title()))
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.selection_fg),
            Space::with_width(Length::Fill),
            text("Tab: Cycle | Ctrl+T: Theme | Ctrl+Q: Quit")
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.line_number),
            Space::with_width(10),
            text(format!("[{}]", theme_text))
                .size(HEADER_SIZE)
                .font(Font::MONOSPACE)
                .color(colors.directory),
            Space::with_width(5),
        ]
        .padding([3, 5]);

        container(status)
            .width(Length::Fill)
            .into()
    }

    pub fn theme(&self) -> Theme {
        get_iced_theme(&self.theme_mode)
    }

    pub fn title(&self) -> String {
        let workspace = self
            .workspace_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "nterm".to_string());

        format!("nterm - {}", workspace)
    }
}

impl Default for NtermGui {
    fn default() -> Self {
        Self::new().0
    }
}
