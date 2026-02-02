# UI Implementation Summary

This document summarizes the Windows (C#/WPF) and Linux (GTK4) UI shells created for AWB-RS.

## Overview

Two native UI implementations have been created:

1. **Windows UI** (`ui/windows/AWBrowser/`) - C# WPF application using P/Invoke
2. **Linux UI** (`ui/linux/awb_gtk/`) - GTK4/libadwaita Rust application

Both UIs provide similar functionality but use different approaches to integrate with the Rust backend.

## Windows UI - C#/WPF

### Architecture

```
C# WPF Application
    ↓ (P/Invoke via NativeMethods.cs)
awb_ffi.dll (Rust C ABI)
    ↓
Rust Core Crates
```

### Files Created

```
ui/windows/AWBrowser/
├── AWBrowser.csproj              # .NET 8 WPF project
├── App.xaml                      # Application entry point
├── App.xaml.cs
├── NativeMethods.cs              # P/Invoke declarations for Rust FFI
├── MainWindow.xaml               # Main window layout
├── MainWindow.xaml.cs
├── Views/
│   ├── LoginWindow.xaml          # Login dialog
│   ├── LoginWindow.xaml.cs
│   ├── EditorView.xaml           # Split diff editor
│   ├── EditorView.xaml.cs
│   ├── RuleEditorView.xaml       # Rules panel
│   └── RuleEditorView.xaml.cs
├── ViewModels/
│   └── SessionViewModel.cs       # MVVM state management
├── Models/
│   ├── PageModel.cs              # Page data model
│   └── RuleModel.cs              # Rule data model
└── README.md
```

### Features

- **Three-panel layout**: Page list (left), editor (center), rules (right)
- **Menu bar**: File, Edit, View, Tools, Help
- **Toolbar**: Quick access to common operations
- **Status bar**: Progress indicator, page count, status messages
- **Tabbed editor**: Source view and split diff view
- **Rule editor**: DataGrid with add/remove/reorder
- **MVVM architecture**: Separation of concerns
- **P/Invoke integration**: Ready for Rust FFI calls

### Building

```bash
cd ui/windows/AWBrowser
dotnet restore
dotnet build
dotnet run
```

**Prerequisites**: .NET 8 SDK, Windows OS

### FFI Integration

`NativeMethods.cs` declares P/Invoke signatures:
- `awb_version()` - Get library version (implemented)
- `awb_free_string()` - Free native strings (implemented)
- Placeholders for: `create_session()`, `login()`, `fetch_list()`, `get_page()`, `apply_rules()`, `save_page()`, `compute_diff()`

The Rust FFI C API is in `crates/awb_ffi/src/c_api.rs` and exports these functions via `#[no_mangle]`.

## Linux UI - GTK4

### Architecture

```
GTK4 Rust Application
    ↓ (Direct crate usage)
Rust Core Crates
```

### Files Created

```
ui/linux/awb_gtk/
├── Cargo.toml                    # Updated with GTK4 dependencies
├── src/
│   ├── main.rs                   # Application entry point
│   ├── lib.rs                    # Library exports
│   ├── app.rs                    # Application setup
│   └── views/
│       ├── mod.rs
│       ├── main_window.rs        # Main window with three-panel layout
│       ├── login_dialog.rs       # Login dialog (libadwaita style)
│       ├── editor_view.rs        # Source/diff editor
│       ├── rule_editor.rs        # Rules panel
│       └── page_list.rs          # Page list sidebar with search
└── README.md
```

### Features

- **Adaptive three-panel layout**: Modern libadwaita styling
- **Header bar**: Modern GNOME-style header with menu
- **Toolbar actions**: Integrated into header bar
- **Status bar**: Progress indicator at bottom
- **Notebook/tabs**: Source and diff views
- **Rule editor**: ListBox with inline editing
- **Page list**: Searchable sidebar
- **Native dialogs**: Login dialog using AdwDialog
- **Direct Rust integration**: No FFI overhead

### Building

```bash
cd ui/linux/awb_gtk
cargo build --release
cargo run --release
```

**Prerequisites**: Rust toolchain, GTK4, libadwaita

**Note**: Will not compile on macOS/Windows without GTK4 installed. This is Linux-specific.

### GTK4 Dependencies

Added to `Cargo.toml`:
```toml
gtk = { package = "gtk4", version = "0.9", features = ["v4_10"] }
libadwaita = { version = "0.7", features = ["v1_5"] }
glib = "0.20"
gio = "0.20"
sourceview = { package = "sourceview5", version = "0.9" }
```

## Comparison

| Aspect | Windows (WPF) | Linux (GTK4) |
|--------|---------------|--------------|
| **Language** | C# | Rust |
| **UI Framework** | WPF | GTK4/libadwaita |
| **Integration** | P/Invoke FFI | Direct crate usage |
| **Build System** | .NET 8 SDK | Cargo |
| **MVVM** | Yes (CommunityToolkit.Mvvm) | No (GTK signals/properties) |
| **Data Binding** | Two-way WPF binding | Manual signal handlers |
| **Theme Support** | Windows themes | libadwaita adaptive |
| **Platform** | Windows only | Linux only |
| **FFI Overhead** | Yes (marshaling) | No (native Rust) |
| **Distribution** | .exe + awb_ffi.dll | Single binary |

## Common UI Elements

Both UIs implement the same logical structure:

### Main Window Layout
```
┌────────────────────────────────────────────────────────────┐
│ Menu Bar / Header Bar                                      │
├───────────┬────────────────────────────┬───────────────────┤
│           │                            │                   │
│   Page    │        Editor              │      Rules        │
│   List    │   ┌──────────────────┐     │   ┌───────────┐   │
│           │   │ Source View      │     │   │ Rule 1    │   │
│ ┌───────┐ │   │                  │     │   │ Rule 2    │   │
│ │Page 1 │ │   │ (Tabbed:         │     │   │ Rule 3    │   │
│ │Page 2 │ │   │  Source/Diff)    │     │   └───────────┘   │
│ │Page 3 │ │   │                  │     │   [+] [-] [↑] [↓] │
│ └───────┘ │   └──────────────────┘     │                   │
│           │                            │                   │
├───────────┴────────────────────────────┴───────────────────┤
│ Status: Ready         | 0 pages | [████████░░] 80%        │
└────────────────────────────────────────────────────────────┘
```

### Login Dialog
- Wiki URL input (default: `https://en.wikipedia.org/w/api.php`)
- Username input
- Password input
- Login/Cancel buttons

### Editor View
- **Source tab**: Editable monospace text area
- **Diff tab**: Split view showing before/after comparison

### Rule Editor
- List/grid of find/replace rules
- Fields: Enabled (checkbox), Pattern, Replacement, IsRegex, CaseSensitive
- Actions: Add, Remove, Move Up, Move Down

## Implementation Status

### ✅ Implemented (Both UIs)
- Main window structure with three panels
- Menu/header bar with File, Edit, View, Tools, Help
- Login dialog with validation
- Editor with source and diff views
- Rule editor with add/remove/reorder UI
- Page list with search
- Status bar with progress indicator
- Basic MVVM/MVC architecture

### ⏳ To Be Implemented (Both UIs)
- Actual FFI calls to Rust backend (Windows)
- Direct crate integration (Linux)
- MediaWiki API session management
- Page fetching from wiki
- Rule application via engine
- Diff computation and visualization
- Save functionality with edit summaries
- Settings dialog
- Syntax highlighting for WikiText
- Error handling and toast notifications
- Undo/redo for editor
- Keyboard shortcuts

## Next Steps

### For Windows UI:
1. Build `awb_ffi.dll` from `crates/awb_ffi`
2. Complete P/Invoke declarations in `NativeMethods.cs`
3. Implement FFI calls in MainWindow event handlers
4. Add error handling for FFI calls
5. Test on Windows system

### For Linux UI:
1. Add direct calls to `awb_engine`, `awb_mw_api` crates
2. Implement async operations with `glib::spawn_future_local`
3. Add GtkSourceView for syntax highlighting
4. Implement diff visualization
5. Test on Linux system with GTK4 installed

### For Both:
1. Implement session state management
2. Add page caching
3. Implement autosave
4. Add logging and telemetry
5. Create installers/packages

## Testing Notes

### Windows
- **Compile check**: `dotnet build` (requires Windows)
- **Runtime**: Requires `awb_ffi.dll` in same directory as executable
- **Testing**: Mock FFI functions for UI testing

### Linux
- **Compile check**: `cargo check -p awb_gtk` (requires GTK4 development libraries)
- **Runtime**: Requires GTK4 and libadwaita runtime
- **Testing**: Cannot build on macOS without GTK4 installed (expected)

## File Counts

- **Windows UI**: 16 files (8 .cs, 6 .xaml, 1 .csproj, 1 README)
- **Linux UI**: 10 files (9 .rs, 1 Cargo.toml update, 1 README)

## Repository Structure

```
awb-rs/
├── crates/
│   ├── awb_ffi/              # FFI layer (for Windows/macOS)
│   └── ...
└── ui/
    ├── windows/
    │   └── AWBrowser/        # C# WPF application
    ├── linux/
    │   └── awb_gtk/          # GTK4 Rust application
    └── macos/
        └── AWBrowser/        # Swift/SwiftUI application (existing)
```

## Conclusion

Both Windows and Linux UI shells are now structurally complete with:
- Full three-panel layout
- All core UI components
- Architecture ready for backend integration
- Platform-appropriate styling and conventions

The implementations follow platform best practices:
- Windows: WPF with MVVM, P/Invoke for native interop
- Linux: GTK4 with libadwaita, direct Rust integration

Ready for backend integration phase.
