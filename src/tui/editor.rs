use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, StatefulWidget, Widget},
};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect_tui::into_span;

/// Cache for syntax-highlighted lines to avoid re-processing unchanged content
struct HighlightCache {
    lines: Vec<Option<Line<'static>>>,
    line_hashes: Vec<u64>,
    extension: Option<String>,
}

impl HighlightCache {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            line_hashes: Vec::new(),
            extension: None,
        }
    }

    fn resize(&mut self, line_count: usize) {
        self.lines.resize_with(line_count, || None);
        self.line_hashes.resize(line_count, 0);
    }

    fn invalidate(&mut self, line_idx: usize) {
        if line_idx < self.lines.len() {
            self.lines[line_idx] = None;
        }
    }

    fn invalidate_all(&mut self) {
        for line in &mut self.lines {
            *line = None;
        }
    }

    fn set_extension(&mut self, ext: Option<String>) {
        if self.extension != ext {
            self.extension = ext;
            self.invalidate_all();
        }
    }

    fn hash_line(content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }
}

/// Editor state holding content, cursor position, and syntax highlighting resources
pub struct EditorState {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
    pub file_path: Option<PathBuf>,
    pub modified: bool,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    highlight_cache: HighlightCache,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            file_path: None,
            modified: false,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            highlight_cache: HighlightCache::new(),
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> io::Result<()> {
        let content = fs::read_to_string(&path)?;
        self.lines = content.lines().map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());
        self.highlight_cache.set_extension(ext);
        self.highlight_cache.resize(self.lines.len());

        self.file_path = Some(path);
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
        self.modified = false;
        Ok(())
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn current_line(&self) -> &str {
        self.lines.get(self.cursor_row).map(|s| s.as_str()).unwrap_or("")
    }

    fn current_line_len(&self) -> usize {
        self.current_line().chars().count()
    }

    pub fn insert_char(&mut self, c: char) {
        if let Some(line) = self.lines.get_mut(self.cursor_row) {
            let byte_idx = line.chars().take(self.cursor_col).map(|c| c.len_utf8()).sum();
            line.insert(byte_idx, c);
            self.cursor_col += 1;
            self.highlight_cache.invalidate(self.cursor_row);
            self.modified = true;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            if let Some(line) = self.lines.get_mut(self.cursor_row) {
                let byte_idx: usize = line.chars().take(self.cursor_col).map(|c| c.len_utf8()).sum();
                let char_to_remove = line.chars().nth(self.cursor_col - 1).unwrap();
                let remove_start = byte_idx - char_to_remove.len_utf8();
                line.remove(remove_start);
                self.cursor_col -= 1;
                self.highlight_cache.invalidate(self.cursor_row);
                self.modified = true;
            }
        } else if self.cursor_row > 0 {
            let current_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&current_line);
            self.highlight_cache.invalidate(self.cursor_row);
            self.highlight_cache.resize(self.lines.len());
            self.modified = true;
        }
    }

    pub fn delete(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor_col < line_len {
            if let Some(line) = self.lines.get_mut(self.cursor_row) {
                let byte_idx: usize = line.chars().take(self.cursor_col).map(|c| c.len_utf8()).sum();
                let char_to_remove = line.chars().nth(self.cursor_col).unwrap();
                line.drain(byte_idx..byte_idx + char_to_remove.len_utf8());
                self.highlight_cache.invalidate(self.cursor_row);
                self.modified = true;
            }
        } else if self.cursor_row < self.lines.len() - 1 {
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
            self.highlight_cache.invalidate(self.cursor_row);
            self.highlight_cache.resize(self.lines.len());
            self.modified = true;
        }
    }

    pub fn insert_newline(&mut self) {
        if let Some(line) = self.lines.get_mut(self.cursor_row) {
            let byte_idx: usize = line.chars().take(self.cursor_col).map(|c| c.len_utf8()).sum();
            let remainder = line.split_off(byte_idx);
            self.highlight_cache.invalidate(self.cursor_row);
            self.lines.insert(self.cursor_row + 1, remainder);
            self.cursor_row += 1;
            self.cursor_col = 0;
            self.highlight_cache.resize(self.lines.len());
            self.modified = true;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.cursor_col.min(self.current_line_len());
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.cursor_col = self.cursor_col.min(self.current_line_len());
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.current_line_len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.current_line_len();
    }

    pub fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        } else if self.cursor_row >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.cursor_row - viewport_height + 1;
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    pub fn page_up(&mut self, viewport_height: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(viewport_height);
        // Move cursor to stay in view
        if self.cursor_row >= self.scroll_offset + viewport_height {
            self.cursor_row = self.scroll_offset + viewport_height.saturating_sub(1);
        }
    }

    pub fn page_down(&mut self, viewport_height: usize) {
        let max_scroll = self.lines.len().saturating_sub(viewport_height);
        self.scroll_offset = (self.scroll_offset + viewport_height).min(max_scroll);
        // Move cursor to stay in view
        if self.cursor_row < self.scroll_offset {
            self.cursor_row = self.scroll_offset;
        }
        self.cursor_col = self.cursor_col.min(self.current_line_len());
    }

    pub fn get_highlighted_line(&mut self, line_idx: usize) -> Line<'static> {
        let content = match self.lines.get(line_idx) {
            Some(line) => line.clone(),
            None => return Line::default(),
        };

        let content_hash = HighlightCache::hash_line(&content);

        // Check cache
        if line_idx < self.highlight_cache.lines.len() {
            if let Some(cached) = &self.highlight_cache.lines[line_idx] {
                if self.highlight_cache.line_hashes.get(line_idx) == Some(&content_hash) {
                    return cached.clone();
                }
            }
        }

        // Highlight the line
        let highlighted = self.highlight_line(&content);

        // Cache result
        self.highlight_cache.resize(line_idx + 1);
        self.highlight_cache.lines[line_idx] = Some(highlighted.clone());
        self.highlight_cache.line_hashes[line_idx] = content_hash;

        highlighted
    }

    fn highlight_line(&self, content: &str) -> Line<'static> {
        let ext = self.highlight_cache.extension.as_deref();
        let syntax = ext
            .and_then(|e| self.syntax_set.find_syntax_by_extension(e))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        match highlighter.highlight_line(content, &self.syntax_set) {
            Ok(ranges) => {
                let spans: Vec<Span<'static>> = ranges
                    .into_iter()
                    .filter_map(|segment| {
                        into_span(segment).ok().map(|span| {
                            // Convert borrowed span to owned for 'static lifetime
                            // Only use foreground color, strip background to avoid visual artifacts
                            let style = Style::default().fg(span.style.fg.unwrap_or(Color::Reset));
                            Span::styled(span.content.to_string(), style)
                        })
                    })
                    .collect();
                Line::from(spans)
            }
            Err(_) => Line::from(content.to_string()),
        }
    }
    pub fn copy(&self) -> Option<String> {
        // TODO: Implement selection support. For now, copy current line.
        Some(self.current_line().to_string())
    }

    pub fn paste(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(c);
            }
        }
    }
}

