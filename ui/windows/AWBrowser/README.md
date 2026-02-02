# AWBrowser Windows UI

Windows WPF application for AWB-RS (AutoWikiBrowser Rust Edition).

## Overview

This is a C# WPF application that provides a native Windows interface to the AWB-RS engine via P/Invoke to the Rust FFI layer.

## Architecture

```
┌─────────────────────────────────────┐
│     AWBrowser.exe (C# WPF)          │
│  ┌──────────────────────────────┐   │
│  │  MainWindow (UI)             │   │
│  │  ├─ PageListBox               │   │
│  │  ├─ SourceEditor              │   │
│  │  ├─ DiffView                  │   │
│  │  └─ RulesGrid                 │   │
│  └──────────────────────────────┘   │
│           │                          │
│  ┌────────▼──────────────────────┐   │
│  │  NativeMethods (P/Invoke)     │   │
│  └────────┬──────────────────────┘   │
└───────────┼──────────────────────────┘
            │
            ▼
   ┌────────────────────┐
   │  awb_ffi.dll       │
   │  (Rust C ABI)      │
   └────────────────────┘
```

## Project Structure

```
AWBrowser/
├── AWBrowser.csproj          # Project file (.NET 8 WPF)
├── App.xaml                  # Application entry point
├── App.xaml.cs
├── NativeMethods.cs          # P/Invoke declarations
├── MainWindow.xaml           # Main window UI
├── MainWindow.xaml.cs
├── Views/
│   ├── LoginWindow.xaml      # Login dialog
│   ├── LoginWindow.xaml.cs
│   ├── EditorView.xaml       # Split diff editor
│   ├── EditorView.xaml.cs
│   ├── RuleEditorView.xaml   # Rules panel
│   └── RuleEditorView.xaml.cs
├── ViewModels/
│   └── SessionViewModel.cs   # MVVM view model
└── Models/
    ├── PageModel.cs          # Page data model
    └── RuleModel.cs          # Rule data model
```

## Building

### Prerequisites

- .NET 8 SDK
- Visual Studio 2022 (optional, for IDE support)
- Rust toolchain (to build awb_ffi.dll)

### Build Steps

1. **Build the Rust FFI library:**
   ```bash
   cd ../../..  # Go to awb-rs root
   cargo build --release -p awb_ffi
   ```

2. **Build the WPF application:**
   ```bash
   cd ui/windows/AWBrowser
   dotnet restore
   dotnet build
   ```

3. **Run the application:**
   ```bash
   dotnet run
   ```

## Features

### Implemented
- Main window layout with menu bar, toolbar, status bar
- Three-panel layout: page list, editor, rules
- Login dialog
- Split diff view
- Rule editor grid
- Basic MVVM architecture

### To Be Implemented
- FFI integration (session management, page operations)
- Actual rule application via FFI
- Diff computation and visualization
- Page list fetching from MediaWiki API
- Save functionality
- Settings dialog
- Syntax highlighting for WikiText

## FFI Integration

The `NativeMethods.cs` file declares P/Invoke signatures for the Rust C API. Currently implemented:

- `awb_version()` - Get library version
- `awb_free_string(IntPtr)` - Free native strings

To be added:
- `create_session()` - Create wiki session
- `login()` - Authenticate
- `fetch_list()` - Get page list
- `get_page()` - Fetch page content
- `apply_rules()` - Apply transformations
- `save_page()` - Save changes
- `compute_diff()` - Generate diff

## Development Notes

### MVVM Pattern
The application uses the Model-View-ViewModel pattern:
- **Models**: `PageModel`, `RuleModel` (data structures)
- **Views**: XAML files (UI layout)
- **ViewModels**: `SessionViewModel` (business logic, state)

### Data Binding
WPF data binding connects UI to ViewModels:
```xml
<TextBlock Text="{Binding StatusMessage}"/>
```

### Dependencies
- **CommunityToolkit.Mvvm**: Modern MVVM helpers
- **AvalonEdit**: Text editor with syntax highlighting (future use)
- **DiffPlex**: Diff computation library

## License

MIT OR Apache-2.0 (dual licensed, matching awb-rs)
