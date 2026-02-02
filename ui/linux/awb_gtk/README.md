# AWB GTK - Linux UI

GTK4/libadwaita application for AWB-RS (AutoWikiBrowser Rust Edition).

## Overview

This is a native Linux application built with GTK4 and libadwaita that provides a modern, adaptive interface to the AWB-RS engine.

## Architecture

```
┌─────────────────────────────────────┐
│     awb-gtk (GTK4/Rust)             │
│  ┌──────────────────────────────┐   │
│  │  MainWindow (UI)             │   │
│  │  ├─ PageList                 │   │
│  │  ├─ EditorView                │   │
│  │  ├─ RuleEditor                │   │
│  │  └─ StatusBar                 │   │
│  └──────────────────────────────┘   │
│           │                          │
│  ┌────────▼──────────────────────┐   │
│  │  AWB Crates (Direct Rust)     │   │
│  │  - awb_domain                 │   │
│  │  - awb_engine                 │   │
│  │  - awb_mw_api                 │   │
│  └────────────────────────────────┘   │
└─────────────────────────────────────┘
```

Unlike the Windows/macOS UIs that use FFI, the GTK version directly uses the Rust crates since it's all Rust code.

## Project Structure

```
awb_gtk/
├── Cargo.toml
├── src/
│   ├── main.rs              # Application entry point
│   ├── lib.rs               # Library exports
│   ├── app.rs               # Application setup
│   └── views/
│       ├── mod.rs
│       ├── main_window.rs   # Main window with three-panel layout
│       ├── login_dialog.rs  # Login dialog
│       ├── editor_view.rs   # Source/diff editor
│       ├── rule_editor.rs   # Rules panel
│       └── page_list.rs     # Page list sidebar
└── README.md
```

## Building

### Prerequisites

- Rust toolchain (1.85+)
- GTK4 development libraries
- libadwaita development libraries

#### Ubuntu/Debian
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev
```

#### Fedora
```bash
sudo dnf install gtk4-devel libadwaita-devel
```

#### Arch Linux
```bash
sudo pacman -S gtk4 libadwaita
```

### Build Steps

1. **Build the application:**
   ```bash
   cd ui/linux/awb_gtk
   cargo build --release
   ```

2. **Run the application:**
   ```bash
   cargo run --release
   ```

## Features

### Implemented
- Main window with adaptive three-panel layout
- Modern libadwaita styling
- Login dialog with preferences layout
- Source editor and diff view (tabbed)
- Rule editor with add/remove/reorder
- Page list with search
- Status bar with progress indicator
- Menu system

### To Be Implemented
- Direct integration with awb_engine for rule application
- MediaWiki API integration for page fetching
- Syntax highlighting for WikiText (using GtkSourceView)
- Diff computation and visualization
- Save functionality
- Settings dialog
- Toast notifications for errors/success

## GTK4 vs Windows/macOS

| Feature | GTK4 | Windows (WPF) | macOS (SwiftUI) |
|---------|------|---------------|-----------------|
| Language | Rust | C# | Swift |
| UI Framework | GTK4/libadwaita | WPF | SwiftUI |
| Integration | Direct crate usage | FFI (P/Invoke) | FFI (Swift C interop) |
| Theme Support | Yes (libadwaita) | Windows themes | macOS themes |
| Adaptive Layout | Yes | Partial | Yes |

### Why Direct Crate Usage?

Since both the UI and engine are Rust, we can:
- Use crates directly without FFI overhead
- Share types and data structures
- Get better type safety and error handling
- Avoid marshaling and serialization costs

## Development Notes

### GTK4 Patterns

The application follows GTK4 best practices:
- Uses `gtk::glib::clone!` macro for signal handlers
- Implements widget composition for reusable components
- Uses `gtk::Builder` patterns where appropriate
- Follows libadwaita HIG (Human Interface Guidelines)

### State Management

Unlike the Windows/macOS versions that use ViewModels, GTK4 uses:
- Direct widget property bindings
- Signal handlers for state updates
- GObject properties for complex state

### Async Operations

GTK4 + Tokio integration for async MediaWiki operations:
```rust
use gtk::glib;

glib::spawn_future_local(async move {
    // Async operations here
});
```

## Platform Support

This UI is designed for Linux but may work on other Unix-like systems (BSD, etc.) where GTK4 is available. It will NOT work on Windows or macOS without significant GTK4 setup.

For Windows → use `ui/windows/AWBrowser` (C#/WPF)
For macOS → use `ui/macos/AWBrowser` (Swift/SwiftUI)

## License

MIT OR Apache-2.0 (dual licensed, matching awb-rs)
