// Terminal emulation using vt100 parser
// Simpler approach that works with both TUI and GUI

use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use parking_lot::RwLock;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

/// Terminal size in cells
#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl TerminalSize {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}

/// RGB color for terminal cells
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerminalColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0)
    }

    pub fn white() -> Self {
        Self::new(211, 215, 207)
    }

    /// Convert to iced Color
    pub fn to_iced_color(&self) -> iced::Color {
        iced::Color::from_rgb8(self.r, self.g, self.b)
    }

    /// Convert from vt100 color
    pub fn from_vt100_color(color: vt100::Color) -> Self {
        match color {
            vt100::Color::Default => Self::white(),
            vt100::Color::Idx(idx) => Self::from_256_color(idx),
            vt100::Color::Rgb(r, g, b) => Self::new(r, g, b),
        }
    }

    /// Convert 256 color index to RGB
    pub fn from_256_color(idx: u8) -> Self {
        // Standard 16 colors
        if idx < 16 {
            return match idx {
                0 => Self::new(0, 0, 0),         // Black
                1 => Self::new(128, 0, 0),       // Red
                2 => Self::new(0, 128, 0),       // Green
                3 => Self::new(128, 128, 0),     // Yellow
                4 => Self::new(0, 0, 128),       // Blue
                5 => Self::new(128, 0, 128),     // Magenta
                6 => Self::new(0, 128, 128),     // Cyan
                7 => Self::new(192, 192, 192),   // White
                8 => Self::new(128, 128, 128),   // Bright Black
                9 => Self::new(255, 0, 0),       // Bright Red
                10 => Self::new(0, 255, 0),      // Bright Green
                11 => Self::new(255, 255, 0),    // Bright Yellow
                12 => Self::new(0, 0, 255),      // Bright Blue
                13 => Self::new(255, 0, 255),    // Bright Magenta
                14 => Self::new(0, 255, 255),    // Bright Cyan
                15 => Self::new(255, 255, 255),  // Bright White
                _ => Self::white(),
            };
        }

        // 216 color cube (16-231)
        if idx < 232 {
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            return Self::new(
                if r > 0 { r * 40 + 55 } else { 0 },
                if g > 0 { g * 40 + 55 } else { 0 },
                if b > 0 { b * 40 + 55 } else { 0 },
            );
        }

        // Grayscale (232-255)
        let gray = (idx - 232) * 10 + 8;
        Self::new(gray, gray, gray)
    }
}

impl Default for TerminalColor {
    fn default() -> Self {
        Self::white()
    }
}

/// A single terminal cell
#[derive(Debug, Clone)]
pub struct TerminalCell {
    pub c: char,
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: TerminalColor::white(),
            bg: TerminalColor::black(),
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}

/// Terminal events
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Terminal output updated
    Output,
    /// Bell character received
    Bell,
    /// Title changed
    Title(String),
    /// Process exited
    Exit(i32),
    /// Error occurred
    Error(String),
}

/// Terminal emulator
pub struct Terminal {
    parser: Arc<RwLock<vt100::Parser>>,
    writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
    event_rx: Receiver<TerminalEvent>,
    size: TerminalSize,
    _reader_thread: thread::JoinHandle<()>,
}

impl Terminal {
    /// Create a new terminal with the given size
    pub fn new(size: TerminalSize) -> Result<Self, String> {
        Self::spawn(None, size)
    }

