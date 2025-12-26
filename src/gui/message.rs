// Message types for iced application

use iced::keyboard;

/// Identifies which divider is being dragged
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divider {
    /// Horizontal divider between file tree and middle section
    FileTreeRight,
    /// Horizontal divider between middle section and chat
    ChatLeft,
    /// Vertical divider between editor and terminal
    EditorBottom,
}

#[derive(Debug, Clone)]
pub enum Message {
    // File tree interactions
    FileTreeSelect(usize),
    FileTreeToggle(usize),
    FileTreeUp,
    FileTreeDown,

    // Editor
    EditorScroll(f32),

    // Terminal
    TerminalStart,
    TerminalInput(String),
    TerminalTick,

    // Chat
    ChatInputChanged(String),
    ChatSend,

    // Theme
    ToggleTheme,

    // Panel focus
    FocusPanel(Panel),
    CyclePanel,

    // Panel resizing
    DividerDragStart(Divider),
    DividerDrag(f32, f32),  // (x, y) position
    DividerDragEnd,

    // Keyboard events
    KeyPressed(keyboard::Key, keyboard::Modifiers),

    // Menu dropdown
    MenuToggle(usize),  // Toggle menu dropdown by index
    MenuClose,          // Close any open menu

    // Menu actions (matching TUI)
    // File menu (0)
    MenuSettings,
    MenuFileSearch,
    MenuExit,
    // Edit menu (1)
    MenuCopy,
    MenuPaste,
    // View menu (2)
    MenuResetLayout,
    MenuToggleTheme,
    // Help menu (3)
    MenuAbout,

    // Application
    Quit,

    // Window events
    WindowResized(u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    FileTree,
    Editor,
    Terminal,
    Chat,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::FileTree => Panel::Editor,
            Panel::Editor => Panel::Terminal,
            Panel::Terminal => Panel::Chat,
            Panel::Chat => Panel::FileTree,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Panel::FileTree => "File Tree",
            Panel::Editor => "Editor",
            Panel::Terminal => "Terminal",
            Panel::Chat => "AI Chat",
        }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Panel::Editor
    }
}
