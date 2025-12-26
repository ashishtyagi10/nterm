// Syntax highlighting for iced GUI editor

use iced::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;

/// A highlighted text segment with color information
#[derive(Debug, Clone)]
pub struct HighlightedSpan {
    pub text: String,
    pub color: Color,
}

/// Syntax highlighter for the GUI editor
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Convert syntect style to iced Color
    fn style_to_color(style: &Style) -> Color {
        Color::from_rgb8(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        )
    }

    /// Get the file extension from a path
    pub fn extension_from_path(path: &std::path::Path) -> Option<String> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
    }

    /// Highlight a single line of code
    pub fn highlight_line(&self, line: &str, extension: Option<&str>) -> Vec<HighlightedSpan> {
        let syntax = extension
            .and_then(|ext| self.syntax_set.find_syntax_by_extension(ext))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        match highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => {
                ranges
                    .into_iter()
                    .map(|(style, text)| HighlightedSpan {
                        text: text.to_string(),
                        color: Self::style_to_color(&style),
                    })
                    .collect()
            }
            Err(_) => {
                // Fall back to plain text
                vec![HighlightedSpan {
                    text: line.to_string(),
                    color: Color::from_rgb8(208, 208, 208), // Default gray
                }]
            }
        }
    }

    /// Highlight all lines in the content
    pub fn highlight_content(&self, content: &str, extension: Option<&str>) -> Vec<Vec<HighlightedSpan>> {
        content
            .lines()
            .map(|line| self.highlight_line(line, extension))
            .collect()
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
