# AWB Plugins

Plugin system for AutoWikiBrowser Rust (AWB-RS) allowing user-defined text transformations via Lua scripts and WebAssembly modules.

## Features

- **Lua Plugins**: Write plugins in Lua with MediaWiki helper functions
- **WASM Plugins**: Write plugins in any language that compiles to WebAssembly (Rust, C, AssemblyScript, etc.)
- **Sandboxing**: Automatic resource limits and security restrictions
- **Integration**: Seamless integration with AWB's FixModule system

## Quick Start

### Loading Plugins

```rust
use awb_plugins::PluginManager;

let mut manager = PluginManager::new();
manager.load_from_directory("./plugins")?;

// Apply all enabled plugins
let result = manager.apply_all("Some wikitext")?;
```

### Creating a Lua Plugin

Create a file `uppercase.lua`:

```lua
description = "Converts text to uppercase"

function transform(text)
    return string.upper(text)
end
```

### Creating a WASM Plugin

Compile from Rust (or any WASM-compatible language):

```rust
#[no_mangle]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let mut buf = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn transform(ptr: *const u8, len: i32) -> *const u8 {
    let input = unsafe {
        std::slice::from_raw_parts(ptr, len as usize)
    };
    let text = String::from_utf8_lossy(input);

    // Transform the text
    let result = text.to_uppercase();

    // Return length-prefixed string
    let result_bytes = result.as_bytes();
    let result_len = result_bytes.len() as i32;

    let total_size = 4 + result_bytes.len();
    let output = alloc(total_size as i32);

    unsafe {
        // Write length
        *(output as *mut i32) = result_len;
        // Write string data
        std::ptr::copy_nonoverlapping(
            result_bytes.as_ptr(),
            output.add(4),
            result_bytes.len()
        );
    }

    output
}
```

Compile with:
```bash
cargo build --target wasm32-unknown-unknown --release
```

## Lua API

### Standard Libraries

Lua plugins have access to these standard libraries:
- `string` - String manipulation
- `table` - Table operations
- `math` - Mathematical functions
- `utf8` - UTF-8 string support

The following are **blocked** for security:
- `os` - Operating system functions
- `io` - File I/O
- `debug` - Debug interface
- `package` - Module loading

### MediaWiki Helper Functions

The `mw` table provides MediaWiki-specific helpers:

#### `mw.title(text)`
Extract the page title from wikitext.

```lua
local title = mw.title("== Main Title ==\nContent")
-- Returns: "Main Title"
```

#### `mw.is_redirect(text)`
Check if the page is a redirect.

```lua
if mw.is_redirect(text) then
    print("This is a redirect page")
end
```

#### `mw.categories(text)`
Extract all categories from wikitext.

```lua
local cats = mw.categories(text)
for i, category in ipairs(cats) do
    print(category)
end
```

### Example Lua Plugins

**Remove external links:**
```lua
description = "Removes all external links"

function transform(text)
    -- Remove [http://... ] style links
    text = text:gsub("%[http[s]?://[^%]]+%]", "")
    return text
end
```

**Fix double spaces:**
```lua
description = "Replaces multiple spaces with single space"

function transform(text)
    return text:gsub("  +", " ")
end
```

**Category counter:**
```lua
description = "Adds category count at the end"

function transform(text)
    local cats = mw.categories(text)
    local count = #cats
    return text .. "\n<!-- " .. count .. " categories -->"
end
```

## WASM API

WASM plugins must export two functions:

### `alloc(size: i32) -> *mut u8`
Allocates memory for string passing. Return a pointer to allocated memory.

### `transform(ptr: i32, len: i32) -> i32`
Transforms the input text.
- **Input**: Pointer and length of UTF-8 string
- **Output**: Pointer to length-prefixed result (4 bytes length + string data)

## Sandboxing

All plugins run with strict resource limits:

| Resource | Default Limit | Purpose |
|----------|---------------|---------|
| Execution Time | 5 seconds | Prevent infinite loops |
| Memory (Lua) | 1 MB | Prevent memory exhaustion |
| Instructions (Lua) | 1,000,000 | Prevent runaway code |
| Fuel (WASM) | 10,000,000 | Control computation cost |

