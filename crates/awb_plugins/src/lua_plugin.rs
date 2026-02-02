use crate::error::{PluginError, Result};
use crate::plugin_trait::{Plugin, PluginType};
use crate::sandbox::SandboxConfig;
use mlua::{Lua, Value};
use std::path::Path;
use std::sync::OnceLock;
use tracing::debug;

/// A plugin that executes Lua scripts to transform wikitext
pub struct LuaPlugin {
    name: String,
    description: String,
    lua: Lua,
    config: SandboxConfig,
    instruction_counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl LuaPlugin {
    /// Load a Lua plugin from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let script = std::fs::read_to_string(path).map_err(|e| {
            PluginError::LoadFailed(format!("Failed to read Lua file {}: {}", path.display(), e))
        })?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .or_else(|| path.file_stem().and_then(|s| s.to_str()))
            .unwrap_or("unknown")
            .to_string();

        Self::from_string(&name, &script, SandboxConfig::default())
    }

    /// Load a Lua plugin from a string with custom configuration
    pub fn from_string(name: &str, script: &str, config: SandboxConfig) -> Result<Self> {
        let lua = Lua::new();

        // Apply sandboxing - remove dangerous modules
        Self::apply_sandbox(&lua)?;

        // Set memory limit
        let _ = lua.set_memory_limit(config.memory_limit);

        // Add MediaWiki helper functions
        Self::add_mw_helpers(&lua)?;

        // Load the script
        lua.load(script)
            .exec()
            .map_err(|e| PluginError::LoadFailed(format!("Failed to load Lua script: {}", e)))?;

        // Extract description if provided
        let description = lua
            .globals()
            .get::<String>("description")
            .ok()
            .unwrap_or_else(|| format!("Lua plugin: {}", name));

        debug!("Loaded Lua plugin: {} - {}", name, description);

        Ok(Self {
            name: name.to_string(),
            description,
            lua,
            config,
            instruction_counter: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Apply sandboxing by removing dangerous Lua standard libraries
    fn apply_sandbox(lua: &Lua) -> Result<()> {
        let globals = lua.globals();

        // Remove dangerous modules and functions
        for module in &[
            "os", "io", "debug", "package", "dofile", "loadfile", "require",
            "load", "loadstring", "collectgarbage", "rawget", "rawset",
            "rawequal", "rawlen", "getmetatable", "setmetatable"
        ] {
            globals.set(*module, Value::Nil)?;
        }

        debug!("Applied Lua sandbox: removed dangerous modules and functions");
        Ok(())
    }

    /// Add MediaWiki-specific helper functions to the Lua environment
    fn add_mw_helpers(lua: &Lua) -> Result<()> {
        static TITLE_REGEX: OnceLock<regex::Regex> = OnceLock::new();
        static REDIRECT_REGEX: OnceLock<regex::Regex> = OnceLock::new();
        static CATEGORY_REGEX: OnceLock<regex::Regex> = OnceLock::new();

        let globals = lua.globals();
        let mw_table = lua.create_table()?;

        // mw.title(text) - Extract the page title from wikitext
        let title_fn = lua.create_function(|_, text: String| {
            let title_regex = TITLE_REGEX.get_or_init(|| {
                regex::Regex::new(r"(?m)^=+\s*(.+?)\s*=+\s*$").unwrap()
            });
            if let Some(caps) = title_regex.captures(&text) {
                Ok(caps.get(1).map(|m| m.as_str().to_string()))
            } else {
                Ok(None)
            }
        })?;
        mw_table.set("title", title_fn)?;

        // mw.is_redirect(text) - Check if page is a redirect
        let redirect_fn = lua.create_function(|_, text: String| {
            let redirect_regex = REDIRECT_REGEX.get_or_init(|| {
                regex::Regex::new(r"(?i)^#REDIRECT\s*\[\[").unwrap()
            });
            Ok(redirect_regex.is_match(&text))
        })?;
        mw_table.set("is_redirect", redirect_fn)?;

        // mw.categories(text) - Extract all categories from wikitext
        let categories_fn = lua.create_function(|lua, text: String| {
            let cat_regex = CATEGORY_REGEX.get_or_init(|| {
                regex::Regex::new(r"\[\[Category:([^\]]+)\]\]").unwrap()
            });
            let categories: Vec<String> = cat_regex
                .captures_iter(&text)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .collect();

            let table = lua.create_table()?;
            for (i, cat) in categories.iter().enumerate() {
                table.set(i + 1, cat.clone())?;
            }
            Ok(table)
        })?;
        mw_table.set("categories", categories_fn)?;

        globals.set("mw", mw_table)?;

        debug!("Added MediaWiki helper functions to Lua environment");
        Ok(())
    }

    /// Execute the transform function with instruction count limit
    fn execute_transform(&self, input: &str, cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<String> {
        // Reset counter before each execution
        self.instruction_counter.store(0, std::sync::atomic::Ordering::Relaxed);

        // Set instruction hook if limit is configured or for cancellation
        let counter = self.instruction_counter.clone();
        let limit = self.config.instruction_limit;
        self.lua.set_hook(
            mlua::HookTriggers {
                every_nth_instruction: Some(1000),
                ..Default::default()
            },
            move |_lua, _debug| {
                // Check cancellation flag first
                if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(mlua::Error::RuntimeError(
                        "Execution cancelled due to timeout".to_string(),
                    ));
                }

                // Check instruction limit if configured
                if let Some(limit) = limit {
                    let count = counter.fetch_add(1000, std::sync::atomic::Ordering::Relaxed);
                    if count > limit {
                        return Err(mlua::Error::RuntimeError(
                            "Instruction limit exceeded".to_string(),
                        ));
                    }
                }
                Ok(mlua::VmState::Continue)
            },
        );

        // Get the transform function
        let globals = self.lua.globals();
        let transform: mlua::Function = globals
            .get("transform")
            .map_err(|e| PluginError::LoadFailed(format!("transform() function not found: {}", e)))?;

        // Call the transform function
        let result: String = transform
            .call(input.to_string())
            .map_err(|e| PluginError::ExecutionFailed(format!("Lua execution error: {}", e)))?;

        // Remove hook
        self.lua.remove_hook();

        Ok(result)
    }
}

impl Plugin for LuaPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn transform(&self, input: &str) -> Result<String> {
        // Execute with cancellation flag
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_flag_thread = cancel_flag.clone();
        let done_flag_thread = done_flag.clone();
        let cancel_flag_exec = cancel_flag.clone();
        let timeout = self.config.timeout;

        // Spawn a timeout handler thread that sets the cancellation flag
        std::thread::spawn(move || {
            let check_interval = std::time::Duration::from_millis(100);
            let start = std::time::Instant::now();
            loop {
                std::thread::sleep(check_interval);
                if done_flag_thread.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if start.elapsed() >= timeout {
                    cancel_flag_thread.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
            }
        });

        // Execute in current thread - the Lua hook will check cancel_flag
        let result = self.execute_transform(input, cancel_flag_exec);
        done_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        result
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Lua
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_lua_transform() {
        let script = r#"
            description = "Test plugin that converts text to uppercase"

            function transform(text)
                return string.upper(text)
            end
        "#;

        let plugin = LuaPlugin::from_string("test", script, SandboxConfig::default()).unwrap();
        assert_eq!(plugin.name(), "test");
        assert!(plugin.description().contains("uppercase"));

        let result = plugin.transform("hello world").unwrap();
        assert_eq!(result, "HELLO WORLD");
    }

    #[test]
    fn test_mw_helpers() {
        let script = r#"
            function transform(text)
                if mw.is_redirect(text) then
                    return "REDIRECT"
                else
                    return "NOT_REDIRECT"
                end
            end
        "#;

        let plugin = LuaPlugin::from_string("redirect_test", script, SandboxConfig::default()).unwrap();

        let redirect_text = "#REDIRECT [[Main Page]]";
        let result = plugin.transform(redirect_text).unwrap();
        assert_eq!(result, "REDIRECT");

        let normal_text = "Some article content";
        let result = plugin.transform(normal_text).unwrap();
        assert_eq!(result, "NOT_REDIRECT");
    }

    #[test]
    fn test_sandboxing_blocks_os() {
        let script = r#"
            function transform(text)
                os.execute("echo hacked")
                return text
            end
        "#;

        let plugin = LuaPlugin::from_string("malicious", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform("test");
        assert!(result.is_err());
    }

    #[test]
    fn test_instruction_limit() {
        let script = r#"
            function transform(text)
                -- Infinite loop
                while true do
                    text = text .. "a"
                end
                return text
            end
        "#;

        let plugin = LuaPlugin::from_string("infinite", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform("test");
        assert!(result.is_err());
    }

    #[test]
    fn test_categories_helper() {
        let script = r#"
            function transform(text)
                local cats = mw.categories(text)
                local result = ""
                for i, cat in ipairs(cats) do
                    result = result .. cat .. ","
                end
                return result
            end
        "#;

        let plugin = LuaPlugin::from_string("cat_test", script, SandboxConfig::default()).unwrap();

        let text = "Some text\n[[Category:Foo]]\n[[Category:Bar]]";
        let result = plugin.transform(text).unwrap();
        assert_eq!(result, "Foo,Bar,");
    }
}
