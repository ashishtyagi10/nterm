use std::fs;
use std::io;
use std::path::PathBuf;

use ratatui::{
    backend::Backend,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame, Terminal,
};

use super::theme::Theme;
use crate::shared::{Config, RecentWorkspace};
use crate::shared::{FileNode, VisibleItem, flatten_node};

#[derive(Clone, Copy, PartialEq)]
pub enum SelectorSection {
    Recent,
    Browser,
}

pub struct WorkspaceSelector {
    recent_workspaces: Vec<RecentWorkspace>,
    recent_state: ListState,

    browser_tree: Vec<FileNode>,
    browser_visible_items: Vec<VisibleItem>,
    browser_state: ListState,
    browser_scroll_state: ScrollbarState,

    current_path: PathBuf,
    active_section: SelectorSection,
    selected_workspace: Option<PathBuf>,
    should_quit: bool,

    theme: Theme,
}

impl WorkspaceSelector {
    pub fn new(config: &Config) -> Self {
        let recent_workspaces = config.get_recent_workspaces().to_vec();
        let theme = Theme::new(config.theme);

        // Start browsing from home directory
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

        let mut selector = Self {
            recent_workspaces,
            recent_state: ListState::default(),

            browser_tree: Vec::new(),
            browser_visible_items: Vec::new(),
            browser_state: ListState::default(),
            browser_scroll_state: ScrollbarState::default(),

            current_path: home,
            active_section: SelectorSection::Recent,
            selected_workspace: None,
            should_quit: false,

            theme,
        };

        // Initialize browser tree
        selector.refresh_browser();

        // Select first item in recent if available, otherwise switch to browser
        if !selector.recent_workspaces.is_empty() {
            selector.recent_state.select(Some(0));
        } else {
            selector.active_section = SelectorSection::Browser;
            if !selector.browser_visible_items.is_empty() {
                selector.browser_state.select(Some(0));
            }
        }

        selector
    }

    fn refresh_browser(&mut self) {
        self.browser_tree.clear();

        if let Ok(entries) = fs::read_dir(&self.current_path) {
            let mut dirs: Vec<FileNode> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| FileNode::from_path(e.path(), 0))
                .filter(|node| !node.name.starts_with('.'))
                .collect();

            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            self.browser_tree = dirs;
        }

