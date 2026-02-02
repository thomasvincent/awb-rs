# awb-rs

[![CI](https://github.com/thomasvincent/awb-rs/workflows/CI/badge.svg)](https://github.com/thomasvincent/awb-rs/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A modern, high-performance AutoWikiBrowser (AWB) rewrite in Rust, providing automated Wikipedia editing capabilities with a focus on safety, performance, and extensibility.

## Architecture

```
awb-rs/
├── awb-core/           # Core business logic and abstractions
├── awb-api/            # MediaWiki API client implementation
├── awb-cli/            # Command-line interface
├── awb-rules/          # Rule engine for edit automation
├── awb-lua-plugin/     # Lua plugin runtime
├── awb-wasm-plugin/    # WebAssembly plugin runtime
├── awb-security/       # Security and sandboxing
├── awb-db/             # Persistent state management
├── awb-ui/             # Terminal UI components
├── awb-fixtures/       # Test fixtures and mock data
└── awb-integration/    # Integration test suite
```

**Data Flow:**
```
CLI → Core → API → MediaWiki
       ↓
    Rules Engine → Plugins (Lua/WASM)
       ↓
    Security Layer → Sandboxed Execution
       ↓
    Database → State Persistence
```

## Features

- **Modern Architecture**: Async/await with Tokio runtime for high performance
- **Multi-Wiki Support**: Work with any MediaWiki-powered wiki
- **Rule-Based Editing**: Declarative TOML-based rule definitions
- **Plugin System**: Extensible via Lua scripts and WebAssembly modules
- **Security Hardened**: Sandboxed plugin execution with resource limits
- **Batch Operations**: Efficient bulk page processing
- **Interactive TUI**: Rich terminal interface for monitoring and control
- **Dry-Run Mode**: Preview changes before applying them
- **State Management**: Resume interrupted sessions
- **Rate Limiting**: Respects wiki API limits and best practices

## Build Instructions

### Prerequisites

- **Rust**: Version 1.85 or higher
- **Cargo**: Included with Rust installation

### Building from Source

```bash
# Clone the repository
git clone https://github.com/thomasvincent/awb-rs.git
cd awb-rs

# Build all workspace crates
cargo build --workspace --release

# Run tests to verify installation
cargo test --workspace

# Install the binary
cargo install --path awb-cli
```

## Usage Examples

### Authentication

```bash
# Login to a wiki (stores credentials securely)
awb-rs login \
  --wiki https://en.wikipedia.org/w/api.php \
  --username MyBot \
  --password-stdin

# Login with OAuth
awb-rs login \
  --wiki https://en.wikipedia.org/w/api.php \
  --oauth-consumer-key YOUR_KEY \
  --oauth-consumer-secret YOUR_SECRET
```

### Listing Pages

```bash
# List pages from a category
awb-rs list \
  --source category \
  --query "Category:Stubs" \
  --wiki https://en.wikipedia.org/w/api.php

# List pages from a search
awb-rs list \
  --source search \
  --query "hastemplate:cleanup" \
  --wiki https://en.wikipedia.org/w/api.php

# List pages from a file
awb-rs list \
  --source file \
  --query pages.txt \
  --wiki https://en.wikipedia.org/w/api.php
```

### Running Edit Rules

```bash
# Run rules with a profile (dry-run mode)
awb-rs run \
  --wiki https://en.wikipedia.org/w/api.php \
  --profile my-rules.toml \
  --dry-run

# Apply rules to real pages
awb-rs run \
  --wiki https://en.wikipedia.org/w/api.php \
  --profile my-rules.toml \
  --source category \
  --query "Category:Articles needing cleanup"
```

### Automated Bot Mode

```bash
# Run in unattended bot mode with limits
awb-rs bot \
  --wiki https://en.wikipedia.org/w/api.php \
  --profile my-rules.toml \
  --max-edits 100 \
  --rate-limit 6 \
  --dry-run

# Resume from previous session
awb-rs bot \
  --wiki https://en.wikipedia.org/w/api.php \
  --profile my-rules.toml \
  --resume session-12345.db
```

### Example Rule Profile

Create a `my-rules.toml` file:

```toml
[profile]
name = "Cleanup Bot"
description = "Fix common formatting issues"
edit_summary = "Automated cleanup (Bot)"

[[rules]]
name = "Fix double spaces"
pattern = "  +"
replacement = " "
regex = true

[[rules]]
name = "Fix external links"
pattern = "http://en.wikipedia.org"
replacement = "https://en.wikipedia.org"

[[rules]]
name = "Add missing categories"
plugin = "add_categories.lua"
```

## Plugin System

awb-rs supports two plugin types:

### Lua Plugins

```lua
-- add_categories.lua
function transform(page)
  if not page.content:match("%[%[Category:") then
    page.content = page.content .. "\n[[Category:Uncategorized]]"
  end
  return page
end
```

### WebAssembly Plugins

```rust
// Compile to WASM with: cargo build --target wasm32-wasi
#[no_mangle]
pub extern "C" fn transform(content: &str) -> String {
    content.replace("foo", "bar")
}
```

**Plugin Features:**
- Sandboxed execution with resource limits (memory, CPU time)
- Access to safe subset of APIs (no filesystem, network)
- Version compatibility checking
- Hot-reload support in development mode

## Security Model

- **Credential Storage**: Uses OS keyring for secure password storage
- **Plugin Sandboxing**: Lua and WASM run in isolated environments
- **Resource Limits**: CPU time, memory, and instruction count limits per plugin
- **API Rate Limiting**: Respects Retry-After headers and configurable delays
- **Edit Validation**: Preview changes before applying, audit logs
- **OAuth Support**: Preferred authentication method for bots

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Project Status

This is an active rewrite of AutoWikiBrowser in Rust. Current status:

- [x] Core API client
- [x] Rule engine
- [x] CLI interface
- [x] Plugin system (Lua/WASM)
- [x] Security sandboxing
- [ ] Terminal UI (in progress)
- [ ] GUI (planned)
- [ ] Advanced diff visualization (planned)

## Links

- [Documentation](https://docs.rs/awb-rs)
- [Issue Tracker](https://github.com/thomasvincent/awb-rs/issues)
- [Original AWB](https://en.wikipedia.org/wiki/Wikipedia:AutoWikiBrowser)
