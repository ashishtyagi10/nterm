// Custom widget styles for terminal look

use iced::widget::{button, container, scrollable};
use iced::{Border, Color, Background};
use super::theme::TerminalColors;

/// Button style for file tree items
pub fn file_tree_button(colors: &TerminalColors, is_selected: bool, is_directory: bool) -> button::Style {
    let (bg, fg) = if is_selected {
        (Some(colors.selection_bg.into()), colors.selection_fg)
    } else {
        (None, if is_directory { colors.directory } else { colors.file })
    };

    button::Style {
        background: bg,
        text_color: fg,
        border: Border::default(),
        ..Default::default()
    }
}

/// Hover style for file tree items
pub fn file_tree_button_hover(colors: &TerminalColors, is_directory: bool) -> button::Style {
    button::Style {
        background: Some(colors.selection_bg.into()),
        text_color: if is_directory { colors.directory } else { colors.selection_fg },
        border: Border::default(),
        ..Default::default()
    }
}

/// Menu button style
pub fn menu_button(colors: &TerminalColors) -> button::Style {
    button::Style {
        background: None,
        text_color: colors.foreground,
        border: Border::default(),
        ..Default::default()
    }
}

/// Menu button hover style
pub fn menu_button_hover(colors: &TerminalColors) -> button::Style {
    button::Style {
        background: Some(colors.selection_bg.into()),
        text_color: colors.selection_fg,
        border: Border::default(),
        ..Default::default()
    }
}

/// Header/title bar style
pub fn header_style(colors: &TerminalColors) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(
            colors.background.r * 0.8,
            colors.background.g * 0.8,
            colors.background.b * 0.8,
        ))),
        border: Border {
            color: colors.border,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Scrollable style
pub fn scrollable_style(colors: &TerminalColors) -> scrollable::Style {
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: scrollable::Rail {
            background: Some(colors.background.into()),
            border: Border::default(),
            scroller: scrollable::Scroller {
                color: colors.border,
                border: Border::default(),
            },
        },
        horizontal_rail: scrollable::Rail {
            background: Some(colors.background.into()),
            border: Border::default(),
            scroller: scrollable::Scroller {
                color: colors.border,
                border: Border::default(),
            },
        },
        gap: None,
    }
}
