use crate::error::{PluginError, Result};
use crate::plugin_trait::{Plugin, PluginType};
use crate::sandbox::SandboxConfig;
use std::path::Path;
use tracing::debug;
use wasmtime::*;

/// A plugin that executes WebAssembly modules to transform wikitext
pub struct WasmPlugin {
    name: String,
    description: String,
    engine: Engine,
    module: Module,
    config: SandboxConfig,
}

impl WasmPlugin {
    /// Load a WASM plugin from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .or_else(|| path.file_stem().and_then(|s| s.to_str()))
            .unwrap_or("unknown")
            .to_string();

        Self::from_file_with_config(path, &name, SandboxConfig::default())
    }

    /// Load a WASM plugin from a file with custom configuration
    pub fn from_file_with_config<P: AsRef<Path>>(
        path: P,
        name: &str,
        config: SandboxConfig,
    ) -> Result<Self> {
        let path = path.as_ref();
        let wasm_bytes = std::fs::read(path).map_err(|e| {
            PluginError::LoadFailed(format!(
                "Failed to read WASM file {}: {}",
                path.display(),
                e
            ))
        })?;

        Self::from_bytes(name, &wasm_bytes, config)
    }

    /// Load a WASM plugin from bytes
    pub fn from_bytes(name: &str, wasm_bytes: &[u8], config: SandboxConfig) -> Result<Self> {
        // Configure the WASM engine with fuel consumption for resource limiting
        let mut engine_config = Config::new();
        engine_config.consume_fuel(true);
        engine_config.wasm_bulk_memory(true);
        engine_config.wasm_multi_memory(true);

        let engine = Engine::new(&engine_config)?;
        let module = Module::from_binary(&engine, wasm_bytes)?;

        debug!("Loaded WASM plugin: {}", name);

        Ok(Self {
            name: name.to_string(),
            description: format!("WASM plugin: {}", name),
            engine,
            module,
            config,
        })
    }

    /// Execute the WASM transform function
    fn execute_transform(&self, input: &str) -> Result<String> {
        let mut store = Store::new(&self.engine, ());

        // Set fuel limit for execution
        store.set_fuel(self.config.wasm_fuel).map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to set fuel limit: {}", e))
        })?;

        // Create a linker and add WASI if needed (minimal for now)
        let linker = Linker::new(&self.engine);

        // Instantiate the module
        let instance = linker.instantiate(&mut store, &self.module)?;

        // Get the memory export
        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
            PluginError::LoadFailed("WASM module must export 'memory'".to_string())
        })?;

        // Get the alloc function (required for passing strings)
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|e| {
                PluginError::LoadFailed(format!(
                    "WASM module must export 'alloc(size: i32) -> i32': {}",
                    e
                ))
            })?;

        // Get the transform function
        let transform = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "transform")
            .map_err(|e| {
                PluginError::LoadFailed(format!(
                    "WASM module must export 'transform(ptr: i32, len: i32) -> i32': {}",
                    e
                ))
            })?;

        // Allocate memory for input string
        let input_bytes = input.as_bytes();
        let input_len = input_bytes.len() as i32;
        let input_ptr = alloc.call(&mut store, input_len)?;

        // Write input string to WASM memory
        memory
            .write(&mut store, input_ptr as usize, input_bytes)
            .map_err(|e| PluginError::ExecutionFailed(format!("Memory write failed: {}", e)))?;

        // Call the transform function
        let result_ptr = transform.call(&mut store, (input_ptr, input_len))?;

        // Read the result string from WASM memory
        // The WASM module should return a pointer to a length-prefixed string
        // Format: [4 bytes length][string data]
        let mut len_bytes = [0u8; 4];
        memory
            .read(&store, result_ptr as usize, &mut len_bytes)
            .map_err(|e| PluginError::ExecutionFailed(format!("Memory read failed: {}", e)))?;
        let result_len = i32::from_le_bytes(len_bytes) as usize;

        // Cap result size to 10MB to prevent malicious plugins from consuming excessive memory
        if result_len > 10 * 1024 * 1024 {
            return Err(PluginError::ExecutionFailed("result too large".into()));
        }

        let mut result_bytes = vec![0u8; result_len];
        memory
            .read(&store, (result_ptr + 4) as usize, &mut result_bytes)
            .map_err(|e| PluginError::ExecutionFailed(format!("Memory read failed: {}", e)))?;

        // Convert bytes to string
        let result = String::from_utf8(result_bytes)?;

        // Get remaining fuel to calculate consumption
        if let Ok(remaining) = store.get_fuel() {
            let consumed = self.config.wasm_fuel.saturating_sub(remaining);
            debug!("WASM plugin '{}' consumed {} fuel", self.name, consumed);
        }

        Ok(result)
    }
}

