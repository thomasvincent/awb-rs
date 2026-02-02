use crate::error::{PluginError, Result};
use crate::lua_plugin::LuaPlugin;
use crate::plugin_trait::Plugin;
use crate::sandbox::SandboxConfig;
use crate::wasm_plugin::WasmPlugin;
use awb_engine::general_fixes::{FixContext, FixModule};
use indexmap::IndexMap;
use std::borrow::Cow;
use std::path::Path;
use tracing::{debug, info, warn};

/// Manages a collection of plugins and integrates them with the AWB fix pipeline
pub struct PluginManager {
    plugins: IndexMap<String, Box<dyn Plugin>>,
    enabled: IndexMap<String, bool>,
    #[allow(dead_code)]
    config: SandboxConfig,
}

impl PluginManager {
    /// Create a new plugin manager with default configuration
    pub fn new() -> Self {
        Self::with_config(SandboxConfig::default())
    }

    /// Create a new plugin manager with custom sandbox configuration
    pub fn with_config(config: SandboxConfig) -> Self {
        Self {
            plugins: IndexMap::new(),
            enabled: IndexMap::new(),
            config,
        }
    }

    /// Load all plugins from a directory
    ///
    /// Scans for *.lua and *.wasm files and loads them as plugins
    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<usize> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Err(PluginError::LoadFailed(format!(
                "Plugin directory does not exist: {}",
                dir.display()
            )));
        }

        if !dir.is_dir() {
            return Err(PluginError::LoadFailed(format!(
                "Path is not a directory: {}",
                dir.display()
            )));
        }

        let mut loaded_count = 0;

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                match path.extension().and_then(|s| s.to_str()) {
                    Some("lua") => match self.load_lua_plugin(&path) {
                        Ok(name) => {
                            info!("Loaded Lua plugin: {}", name);
                            loaded_count += 1;
                        }
                        Err(e) => {
                            warn!("Failed to load Lua plugin {}: {}", path.display(), e);
                        }
                    },
                    Some("wasm") => match self.load_wasm_plugin(&path) {
                        Ok(name) => {
                            info!("Loaded WASM plugin: {}", name);
                            loaded_count += 1;
                        }
                        Err(e) => {
                            warn!("Failed to load WASM plugin {}: {}", path.display(), e);
                        }
                    },
                    _ => {
                        debug!("Skipping non-plugin file: {}", path.display());
                    }
                }
            }
        }

        info!("Loaded {} plugins from {}", loaded_count, dir.display());

        Ok(loaded_count)
    }

    /// Load a Lua plugin from a file
    pub fn load_lua_plugin<P: AsRef<Path>>(&mut self, path: P) -> Result<String> {
        let plugin = LuaPlugin::from_file(path)?;
        let name = plugin.name().to_string();
        self.add_plugin(Box::new(plugin));
        Ok(name)
    }

    /// Load a WASM plugin from a file
    pub fn load_wasm_plugin<P: AsRef<Path>>(&mut self, path: P) -> Result<String> {
        let plugin = WasmPlugin::from_file(path)?;
        let name = plugin.name().to_string();
        self.add_plugin(Box::new(plugin));
        Ok(name)
    }

    /// Add a plugin to the manager
    pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) {
        let name = plugin.name().to_string();
        self.enabled.insert(name.clone(), true); // Enable by default
        self.plugins.insert(name, plugin);
    }

    /// Remove a plugin by name
    pub fn remove_plugin(&mut self, name: &str) -> Option<Box<dyn Plugin>> {
        self.enabled.swap_remove(name);
        self.plugins.swap_remove(name)
    }

    /// Enable a plugin by name
    pub fn enable_plugin(&mut self, name: &str) -> bool {
        if self.plugins.contains_key(name) {
            self.enabled.insert(name.to_string(), true);
            true
        } else {
            false
        }
    }

    /// Disable a plugin by name
    pub fn disable_plugin(&mut self, name: &str) -> bool {
        if self.plugins.contains_key(name) {
            self.enabled.insert(name.to_string(), false);
            true
        } else {
            false
        }
    }

    /// Check if a plugin is enabled
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.get(name).copied().unwrap_or(false)
    }

    /// Get a list of all plugin names
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get a reference to a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    /// Apply all enabled plugins to the input text in order
    pub fn apply_all(&self, input: &str) -> Result<String> {
        let mut result = input.to_string();

        for (name, plugin) in &self.plugins {
            if self.is_enabled(name) {
                match plugin.transform(&result) {
                    Ok(transformed) => {
                        if transformed != result {
                            debug!("Plugin '{}' modified text", name);
                        }
                        result = transformed;
                    }
                    Err(e) => {
                        warn!("Plugin '{}' failed: {}", name, e);
                        // Continue with other plugins even if one fails
                    }
                }
            }
        }

        Ok(result)
    }

    /// Apply a specific plugin by name
    pub fn apply_plugin(&self, name: &str, input: &str) -> Result<String> {
        let plugin = self
            .plugins
            .get(name)
            .ok_or_else(|| PluginError::LoadFailed(format!("Plugin '{}' not found", name)))?;

        if !self.is_enabled(name) {
            return Ok(input.to_string());
        }

        plugin.transform(input)
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get the number of enabled plugins
    pub fn enabled_count(&self) -> usize {
        self.enabled.values().filter(|&&v| v).count()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter to integrate PluginManager with the AWB FixModule system
pub struct PluginFixModule {
    manager: PluginManager,
}

impl PluginFixModule {
    /// Create a new PluginFixModule
    pub fn new(manager: PluginManager) -> Self {
        Self { manager }
    }

    /// Load plugins from a directory and create a FixModule
    pub fn from_directory<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let mut manager = PluginManager::new();
        manager.load_from_directory(dir)?;
        Ok(Self::new(manager))
    }

    /// Get a reference to the underlying plugin manager
    pub fn manager(&self) -> &PluginManager {
        &self.manager
    }

    /// Get a mutable reference to the underlying plugin manager
    pub fn manager_mut(&mut self) -> &mut PluginManager {
        &mut self.manager
    }
}

impl FixModule for PluginFixModule {
    fn id(&self) -> &str {
        "plugins"
    }

    fn display_name(&self) -> &str {
        "User Plugins"
    }

    fn category(&self) -> &str {
        "Plugins"
    }

    fn description(&self) -> &str {
        "User-defined plugins (Lua and WASM)"
    }

    fn apply<'a>(&self, text: &'a str, _context: &FixContext) -> Cow<'a, str> {
        match self.manager.apply_all(text) {
            Ok(result) => {
                if result == text {
                    Cow::Borrowed(text)
                } else {
                    Cow::Owned(result)
                }
            }
            Err(e) => {
                warn!("Plugin execution failed: {}", e);
                Cow::Borrowed(text)
            }
        }
    }

    fn default_enabled(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua_plugin::LuaPlugin;

    #[test]
    fn test_plugin_manager_basic() {
        let mut manager = PluginManager::new();

        let script1 = r#"
            function transform(text)
                return string.upper(text)
            end
        "#;
        let plugin1 = LuaPlugin::from_string("upper", script1, SandboxConfig::default()).unwrap();

        let script2 = r#"
            function transform(text)
                return text .. "!"
            end
        "#;
        let plugin2 = LuaPlugin::from_string("exclaim", script2, SandboxConfig::default()).unwrap();

        manager.add_plugin(Box::new(plugin1));
        manager.add_plugin(Box::new(plugin2));

        assert_eq!(manager.plugin_count(), 2);
        assert_eq!(manager.enabled_count(), 2);

        let result = manager.apply_all("hello").unwrap();
        // Should apply both: uppercase then add exclamation
        assert_eq!(result, "HELLO!");
    }

    #[test]
    fn test_plugin_enable_disable() {
        let mut manager = PluginManager::new();

        let script = r#"
            function transform(text)
                return string.upper(text)
            end
        "#;
        let plugin = LuaPlugin::from_string("upper", script, SandboxConfig::default()).unwrap();
        manager.add_plugin(Box::new(plugin));

        assert!(manager.is_enabled("upper"));

        manager.disable_plugin("upper");
        assert!(!manager.is_enabled("upper"));

        let result = manager.apply_all("hello").unwrap();
        assert_eq!(result, "hello"); // No transformation

        manager.enable_plugin("upper");
        let result = manager.apply_all("hello").unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_plugin_fix_module_integration() {
        use awb_domain::types::{Namespace, Title};

        let mut manager = PluginManager::new();

        let script = r#"
            function transform(text)
                return string.upper(text)
            end
        "#;
        let plugin = LuaPlugin::from_string("upper", script, SandboxConfig::default()).unwrap();
        manager.add_plugin(Box::new(plugin));

        let fix_module = PluginFixModule::new(manager);

        assert_eq!(fix_module.id(), "plugins");
        assert_eq!(fix_module.category(), "Plugins");

        let context = FixContext {
            title: Title::new(Namespace::MAIN, "Test"),
            namespace: Namespace::MAIN,
            is_redirect: false,
        };

        let result = fix_module.apply("hello world", &context);
        assert_eq!(result, "HELLO WORLD");
    }

    #[test]
    fn test_plugin_error_handling() {
        let mut manager = PluginManager::new();

        let script = r#"
            function transform(text)
                error("intentional error")
            end
        "#;
        let plugin = LuaPlugin::from_string("error", script, SandboxConfig::default()).unwrap();
        manager.add_plugin(Box::new(plugin));

        // apply_all should continue even if plugin fails
        let result = manager.apply_all("test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test"); // Text unchanged due to error
    }
}