    /// Spawn a terminal with a specific command
    pub fn spawn(command: Option<&str>, size: TerminalSize) -> Result<Self, String> {
        let pty_system = NativePtySystem::default();

        let pty_size = PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(pty_size)
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        // Build command
        let mut cmd = if let Some(command) = command {
            let parts: Vec<&str> = command.split_whitespace().collect();
            if parts.is_empty() {
                return Err("Empty command".to_string());
            }
            let mut cmd = CommandBuilder::new(parts[0]);
            for arg in &parts[1..] {
                cmd.arg(*arg);
            }
            cmd
        } else {
            // Default to user's shell
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            CommandBuilder::new(shell)
        };

        // Set working directory
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        // Set TERM environment variable
        cmd.env("TERM", "xterm-256color");

        // Spawn the child process
        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        // Create vt100 parser
        let parser = Arc::new(RwLock::new(vt100::Parser::new(size.rows, size.cols, 1000)));

        // Get writer for input
        let writer = Arc::new(parking_lot::Mutex::new(
            pair.master
                .take_writer()
                .map_err(|e| format!("Failed to get writer: {}", e))?,
        ));

        // Create event channel
        let (event_tx, event_rx): (Sender<TerminalEvent>, Receiver<TerminalEvent>) = channel();

        // Spawn reader thread
        let reader_parser = Arc::clone(&parser);
        let mut reader = pair.master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;

        let reader_thread = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = event_tx.send(TerminalEvent::Exit(0));
                        break;
                    }
                    Ok(n) => {
                        // Process in a scoped block to release the lock quickly
                        {
                            reader_parser.write().process(&buf[..n]);
                        }
                        let _ = event_tx.send(TerminalEvent::Output);
                        // Yield to allow GUI thread to acquire read lock
                        thread::yield_now();
                    }
                    Err(e) => {
                        let _ = event_tx.send(TerminalEvent::Error(format!("Read error: {}", e)));
                        break;
                    }
                }
            }
        });

        Ok(Self {
            parser,
            writer,
            event_rx,
            size,
            _reader_thread: reader_thread,
        })
    }

    /// Write input to terminal (keyboard)
    pub fn input(&self, data: &[u8]) -> Result<(), String> {
        let mut writer = self.writer.lock();
        writer
            .write_all(data)
            .map_err(|e| format!("Write error: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Flush error: {}", e))
    }

    /// Write a string as input
    pub fn input_str(&self, s: &str) -> Result<(), String> {
        self.input(s.as_bytes())
    }

    /// Send interrupt signal (Ctrl+C)
    pub fn send_interrupt(&self) -> Result<(), String> {
        self.input(&[0x03])
    }

    /// Send EOF (Ctrl+D)
    pub fn send_eof(&self) -> Result<(), String> {
        self.input(&[0x04])
    }

    /// Send suspend signal (Ctrl+Z)
    pub fn send_suspend(&self) -> Result<(), String> {
        self.input(&[0x1A])
    }

    /// Resize terminal
    pub fn resize(&mut self, size: TerminalSize) {
        self.size = size;
        self.parser.write().set_size(size.rows, size.cols);
    }

    /// Get current size
    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// Get pending events (non-blocking)
    pub fn poll_events(&self) -> Vec<TerminalEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get all cells in the terminal grid
    pub fn cells(&self) -> Vec<Vec<TerminalCell>> {
        let parser = self.parser.read();
        let screen = parser.screen();

        let mut rows = Vec::with_capacity(self.size.rows as usize);

        for row_idx in 0..self.size.rows {
            let mut row = Vec::with_capacity(self.size.cols as usize);

            for col_idx in 0..self.size.cols {
                let cell = screen.cell(row_idx, col_idx);

                if let Some(cell) = cell {
                    row.push(TerminalCell {
                        c: cell.contents().chars().next().unwrap_or(' '),
                        fg: TerminalColor::from_vt100_color(cell.fgcolor()),
                        bg: TerminalColor::from_vt100_color(cell.bgcolor()),
                        bold: cell.bold(),
                        italic: cell.italic(),
                        underline: cell.underline(),
                        inverse: cell.inverse(),
                    });
                } else {
                    row.push(TerminalCell::default());
                }
            }

            rows.push(row);
        }

        rows
    }

    /// Get a single row of cells
    pub fn row(&self, row_idx: u16) -> Vec<TerminalCell> {
        let parser = self.parser.read();
        let screen = parser.screen();

        let mut row = Vec::with_capacity(self.size.cols as usize);

        for col_idx in 0..self.size.cols {
            let cell = screen.cell(row_idx, col_idx);

            if let Some(cell) = cell {
                row.push(TerminalCell {
                    c: cell.contents().chars().next().unwrap_or(' '),
                    fg: TerminalColor::from_vt100_color(cell.fgcolor()),
                    bg: TerminalColor::from_vt100_color(cell.bgcolor()),
                    bold: cell.bold(),
                    italic: cell.italic(),
                    underline: cell.underline(),
                    inverse: cell.inverse(),
                });
            } else {
                row.push(TerminalCell::default());
            }
        }

        row
    }

    /// Get cursor position (row, col)
    pub fn cursor_position(&self) -> (u16, u16) {
        let parser = self.parser.read();
        let screen = parser.screen();
        screen.cursor_position()
    }

    /// Check if cursor is visible
    pub fn cursor_visible(&self) -> bool {
        let parser = self.parser.read();
        !parser.screen().hide_cursor()
    }

    /// Get the terminal contents as a string (for debugging)
    pub fn contents(&self) -> String {
        let parser = self.parser.read();
        parser.screen().contents()
    }

    /// Get scrollback buffer size
    pub fn scrollback_len(&self) -> usize {
        let parser = self.parser.read();
        parser.screen().scrollback()
    }
}
