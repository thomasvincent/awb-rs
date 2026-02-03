use serde::Deserialize;
use std::collections::HashSet;

/// Classification of a fix module's impact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixClassification {
    /// Pure whitespace/formatting, no semantic change
    Cosmetic,
    /// Structural maintenance (reordering, dedup) preserving semantics
    Maintenance,
    /// Style-sensitive changes that may be contentious
    StyleSensitive,
    /// Editorial changes requiring human review
    Editorial,
}

/// Result of applying fixes with configuration.
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// The final text after all applicable fixes
    pub final_text: String,
    /// IDs of fixes that actually changed the text
    pub changed_ids: Vec<String>,
    /// Whether all changes were cosmetic-only
    pub is_cosmetic_only: bool,
}

/// Configuration for controlling which fixes are applied.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixConfig {
    /// Maximum strictness tier to apply (0-3)
    #[serde(default = "default_tier")]
    pub strictness_tier: u8,
    /// Explicit list of fix IDs to enable (if non-empty, only these run)
    #[serde(default)]
    pub enabled_fixes: HashSet<String>,
    /// Explicit list of fix IDs to disable
    #[serde(default)]
    pub disabled_fixes: HashSet<String>,
    /// If false, reject edits that produce only cosmetic changes
    #[serde(default)]
    pub allow_cosmetic_only: bool,
}

fn default_tier() -> u8 {
    1
}

impl Default for FixConfig {
    fn default() -> Self {
        Self {
            strictness_tier: 1,
            enabled_fixes: HashSet::new(),
            disabled_fixes: HashSet::new(),
            allow_cosmetic_only: false,
        }
    }
}

/// Error from fix configuration or application.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FixConfigError {
    #[error("strictness_tier must be 0-3, got {0}")]
    InvalidTier(u8),
    #[error("unknown fix ID in enabled_fixes: {0}")]
    UnknownEnabledId(String),
    #[error("unknown fix ID in disabled_fixes: {0}")]
    UnknownDisabledId(String),
    #[error("TOML parse error: {0}")]
    ParseError(String),
}

impl FixConfig {
    /// Parse from TOML string.
    pub fn from_toml(s: &str) -> Result<Self, FixConfigError> {
        toml::from_str(s).map_err(|e| FixConfigError::ParseError(e.to_string()))
    }

    /// Validate that tier is in range and all fix IDs are known.
    pub fn validate(&self, known_ids: &HashSet<&str>) -> Result<(), FixConfigError> {
        if self.strictness_tier > 3 {
            return Err(FixConfigError::InvalidTier(self.strictness_tier));
        }
        for id in &self.enabled_fixes {
            if !known_ids.contains(id.as_str()) {
                return Err(FixConfigError::UnknownEnabledId(id.clone()));
            }
        }
        for id in &self.disabled_fixes {
            if !known_ids.contains(id.as_str()) {
                return Err(FixConfigError::UnknownDisabledId(id.clone()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = FixConfig::default();
        assert_eq!(cfg.strictness_tier, 1);
        assert!(cfg.enabled_fixes.is_empty());
        assert!(cfg.disabled_fixes.is_empty());
        assert!(!cfg.allow_cosmetic_only);
    }

    #[test]
    fn test_from_toml_minimal() {
        let cfg = FixConfig::from_toml("strictness_tier = 0\n").unwrap();
        assert_eq!(cfg.strictness_tier, 0);
    }

    #[test]
    fn test_from_toml_full() {
        let toml = r#"
strictness_tier = 2
enabled_fixes = ["whitespace_cleanup", "heading_spacing"]
disabled_fixes = ["citation_formatting"]
allow_cosmetic_only = true
"#;
        let cfg = FixConfig::from_toml(toml).unwrap();
        assert_eq!(cfg.strictness_tier, 2);
        assert!(cfg.enabled_fixes.contains("whitespace_cleanup"));
        assert!(cfg.disabled_fixes.contains("citation_formatting"));
        assert!(cfg.allow_cosmetic_only);
    }

    #[test]
    fn test_validate_bad_tier() {
        let cfg = FixConfig {
            strictness_tier: 5,
            ..Default::default()
        };
        let known: HashSet<&str> = HashSet::new();
        assert!(matches!(
            cfg.validate(&known),
            Err(FixConfigError::InvalidTier(5))
        ));
    }

    #[test]
    fn test_validate_unknown_enabled() {
        let mut cfg = FixConfig::default();
        cfg.enabled_fixes.insert("nonexistent".to_string());
        let known: HashSet<&str> = ["whitespace_cleanup"].into_iter().collect();
        assert!(matches!(
            cfg.validate(&known),
            Err(FixConfigError::UnknownEnabledId(_))
        ));
    }

    #[test]
    fn test_validate_unknown_disabled() {
        let mut cfg = FixConfig::default();
        cfg.disabled_fixes.insert("bogus".to_string());
        let known: HashSet<&str> = ["whitespace_cleanup"].into_iter().collect();
        assert!(matches!(
            cfg.validate(&known),
            Err(FixConfigError::UnknownDisabledId(_))
        ));
    }

    #[test]
    fn test_validate_ok() {
        let mut cfg = FixConfig::default();
        cfg.enabled_fixes.insert("whitespace_cleanup".to_string());
        let known: HashSet<&str> = ["whitespace_cleanup"].into_iter().collect();
        assert!(cfg.validate(&known).is_ok());
    }

    #[test]
    fn test_from_toml_unknown_field_rejected() {
        let result = FixConfig::from_toml("bogus_field = true\n");
        assert!(result.is_err());
    }
}