/// Widget for rendering the editor with syntax highlighting
pub struct EditorWidget<'a> {
    block: Option<Block<'a>>,
    line_number_style: Style,
    cursor_style: Style,
    focused: bool,
}

impl<'a> EditorWidget<'a> {
    pub fn new() -> Self {
        Self {
            block: None,
            line_number_style: Style::default().fg(Color::DarkGray),
            cursor_style: Style::default().bg(Color::White).fg(Color::Black),
            focused: false,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn line_number_style(mut self, style: Style) -> Self {
        self.line_number_style = style;
        self
    }

    pub fn cursor_style(mut self, style: Style) -> Self {
        self.cursor_style = style;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl<'a> StatefulWidget for EditorWidget<'a> {
    type State = EditorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Render block and get inner area
        let inner_area = match &self.block {
            Some(block) => {
                let inner = block.inner(area);
                block.clone().render(area, buf);
                inner
            }
            None => area,
        };

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        // Calculate gutter width
        let line_count = state.line_count();
        let gutter_width = ((line_count.max(1) as f64).log10().floor() as u16) + 3;
        let _content_width = inner_area.width.saturating_sub(gutter_width);
        let viewport_height = inner_area.height as usize;

        // Ensure cursor is visible
        state.ensure_cursor_visible(viewport_height);

        // Render visible lines
        for (view_row, line_idx) in (state.scroll_offset..)
            .take(viewport_height)
            .enumerate()
        {
            let y = inner_area.y + view_row as u16;

            if line_idx < line_count {
                // Render line number
                let line_num = format!("{:>width$} ", line_idx + 1, width = (gutter_width - 2) as usize);
                buf.set_string(inner_area.x, y, &line_num, self.line_number_style);

                // Render highlighted content
                let content_x = inner_area.x + gutter_width;
                let highlighted_line = state.get_highlighted_line(line_idx);

                let mut x = content_x;
                for span in highlighted_line.spans.iter() {
                    let text = span.content.as_ref();
                    for ch in text.chars() {
                        if x >= inner_area.x + inner_area.width {
                            break;
                        }
                        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
                        buf.set_string(x, y, &ch.to_string(), span.style);
                        x += char_width;
                    }
                }

                // Render cursor
                if self.focused && line_idx == state.cursor_row {
                    let cursor_x = content_x + state.cursor_col as u16;
                    if cursor_x < inner_area.x + inner_area.width {
                        let cursor_char = state.lines.get(line_idx)
                            .and_then(|l| l.chars().nth(state.cursor_col))
                            .unwrap_or(' ');
                        buf.set_string(cursor_x, y, &cursor_char.to_string(), self.cursor_style);
                    }
                }
            }
        }
    }
}