        self.update_visible_items();
    }

    fn update_visible_items(&mut self) {
        self.browser_visible_items.clear();
        for node in &self.browser_tree {
            flatten_node(node, &mut self.browser_visible_items);
        }
        self.browser_scroll_state = self.browser_scroll_state.content_length(self.browser_visible_items.len());
    }

    fn toggle_expand(&mut self, idx: usize) {
        if idx >= self.browser_visible_items.len() {
            return;
        }

        let target_path = self.browser_visible_items[idx].path.clone();

        fn toggle_recursive(nodes: &mut Vec<FileNode>, target: &PathBuf) -> bool {
            for node in nodes.iter_mut() {
                if &node.path == target {
                    node.toggle_expand();
                    return true;
                }
                if node.expanded && toggle_recursive(&mut node.children, target) {
                    return true;
                }
            }
            false
        }

        toggle_recursive(&mut self.browser_tree, &target_path);
        self.update_visible_items();
    }

    fn navigate_into(&mut self, idx: usize) {
        if idx >= self.browser_visible_items.len() {
            return;
        }

        let path = self.browser_visible_items[idx].path.clone();
        if path.is_dir() {
            self.current_path = path;
            self.refresh_browser();
            self.browser_state.select(Some(0));
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.refresh_browser();
            self.browser_state.select(Some(0));
        }
    }

    pub fn run<B: Backend + io::Write>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> io::Result<Option<PathBuf>> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => {
                        self.should_quit = true;
                    }
                    KeyCode::Tab => {
                        self.active_section = match self.active_section {
                            SelectorSection::Recent => {
                                if !self.browser_visible_items.is_empty()
                                    && self.browser_state.selected().is_none()
                                {
                                    self.browser_state.select(Some(0));
                                }
                                SelectorSection::Browser
                            }
                            SelectorSection::Browser => {
                                if !self.recent_workspaces.is_empty() {
                                    if self.recent_state.selected().is_none() {
                                        self.recent_state.select(Some(0));
                                    }
                                    SelectorSection::Recent
                                } else {
                                    SelectorSection::Browser
                                }
                            }
                        };
                    }
                    KeyCode::Up => match self.active_section {
                        SelectorSection::Recent => {
                            if let Some(i) = self.recent_state.selected() {
                                if i > 0 {
                                    self.recent_state.select(Some(i - 1));
                                }
                            }
                        }
                        SelectorSection::Browser => {
                            if let Some(i) = self.browser_state.selected() {
                                if i > 0 {
                                    self.browser_state.select(Some(i - 1));
                                }
                            }
                        }
                    },
                    KeyCode::Down => match self.active_section {
                        SelectorSection::Recent => {
                            if let Some(i) = self.recent_state.selected() {
                                if i + 1 < self.recent_workspaces.len() {
                                    self.recent_state.select(Some(i + 1));
                                }
                            }
                        }
                        SelectorSection::Browser => {
                            if let Some(i) = self.browser_state.selected() {
                                if i + 1 < self.browser_visible_items.len() {
                                    self.browser_state.select(Some(i + 1));
                                }
                            }
                        }
                    },
                    KeyCode::Right => {
                        if self.active_section == SelectorSection::Browser {
                            if let Some(i) = self.browser_state.selected() {
                                if i < self.browser_visible_items.len() {
                                    let item = &self.browser_visible_items[i];
                                    if item.is_dir && !item.expanded {
                                        self.toggle_expand(i);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Left => {
                        if self.active_section == SelectorSection::Browser {
                            if let Some(i) = self.browser_state.selected() {
                                if i < self.browser_visible_items.len() {
                                    let item = &self.browser_visible_items[i];
                                    if item.is_dir && item.expanded {
                                        self.toggle_expand(i);
                                    } else {
                                        self.navigate_up();
                                    }
                                }
                            } else {
                                self.navigate_up();
                            }
                        }
                    }
                    KeyCode::Enter => match self.active_section {
                        SelectorSection::Recent => {
                            if let Some(i) = self.recent_state.selected() {
                                if i < self.recent_workspaces.len() {
                                    self.selected_workspace =
                                        Some(self.recent_workspaces[i].path.clone());
                                }
                            }
                        }
                        SelectorSection::Browser => {
                            if let Some(i) = self.browser_state.selected() {
                                if i < self.browser_visible_items.len() {
                                    self.selected_workspace =
                                        Some(self.browser_visible_items[i].path.clone());
                                }
                            } else {
                                // No item selected, select current directory
                                self.selected_workspace = Some(self.current_path.clone());
                            }
                        }
                    },
                    KeyCode::Char(' ') => {
                        // Space to select current browsing directory
                        if self.active_section == SelectorSection::Browser {
                            self.selected_workspace = Some(self.current_path.clone());
                        }
                    }
                    _ => {}
                }
            }

            if self.should_quit {
                return Ok(None);
            }

            if self.selected_workspace.is_some() {
                return Ok(self.selected_workspace.take());
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let area = f.area();

        // Clear the screen
        f.render_widget(Clear, area);

        // Main layout
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(self.recent_section_height()),  // Recent section
                Constraint::Min(10),    // Browser section
                Constraint::Length(2),  // Footer
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Select Workspace")
            .style(Style::default().fg(self.theme.foreground).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(self.theme.border)),
            )
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(title, main_chunks[0]);

        // Recent workspaces section
        self.render_recent_section(f, main_chunks[1]);

        // Browser section
        self.render_browser_section(f, main_chunks[2]);

        // Footer
        self.render_footer(f, main_chunks[3]);
    }

    fn recent_section_height(&self) -> u16 {
        if self.recent_workspaces.is_empty() {
            3 // Just header for "No recent workspaces"
        } else {
            (self.recent_workspaces.len() as u16 + 3).min(8) // Header + items + borders, max 8
        }
    }

    fn render_recent_section(&mut self, f: &mut Frame, area: Rect) {
        let is_active = self.active_section == SelectorSection::Recent;
        let border_color = if is_active {
            self.theme.border_active
        } else {
            self.theme.border
        };

        let block = Block::default()
            .title(" Recent Workspaces (Tab to switch) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        if self.recent_workspaces.is_empty() {
            let content = Paragraph::new("No recent workspaces")
                .style(Style::default().fg(self.theme.line_number))
                .block(block);
            f.render_widget(content, area);
            return;
        }

        let items: Vec<ListItem> = self
            .recent_workspaces
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let is_selected = self.recent_state.selected() == Some(i) && is_active;
                let style = if is_selected {
                    Style::default()
                        .fg(self.theme.selection_fg)
                        .bg(self.theme.selection_bg)
                } else {
                    Style::default().fg(self.theme.foreground)
                };

                let prefix = if is_selected { ">" } else { " " };
                let display_path = self.format_path(&w.path);
                let time_ago = self.format_time_ago(w.last_accessed);

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", prefix), style),
                    Span::styled(display_path, style),
                    Span::styled(format!("  ({})", time_ago), Style::default().fg(self.theme.line_number)),
                ]))
            })
            .collect();

        let list = List::new(items).block(block);
        f.render_stateful_widget(list, area, &mut self.recent_state);
    }

    fn render_browser_section(&mut self, f: &mut Frame, area: Rect) {
        let is_active = self.active_section == SelectorSection::Browser;
        let border_color = if is_active {
            self.theme.border_active
        } else {
            self.theme.border
        };

        // Split into header and list
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);

        // Current path header
        let current_path_display = self.format_path(&self.current_path);
        let header = Paragraph::new(format!(" Current: {}", current_path_display))
            .style(Style::default().fg(self.theme.line_number))
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_style(Style::default().fg(border_color))
                    .title(" Browse Directories "),
            );
        f.render_widget(header, chunks[0]);

        // Directory list
        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
            .border_style(Style::default().fg(border_color));

        if self.browser_visible_items.is_empty() {
            let content = Paragraph::new("  (empty directory)")
                .style(Style::default().fg(self.theme.line_number))
                .block(block);
            f.render_widget(content, chunks[1]);
            return;
        }

        let items: Vec<ListItem> = self
            .browser_visible_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = self.browser_state.selected() == Some(i) && is_active;
                let style = if is_selected {
                    Style::default()
                        .fg(self.theme.selection_fg)
                        .bg(self.theme.selection_bg)
                } else {
                    Style::default().fg(self.theme.foreground)
                };

                let indent = "  ".repeat(item.depth);
                let icon = if item.expanded { "v " } else { "> " };
                let prefix = if is_selected && is_active { ">" } else { " " };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", prefix), style),
                    Span::raw(indent),
                    Span::styled(icon, Style::default().fg(self.theme.directory)),
                    Span::styled(format!("{}/", item.name), style),
                ]))
            })
            .collect();

        let list = List::new(items).block(block);

        // Calculate scroll offset
        if let Some(selected) = self.browser_state.selected() {
            self.browser_scroll_state = self.browser_scroll_state.position(selected);
        }

        f.render_stateful_widget(list, chunks[1], &mut self.browser_state);

        // Scrollbar
        if self.browser_visible_items.len() > (chunks[1].height as usize - 2) {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None);
            f.render_stateful_widget(
                scrollbar,
                chunks[1].inner(ratatui::layout::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut self.browser_scroll_state,
            );
        }
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let help_text = " Enter: Select | Space: Select Current Dir | Tab: Switch | ←/→: Expand | Esc: Quit ";
        let footer = Paragraph::new(help_text)
            .style(Style::default().fg(self.theme.line_number))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(footer, area);
    }

    fn format_path(&self, path: &PathBuf) -> String {
        if let Some(home) = dirs::home_dir() {
            if let Ok(stripped) = path.strip_prefix(&home) {
                return format!("~/{}", stripped.display());
            }
        }
        path.display().to_string()
    }

    fn format_time_ago(&self, timestamp: u64) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let diff = now.saturating_sub(timestamp);

        if diff < 60 {
            "just now".to_string()
        } else if diff < 3600 {
            format!("{} min ago", diff / 60)
        } else if diff < 86400 {
            format!("{} hours ago", diff / 3600)
        } else if diff < 604800 {
            format!("{} days ago", diff / 86400)
        } else {
            format!("{} weeks ago", diff / 604800)
        }
    }
}
