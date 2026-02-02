//! Basic usage example for AWB plugins
//!
//! Run with: cargo run --example basic_usage

use awb_plugins::{LuaPlugin, Plugin, PluginManager, SandboxConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== AWB Plugins Basic Usage Example ===\n");

    // Create a plugin manager
    let mut manager = PluginManager::new();

    // Example 1: Load a simple inline Lua plugin
    println!("1. Creating inline Lua plugin...");
    let uppercase_script = r#"
        description = "Converts text to uppercase"

        function transform(text)
            return string.upper(text)
        end
    "#;

    let plugin = LuaPlugin::from_string("uppercase", uppercase_script, SandboxConfig::default())?;
    manager.add_plugin(Box::new(plugin));

    println!("   Loaded plugin: uppercase");
    println!("   Description: Converts text to uppercase\n");

    // Example 2: Load another inline plugin
    println!("2. Creating another Lua plugin...");
    let exclaim_script = r#"
        description = "Adds exclamation marks"

        function transform(text)
            return text .. "!!!"
        end
    "#;

    let plugin = LuaPlugin::from_string("exclaim", exclaim_script, SandboxConfig::default())?;
    manager.add_plugin(Box::new(plugin));

    println!("   Loaded plugin: exclaim");
    println!("   Description: Adds exclamation marks\n");

    // Example 3: Apply plugins
    println!("3. Applying plugins to text...");
    let input = "hello world";
    println!("   Input: {}", input);

    let result = manager.apply_all(input)?;
    println!("   Output: {}\n", result);

    // Example 4: Using MediaWiki helpers
    println!("4. Using MediaWiki helper functions...");
    let mw_script = r#"
        description = "Counts categories"

        function transform(text)
            local cats = mw.categories(text)
            local count = #cats

            if count > 0 then
                return text .. "\n<!-- Found " .. count .. " categories -->"
            end

            return text
        end
    "#;

    let mw_plugin = LuaPlugin::from_string("cat_counter", mw_script, SandboxConfig::default())?;

    let wiki_text = "Some article text\n[[Category:Foo]]\n[[Category:Bar]]";
    println!("   Input:\n{}", wiki_text);

    let result = mw_plugin.transform(wiki_text)?;
    println!("\n   Output:\n{}\n", result);

    // Example 5: Plugin management
    println!("5. Plugin management...");
    println!("   Total plugins: {}", manager.plugin_count());
    println!("   Enabled plugins: {}", manager.enabled_count());

    println!("\n   Disabling 'exclaim' plugin...");
    manager.disable_plugin("exclaim");

    let result2 = manager.apply_all("test")?;
    println!("   Result without exclaim: {}", result2);

    println!("\n   Re-enabling 'exclaim' plugin...");
    manager.enable_plugin("exclaim");

    let result3 = manager.apply_all("test")?;
    println!("   Result with exclaim: {}\n", result3);

    // Example 6: List all plugins
    println!("6. Listing all plugins:");
    for name in manager.plugin_names() {
        let status = if manager.is_enabled(&name) {
            "enabled"
        } else {
            "disabled"
        };
        let plugin = manager.get_plugin(&name).unwrap();
        println!("   - {} ({}): {}", name, status, plugin.description());
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
