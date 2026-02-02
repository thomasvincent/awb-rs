# AWB Plugins Implementation Summary

## Overview

A complete plugin system for AWB-RS that allows user-defined text transformations via Lua scripts and WebAssembly modules.

## Components Created

### Core Modules

1. **`src/error.rs`**
   - `PluginError` enum with variants for all error cases
   - Conversions from mlua::Error, wasmtime::Error, std::io::Error, FromUtf8Error
   - Custom errors for LoadFailed, ExecutionFailed, InvalidReturn, Timeout, Sandboxed

2. **`src/plugin_trait.rs`**
   - `Plugin` trait defining the interface all plugins must implement
   - `PluginType` enum (Lua, Wasm, Native)
   - Methods: name(), description(), transform(), plugin_type()

3. **`src/sandbox.rs`**
   - `SandboxConfig` struct with resource limits
   - Default limits: 5s timeout, 1MB memory, 1M instructions, 10M WASM fuel
   - Helper methods: with_timeout(), unlimited()

4. **`src/lua_plugin.rs`**
   - `LuaPlugin` implementation
   - Sandboxing: removes os, io, debug, package modules
   - Memory and instruction limits
   - MediaWiki helpers: mw.title(), mw.is_redirect(), mw.categories()
   - Loading from file or string
   - Comprehensive unit tests (6 tests)

5. **`src/wasm_plugin.rs`**
   - `WasmPlugin` implementation using wasmtime 27
   - Fuel-based execution limiting
   - Memory-based string passing (length-prefixed format)
   - Required exports: alloc(i32) -> i32, transform(i32, i32) -> i32
   - Unit tests including WAT-based test module

6. **`src/plugin_manager.rs`**
   - `PluginManager` for loading and managing multiple plugins
   - Loading from directory (scans *.lua and *.wasm files)
   - Enable/disable individual plugins
   - Apply all plugins in sequence
   - `PluginFixModule` adapter for AWB's FixModule system
   - 4 comprehensive integration tests

7. **`src/lib.rs`**
   - Public API exports
   - Module documentation with examples

### Dependencies Added

**Cargo.toml:**
- mlua 0.10 with lua54, vendored, async, send features
- wasmtime 27 for WASM execution
- Standard dependencies: serde, thiserror, regex, tracing

**Workspace Cargo.toml:**
- Added awb_plugins to workspace members
- Added mlua and wasmtime to workspace dependencies

### Tests

All tests pass (11 unit tests total):

**Lua Plugin Tests:**
- test_simple_lua_transform - Basic uppercase transformation
- test_mw_helpers - MediaWiki helper function usage
- test_sandboxing_blocks_os - Security: blocks os.execute()
- test_instruction_limit - Resource limit enforcement
- test_categories_helper - Category extraction helper

**WASM Plugin Tests:**
- test_wasm_plugin_uppercase - Basic WASM transformation
- test_wasm_fuel_limiting - Fuel-based resource limiting

**Plugin Manager Tests:**
- test_plugin_manager_basic - Loading and applying multiple plugins
- test_plugin_enable_disable - Enable/disable functionality
- test_plugin_fix_module_integration - Integration with FixModule
- test_plugin_error_handling - Error handling and recovery

### Documentation

1. **README.md** (comprehensive, 400+ lines)
   - Feature overview
   - Quick start guide
   - Lua API documentation
   - WASM API documentation
   - Sandboxing details
   - Integration guide
   - Error handling
   - Best practices
   - Security considerations
   - Performance tips
   - Troubleshooting

2. **IMPLEMENTATION.md** (this file)
   - Technical implementation details

### Examples

**Lua Example Plugins:**
1. `examples/lua/remove_external_links.lua` - Removes external links
2. `examples/lua/fix_double_spaces.lua` - Fixes spacing issues
3. `examples/lua/category_counter.lua` - Counts and comments categories
4. `examples/lua/stub_detector.lua` - Detects stubs and adds templates

**Rust Example:**
- `examples/basic_usage.rs` - Complete working example demonstrating:
  - Creating and loading plugins
  - Applying transformations
  - Using MediaWiki helpers
  - Plugin management (enable/disable)
  - Listing plugins

## Architecture

### Plugin Trait Hierarchy

```
Plugin (trait)
├── LuaPlugin
├── WasmPlugin
└── [Future: NativePlugin]
```

### Data Flow

