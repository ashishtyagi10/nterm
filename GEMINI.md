# nterm - Terminal IDE

**nterm** is a feature-rich, terminal-based Integrated Development Environment (IDE) built in Rust. It aims to provide a VS Code-like experience within the terminal, featuring a file tree, code editor with syntax highlighting, an integrated terminal, and an AI chat interface.

## Project Overview

*   **Language:** Rust (Edition 2021)
*   **Core Frameworks:**
    *   `ratatui`: For the Terminal User Interface (TUI).
    *   `crossterm`: For terminal event handling and rendering.
    *   `portable-pty`: For the integrated terminal emulation.
    *   `tui-textarea`: For the code editor and input fields.
    *   `arboard`: For cross-platform clipboard support.
    *   `walkdir`: For file system traversal and search.

## Features

*   **4-Panel Layout**: Resizable split view containing:
    *   **File Tree**: Navigable directory structure.
    *   **Editor**: Code editor with line numbers and basic syntax highlighting.
    *   **Terminal**: Fully functional PTY-backed terminal emulator.
    *   **AI Chat**: Interface for AI assistance (simulated).
*   **Integrated Terminal**: Supports standard shell commands, interactive apps (vim, htop), mouse scrolling, and clipboard paste (`Ctrl+V`).
*   **Code Editor**: Supports line numbers, scrolling, and regex-based syntax highlighting for common languages (Rust, Python, JS, Go, etc.).
*   **File Search**: Quick Open dialog (`Ctrl+P`) to fuzzy search and open files.
*   **Menu System**: Interactive top menu bar with mouse support.
*   **Key Bindings**: Configurable key mappings for common actions (`Ctrl+Q` to Quit, `Ctrl+R` to Reset, `Ctrl+H` to Dump History).

## Building and Running

### Prerequisites
*   Rust toolchain (`cargo`, `rustc`).

### Commands

*   **Run Development Mode:**
    ```bash
    cargo run
    ```
*   **Build Release Binary:**
    ```bash
    cargo build --release
    ```
    The binary will be located at `target/release/nterm`.

## Architecture & Code Structure

The project is modularized into several files within the `src/` directory:

*   **`src/main.rs`**: The entry point. Handles the main event loop (`run_app`), input processing, and high-level coordination.
*   **`src/app.rs`**: Defines the `App` struct, which holds the entire application state (file tree, editor content, terminal state, search results). Handles state updates and initialization.
*   **`src/ui.rs`**: Responsible for rendering the UI. Contains the `ui` function which draws widgets to the frame based on the `App` state. Implements custom rendering logic for the editor and scrollbars.
*   **`src/file_tree.rs`**: Manages the file tree data structure (`FileNode`) and logic for expanding/collapsing directories and flattening the tree for display.
*   **`src/action.rs`**: Defines the `Action` enum, representing abstract commands (e.g., `Quit`, `SwitchFocus`) decoupled from specific key inputs.

## Key Bindings

| Action | Shortcut | Description |
| :--- | :--- | :--- |
| **Quit** | `Ctrl+Q` | Exit the application. |
| **File Search** | `Ctrl+P` | Open the "Quick Open" file search modal. |
| **Dump History** | `Ctrl+H` | Copy terminal output history to the Editor. |
| **Reset Layout** | `Ctrl+R` | Reset panel focus to the Editor. |
| **Menu** | `F1` / `Esc` | Toggle the top menu bar. |
| **Paste** | `Ctrl+V` | Paste clipboard content (Terminal panel). |
| **Switch Focus** | `Tab` | Cycle focus between panels. |

## Development Conventions

*   **State Management**: The `App` struct is the single source of truth. UI rendering is a pure function of this state.
*   **Event Loop**: The main loop uses a channel-based architecture to handle inputs, PTY events, and ticks efficiently without busy waiting.
*   **Scrolling**: Custom scrollbar logic is implemented to decouple view position from selection state in lists.
