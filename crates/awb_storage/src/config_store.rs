use crate::error::StorageError;
use awb_domain::profile::Profile;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

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
        std::fs::write(&self.path, data)?;
        Ok(())
    }

    pub fn load_preferences(&self) -> Result<Preferences, StorageError> {
        Ok(self.load_file()?.preferences)
    }

    pub fn save_preferences(&self, prefs: &Preferences) -> Result<(), StorageError> {
        let mut config = self.load_file()?;
        config.preferences = prefs.clone();
        self.save_file(&config)
    }

    pub fn load_profile(&self, id: &str) -> Result<Profile, StorageError> {
        let config = self.load_file()?;
        config.profiles.get(id).cloned()
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<(), StorageError> {
        let mut config = self.load_file()?;
        config.profiles.insert(profile.id.clone(), profile.clone());
        self.save_file(&config)
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, StorageError> {
        let config = self.load_file()?;
        Ok(config.profiles.into_values().collect())
    }
}