### Custom Sandbox Configuration

```rust
use awb_plugins::{PluginManager, SandboxConfig};
use std::time::Duration;

let config = SandboxConfig {
    timeout: Duration::from_secs(10),
    memory_limit: 2 * 1024 * 1024, // 2MB
    instruction_limit: Some(5_000_000),
    wasm_fuel: 50_000_000,
};

let manager = PluginManager::with_config(config);
```

## Integration with AWB

Plugins integrate seamlessly with AWB's fix pipeline:

```rust
use awb_plugins::PluginFixModule;
use awb_engine::general_fixes::FixRegistry;

// Load plugins as a fix module
let plugin_module = PluginFixModule::from_directory("./plugins")?;

// Add to fix registry
let mut registry = FixRegistry::new();
registry.add_module(Box::new(plugin_module));
```

## Plugin Manager API

### Loading Plugins

```rust
// From directory
manager.load_from_directory("./plugins")?;

// Individual files
manager.load_lua_plugin("scripts/my_plugin.lua")?;
manager.load_wasm_plugin("modules/my_plugin.wasm")?;
```

### Managing Plugins

```rust
// List all plugins
let names = manager.plugin_names();

// Enable/disable
manager.disable_plugin("uppercase");
manager.enable_plugin("uppercase");

// Check status
if manager.is_enabled("uppercase") {
    println!("Plugin is active");
}

// Get plugin count
println!("Loaded: {}, Enabled: {}",
    manager.plugin_count(),
    manager.enabled_count()
);
```

### Applying Plugins

```rust
// Apply all enabled plugins in order
let result = manager.apply_all(text)?;

// Apply a specific plugin
let result = manager.apply_plugin("uppercase", text)?;
```

## Error Handling

The plugin system provides detailed error types:

```rust
use awb_plugins::PluginError;

match manager.apply_plugin("my_plugin", text) {
    Ok(result) => println!("Success: {}", result),
    Err(PluginError::LoadFailed(msg)) => eprintln!("Load error: {}", msg),
    Err(PluginError::ExecutionFailed(msg)) => eprintln!("Execution error: {}", msg),
    Err(PluginError::Timeout(secs)) => eprintln!("Timed out after {}s", secs),
    Err(PluginError::Sandboxed(msg)) => eprintln!("Sandbox violation: {}", msg),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Best Practices

1. **Test plugins independently** before loading them into AWB
2. **Set appropriate timeouts** for complex transformations
3. **Handle errors gracefully** - one plugin failure shouldn't break the pipeline
4. **Document your plugins** with clear descriptions
5. **Use MediaWiki helpers** instead of reimplementing common operations
6. **Profile WASM plugins** to optimize fuel consumption
7. **Version your plugins** and track changes

## Security Considerations

- Lua plugins cannot access the filesystem, network, or OS functions
- WASM plugins run in a sandboxed environment with no system access
- Resource limits prevent denial-of-service attacks
- All string data is validated as UTF-8
- Memory allocation is controlled and limited

## Performance Tips

### Lua
- Avoid string concatenation in loops (use tables instead)
- Cache regex patterns outside transform function if possible
- Use string.gsub instead of manual character iteration

### WASM
- Minimize memory allocations
- Use efficient string algorithms
- Consider compiling with optimizations (`--release`)
- Profile fuel consumption and optimize hot paths

## Troubleshooting

**Plugin not loading:**
- Check file extension (`.lua` or `.wasm`)
- Verify file permissions
- Check syntax (for Lua) or WASM exports

**Plugin times out:**
- Increase timeout in SandboxConfig
- Optimize algorithm
- Check for infinite loops

**Memory errors:**
- Increase memory_limit
- Reduce data structure sizes
- Check for memory leaks in WASM

**Incorrect results:**
- Verify transform function logic
- Test with simple inputs first
- Check UTF-8 encoding

## Examples Directory Structure

```
plugins/
├── lua/
│   ├── remove_external_links.lua
│   ├── fix_double_spaces.lua
│   └── category_sorter.lua
└── wasm/
    ├── markdown_to_wikitext.wasm
    └── citation_formatter.wasm
```

## License

MIT OR Apache-2.0
