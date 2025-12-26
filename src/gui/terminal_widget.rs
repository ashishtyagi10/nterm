// Terminal widget for iced GUI
// Renders terminal cells as a scrollable grid of styled text

use iced::widget::{column, container, scrollable, text, Column};
use iced::{Color, Element, Font, Length};

use crate::shared::{Terminal, TerminalCell, TerminalEvent, TerminalSize};
use super::message::Message;
use super::theme::TerminalColors;

/// Terminal view state
pub struct TerminalView {
    terminal: Option<Terminal>,
    rows: u16,
    cols: u16,
    has_exited: bool,
    exit_code: Option<i32>,
}

impl TerminalView {
    pub fn new() -> Self {
        Self {
            terminal: None,
            rows: 24,
            cols: 80,
            has_exited: false,
            exit_code: None,
        }
    }

    /// Start the terminal with default shell
    pub fn start(&mut self) -> Result<(), String> {
        let size = TerminalSize::new(self.rows, self.cols);
        self.terminal = Some(Terminal::new(size)?);
        self.has_exited = false;
        self.exit_code = None;
        Ok(())
    }

    /// Start the terminal with a specific command
    pub fn start_command(&mut self, command: &str) -> Result<(), String> {
        let size = TerminalSize::new(self.rows, self.cols);
        self.terminal = Some(Terminal::spawn(Some(command), size)?);
        self.has_exited = false;
        self.exit_code = None;
        Ok(())
    }

    /// Check if terminal is running
    pub fn is_running(&self) -> bool {
        self.terminal.is_some() && !self.has_exited
    }

    /// Send keyboard input
    pub fn input(&self, data: &str) -> Result<(), String> {
        if let Some(ref term) = self.terminal {
            term.input_str(data)
        } else {
            Err("Terminal not running".to_string())
        }
    }

    /// Send raw bytes
    pub fn input_bytes(&self, data: &[u8]) -> Result<(), String> {
        if let Some(ref term) = self.terminal {
            term.input(data)
        } else {
            Err("Terminal not running".to_string())
        }
    }

    /// Process terminal events and return true if there was output
    pub fn tick(&mut self) -> bool {
        if let Some(ref term) = self.terminal {
            let events = term.poll_events();
            let mut had_output = false;

            for event in events {
                match event {
                    TerminalEvent::Output => {
                        had_output = true;
                    }
                    TerminalEvent::Exit(code) => {
                        self.has_exited = true;
                        self.exit_code = Some(code);
                    }
                    TerminalEvent::Error(e) => {
                        eprintln!("Terminal error: {}", e);
                        self.has_exited = true;
                    }
                    _ => {}
                }
            }
            had_output
        } else {
            false
        }
    }

    /// Get terminal cells
    pub fn cells(&self) -> Vec<Vec<TerminalCell>> {
        if let Some(ref term) = self.terminal {
            term.cells()
        } else {
            Vec::new()
        }
    }

    /// Get cursor position
    pub fn cursor_position(&self) -> (u16, u16) {
        if let Some(ref term) = self.terminal {
            term.cursor_position()
        } else {
            (0, 0)
        }
    }

    /// Check if cursor is visible
    pub fn cursor_visible(&self) -> bool {
        if let Some(ref term) = self.terminal {
            term.cursor_visible()
        } else {
            false
        }
    }

    /// Resize the terminal
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        if let Some(ref mut term) = self.terminal {
            term.resize(TerminalSize::new(rows, cols));
        }
    }

    /// Send interrupt (Ctrl+C)
    pub fn send_interrupt(&self) -> Result<(), String> {
        self.input_bytes(&[0x03])
    }

    /// Send EOF (Ctrl+D)
    pub fn send_eof(&self) -> Result<(), String> {
        self.input_bytes(&[0x04])
    }

    /// Render the terminal as iced elements
    pub fn view<'a>(&'a self, colors: &TerminalColors) -> Element<'a, Message> {
        if self.terminal.is_none() {
            return container(
                column![
                    text("Terminal")
                        .size(14)
                        .font(Font::MONOSPACE)
                        .color(colors.foreground),
                    text("")
                        .size(13)
                        .font(Font::MONOSPACE),
                    text("Press Enter or click Start to launch terminal")
                        .size(13)
                        .font(Font::MONOSPACE)
                        .color(colors.line_number),
                ]
                .spacing(5)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into();
        }

        if self.has_exited {
            let exit_msg = match self.exit_code {
                Some(code) => format!("Process exited with code {}", code),
                None => "Process exited".to_string(),
            };
            return container(
                column![
                    text(exit_msg)
                        .size(13)
                        .font(Font::MONOSPACE)
                        .color(colors.line_number),
                    text("Press Enter to restart")
                        .size(13)
                        .font(Font::MONOSPACE)
                        .color(colors.line_number),
                ]
                .spacing(5)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .into();
        }

        let cells = self.cells();
        let (cursor_row, _cursor_col) = self.cursor_position();
        let cursor_visible = self.cursor_visible();

        let rows: Vec<Element<'a, Message>> = cells
            .iter()
            .enumerate()
            .map(|(row_idx, row_cells)| {
                // Build the line with proper coloring
                let line_text: String = row_cells.iter().map(|cell| cell.c).collect();

                // For cursor highlighting, we'll check if cursor is on this row
                let is_cursor_row = row_idx as u16 == cursor_row && cursor_visible;

                // Use terminal foreground/background from first cell with content
                // For simplicity, we use the default terminal foreground
                let fg_color = if is_cursor_row {
                    // Highlight cursor row slightly
                    Color::from_rgb8(
                        ((colors.foreground.r * 255.0) as u8).saturating_add(20),
                        ((colors.foreground.g * 255.0) as u8).saturating_add(20),
                        ((colors.foreground.b * 255.0) as u8).saturating_add(20),
                    )
                } else {
                    colors.foreground
                };

                text(line_text)
                    .size(13)
                    .font(Font::MONOSPACE)
                    .color(fg_color)
                    .into()
            })
            .collect();

        let content = Column::with_children(rows)
            .spacing(0)
            .padding(5);

        scrollable(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}
