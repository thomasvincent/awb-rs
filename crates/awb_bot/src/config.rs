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

    /// Skip edits that are cosmetic-only (WP:COSMETIC compliance).
    /// Default: true — bots should not make cosmetic-only edits unattended.
    #[serde(default = "default_skip_cosmetic_only")]
    pub skip_cosmetic_only: bool,

    /// Bot username for {{bots}}/{{nobots}} policy compliance
    pub bot_name: String,

    /// Allowed namespaces (empty = all allowed)
    #[serde(default)]
    pub allowed_namespaces: std::collections::HashSet<awb_domain::types::Namespace>,

    /// Path to save checkpoint file for crash recovery
    pub checkpoint_path: Option<PathBuf>,

    /// Delay between edits (default: 10 seconds)
    #[serde(default = "default_edit_delay")]
    pub edit_delay: Duration,

    /// Save checkpoint every N pages (default: 25). Set to 1 to save after every page.
    #[serde(default = "default_save_every_n")]
    pub save_every_n: u32,
}

fn default_edit_delay() -> Duration {
    Duration::from_secs(10)
}

fn default_save_every_n() -> u32 {
    25
}

fn default_skip_cosmetic_only() -> bool {
    true
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            max_edits: None,
            max_runtime: None,
            skip_no_change: true,
            skip_on_warning: false,
            emergency_stop_file: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".awb-rs")
                .join("stop"),
            log_every_n: 10,
            dry_run: false,
            skip_cosmetic_only: default_skip_cosmetic_only(),
            bot_name: "AWB-RS".to_string(),
            allowed_namespaces: {
                let mut ns = std::collections::HashSet::new();
                ns.insert(awb_domain::types::Namespace::MAIN);
                ns
            },
            checkpoint_path: None,
            edit_delay: default_edit_delay(),
            save_every_n: default_save_every_n(),
        }
    }
}

impl BotConfig {
    /// Create a new bot config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of edits
    #[must_use]
    pub fn with_max_edits(mut self, max: u32) -> Self {
        self.max_edits = Some(max);
        self
    }

    /// Set maximum runtime
    #[must_use]
    pub fn with_max_runtime(mut self, duration: Duration) -> Self {
        self.max_runtime = Some(duration);
        self
    }

    /// Set whether to skip pages with no changes
    #[must_use]
    pub fn with_skip_no_change(mut self, skip: bool) -> Self {
        self.skip_no_change = skip;
        self
    }

    /// Set whether to skip pages with warnings
    #[must_use]
    pub fn with_skip_on_warning(mut self, skip: bool) -> Self {
        self.skip_on_warning = skip;
        self
    }

    /// Set emergency stop file path
    #[must_use]
    pub fn with_emergency_stop_file(mut self, path: PathBuf) -> Self {
        self.emergency_stop_file = path;
        self
    }

    /// Set log interval
    #[must_use]
    pub fn with_log_every_n(mut self, n: u32) -> Self {
        self.log_every_n = n;
        self
    }

    /// Enable dry-run mode
    #[must_use]
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set bot name for {{bots}}/{{nobots}} policy compliance
    #[must_use]
    pub fn with_bot_name(mut self, name: impl Into<String>) -> Self {
        self.bot_name = name.into();
        self
    }

    /// Set allowed namespaces (empty = all allowed)
    #[must_use]
    pub fn with_allowed_namespaces(
        mut self,
        namespaces: std::collections::HashSet<awb_domain::types::Namespace>,
    ) -> Self {
        self.allowed_namespaces = namespaces;
        self
    }

    /// Set checkpoint path for crash recovery
    #[must_use]
    pub fn with_checkpoint_path(mut self, path: PathBuf) -> Self {
        self.checkpoint_path = Some(path);
        self
    }

    /// Set edit delay between successful edits
    #[must_use]
    pub fn with_edit_delay(mut self, delay: Duration) -> Self {
        self.edit_delay = delay;
        self
    }

    /// Set checkpoint save cadence
    #[must_use]
    pub fn with_save_every_n(mut self, n: u32) -> Self {
        self.save_every_n = n.max(1); // At least every page
        self
    }

    /// Check if a namespace is allowed under the current policy.
    /// Empty allowed set means all namespaces are permitted.
    pub fn is_namespace_allowed(&self, ns: awb_domain::types::Namespace) -> bool {
        self.allowed_namespaces.is_empty() || self.allowed_namespaces.contains(&ns)
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
