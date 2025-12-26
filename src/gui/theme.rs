// Terminal-style theme for iced GUI

use iced::theme::Palette;
use iced::{Border, Color, Theme as IcedTheme};
use iced::widget::container;
use crate::shared::ThemeMode;

/// Terminal color palette matching the TUI version
#[derive(Debug, Clone, Copy)]
pub struct TerminalColors {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub border_active: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub directory: Color,
    pub file: Color,
    pub line_number: Color,
    pub comment: Color,
    pub keyword: Color,
    pub string: Color,
}

impl TerminalColors {
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.10, 0.10, 0.10),      // #1a1a1a
            foreground: Color::from_rgb(0.82, 0.82, 0.82),      // #d0d0d0
            border: Color::from_rgb(0.35, 0.35, 0.35),          // #585858
            border_active: Color::from_rgb(0.0, 0.69, 1.0),     // #00afff - bright blue
            selection_bg: Color::from_rgb(0.23, 0.23, 0.23),    // #3a3a3a
            selection_fg: Color::from_rgb(1.0, 1.0, 1.0),       // #ffffff
            directory: Color::from_rgb(0.0, 0.69, 1.0),         // #00afff - blue
            file: Color::from_rgb(0.82, 0.82, 0.82),            // #d0d0d0
            line_number: Color::from_rgb(0.45, 0.45, 0.45),     // #737373
            comment: Color::from_rgb(0.45, 0.55, 0.45),         // greenish gray
            keyword: Color::from_rgb(0.8, 0.4, 0.8),            // purple
            string: Color::from_rgb(0.6, 0.8, 0.4),             // green
        }
    }

    pub fn light() -> Self {
        Self {
            background: Color::from_rgb(0.98, 0.98, 0.98),      // #fafafa
            foreground: Color::from_rgb(0.11, 0.11, 0.11),      // #1c1c1c
            border: Color::from_rgb(0.54, 0.54, 0.54),          // #8a8a8a
            border_active: Color::from_rgb(0.0, 0.53, 1.0),     // #0087ff
            selection_bg: Color::from_rgb(0.85, 0.85, 0.85),    // light gray
            selection_fg: Color::from_rgb(0.0, 0.0, 0.0),       // black
            directory: Color::from_rgb(0.0, 0.53, 1.0),         // #0087ff
            file: Color::from_rgb(0.11, 0.11, 0.11),            // #1c1c1c
            line_number: Color::from_rgb(0.6, 0.6, 0.6),        // gray
            comment: Color::from_rgb(0.4, 0.5, 0.4),            // greenish gray
            keyword: Color::from_rgb(0.6, 0.2, 0.6),            // purple
            string: Color::from_rgb(0.3, 0.6, 0.2),             // green
        }
    }

    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }
}

/// Get the iced theme based on mode
pub fn get_iced_theme(mode: &ThemeMode) -> IcedTheme {
    let colors = TerminalColors::from_mode(*mode);

    IcedTheme::custom(
        "Terminal".to_string(),
        Palette {
            background: colors.background,
            text: colors.foreground,
            primary: colors.border_active,
            success: Color::from_rgb(0.4, 0.8, 0.4),
            danger: Color::from_rgb(0.9, 0.3, 0.3),
        },
    )
}

/// Panel container style with border
pub fn panel_style(colors: &TerminalColors, is_active: bool) -> container::Style {
    let border_color = if is_active {
        colors.border_active
    } else {
        colors.border
    };

    container::Style {
        background: Some(colors.background.into()),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Transparent container style (no background)
pub fn transparent_style() -> container::Style {
    container::Style {
        background: None,
        ..Default::default()
    }
}
