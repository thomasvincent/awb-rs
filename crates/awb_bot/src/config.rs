use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for bot mode operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Maximum number of edits to perform (None = unlimited)
    pub max_edits: Option<u32>,

    /// Maximum runtime duration (None = unlimited)
    pub max_runtime: Option<Duration>,

    /// Skip pages where rules make no changes
    pub skip_no_change: bool,

    /// Skip pages that have warnings
    pub skip_on_warning: bool,

    /// Path to emergency stop file - bot stops if this file exists
    pub emergency_stop_file: PathBuf,

    /// Log progress every N pages
    pub log_every_n: u32,

    /// Dry-run mode - show diffs without saving
    pub dry_run: bool,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            max_edits: None,
            max_runtime: None,
            skip_no_change: true,
            skip_on_warning: false,
            emergency_stop_file: PathBuf::from("/tmp/awb-rs-stop"),
            log_every_n: 10,
            dry_run: false,
        }
    }
}

impl BotConfig {
    /// Create a new bot config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of edits
    pub fn with_max_edits(mut self, max: u32) -> Self {
        self.max_edits = Some(max);
        self
    }

    /// Set maximum runtime
    pub fn with_max_runtime(mut self, duration: Duration) -> Self {
        self.max_runtime = Some(duration);
        self
    }

    /// Set whether to skip pages with no changes
    pub fn with_skip_no_change(mut self, skip: bool) -> Self {
        self.skip_no_change = skip;
        self
    }

    /// Set whether to skip pages with warnings
    pub fn with_skip_on_warning(mut self, skip: bool) -> Self {
        self.skip_on_warning = skip;
        self
    }

    /// Set emergency stop file path
    pub fn with_emergency_stop_file(mut self, path: PathBuf) -> Self {
        self.emergency_stop_file = path;
        self
    }

    /// Set log interval
    pub fn with_log_every_n(mut self, n: u32) -> Self {
        self.log_every_n = n;
        self
    }

    /// Enable dry-run mode
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_config_default() {
        let config = BotConfig::default();
        assert_eq!(config.max_edits, None);
        assert_eq!(config.max_runtime, None);
        assert!(config.skip_no_change);
        assert!(!config.skip_on_warning);
        assert_eq!(config.log_every_n, 10);
        assert!(!config.dry_run);
    }

    #[test]
    fn test_bot_config_builder() {
        let config = BotConfig::new()
            .with_max_edits(100)
            .with_skip_no_change(false)
            .with_dry_run(true);

        assert_eq!(config.max_edits, Some(100));
        assert!(!config.skip_no_change);
        assert!(config.dry_run);
    }

    #[test]
    fn test_bot_config_serialization() {
        let config = BotConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: BotConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.max_edits, deserialized.max_edits);
        assert_eq!(config.skip_no_change, deserialized.skip_no_change);
    }
}
