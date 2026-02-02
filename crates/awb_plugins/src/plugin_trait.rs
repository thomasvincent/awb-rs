use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    Lua,
    Wasm,
    Native,
}

/// Core trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Unique identifier for the plugin
    fn name(&self) -> &str;

    /// Human-readable description of what the plugin does
    fn description(&self) -> &str;

    /// Transform input text and return the modified version
    fn transform(&self, input: &str) -> Result<String>;

    /// The type of plugin (Lua, WASM, or Native)
    fn plugin_type(&self) -> PluginType;
}
