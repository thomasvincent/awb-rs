use std::time::Duration;

/// Maximum allowed memory limit (256MB)
pub const MAX_MEMORY_LIMIT: usize = 256 * 1024 * 1024;

/// Configuration for plugin sandboxing and resource limits
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum execution time for a plugin
    pub timeout: Duration,

    /// Maximum memory usage in bytes (Lua only)
    /// Default is 16MB, as 1MB is too small for real wiki articles with templates
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
            memory_limit: 16 * 1024 * 1024, // 16MB - real wiki articles with templates need more than 1MB
            instruction_limit: Some(1_000_000),
            wasm_fuel: 10_000_000,
        }
        .validated()
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

    /// Validate and cap the configuration at safe limits
    pub fn validated(mut self) -> Self {
        // Cap memory limit at MAX_MEMORY_LIMIT
        if self.memory_limit > MAX_MEMORY_LIMIT {
            self.memory_limit = MAX_MEMORY_LIMIT;
        }
        self
    }
}
