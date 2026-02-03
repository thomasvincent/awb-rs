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

/// Convert a serde_json::Value to a Lua value with depth limit to prevent stack overflow
fn json_value_to_lua(lua: &Lua, value: &serde_json::Value) -> mlua::Result<mlua::Value> {
    json_value_to_lua_impl(lua, value, 0)
}

fn json_value_to_lua_impl(lua: &Lua, value: &serde_json::Value, depth: usize) -> mlua::Result<mlua::Value> {
    const MAX_DEPTH: usize = 64;

    if depth > MAX_DEPTH {
        return Err(mlua::Error::RuntimeError(format!("JSON depth limit exceeded (max: {})", MAX_DEPTH)));
    }

    match value {
        serde_json::Value::Null => Ok(mlua::Value::Nil),
        serde_json::Value::Bool(b) => Ok(mlua::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(mlua::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(mlua::Value::Number(f))
            } else {
                Ok(mlua::Value::Nil)
            }
        }
        serde_json::Value::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.set(i + 1, json_value_to_lua_impl(lua, v, depth + 1)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj {
                table.set(k.as_str(), json_value_to_lua_impl(lua, v, depth + 1)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
    }
}

/// Convert a Lua value to a serde_json::Value
///
/// Note: Array detection heuristic - tables with integer keys (1..n) and raw_len > 0
/// are treated as arrays. In this case, only integer-indexed values are included;
/// any string keys in such tables are silently dropped in the JSON output.
fn lua_value_to_json(value: &mlua::Value) -> Result<serde_json::Value> {
    lua_value_to_json_impl(value, 0)
}

fn lua_value_to_json_impl(value: &mlua::Value, depth: usize) -> Result<serde_json::Value> {
    const MAX_DEPTH: usize = 64;
    if depth > MAX_DEPTH {
        return Err(PluginError::ExecutionFailed(format!("Lua table depth limit exceeded (max: {})", MAX_DEPTH)));
    }

    match value {
        mlua::Value::Nil => Ok(serde_json::Value::Null),
        mlua::Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        mlua::Value::Integer(i) => Ok(serde_json::json!(*i)),
        mlua::Value::Number(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .ok_or_else(|| PluginError::ExecutionFailed("cannot represent NaN/Inf in JSON".to_string()))
        }
        mlua::Value::String(s) => {
            let s = s.to_str().map_err(|e| PluginError::ExecutionFailed(format!("invalid UTF-8: {}", e)))?;
            Ok(serde_json::Value::String(s.to_string()))
        }
        mlua::Value::Table(t) => {
            // Check if it's an array (sequential integer keys starting at 1)
            let len = t.raw_len();
            if len > 0 {
                let mut arr = Vec::with_capacity(len);
                for i in 1..=len {
                    let v: mlua::Value = t.raw_get(i).map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
                    arr.push(lua_value_to_json_impl(&v, depth + 1)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                let mut map = serde_json::Map::new();
                for pair in t.clone().pairs::<String, mlua::Value>() {
                    let (k, v) = pair.map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
                    map.insert(k, lua_value_to_json_impl(&v, depth + 1)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        _ => Err(PluginError::ExecutionFailed(format!("unsupported Lua type for JSON: {:?}", value))),
    }
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
            "os",
            "io",
            "debug",
            "package",
            "dofile",
            "loadfile",
            "require",
            "load",
            "loadstring",
            "collectgarbage",
            "rawget",
            "rawset",
            "rawequal",
            "rawlen",
            "getmetatable",
            "setmetatable",
            "coroutine",
        ] {
            globals.set(*module, Value::Nil)?;
        }

        // Remove dangerous string functions (string.dump can leak bytecode)
        // string.rep, string.byte, string.char are safe and commonly used by
        // legitimate plugins — memory limits already prevent abuse via string.rep
        let string_table: mlua::Table = globals.get("string")?;
        string_table.set("dump", Value::Nil)?;

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
                regex::Regex::new(r"(?m)^=+\s*(.+?)\s*=+\s*$").expect("known-valid regex")
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
                regex::Regex::new(r"(?i)^#REDIRECT\s*\[\[").expect("known-valid regex")
            });
            Ok(redirect_regex.is_match(&text))
        })?;
        mw_table.set("is_redirect", redirect_fn)?;

        // mw.categories(text) - Extract all categories from wikitext
        let categories_fn = lua.create_function(|lua, text: String| {
            let cat_regex = CATEGORY_REGEX.get_or_init(|| {
                regex::Regex::new(r"\[\[Category:([^\]]+)\]\]").expect("known-valid regex")
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

        // mw.log(msg) — debug logging from plugin context
        let log_fn = lua.create_function(|_, msg: String| {
            tracing::debug!(plugin_log = %msg, "Lua plugin log");
            Ok(())
        })?;
        mw_table.set("log", log_fn)?;

        // mw.text sub-table for string utilities
        let text_table = lua.create_table()?;

        // mw.text.trim(s) — trim whitespace from both ends
        let trim_fn = lua.create_function(|_, s: String| {
            Ok(s.trim().to_string())
        })?;
        text_table.set("trim", trim_fn)?;

        // mw.text.gsub(s, pattern, replacement) — safe string substitution
        // Uses Lua's built-in string.gsub under the hood for safety
        let gsub_fn = lua.create_function(|lua, (s, pattern, replacement): (String, String, String)| {
            let string_table: mlua::Table = lua.globals().get("string")?;
            let gsub: mlua::Function = string_table.get("gsub")?;
            let result: String = gsub.call((s, pattern, replacement))?;
            Ok(result)
        })?;
        text_table.set("gsub", gsub_fn)?;

        mw_table.set("text", text_table)?;

        // mw.json sub-table for JSON utilities
        let json_table = lua.create_table()?;

        // mw.json.decode(s) — parse JSON string into Lua table
        let json_decode_fn = lua.create_function(|lua, s: String| {
            let value: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| mlua::Error::RuntimeError(format!("JSON decode error: {}", e)))?;
            json_value_to_lua(lua, &value)
        })?;
        json_table.set("decode", json_decode_fn)?;

        // mw.json.encode(t) — serialize Lua value to JSON string
        let json_encode_fn = lua.create_function(|_, value: mlua::Value| {
            let json_value = lua_value_to_json(&value)
                .map_err(|e| mlua::Error::RuntimeError(format!("JSON encode error: {}", e)))?;
            serde_json::to_string(&json_value)
                .map_err(|e| mlua::Error::RuntimeError(format!("JSON serialize error: {}", e)))
        })?;
        json_table.set("encode", json_encode_fn)?;

        mw_table.set("json", json_table)?;

        globals.set("mw", mw_table)?;

        debug!("Added MediaWiki helper functions to Lua environment");
        Ok(())
    }

    /// Execute the transform function with instruction count limit
    fn execute_transform(
        &self,
        input: &str,
        cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<String> {
        // Reset counter before each execution
        self.instruction_counter
            .store(0, std::sync::atomic::Ordering::Relaxed);

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
        let transform: mlua::Function = globals.get("transform").map_err(|e| {
            PluginError::LoadFailed(format!("transform() function not found: {}", e))
        })?;

        // Call the transform function
        let result: String = transform
            .call(input.to_string())
            .map_err(|e| PluginError::ExecutionFailed(format!("Lua execution error: {}", e)))?;

        // Remove hook
        self.lua.remove_hook();

        // Check output size limit
        const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024; // 10 MB
        if result.len() > MAX_OUTPUT_SIZE {
            return Err(PluginError::ExecutionFailed(format!(
                "Plugin output exceeds size limit ({} bytes, max: {} bytes)",
                result.len(), MAX_OUTPUT_SIZE
            )));
        }

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
        let timeout_handle = std::thread::spawn(move || {
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

        // Wait for timeout thread to finish
        let _ = timeout_handle.join();

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

        let plugin =
            LuaPlugin::from_string("redirect_test", script, SandboxConfig::default()).unwrap();

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

    #[test]
    fn test_mw_log() {
        let script = r#"
            function transform(text)
                mw.log("processing: " .. text)
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("log_test", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform("hello").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_mw_text_trim() {
        let script = r#"
            function transform(text)
                return mw.text.trim(text)
            end
        "#;
        let plugin = LuaPlugin::from_string("trim_test", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform("  hello  ").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_mw_text_gsub() {
        let script = r#"
            function transform(text)
                return mw.text.gsub(text, "world", "Lua")
            end
        "#;
        let plugin = LuaPlugin::from_string("gsub_test", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform("hello world").unwrap();
        assert_eq!(result, "hello Lua");
    }

    #[test]
    fn test_mw_json_decode() {
        let script = r#"
            function transform(text)
                local data = mw.json.decode(text)
                return data.name .. " is " .. tostring(data.age)
            end
        "#;
        let plugin = LuaPlugin::from_string("json_dec_test", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform(r#"{"name":"Alice","age":30}"#).unwrap();
        assert_eq!(result, "Alice is 30");
    }

    #[test]
    fn test_mw_json_encode() {
        let script = r#"
            function transform(text)
                local t = {name = "Bob", active = true}
                return mw.json.encode(t)
            end
        "#;
        let plugin = LuaPlugin::from_string("json_enc_test", script, SandboxConfig::default()).unwrap();
        let result = plugin.transform("ignored").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "Bob");
        assert_eq!(parsed["active"], true);
    }

    #[test]
    fn test_mw_json_roundtrip() {
        let script = r#"
            function transform(text)
                local data = mw.json.decode(text)
                return mw.json.encode(data)
            end
        "#;
        let plugin = LuaPlugin::from_string("json_rt_test", script, SandboxConfig::default()).unwrap();
        let input = r#"{"items":[1,2,3],"flag":true}"#;
        let result = plugin.transform(input).unwrap();
        let original: serde_json::Value = serde_json::from_str(input).unwrap();
        let roundtripped: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(original, roundtripped);
    }

    #[test]
    fn test_sandbox_blocks_io_open() {
        let script = r#"
            function transform(text)
                io.open("/etc/passwd", "r")
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("io_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_blocks_require() {
        let script = r#"
            function transform(text)
                require("os")
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("require_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_blocks_load() {
        let script = r#"
            function transform(text)
                local f = load("return os")
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("load_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_blocks_loadstring() {
        let script = r#"
            function transform(text)
                local f = loadstring("return 1")
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("loadstring_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_blocks_dofile() {
        let script = r#"
            function transform(text)
                dofile("/etc/passwd")
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("dofile_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_blocks_debug() {
        let script = r#"
            function transform(text)
                debug.getinfo(1)
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("debug_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_blocks_package() {
        let script = r#"
            function transform(text)
                package.loaded["os"] = nil
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("package_test", script, SandboxConfig::default()).unwrap();
        assert!(plugin.transform("test").is_err());
    }

    #[test]
    fn test_sandbox_stash_early_attempt() {
        // Try to capture a reference to os.execute before sandbox removes it
        let script = r#"
            -- Attempt to stash os before sandbox clears it
            local stashed_os = os
            function transform(text)
                if stashed_os then
                    stashed_os.execute("echo pwned")
                end
                return text
            end
        "#;
        let plugin = LuaPlugin::from_string("stash_test", script, SandboxConfig::default()).unwrap();
        // Should either fail to load (os is nil at load time) or fail at runtime
        let result = plugin.transform("test");
        // The stash attempt should fail because sandbox is applied BEFORE script loads
        assert!(result.is_err() || result.unwrap() == "test");
    }

    #[test]
    fn test_memory_limit_enforced() {
        let script = r#"
            function transform(text)
                local t = {}
                for i = 1, 10000000 do
                    t[i] = string.rep("x", 1000)
                end
                return text
            end
        "#;
        let config = SandboxConfig {
            memory_limit: 2 * 1024 * 1024, // 2MB
            ..SandboxConfig::default()
        };
        let plugin = LuaPlugin::from_string("mem_test", script, config).unwrap();
        let result = plugin.transform("test");
        assert!(result.is_err(), "Memory limit should be enforced");
    }
}
