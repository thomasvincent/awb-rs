use std::time::Duration;

/// Configuration for plugin sandboxing and resource limits
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum execution time for a plugin
    pub timeout: Duration,

    /// Maximum memory usage in bytes (Lua only)
    pub memory_limit: usize,

    /// Maximum number of instructions (Lua only)
    pub instruction_limit: Option<u64>,

    /// Maximum fuel for WASM execution
    pub wasm_fuel: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            memory_limit: 1024 * 1024, // 1MB
            instruction_limit: Some(1_000_000),
            wasm_fuel: 10_000_000,
        }
    }
}

impl SandboxConfig {
    /// Create a new sandbox configuration with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_secs),
            ..Default::default()
        }
    }

    /// Create a configuration with no instruction limits (use with caution)
    /// WARNING: Disables all sandbox limits. For testing only.
    #[cfg(test)]
    pub fn unlimited() -> Self {
        Self {
            instruction_limit: None,
            wasm_fuel: u64::MAX,
            ..Default::default()
        }
    }
}
