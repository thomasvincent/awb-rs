use awb_plugins::error::PluginError;
use awb_plugins::lua_plugin::LuaPlugin;
use awb_plugins::plugin_manager::PluginManager;
use awb_plugins::plugin_trait::Plugin;
use awb_plugins::sandbox::SandboxConfig;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_plugin_manager_load_lua_plugins() {
    let temp_dir = TempDir::new().unwrap();

    // Create test Lua plugins
    let plugin1 = r#"
        function metadata()
            return {
                name = "uppercase",
                version = "1.0.0",
                description = "Converts text to uppercase"
            }
        end

        function transform(text)
            return string.upper(text)
        end
    "#;

    let plugin2 = r#"
        function metadata()
            return {
                name = "append_suffix",
                version = "1.0.0",
                description = "Appends a suffix"
            }
        end

        function transform(text)
            return text .. " [processed]"
        end
    "#;

    std::fs::write(temp_dir.path().join("uppercase.lua"), plugin1).unwrap();
    std::fs::write(temp_dir.path().join("append.lua"), plugin2).unwrap();

    let mut manager = PluginManager::new();
    let count = manager.load_from_directory(temp_dir.path()).unwrap();

    assert_eq!(count, 2);
    assert_eq!(manager.plugin_count(), 2);
}

#[test]
fn test_lua_plugin_with_mediawiki_helpers() {
    let script = r#"
        function transform(text)
            -- Test mw.title helper
            if mw.title then
                text = text .. " (has title helper)"
            end

            -- Test mw.is_redirect helper
            if mw.is_redirect ~= nil then
                text = text .. " (has redirect check)"
            end

            -- Test mw.categories helper
            if mw.categories then
                text = text .. " (has categories)"
            end

            return text
        end
    "#;

    let plugin =
        LuaPlugin::from_string("mediawiki_test", script, SandboxConfig::default()).unwrap();
    let result = plugin.transform("test").unwrap();

    assert!(result.contains("(has title helper)"));
    assert!(result.contains("(has redirect check)"));
    assert!(result.contains("(has categories)"));
}

#[test]
fn test_lua_plugin_sandbox_blocks_dangerous_functions() {
    // Test that os.execute is blocked
    let script_os = r#"
        function transform(text)
            os.execute("echo 'dangerous'")
            return text
        end
    "#;

    let plugin =
        LuaPlugin::from_string("dangerous_os", script_os, SandboxConfig::default()).unwrap();
    let result = plugin.transform("test");
    assert!(result.is_err(), "os.execute should be blocked by sandbox");

    // Test that io.open is blocked
    let script_io = r#"
        function transform(text)
            local f = io.open("/etc/passwd", "r")
            return text
        end
    "#;

    let plugin =
        LuaPlugin::from_string("dangerous_io", script_io, SandboxConfig::default()).unwrap();
    let result = plugin.transform("test");
    assert!(result.is_err(), "io.open should be blocked by sandbox");

    // Test that load is blocked
    let script_load = r#"
        function transform(text)
            load("dangerous code")
            return text
        end
    "#;

    let plugin =
        LuaPlugin::from_string("dangerous_load", script_load, SandboxConfig::default()).unwrap();
    let result = plugin.transform("test");
    assert!(result.is_err(), "load should be blocked by sandbox");

    // Test that dofile is blocked
    let script_dofile = r#"
        function transform(text)
            dofile("/etc/passwd")
            return text
        end
    "#;

    let plugin =
        LuaPlugin::from_string("dangerous_dofile", script_dofile, SandboxConfig::default())
            .unwrap();
    let result = plugin.transform("test");
    assert!(result.is_err(), "dofile should be blocked by sandbox");
}

#[test]
fn test_lua_plugin_timeout() {
    let script = r#"
        function transform(text)
            -- Infinite loop
            while true do
                text = text .. "x"
            end
            return text
        end
    "#;

    let config = SandboxConfig {
        timeout: Duration::from_millis(100),
        memory_limit: 10 * 1024 * 1024, // 10MB
        instruction_limit: Some(1_000_000),
        wasm_fuel: 10_000_000,
    };

    let plugin = LuaPlugin::from_string("infinite_loop", script, config).unwrap();
    let result = plugin.transform("test");

    assert!(result.is_err(), "Infinite loop should timeout");
    // The error might be Timeout or ExecutionFailed with timeout message
    if let Err(e) = result {
        let error_msg = format!("{:?}", e);
        assert!(
            matches!(e, PluginError::Timeout { .. })
                || error_msg.contains("timeout")
                || error_msg.contains("cancelled"),
            "Expected timeout-related error, got: {:?}",
            e
        );
    }
}

