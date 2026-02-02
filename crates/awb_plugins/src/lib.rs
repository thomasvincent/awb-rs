//! # AWB Plugins
//!
//! Plugin system for AutoWikiBrowser Rust (AWB-RS) allowing user-defined text transformations
//! via Lua scripts and WebAssembly modules.
//!
//! ## Features
//!
//! - **Lua Plugins**: Write plugins in Lua with MediaWiki helper functions
//! - **WASM Plugins**: Write plugins in any language that compiles to WebAssembly
//! - **Sandboxing**: Automatic resource limits and security restrictions
//! - **Integration**: Seamless integration with AWB's FixModule system
//!
//! ## Example: Loading Lua Plugins
//!
//! ```rust,no_run
//! use awb_plugins::PluginManager;
//!
//! let mut manager = PluginManager::new();
//! manager.load_from_directory("./plugins").unwrap();
//!
//! let result = manager.apply_all("Some wikitext").unwrap();
//! ```
//!
//! ## Example: Creating a Lua Plugin
//!
//! ```lua
//! -- uppercase.lua
//! description = "Converts text to uppercase"
//!
//! function transform(text)
//!     return string.upper(text)
//! end
//! ```
//!
//! ## Sandboxing
//!
//! All plugins run in a sandboxed environment with:
//! - Memory limits (1MB default for Lua)
//! - Execution time limits (5s default)
//! - No filesystem or network access
//! - Instruction count limits
//!
//! ## MediaWiki Helper Functions (Lua)
//!
//! Lua plugins have access to `mw` table with helper functions:
//! - `mw.title(text)` - Extract page title
//! - `mw.is_redirect(text)` - Check if page is a redirect
//! - `mw.categories(text)` - Extract all categories

pub mod error;
pub mod lua_plugin;
pub mod plugin_manager;
pub mod plugin_trait;
pub mod sandbox;
pub mod wasm_plugin;

// Re-export main types
pub use error::{PluginError, Result};
pub use lua_plugin::LuaPlugin;
pub use plugin_manager::{PluginFixModule, PluginManager};
pub use plugin_trait::{Plugin, PluginType};
pub use sandbox::SandboxConfig;
pub use wasm_plugin::WasmPlugin;
