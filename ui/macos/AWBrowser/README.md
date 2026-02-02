# AWBrowser - macOS Native UI

This is the macOS native user interface for AWB-RS (AutoWikiBrowser for Rust).

## Architecture

- **Swift UI**: Modern SwiftUI-based interface
- **FFI Bridge**: Calls into Rust core via UniFFI
- **MVVM Pattern**: Clean separation between views and business logic

## Structure

```
Sources/AWBrowser/
├── App.swift                  # App entry point
├── Views/
│   ├── LoginView.swift        # Authentication UI
│   ├── MainView.swift         # Main window with sidebar
│   ├── EditorView.swift       # Split editor with diff
│   └── RuleEditorView.swift   # Find/replace rule management
├── ViewModels/
│   └── SessionViewModel.swift # State management, FFI calls
└── Models/
    └── PageModel.swift        # Data models matching FFI
```

## Building

### Prerequisites

1. **Build Rust FFI library**:
   ```bash
   cd ../../..  # Return to project root
   cargo build -p awb_ffi --release
   ```

2. **Generate UniFFI bindings**:
   ```bash
   cargo run --bin uniffi-bindgen generate \
     crates/awb_ffi/src/awb_ffi.udl \
     --language swift \
     --out-dir ui/macos/AWBrowser/Sources/AWBrowser/Generated
   ```

3. **Build Swift package**:
   ```bash
   swift build
   ```

### Xcode

To create an Xcode project:

```bash
swift package generate-xcodeproj
```

Then open `AWBrowser.xcodeproj` in Xcode.

## FFI Interface

The Swift code calls into Rust via these FFI functions:

- `create_session(wiki_url, username, password) -> SessionHandle`
- `login(handle) -> Result<(), FfiError>`
- `fetch_list(handle, source, query) -> Result<Vec<String>, FfiError>`
- `get_page(handle, title) -> Result<PageInfo, FfiError>`
- `apply_rules(handle, content, rules_json) -> Result<TransformResult, FfiError>`
- `save_page(handle, title, content, summary) -> Result<(), FfiError>`
- `compute_diff(old, new) -> String`

## Current Status

- ✅ UI structure implemented
- ✅ FFI layer defined in Rust
- ✅ Swift models matching FFI types
- ⚠️ UniFFI bindings not yet generated (requires `uniffi-bindgen` CLI)
- ⚠️ Placeholder FFI functions in SessionViewModel (will be replaced by generated bindings)
- 🔲 Actual MediaWiki API integration in Rust
- 🔲 Persistent session storage
- 🔲 Rule import/export

## Integration Steps

1. The Rust crate `awb_ffi` exposes a UniFFI interface via `awb_ffi.udl`
2. UniFFI generates Swift bindings from the UDL file
3. The generated Swift code is imported into this package
4. `SessionViewModel` calls the generated FFI functions
5. The UI remains decoupled from the FFI details

## Testing

Run Swift tests:
```bash
swift test
```

Note: Full integration testing requires the Rust library to be compiled and bindings generated.
