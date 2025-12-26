// Terminal emulation module
// Provides shared terminal functionality for both TUI and GUI

mod term;

pub use term::{Terminal, TerminalCell, TerminalColor, TerminalEvent, TerminalSize};
