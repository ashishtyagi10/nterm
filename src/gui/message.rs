// Message types for iced application

use iced::keyboard;

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

    // Keyboard events
    KeyPressed(keyboard::Key, keyboard::Modifiers),

    // Menu actions
    MenuNewFile,
    MenuOpenFolder,
    MenuSaveFile,
    MenuSettings,
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
