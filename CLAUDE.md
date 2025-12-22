# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

nterm is a terminal-based IDE built in Rust using ratatui for TUI. It provides a 4-panel layout (file tree, editor, terminal, AI chat) similar to VS Code.

## Build Commands

```bash
cargo run          # Run development mode
cargo build --release  # Build release binary (outputs to target/release/nterm)
cargo check        # Type check without building
cargo test         # Run tests
```

## Architecture

### Core Modules

- **main.rs**: Entry point with main event loop (`run_app`). Handles input processing and routes events to appropriate panels. Uses channel-based architecture for PTY, input, and tick events.

- **app.rs**: Central `App` struct holding all application state. Key fields:
  - `event_rx/event_tx`: Channel for `AppEvent` (Input, PtyData, Tick, AiResponse)
  - `terminal_screen`: Arc<RwLock<Parser>> for PTY screen state
  - `key_map`: HashMap mapping (KeyCode, KeyModifiers) -> Action
  - `active_panel`: Current focus (FileTree, Editor, Terminal, Chat)

- **ui.rs**: Pure rendering logic. `ui()` draws all widgets based on App state. `get_layout_chunks()` returns `AppLayout` struct with panel rectangles.

- **file_tree.rs**: `FileNode` tree structure with lazy loading. `flatten_node()` converts tree to `VisibleItem` list for display. `toggle_node_recursive()` handles expand/collapse.

- **action.rs**: `Action` enum for abstract commands decoupled from key bindings.

- **ai.rs**: `Model` enum (Gemini, Echo) and async `send_message()` for AI chat. Gemini integration uses REST API.

- **config.rs**: `Config` struct for persistent settings (stored at `~/.nterm_config.json`).

### Event Flow

1. Three threads feed events into `event_tx` channel: PTY reader, input reader, tick timer
2. Main loop receives via `event_rx`, processes up to 50 events per iteration
3. Global actions (Quit, SwitchFocus, etc.) checked first via `key_map`
4. Panel-specific input handling based on `active_panel`

### Key Patterns

- State is mutated only in main.rs event handlers or App methods
- UI rendering in ui.rs is a pure function of App state
- PTY uses portable-pty crate with vt100 parser via tui-term
- TextArea widget used for editor and all text inputs

## Key Bindings

| Shortcut | Action |
|----------|--------|
| Ctrl+Q | Quit |
| Ctrl+P | File search modal |
| Ctrl+S | Settings modal |
| Ctrl+M | Cycle AI model |
| Ctrl+H | Dump terminal history to editor |
| Ctrl+R | Reset layout (focus editor) |
| Tab | Cycle panel focus |
| Ctrl+V | Paste in terminal |
| Ctrl+C | Copy selection in editor |
