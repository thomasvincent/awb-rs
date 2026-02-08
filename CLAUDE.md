# CLAUDE.md

High-performance AutoWikiBrowser rewrite providing automated Wikipedia editing with sandboxed plugins.

## Stack
- Rust (edition 2024, MSRV 1.85)
- Workspace with 11+ crates
- Async runtime: Tokio

## Build & Test
```bash
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Architecture
- Domain-driven design with separate crates for API, engine, storage, security
- Plugin runtimes: Lua (mlua) and WebAssembly (wasmtime)
- OAuth support via oauth2 crate
- FFI bindings generated with uniffi

## Notes
- CLI binary: `awb-cli`
- Rule definitions in TOML
- Credentials stored in OS keyring
