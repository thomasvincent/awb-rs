# Contributing to awb-rs

Thank you for your interest in contributing to awb-rs! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Be respectful, constructive, and collaborative. We aim to maintain a welcoming environment for all contributors.

## Getting Started

### Development Environment Setup

1. **Install Rust**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup update stable
   ```

2. **Clone the repository**:
   ```bash
   git clone https://github.com/thomasvincent/awb-rs.git
   cd awb-rs
   ```

3. **Install development tools**:
   ```bash
   # Format checker
   rustup component add rustfmt

   # Linter
   rustup component add clippy

   # WASM target (for plugin development)
   rustup target add wasm32-wasi
   ```

4. **Verify installation**:
   ```bash
   cargo build --workspace
   cargo test --workspace
   ```

### Project Structure

```
awb-rs/
├── awb-core/           # Core types, traits, and business logic
├── awb-api/            # MediaWiki API client implementation
├── awb-cli/            # Command-line interface and argument parsing
├── awb-rules/          # Rule engine and TOML parser
├── awb-lua-plugin/     # Lua plugin runtime (mlua)
├── awb-wasm-plugin/    # WebAssembly plugin runtime (wasmtime)
├── awb-security/       # Sandboxing and resource limits
├── awb-db/             # Database and state persistence (sled)
├── awb-ui/             # Terminal UI components (ratatui)
├── awb-fixtures/       # Test fixtures and mock data
└── awb-integration/    # End-to-end integration tests
```

### Architecture Overview

**Core Abstractions:**
- `WikiClient` trait (awb-api): HTTP client interface for MediaWiki APIs
- `RuleEngine` (awb-rules): Applies transformation rules to page content
- `PluginRuntime` (awb-lua-plugin, awb-wasm-plugin): Executes user-provided code
- `SecurityContext` (awb-security): Enforces sandboxing and resource limits

**Data Flow:**
1. CLI parses arguments → Creates configuration
2. Core initializes WikiClient and RuleEngine
3. Pages are fetched via WikiClient
4. Rules/plugins transform content
5. Changes are validated and applied
6. State is persisted to database

## Development Workflow

### 1. Code Style

We follow standard Rust conventions:

```bash
# Format code before committing
cargo fmt --all

# Check formatting (CI will enforce this)
cargo fmt --all -- --check

# Run clippy linter with strict warnings
cargo clippy --workspace -- -D warnings

# Fix clippy suggestions automatically
cargo clippy --workspace --fix
```

**Style Guidelines:**
- Use descriptive variable names (`page_title` not `pt`)
- Prefer `Result<T, E>` over panics for error handling
- Document public APIs with `///` doc comments
- Keep functions focused (< 50 lines where possible)
- Use `tracing::error!` / `warn!` / `info!` for logging (not `println!`)

### 2. Testing Requirements

All code changes must include tests:

```bash
# Run all tests
cargo test --workspace

# Run tests with logging output
RUST_LOG=debug cargo test --workspace -- --nocapture

# Run specific crate tests
cargo test -p awb-core

# Run integration tests only
cargo test -p awb-integration
```

**Test Coverage Guidelines:**
- Unit tests: Test individual functions in `#[cfg(test)]` modules
- Integration tests: Test crate public APIs in `tests/` directory
- Fixtures: Use `awb-fixtures` for shared test data
- Mock APIs: Use `wiremock` for HTTP mocking (see awb-api tests)

**Required Test Cases:**
- Happy path (expected input/output)
- Error cases (invalid input, network failures)
- Edge cases (empty strings, Unicode, very large pages)

### 3. Documentation

- Add doc comments to all public items:
  ```rust
  /// Fetches a page from the MediaWiki API.
  ///
  /// # Arguments
  /// * `title` - The page title to fetch
  ///
  /// # Returns
  /// The page content as a `String`
  ///
  /// # Errors
  /// Returns `ApiError` if the page doesn't exist or network fails
  pub async fn fetch_page(&self, title: &str) -> Result<String, ApiError>
  ```

- Update README.md if adding user-facing features
- Add examples to doc comments when helpful

### 4. Pull Request Process

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/my-new-feature
   ```

2. **Make your changes**:
   - Write code following style guidelines
   - Add tests for new functionality
   - Update documentation

3. **Run checks locally**:
   ```bash
   cargo fmt --all
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```

4. **Commit with descriptive messages**:
   ```bash
   git commit -m "feat(api): Add support for OAuth authentication

   - Implement OAuth 1.0a handshake
   - Add token storage to credentials manager
   - Update CLI to accept OAuth parameters

   Closes #123"
   ```

   **Commit Message Format:**
   - Prefix: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `perf:`, `chore:`
   - Scope in parentheses: `feat(api):`, `fix(rules):`
   - Imperative mood: "Add feature" not "Added feature"
   - Reference issues: `Closes #123`, `Fixes #456`

5. **Push and create PR**:
   ```bash
   git push origin feature/my-new-feature
   ```
   Then open a pull request on GitHub.

6. **PR Review**:
   - CI must pass (formatting, clippy, tests)
   - At least one maintainer approval required
   - Address review feedback promptly
   - Squash commits if requested

### 5. What to Contribute

**Good First Issues:**
- Bug fixes with reproduction steps
- Documentation improvements
- Test coverage additions
- Error message improvements

**Bigger Projects:**
- New rule types (regex, template-based)
- Plugin API enhancements
- UI/TUI improvements
- Performance optimizations

**Before Starting Large Features:**
- Open an issue to discuss the design
- Get feedback from maintainers
- Break work into smaller PRs if possible

## Crate-Specific Guidelines

### awb-api
- Mock all HTTP calls in tests using `wiremock`
- Add integration tests for new API endpoints
- Follow MediaWiki API conventions

### awb-rules
- Test rules with edge cases (empty pages, Unicode)
- Document rule syntax in `rules.md`
- Benchmark performance for regex-heavy rules

### awb-lua-plugin / awb-wasm-plugin
- Test sandboxing with malicious code samples
- Verify resource limits (CPU, memory) are enforced
- Check error messages are user-friendly

### awb-security
- Security-related changes require extra scrutiny
- Add tests for bypass attempts
- Document security assumptions

## Release Process

(For maintainers only)

1. Update version in all `Cargo.toml` files
2. Update CHANGELOG.md
3. Run `cargo test --workspace`
4. Tag release: `git tag v0.2.0`
5. Push: `git push --tags`
6. CI will publish to crates.io

## Getting Help

- **Questions**: Open a GitHub Discussion
- **Bugs**: Open an issue with reproduction steps
- **Security Issues**: Email maintainers directly (see SECURITY.md)

## License

By contributing, you agree that your contributions will be licensed under the same dual MIT/Apache-2.0 license as the project.
