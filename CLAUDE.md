# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

nterm is a terminal-based IDE built in Rust using ratatui for TUI. It provides a 4-panel layout (file tree, editor, terminal, AI chat) similar to VS Code, with full mouse support, syntax highlighting, and theming.

## Build Commands

```bash
cargo run              # Run development mode
cargo build --release  # Build release binary (outputs to target/release/nterm)
cargo check            # Type check without building
cargo test             # Run tests
```

## Architecture

### Core Modules

- **main.rs**: Entry point with main event loop (`run_app`). Handles input processing and routes events to appropriate panels. Uses channel-based architecture for PTY, input, and tick events. Spawns new Terminal window on macOS using osascript.

- **app.rs**: Central `App` struct holding all application state. Key fields:
  - `event_rx/event_tx`: Channel for `AppEvent` (Input, PtyData, Tick, AiResponse)
  - `terminal_screen`: Arc<RwLock<Parser>> for PTY screen state
  - `key_map`: HashMap mapping (KeyCode, KeyModifiers) -> Action
  - `active_panel`: Current focus (FileTree, Editor, Terminal, Chat)
  - `editor_state`: EditorState with full text editing capabilities
  - `current_theme`: Theme for Light/Dark mode support
  - `clipboard`: Optional<Arc<Mutex<Clipboard>>> for system clipboard

- **ui.rs**: Pure rendering logic. `ui()` draws all widgets based on App state. `get_layout_chunks()` returns `AppLayout` struct with panel rectangles. Handles menu bar, search modal, and settings modal rendering.

- **file_tree.rs**: `FileNode` tree structure with lazy loading. `flatten_node()` converts tree to `VisibleItem` list for display. Supports recursive expand/collapse.

- **editor.rs**: Custom text editor implementation with:
  - `EditorState`: Content, cursor position, scroll, file path, modified flag
  - `EditorWidget`: Ratatui stateful widget for rendering
  - `HighlightCache`: Per-line syntax highlighting cache for performance
  - Full text editing operations (insert, delete, cursor movement, copy/paste)
  - Line numbers with dynamic gutter width

- **action.rs**: `Action` enum for abstract commands decoupled from key bindings.

- **ai.rs**: `Model` enum (Gemini, Echo) and async `send_message()` for AI chat. Gemini integration uses REST API with tokio async runtime.

- **config.rs**: `Config` struct for persistent settings (stored at `~/.nterm_config.json`). Stores Gemini API key and theme preference.

- **theme.rs**: `ThemeMode` enum (Light, Dark) and `Theme` struct with comprehensive color palette for all UI elements.

### Event Flow

1. Three threads feed events into `event_tx` channel: PTY reader, input reader, tick timer (250ms)
2. Main loop receives via `event_rx`, processes up to 50 events per iteration
3. Global actions (Quit, SwitchFocus, etc.) checked first via `key_map`
4. Panel-specific input handling based on `active_panel`
5. Mouse events route clicks to panels and handle scrolling

### Key Patterns

- State is mutated only in main.rs event handlers or App methods
- UI rendering in ui.rs is a pure function of App state
- PTY uses portable-pty crate with vt100 parser via tui-term
- TextArea widget (tui-textarea) used for chat input, search, and settings
- Custom EditorState for main editor with syntax highlighting (syntect)
- Highlight cache uses content hashing to avoid re-processing unchanged lines

### Layout System

`get_layout_chunks()` returns `AppLayout` with:
- **Menu Bar** (1 line, full width)
- **Left (20%)**: File Tree
- **Middle (60%)**: Editor and Terminal (vertical split, dynamic based on active panel)
- **Right (20%)**: Chat (80% history, 20% input)

## Key Bindings

### Global

| Shortcut | Action |
|----------|--------|
| Ctrl+Q | Quit |
| Tab | Cycle panel focus (FileTree→Editor→Chat→Terminal) |
| Esc / F1 | Toggle menu |
| Ctrl+P | File search modal |
| Ctrl+S | Settings modal |
| Ctrl+M | Cycle AI model (Gemini↔Echo) |
| Ctrl+H | Dump terminal history to editor |
| Ctrl+R | Reset layout (focus editor) |

### Editor Panel

| Shortcut | Action |
|----------|--------|
| Ctrl+C | Copy current line |
| Ctrl+V | Paste from clipboard |
| Arrow keys | Move cursor |
| Home/End | Line start/end |
| PageUp/Down | Scroll 20 lines |
| Backspace/Delete | Delete character |
| Enter | Insert newline |

### File Tree Panel

| Shortcut | Action |
|----------|--------|
| Up/Down | Navigate files |
| PageUp/Down | Jump 10 items |
| Right Arrow | Expand directory |
| Left Arrow | Collapse directory |
| Enter | Open file / Toggle directory |

### Chat Panel

| Shortcut | Action |
|----------|--------|
| Enter | Send message |
| Up/Down | Scroll history |
| PageUp/Down | Scroll 10 lines |
| Home/End | Jump to top/bottom |

### Terminal Panel

| Shortcut | Action |
|----------|--------|
| Ctrl+C | Send SIGINT |
| Ctrl+D | Send EOF |
| Ctrl+Z | Send SIGSTOP |
| Ctrl+V | Paste from clipboard |
| All other input | Sent directly to PTY |

### Search Modal (Ctrl+P)

| Shortcut | Action |
|----------|--------|
| Esc | Close search |
| Enter | Open selected file |
| Up/Down | Navigate results |

### Settings Modal (Ctrl+S)

| Shortcut | Action |
|----------|--------|
| Esc | Cancel (discard changes) |
| Tab | Toggle Light/Dark theme |
| Enter | Save settings |

## Mouse Support

- **Left click on panel**: Focus that panel
- **Left click on menu**: Open dropdown menu
- **Scroll wheel**: Scroll content in any panel (3-line increments)

## Configuration

**Location**: `~/.nterm_config.json`

```json
{
  "gemini_api_key": "your-api-key-here",
  "theme": "Dark"
}
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| ratatui | TUI framework |
| tokio | Async runtime for AI |
| portable-pty | Cross-platform PTY |
| tui-term | Terminal emulation widget |
| tui-textarea | Text input widget |
| syntect | Syntax highlighting |
| arboard | System clipboard |
| reqwest | HTTP client for Gemini API |
