use thiserror::Error;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("Credential not found for profile {0}")]
    NotFound(String),
    #[error("Keychain access denied")]
    AccessDenied,
    #[error("Keychain error: {0}")]
    Backend(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Trait for OS-specific credential storage.
pub trait CredentialPort: Send + Sync {
    fn get_password(&self, profile_id: &str) -> Result<String, CredentialError>;
    fn set_password(&self, profile_id: &str, password: &str) -> Result<(), CredentialError>;
    fn delete_password(&self, profile_id: &str) -> Result<(), CredentialError>;

    /// Store OAuth tokens (stored as JSON)
    fn get_oauth_token(&self, profile_id: &str) -> Result<String, CredentialError> {
        self.get_password(&format!("{}_oauth_token", profile_id))
    }

    fn set_oauth_token(&self, profile_id: &str, token_json: &str) -> Result<(), CredentialError> {
        self.set_password(&format!("{}_oauth_token", profile_id), token_json)
    }

    fn delete_oauth_token(&self, profile_id: &str) -> Result<(), CredentialError> {
        self.delete_password(&format!("{}_oauth_token", profile_id))
    }
}

/// In-memory credential store for testing.
pub struct InMemoryCredentialStore {
    store: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self { store: std::sync::Mutex::new(std::collections::HashMap::new()) }
    }
}

impl Default for InMemoryCredentialStore {
    fn default() -> Self { Self::new() }
}

impl CredentialPort for InMemoryCredentialStore {
    fn get_password(&self, profile_id: &str) -> Result<String, CredentialError> {
        self.store.lock().unwrap()
            .get(profile_id)
            .cloned()
            .ok_or_else(|| CredentialError::NotFound(profile_id.to_string()))
    }
    fn set_password(&self, profile_id: &str, password: &str) -> Result<(), CredentialError> {
        self.store.lock().unwrap().insert(profile_id.to_string(), password.to_string());
        Ok(())
    }
    fn delete_password(&self, profile_id: &str) -> Result<(), CredentialError> {
        self.store.lock().unwrap().remove(profile_id);
        Ok(())
    }
}

/// File-based credential store that persists credentials to disk
pub struct FileCredentialStore {
    credentials_path: PathBuf,
}

impl FileCredentialStore {
    /// Create a new FileCredentialStore using the default location (~/.awb-rs/credentials.json)
    pub fn new() -> Result<Self, CredentialError> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| CredentialError::Backend("Could not determine home directory".into()))?;

        let credentials_dir = home_dir.join(".awb-rs");
        let credentials_path = credentials_dir.join("credentials.json");

        // Create directory if it doesn't exist
        if !credentials_dir.exists() {
            std::fs::create_dir_all(&credentials_dir)?;

            // Set directory permissions to 0700 on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o700);
                std::fs::set_permissions(&credentials_dir, perms)?;
            }
        }

        Ok(Self { credentials_path })
    }

    /// Load credentials from file
    fn load(&self) -> Result<std::collections::HashMap<String, String>, CredentialError> {
        if !self.credentials_path.exists() {
            return Ok(std::collections::HashMap::new());
        }

        let contents = std::fs::read_to_string(&self.credentials_path)?;
        let map: std::collections::HashMap<String, String> = serde_json::from_str(&contents)?;
        Ok(map)
    }

    /// Save credentials to file with proper permissions
    fn save(&self, credentials: &std::collections::HashMap<String, String>) -> Result<(), CredentialError> {
        let json = serde_json::to_string_pretty(credentials)?;
        std::fs::write(&self.credentials_path, json)?;

        // Set file permissions to 0600 on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&self.credentials_path, perms)?;
        }

        Ok(())
    }
}

impl Default for FileCredentialStore {
    fn default() -> Self {
        Self::new().expect("Failed to create FileCredentialStore")
    }
}

impl CredentialPort for FileCredentialStore {
    fn get_password(&self, profile_id: &str) -> Result<String, CredentialError> {
        let credentials = self.load()?;
        credentials
            .get(profile_id)
            .cloned()
            .ok_or_else(|| CredentialError::NotFound(profile_id.to_string()))
    }

    fn set_password(&self, profile_id: &str, password: &str) -> Result<(), CredentialError> {
        let mut credentials = self.load()?;
        credentials.insert(profile_id.to_string(), password.to_string());
        self.save(&credentials)?;
        Ok(())
    }

    fn delete_password(&self, profile_id: &str) -> Result<(), CredentialError> {
        let mut credentials = self.load()?;
        credentials.remove(profile_id);
        self.save(&credentials)?;
        Ok(())
    }
}
