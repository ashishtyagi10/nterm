// Shared modules used by both TUI and GUI

pub mod ai;
pub mod config;
pub mod file_tree;
pub mod terminal;
pub mod theme;

// Re-export commonly used types
pub use ai::send_message;
pub use config::{Config, RecentWorkspace};
pub use file_tree::{FileNode, VisibleItem, flatten_node, toggle_node_recursive};
pub use terminal::{Terminal, TerminalCell, TerminalColor, TerminalEvent, TerminalSize};
pub use theme::ThemeMode;
