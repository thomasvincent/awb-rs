use crate::error::StorageError;
use awb_domain::profile::Profile;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub default_profile: String,
    pub theme: String,
    pub diff_mode: String,
    pub diff_context_lines: u32,
    pub auto_save_interval_secs: u32,
    pub confirm_large_change_threshold: u32,
    pub log_level: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_profile: "enwiki".to_string(),
            theme: "system".to_string(),
            diff_mode: "side-by-side".to_string(),
            diff_context_lines: 3,
            auto_save_interval_secs: 30,
            confirm_large_change_threshold: 500,
            log_level: "info".to_string(),
        }
    }
}

impl Preferences {
    pub fn validate(&self) -> Result<(), StorageError> {
        // Validate log_level
        match self.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            other => {
                return Err(StorageError::Deserialize(format!(
                    "invalid log_level '{}': expected trace|debug|info|warn|error",
                    other
                )));
            }
        }

        // Validate diff_mode
        match self.diff_mode.as_str() {
            "side-by-side" | "unified" | "inline" => {}
            other => {
                return Err(StorageError::Deserialize(format!(
                    "invalid diff_mode '{}': expected side-by-side|unified|inline",
                    other
                )));
            }
        }

        // Validate theme
        match self.theme.as_str() {
            "system" | "dark" | "light" => {}
            other => {
                return Err(StorageError::Deserialize(format!(
                    "invalid theme '{}': expected system|dark|light",
                    other
                )));
            }
        }

        // Range-check numeric fields
        if self.diff_context_lines < 1 || self.diff_context_lines > 50 {
            return Err(StorageError::Deserialize(format!(
                "diff_context_lines {} out of range 1..=50",
                self.diff_context_lines
            )));
        }

        if self.auto_save_interval_secs < 5 {
            return Err(StorageError::Deserialize(format!(
                "auto_save_interval_secs {} must be >= 5",
                self.auto_save_interval_secs
            )));
        }

        if self.confirm_large_change_threshold < 10 {
            return Err(StorageError::Deserialize(format!(
                "confirm_large_change_threshold {} must be >= 10",
                self.confirm_large_change_threshold
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigFile {
    preferences: Preferences,
    #[serde(default)]
    profiles: std::collections::HashMap<String, Profile>,
}

pub struct TomlConfigStore {
    path: PathBuf,
}

impl TomlConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn load_file(&self) -> Result<ConfigFile, StorageError> {
        if !self.path.exists() {
            return Ok(ConfigFile {
                preferences: Preferences::default(),
                profiles: std::collections::HashMap::new(),
            });
        }
        let data = std::fs::read_to_string(&self.path)?;
        let config: ConfigFile = toml::from_str(&data)?;
        Ok(config)
    }

    fn save_file(&self, config: &ConfigFile) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = toml::to_string_pretty(config)?;
        let tmp_path = self.path.with_extension("tmp");

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp_path)?;
            file.write_all(data.as_bytes())?;
            file.sync_all()?;
            drop(file);
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&tmp_path, &data)?;
        }

        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    pub fn load_preferences(&self) -> Result<Preferences, StorageError> {
        let prefs = self.load_file()?.preferences;
        prefs.validate()?;
        Ok(prefs)
    }

    pub fn save_preferences(&self, prefs: &Preferences) -> Result<(), StorageError> {
        prefs.validate()?;

        // Ensure parent directory exists before creating lock file
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let lock_path = self.path.with_extension("lock");
        let lock_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)?;
        lock_file.lock_exclusive()?;

        let mut config = self.load_file()?;
        config.preferences = prefs.clone();
        self.save_file(&config)?;
        // lock released on drop
        Ok(())
    }