```
Input Text
    ↓
PluginManager
    ↓
Plugin 1 (Lua) → transform() → Output 1
    ↓
Plugin 2 (WASM) → transform() → Output 2
    ↓
Plugin N → transform() → Final Output
```

### Integration with AWB

```
FixRegistry
    ├── Built-in FixModules
    │   ├── WhitespaceCleanup
    │   ├── HeadingSpacing
    │   └── ...
    └── PluginFixModule (wraps PluginManager)
        ├── Lua Plugins
        └── WASM Plugins
```

## Security Features

1. **Lua Sandboxing:**
   - No os, io, debug, package modules
   - Memory limits (1MB default)
   - Instruction count limits (1M default)
   - Execution timeout (5s default)

2. **WASM Sandboxing:**
   - No WASI imports (minimal capabilities)
   - Fuel-based execution limiting
   - No filesystem or network access
   - Memory bounds checking

3. **Input Validation:**
   - UTF-8 validation on all string data
   - Bounds checking on memory operations
   - Error recovery (one plugin failure doesn't break the pipeline)

## Performance Considerations

### Lua
- Vendored Lua 5.4 for consistent behavior
- Hook-based instruction counting (every 1000 instructions)
- Static atomic counter for instruction limits
- Minimal memory allocations in hot path

### WASM
- wasmtime's optimized JIT compilation
- Fuel consumption for fine-grained control
- Length-prefixed string format to avoid strlen()
- Zero-copy memory operations where possible

## Future Enhancements

Possible additions for future versions:

1. **Native Plugin Support:**
   - Load .so/.dylib/.dll plugins
   - FFI bindings for Rust plugins
   - Hot reloading support

2. **Enhanced Lua API:**
   - More MediaWiki helpers (templates, infoboxes, etc.)
   - Regex support
   - JSON parsing
   - HTTP requests (sandboxed)

3. **Plugin Marketplace:**
   - Plugin discovery and installation
   - Version management
   - Dependency resolution

4. **Performance:**
   - Plugin caching and precompilation
   - Parallel plugin execution (where safe)
   - JIT compilation for hot plugins

5. **Debugging:**
   - Plugin debugger integration
   - Execution profiling
   - Trace logging

## Testing Strategy

- Unit tests for each module
- Integration tests for plugin manager
- Example-based testing (examples compile and run)
- Security tests (sandboxing enforcement)
- Resource limit tests (timeouts, memory, fuel)

## Verification

```bash
# Compile the crate
cargo check -p awb_plugins

# Run tests
cargo test -p awb_plugins

# Run example
cargo run --example basic_usage -p awb_plugins

# Build documentation
cargo doc -p awb_plugins --open
```

All commands complete successfully with:
- Zero compilation errors
- 11/11 tests passing
- Example runs and produces correct output

## Files Created

```
crates/awb_plugins/
├── Cargo.toml                          (dependencies)
├── README.md                           (user documentation)
├── IMPLEMENTATION.md                   (this file)
├── src/
│   ├── lib.rs                         (public API)
│   ├── error.rs                       (error types)
│   ├── plugin_trait.rs                (Plugin trait)
│   ├── sandbox.rs                     (sandboxing config)
│   ├── lua_plugin.rs                  (Lua implementation)
│   ├── wasm_plugin.rs                 (WASM implementation)
│   └── plugin_manager.rs              (manager + FixModule adapter)
└── examples/
    ├── basic_usage.rs                 (Rust example)
    └── lua/
        ├── remove_external_links.lua
        ├── fix_double_spaces.lua
        ├── category_counter.lua
        └── stub_detector.lua
```

## Modified Files

```
Cargo.toml (workspace root)
├── Added awb_plugins to members
└── Added mlua and wasmtime to workspace dependencies
```

## Compatibility

- Rust edition: 2024
- Minimum Rust version: 1.85 (as per workspace)
- Dependencies: All using latest stable versions
- Platform: Cross-platform (Linux, macOS, Windows)

## Integration Points

The plugin system integrates with AWB-RS at two levels:

1. **Direct Usage:**
   ```rust
   let mut manager = PluginManager::new();
   manager.load_from_directory("./plugins")?;
   let result = manager.apply_all(text)?;
   ```

2. **Via FixModule:**
   ```rust
   let plugin_module = PluginFixModule::from_directory("./plugins")?;
   fix_registry.add_module(Box::new(plugin_module));
   ```

Both approaches work seamlessly with the existing AWB architecture.
