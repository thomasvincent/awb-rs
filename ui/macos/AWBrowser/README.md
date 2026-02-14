# AWBrowser - macOS Swift UI for AWB-RS

A native macOS application for AutoWikiBrowser Rust Edition.

## Quick Start

### Prerequisites
- macOS 14.0 or later
- Swift 5.9+
- Built `libawb_ffi.dylib` library

### Build Rust FFI Library
```bash
cd ../../..
cargo build --package awb_ffi --release
```

### Build Swift App
```bash
swift build
```

### Run
```bash
.build/debug/AWBrowser
```

## Features

- ✅ MediaWiki login with username/password
- ✅ Page list loading (Category, Transclusions, Links)
- ✅ Page editor with original/modified split view
- ✅ Rule application via FFI
- ✅ Edit summary customization
- ✅ Save functionality
- 🚧 Diff HTML rendering
- 🚧 Rule editor persistence
- 🚧 Batch processing

## Architecture

```
AWBrowser/
├── Sources/
│   ├── AwbFfiC/           # C module for FFI headers
│   │   ├── awb_ffiFFI.h
│   │   └── module.modulemap
│   └── AWBrowser/
│       ├── AwbFfi.swift   # UniFFI-generated bindings
│       ├── App.swift      # Main app entry
│       ├── Models/
│       │   └── PageModel.swift
│       ├── ViewModels/
│       │   └── SessionViewModel.swift
│       └── Views/
│           ├── LoginView.swift
│           ├── MainView.swift
│           ├── EditorView.swift
│           └── RuleEditorView.swift
└── Package.swift
```

## FFI Integration

The app uses UniFFI-generated Swift bindings to call Rust code:

```swift
// Create session
let handle = try createSession(
    wikiUrl: "https://en.wikipedia.org",
    username: "BotName",
    password: "password"
)

// Login
try login(handle: handle)

// Get page
let page = try getPage(handle: handle, title: "Main Page")

// Apply rules
let result = try applyRules(
    handle: handle,
    content: page.wikitext,
    rulesJson: "{\"enabled_rules\":[]}"
)

// Save
try savePage(
    handle: handle,
    title: "Main Page",
    content: result.newWikitext,
    summary: result.summary
)
```

## Development

### Xcode
```bash
open Package.swift
```

### Dependencies
Runtime dependency on `libawb_ffi.dylib` in:
- `../../../target/debug/` (debug builds)
- `../../../target/release/` (release builds)

### Adding Features
1. Update Rust FFI in `crates/awb_ffi/`
2. Regenerate bindings: `cargo build --package awb_ffi`
3. Copy new bindings to `Sources/AWBrowser/AwbFfi.swift`
4. Update ViewModels to use new functions

## License

Same as AWB-RS parent project.