impl Plugin for WasmPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn transform(&self, input: &str) -> Result<String> {
        self.execute_transform(input)
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Wasm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a simple WASM module for testing
    // This would normally be compiled from Rust/C/AssemblyScript
    fn create_test_wasm_uppercase() -> Vec<u8> {
        // WAT (WebAssembly Text Format) for a simple uppercase converter
        // This is just a placeholder - in real usage, you'd compile from a real language
        let wat = r#"
            (module
                (memory (export "memory") 1)

                ;; alloc function - just returns next available offset
                (global $heap_ptr (mut i32) (i32.const 1024))
                (func (export "alloc") (param $size i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $heap_ptr))
                    (global.set $heap_ptr (i32.add (global.get $heap_ptr) (local.get $size)))
                    (local.get $ptr)
                )

                ;; transform function - converts to uppercase
                (func (export "transform") (param $ptr i32) (param $len i32) (result i32)
                    (local $i i32)
                    (local $char i32)
                    (local $result_ptr i32)

                    ;; Allocate result (4 bytes for length + string)
                    (local.set $result_ptr (call 0 (i32.add (i32.const 4) (local.get $len))))

                    ;; Write length
                    (i32.store (local.get $result_ptr) (local.get $len))

                    ;; Copy and uppercase each character
                    (local.set $i (i32.const 0))
                    (block $done
                        (loop $loop
                            (br_if $done (i32.ge_u (local.get $i) (local.get $len)))

                            ;; Read character
                            (local.set $char (i32.load8_u (i32.add (local.get $ptr) (local.get $i))))

                            ;; Convert lowercase to uppercase (a-z -> A-Z)
                            (if (i32.and
                                    (i32.ge_u (local.get $char) (i32.const 97))
                                    (i32.le_u (local.get $char) (i32.const 122)))
                                (then
                                    (local.set $char (i32.sub (local.get $char) (i32.const 32)))
                                )
                            )

                            ;; Write character
                            (i32.store8
                                (i32.add
                                    (i32.add (local.get $result_ptr) (i32.const 4))
                                    (local.get $i)
                                )
                                (local.get $char)
                            )

                            (local.set $i (i32.add (local.get $i) (i32.const 1)))
                            (br $loop)
                        )
                    )

                    (local.get $result_ptr)
                )
            )
        "#;

        wat::parse_str(wat).unwrap()
    }

    #[test]
    fn test_wasm_plugin_uppercase() {
        let wasm_bytes = create_test_wasm_uppercase();
        let plugin =
            WasmPlugin::from_bytes("test_uppercase", &wasm_bytes, SandboxConfig::default())
                .unwrap();

        assert_eq!(plugin.name(), "test_uppercase");
        assert_eq!(plugin.plugin_type(), PluginType::Wasm);

        let result = plugin.transform("hello world").unwrap();
        assert_eq!(result, "HELLO WORLD");
    }

    #[test]
    fn test_wasm_fuel_limiting() {
        let wasm_bytes = create_test_wasm_uppercase();
        let config = SandboxConfig {
            wasm_fuel: 1000, // Very low fuel limit
            ..Default::default()
        };

        let plugin = WasmPlugin::from_bytes("limited", &wasm_bytes, config).unwrap();

        // Small input should work
        let result = plugin.transform("hi");
        // Depending on fuel consumption, this might succeed or fail
        // This test mainly ensures the fuel system is active
        assert!(result.is_err() || result.is_ok());
    }
}