#[test]
fn test_lua_plugin_safe_string_operations() {
    let script = r#"
        function transform(text)
            -- Safe string operations
            text = string.upper(text)
            text = string.gsub(text, "TEST", "RESULT")
            text = string.reverse(text)
            text = string.sub(text, 1, 10)
            return text
        end
    "#;

    let plugin = LuaPlugin::from_string("string_ops", script, SandboxConfig::default()).unwrap();
    let result = plugin.transform("test string").unwrap();

    // These operations should work without errors
    assert!(!result.is_empty());
}

#[test]
fn test_lua_plugin_table_operations() {
    let script = r#"
        function transform(text)
            local words = {}
            for word in string.gmatch(text, "%S+") do
                table.insert(words, string.upper(word))
            end
            return table.concat(words, " ")
        end
    "#;

    let plugin = LuaPlugin::from_string("table_ops", script, SandboxConfig::default()).unwrap();
    let result = plugin.transform("hello world test").unwrap();

    assert_eq!(result, "HELLO WORLD TEST");
}

#[test]
fn test_plugin_manager_enable_disable() {
    let script1 = r#"
        function transform(text)
            return text .. "1"
        end
    "#;

    let script2 = r#"
        function transform(text)
            return text .. "2"
        end
    "#;

    let mut manager = PluginManager::new();
    manager.add_plugin(Box::new(
        LuaPlugin::from_string("plugin1", script1, SandboxConfig::default()).unwrap(),
    ));
    manager.add_plugin(Box::new(
        LuaPlugin::from_string("plugin2", script2, SandboxConfig::default()).unwrap(),
    ));

    // All plugins enabled by default
    let result = manager.apply_all("test").unwrap();
    assert_eq!(result, "test12");

    // Disable plugin1
    manager.disable_plugin("plugin1");
    let result = manager.apply_all("test").unwrap();
    assert_eq!(result, "test2");

    // Re-enable plugin1
    manager.enable_plugin("plugin1");
    let result = manager.apply_all("test").unwrap();
    assert_eq!(result, "test12");
}

#[test]
fn test_plugin_manager_apply_specific_plugin() {
    let script = r#"
        function transform(text)
            return string.upper(text)
        end
    "#;

    let mut manager = PluginManager::new();
    manager.add_plugin(Box::new(
        LuaPlugin::from_string("uppercase", script, SandboxConfig::default()).unwrap(),
    ));

    let result = manager.apply_plugin("uppercase", "hello").unwrap();
    assert_eq!(result, "HELLO");

    // Non-existent plugin should error
    let result = manager.apply_plugin("nonexistent", "hello");
    assert!(result.is_err());
}

#[test]
fn test_plugin_manager_plugin_names() {
    let mut manager = PluginManager::new();

    manager.add_plugin(Box::new(
        LuaPlugin::from_string(
            "plugin1",
            "function transform(t) return t end",
            SandboxConfig::default(),
        )
        .unwrap(),
    ));
    manager.add_plugin(Box::new(
        LuaPlugin::from_string(
            "plugin2",
            "function transform(t) return t end",
            SandboxConfig::default(),
        )
        .unwrap(),
    ));
    manager.add_plugin(Box::new(
        LuaPlugin::from_string(
            "plugin3",
            "function transform(t) return t end",
            SandboxConfig::default(),
        )
        .unwrap(),
    ));

    let names = manager.plugin_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"plugin1".to_string()));
    assert!(names.contains(&"plugin2".to_string()));
    assert!(names.contains(&"plugin3".to_string()));
}

#[test]
fn test_plugin_manager_remove_plugin() {
    let mut manager = PluginManager::new();
    manager.add_plugin(Box::new(
        LuaPlugin::from_string(
            "test",
            "function transform(t) return t end",
            SandboxConfig::default(),
        )
        .unwrap(),
    ));

    assert_eq!(manager.plugin_count(), 1);

    let removed = manager.remove_plugin("test");
    assert!(removed.is_some());
    assert_eq!(manager.plugin_count(), 0);

    // Removing non-existent plugin returns None
    let removed = manager.remove_plugin("nonexistent");
    assert!(removed.is_none());
}