    pub fn load_profile(&self, id: &str) -> Result<Profile, StorageError> {
        let config = self.load_file()?;
        config
            .profiles
            .get(id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<(), StorageError> {
        // Ensure parent directory exists before creating lock file
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let lock_path = self.path.with_extension("lock");
        let lock_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)?;
        lock_file.lock_exclusive()?;

        let mut config = self.load_file()?;
        config.profiles.insert(profile.id.clone(), profile.clone());
        self.save_file(&config)?;
        // lock released on drop
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, StorageError> {
        let config = self.load_file()?;
        Ok(config.profiles.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_preferences_default_values() {
        let prefs = Preferences::default();

        assert_eq!(prefs.default_profile, "enwiki");
        assert_eq!(prefs.theme, "system");
        assert_eq!(prefs.diff_mode, "side-by-side");
        assert_eq!(prefs.diff_context_lines, 3);
        assert_eq!(prefs.auto_save_interval_secs, 30);
        assert_eq!(prefs.confirm_large_change_threshold, 500);
        assert_eq!(prefs.log_level, "info");
    }

    #[test]
    fn test_toml_config_store_save_and_load_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let store = TomlConfigStore::new(&config_path);

        let prefs = Preferences {
            default_profile: "testwiki".to_string(),
            theme: "dark".to_string(),
            diff_mode: "unified".to_string(),
            diff_context_lines: 5,
            auto_save_interval_secs: 60,
            confirm_large_change_threshold: 1000,
            log_level: "debug".to_string(),
        };

        // Save preferences
        let save_result = store.save_preferences(&prefs);
        assert!(save_result.is_ok(), "Should save preferences successfully");

        // Load preferences
        let loaded_prefs = store.load_preferences().unwrap();

        assert_eq!(loaded_prefs.default_profile, "testwiki");
        assert_eq!(loaded_prefs.theme, "dark");
        assert_eq!(loaded_prefs.diff_mode, "unified");
        assert_eq!(loaded_prefs.diff_context_lines, 5);
        assert_eq!(loaded_prefs.auto_save_interval_secs, 60);
        assert_eq!(loaded_prefs.confirm_large_change_threshold, 1000);
        assert_eq!(loaded_prefs.log_level, "debug");
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("nested")
            .join("dir")
            .join("config.toml");
        let store = TomlConfigStore::new(&config_path);

        let prefs = Preferences::default();

        let result = store.save_preferences(&prefs);
        assert!(result.is_ok(), "Should create parent directories");
        assert!(
            config_path.parent().unwrap().exists(),
            "Parent directory should exist"
        );
        assert!(config_path.exists(), "Config file should exist");
    }

    #[test]
    fn test_load_from_nonexistent_file_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.toml");
        let store = TomlConfigStore::new(&config_path);

        let result = store.load_preferences();
        assert!(
            result.is_ok(),
            "Should return default preferences for nonexistent file"
        );

        let prefs = result.unwrap();
        assert_eq!(prefs.default_profile, "enwiki");
    }

    #[test]
    fn test_profile_save_and_load() {
        use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};
        use awb_domain::types::Namespace;
        use std::collections::HashSet;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let store = TomlConfigStore::new(&config_path);

        let mut default_namespaces = HashSet::new();
        default_namespaces.insert(Namespace::MAIN);

        let profile = Profile {
            id: "testwiki".to_string(),
            name: "Test Wiki".to_string(),
            api_url: url::Url::parse("https://test.wikipedia.org/w/api.php").unwrap(),
            auth_method: AuthMethod::BotPassword {
                username: "TestBot".to_string(),
            },
            default_namespaces,
            throttle_policy: ThrottlePolicy {
                min_edit_interval: Duration::from_secs(5),
                maxlag: 5,
                max_retries: 3,
                backoff_base: Duration::from_secs(2),
            },
        };

        // Save profile
        let save_result = store.save_profile(&profile);
        assert!(save_result.is_ok(), "Should save profile successfully");

        // Load profile
        let loaded_profile = store.load_profile("testwiki").unwrap();

        assert_eq!(loaded_profile.id, "testwiki");
        assert_eq!(loaded_profile.name, "Test Wiki");
        assert_eq!(
            loaded_profile.api_url.as_str(),
            "https://test.wikipedia.org/w/api.php"
        );
    }

    #[test]
    fn test_load_nonexistent_profile_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let store = TomlConfigStore::new(&config_path);

        let result = store.load_profile("nonexistent");
        assert!(
            result.is_err(),
            "Should return error for nonexistent profile"
        );

        match result {
            Err(StorageError::NotFound(id)) => assert_eq!(id, "nonexistent"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_list_profiles() {
        use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};
        use awb_domain::types::Namespace;
        use std::collections::HashSet;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let store = TomlConfigStore::new(&config_path);

        let mut default_namespaces = HashSet::new();
        default_namespaces.insert(Namespace::MAIN);

        // Save multiple profiles
        let profile1 = Profile {
            id: "wiki1".to_string(),
            name: "Wiki 1".to_string(),
            api_url: url::Url::parse("https://wiki1.org/w/api.php").unwrap(),
            auth_method: AuthMethod::BotPassword {
                username: "Bot1".to_string(),
            },
            default_namespaces: default_namespaces.clone(),
            throttle_policy: ThrottlePolicy {
                min_edit_interval: Duration::from_secs(5),
                maxlag: 5,
                max_retries: 3,
                backoff_base: Duration::from_secs(2),
            },
        };

        let profile2 = Profile {
            id: "wiki2".to_string(),
            name: "Wiki 2".to_string(),
            api_url: url::Url::parse("https://wiki2.org/w/api.php").unwrap(),
            auth_method: AuthMethod::BotPassword {
                username: "Bot2".to_string(),
            },
            default_namespaces: default_namespaces.clone(),
            throttle_policy: ThrottlePolicy {
                min_edit_interval: Duration::from_secs(5),
                maxlag: 5,
                max_retries: 3,
                backoff_base: Duration::from_secs(2),
            },
        };

        store.save_profile(&profile1).unwrap();
        store.save_profile(&profile2).unwrap();

        // List profiles
        let profiles = store.list_profiles().unwrap();

        assert_eq!(profiles.len(), 2);
        let ids: Vec<String> = profiles.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&"wiki1".to_string()));
        assert!(ids.contains(&"wiki2".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_file_permissions_are_restrictive() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let store = TomlConfigStore::new(&config_path);

        let prefs = Preferences::default();
        store.save_preferences(&prefs).unwrap();

        // Check file permissions
        let metadata = std::fs::metadata(&config_path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Should be 0600 (owner read/write only)
        assert_eq!(mode & 0o777, 0o600, "File permissions should be 0600");
    }

    #[test]
    fn test_validate_default_preferences() {
        let prefs = Preferences::default();
        assert!(prefs.validate().is_ok());
    }

    #[test]
    fn test_validate_rejects_invalid_log_level() {
        let mut prefs = Preferences::default();
        prefs.log_level = "verbose".to_string();
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_invalid_diff_mode() {
        let mut prefs = Preferences::default();
        prefs.diff_mode = "fancy".to_string();
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_zero_context_lines() {
        let mut prefs = Preferences::default();
        prefs.diff_context_lines = 0;
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_excessive_context_lines() {
        let mut prefs = Preferences::default();
        prefs.diff_context_lines = 51;
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_low_auto_save_interval() {
        let mut prefs = Preferences::default();
        prefs.auto_save_interval_secs = 2;
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_low_change_threshold() {
        let mut prefs = Preferences::default();
        prefs.confirm_large_change_threshold = 5;
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_invalid_theme() {
        let mut prefs = Preferences::default();
        prefs.theme = "rainbow".to_string();
        assert!(prefs.validate().is_err());
    }

    #[test]
    fn test_validate_accepts_valid_themes() {
        let mut prefs = Preferences::default();

        prefs.theme = "system".to_string();
        assert!(prefs.validate().is_ok());

        prefs.theme = "dark".to_string();
        assert!(prefs.validate().is_ok());

        prefs.theme = "light".to_string();
        assert!(prefs.validate().is_ok());
    }
}
