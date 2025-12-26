// TUI-specific theme with ratatui colors

use ratatui::style::Color;
use crate::shared::ThemeMode;

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
            background: Color::Reset,
            foreground: Color::Indexed(252),
            border: Color::Indexed(240),
            border_active: Color::Indexed(39),
            selection_bg: Color::Indexed(237),
            selection_fg: Color::Indexed(255),
            status_bar_bg: Color::Indexed(235),
            status_bar_fg: Color::Indexed(250),
            line_number: Color::Indexed(240),
            cursor_bg: Color::Indexed(252),
            cursor_fg: Color::Indexed(235),
            directory: Color::Indexed(39),
            file: Color::Indexed(252),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            background: Color::Indexed(255),
            foreground: Color::Indexed(233),
            border: Color::Indexed(245),
            border_active: Color::Indexed(33),
            selection_bg: Color::Indexed(250),
            selection_fg: Color::Indexed(233),
            status_bar_bg: Color::Indexed(253),
            status_bar_fg: Color::Indexed(233),
            line_number: Color::Indexed(244),
            cursor_bg: Color::Indexed(233),
            cursor_fg: Color::Indexed(255),
            directory: Color::Indexed(33),
            file: Color::Indexed(233),
        }
    }
}
