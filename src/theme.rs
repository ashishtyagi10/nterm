use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Dark
    }
}

pub struct Theme {
    pub mode: ThemeMode,
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub border_active: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub line_number: Color,
    pub cursor_bg: Color,
    pub cursor_fg: Color,
    pub directory: Color,
    pub file: Color,
}

impl Theme {
    pub fn new(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            // Use RGB for specific shades if possible, or standard ANSI colors that look good.
            // Using standard colors for broad compatibility but carefully chosen.
            background: Color::Reset, // Keep terminal default background for transparency support
            foreground: Color::Indexed(252), // Near white (Light Gray)
            border: Color::Indexed(240), // Dark Gray
            border_active: Color::Indexed(39), // Bright Blue
            selection_bg: Color::Indexed(237), // Dark Gray background for selection
            selection_fg: Color::Indexed(255), // Bright White text
            status_bar_bg: Color::Indexed(235), // Very Dark Gray
            status_bar_fg: Color::Indexed(250), // Light Gray
            line_number: Color::Indexed(240), // Dark Gray
            cursor_bg: Color::Indexed(252), // Near White
            cursor_fg: Color::Indexed(235), // Dark back
            directory: Color::Indexed(39), // Blue
            file: Color::Indexed(252), // Near White
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            background: Color::Indexed(255), // Pure White
            foreground: Color::Indexed(233), // Very Dark Gray (almost black)
            border: Color::Indexed(245), // Medium Gray
            border_active: Color::Indexed(33), // Medium Blue
            selection_bg: Color::Indexed(250), // Very Light Gray
            selection_fg: Color::Indexed(233), // Dark Text
            status_bar_bg: Color::Indexed(253), // Light Gray
            status_bar_fg: Color::Indexed(233), // Dark Text
            line_number: Color::Indexed(244), // Gray
            cursor_bg: Color::Indexed(233), // Dark Cursor
            cursor_fg: Color::Indexed(255), // White Text
            directory: Color::Indexed(33), // Blue
            file: Color::Indexed(233), // Dark Text
        }
    }
}