#[test]
fn test_plugin_error_handling_continues_with_other_plugins() {
    let good_script = r#"
        function transform(text)
            return text .. " (good)"
        end
    "#;

    let bad_script = r#"
        function transform(text)
            error("intentional error")
        end
    "#;

    let another_good_script = r#"
        function transform(text)
            return text .. " (another good)"
        end
    "#;

    let mut manager = PluginManager::new();
    manager.add_plugin(Box::new(
        LuaPlugin::from_string("good1", good_script, SandboxConfig::default()).unwrap(),
    ));
    manager.add_plugin(Box::new(
        LuaPlugin::from_string("bad", bad_script, SandboxConfig::default()).unwrap(),
    ));
    manager.add_plugin(Box::new(
        LuaPlugin::from_string("good2", another_good_script, SandboxConfig::default()).unwrap(),
    ));

    // Should continue processing even when one plugin fails
    let result = manager.apply_all("test").unwrap();
    assert!(result.contains("(good)"));
    assert!(result.contains("(another good)"));
}

#[test]
fn test_lua_plugin_metadata_extraction() {
    let script = r#"
        function metadata()
            return {
                name = "test_plugin",
                version = "1.2.3",
                description = "A test plugin",
                author = "Test Author"
            }
        end

        function transform(text)
            return text
        end
    "#;

    let plugin = LuaPlugin::from_string("test_plugin", script, SandboxConfig::default()).unwrap();

    assert_eq!(plugin.name(), "test_plugin");
    // Version is not exposed in the current API
    // Description is auto-generated from the name if not provided via global
    assert!(
        plugin.description().contains("test_plugin")
            || plugin.description().contains("A test plugin")
    );
}

#[test]
fn test_lua_plugin_wikitext_patterns() {
    let script = r#"
        function transform(text)
            -- Replace wikilinks
            text = string.gsub(text, "%[%[([^|%]]+)|([^%]]+)%]%]", "[[%1]]")

            -- Remove HTML comments
            text = string.gsub(text, "<!%-%-.-%--%>", "")

            -- Normalize category syntax
            text = string.gsub(text, "%[%[Category:(%a)", "[[Category:%1")

            return text
        end
    "#;

    let plugin = LuaPlugin::from_string("wikitext", script, SandboxConfig::default()).unwrap();

    let input = "[[Link|display text]] <!-- comment --> [[Category:Test]]";
    let result = plugin.transform(input).unwrap();

    assert!(result.contains("[[Link]]"));
    assert!(!result.contains("<!-- comment -->"));
    assert!(result.contains("[[Category:Test]]"));
}

#[test]
fn test_sandbox_config_custom_timeout() {
    let config = SandboxConfig {
        timeout: Duration::from_millis(50),
        memory_limit: 5 * 1024 * 1024, // 5MB
        instruction_limit: Some(100_000),
        wasm_fuel: 1_000_000,
    };

    let script = r#"
        function transform(text)
            local sum = 0
            for i = 1, 10000000 do
                sum = sum + i
            end
            return text
        end
    "#;

    let plugin = LuaPlugin::from_string("slow", script, config).unwrap();
    let result = plugin.transform("test");

    // This should timeout with the short timeout
    assert!(result.is_err());
}

#[test]
fn test_plugin_manager_nonexistent_directory() {
    let mut manager = PluginManager::new();
    let result = manager.load_from_directory("/nonexistent/directory");

    assert!(result.is_err());
}

#[test]
fn test_plugin_manager_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = PluginManager::new();
    let count = manager.load_from_directory(temp_dir.path()).unwrap();

    assert_eq!(count, 0);
}

#[test]
fn test_plugin_manager_mixed_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create a valid Lua plugin
    let lua_plugin = r#"
        function transform(text)
            return text
        end
    "#;
    std::fs::write(temp_dir.path().join("valid.lua"), lua_plugin).unwrap();

    // Create non-plugin files that should be skipped
    std::fs::write(temp_dir.path().join("readme.txt"), "This is a readme").unwrap();
    std::fs::write(temp_dir.path().join("config.json"), "{}").unwrap();

    let mut manager = PluginManager::new();
    let count = manager.load_from_directory(temp_dir.path()).unwrap();

    // Should only load the Lua plugin
    assert_eq!(count, 1);
}
