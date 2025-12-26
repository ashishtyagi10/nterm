// TUI (Terminal User Interface) module for nterm
// Built with ratatui for terminal-based rendering

pub mod action;
pub mod app;
pub mod editor;
pub mod theme;
pub mod ui;
pub mod workspace_selector;

// Re-export commonly used types
pub use action::Action;
pub use app::{App, AppEvent, ActivePanel};
pub use ui::{ui, get_layout_chunks};
pub use workspace_selector::WorkspaceSelector;
