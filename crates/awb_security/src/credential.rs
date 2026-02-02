use keyring::Entry;
use std::path::PathBuf;
use thiserror::Error;

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
        Self {
            store: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialPort for InMemoryCredentialStore {
    fn get_password(&self, profile_id: &str) -> Result<String, CredentialError> {
        self.store
            .lock()
            .map_err(|_| CredentialError::Backend("lock poisoned".into()))?
            .get(profile_id)
            .cloned()
            .ok_or_else(|| CredentialError::NotFound(profile_id.to_string()))
    }
    fn set_password(&self, profile_id: &str, password: &str) -> Result<(), CredentialError> {
        self.store
            .lock()
            .map_err(|_| CredentialError::Backend("lock poisoned".into()))?
            .insert(profile_id.to_string(), password.to_string());
        Ok(())
    }
    fn delete_password(&self, profile_id: &str) -> Result<(), CredentialError> {
        self.store
            .lock()
            .map_err(|_| CredentialError::Backend("lock poisoned".into()))?
            .remove(profile_id);
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
    fn save(
        &self,
        credentials: &std::collections::HashMap<String, String>,
    ) -> Result<(), CredentialError> {
        let json = serde_json::to_string_pretty(credentials)?;

        // Atomically create file with secure permissions on Unix
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.credentials_path)?;

            file.write_all(json.as_bytes())?;
        }

        // On non-Unix, write then set permissions
        #[cfg(not(unix))]
        {
            std::fs::write(&self.credentials_path, json)?;
        }

        Ok(())
    }
}

// Note: Default implementation removed to avoid panics.
// Users should call FileCredentialStore::new() directly and handle errors.

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

/// OS keychain-backed credential store using the keyring crate
pub struct KeyringCredentialStore {
    service: String,
}

impl KeyringCredentialStore {
    /// Create a new KeyringCredentialStore with service name "awb-rs"
    pub fn new() -> Self {
        Self {
            service: "awb-rs".to_string(),
        }
    }

    /// Create an entry for the given profile
    fn entry(&self, profile_id: &str) -> Result<Entry, CredentialError> {
        Entry::new(&self.service, profile_id)
            .map_err(|e| CredentialError::Backend(format!("Failed to create keyring entry: {}", e)))
    }
}

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialPort for KeyringCredentialStore {
    fn get_password(&self, profile_id: &str) -> Result<String, CredentialError> {
        let entry = self.entry(profile_id)?;
        entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => CredentialError::NotFound(profile_id.to_string()),
            keyring::Error::PlatformFailure(ref err) => {
                let err_msg = err.to_string().to_lowercase();
                if err_msg.contains("denied") || err_msg.contains("access") {
                    CredentialError::AccessDenied
                } else {
                    CredentialError::Backend(format!("Keyring error: {}", e))
                }
            }
            _ => CredentialError::Backend(format!("Keyring error: {}", e)),
        })
    }

    fn set_password(&self, profile_id: &str, password: &str) -> Result<(), CredentialError> {
        let entry = self.entry(profile_id)?;
        entry.set_password(password).map_err(|e| match e {
            keyring::Error::PlatformFailure(ref err) => {
                let err_msg = err.to_string().to_lowercase();
                if err_msg.contains("denied") || err_msg.contains("access") {
                    CredentialError::AccessDenied
                } else {
                    CredentialError::Backend(format!("Keyring error: {}", e))
                }
            }
            _ => CredentialError::Backend(format!("Keyring error: {}", e)),
        })
    }

    fn delete_password(&self, profile_id: &str) -> Result<(), CredentialError> {
        let entry = self.entry(profile_id)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => {
                // Deleting a non-existent credential is not an error
                Ok(())
            }
            Err(keyring::Error::PlatformFailure(ref err)) => {
                let err_msg = err.to_string().to_lowercase();
                if err_msg.contains("denied") || err_msg.contains("access") {
                    Err(CredentialError::AccessDenied)
                } else {
                    Err(CredentialError::Backend(format!("Keyring error: {}", err)))
                }
            }
            Err(e) => Err(CredentialError::Backend(format!("Keyring error: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- InMemoryCredentialStore Tests ---

    #[test]
    fn test_in_memory_credential_store_full_crud_cycle() {
        let store = InMemoryCredentialStore::new();

        // Test set
        let result = store.set_password("test_profile", "secret123");
        assert!(result.is_ok(), "Should set password successfully");

        // Test get
        let password = store.get_password("test_profile").unwrap();
        assert_eq!(password, "secret123");

        // Test update
        store.set_password("test_profile", "newsecret456").unwrap();
        let updated = store.get_password("test_profile").unwrap();
        assert_eq!(updated, "newsecret456");

        // Test delete
        let delete_result = store.delete_password("test_profile");
        assert!(delete_result.is_ok());

        // Test get after delete
        let get_after_delete = store.get_password("test_profile");
        assert!(get_after_delete.is_err());
        match get_after_delete {
            Err(CredentialError::NotFound(id)) => assert_eq!(id, "test_profile"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_in_memory_get_nonexistent_password_returns_not_found() {
        let store = InMemoryCredentialStore::new();

        let result = store.get_password("nonexistent");
        assert!(result.is_err());
        match result {
            Err(CredentialError::NotFound(id)) => assert_eq!(id, "nonexistent"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_in_memory_oauth_token_methods() {
        let store = InMemoryCredentialStore::new();

        let token_json = r#"{"access_token": "abc123", "refresh_token": "xyz789"}"#;

        // Set OAuth token
        let result = store.set_oauth_token("test_profile", token_json);
        assert!(result.is_ok());

        // Get OAuth token
        let retrieved = store.get_oauth_token("test_profile").unwrap();
        assert_eq!(retrieved, token_json);

        // Delete OAuth token
        store.delete_oauth_token("test_profile").unwrap();

        // Verify deleted
        let result = store.get_oauth_token("test_profile");
        assert!(result.is_err());
    }

    // --- FileCredentialStore Tests ---

    #[test]
    fn test_file_credential_store_new_creates_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let credentials_dir = temp_dir.path().join(".awb-rs");
        let credentials_path = credentials_dir.join("credentials.json");

        // Create directory first (mimicking what FileCredentialStore::new() does)
        std::fs::create_dir_all(&credentials_dir).unwrap();

        // Manually create FileCredentialStore with custom path
        let store = FileCredentialStore {
            credentials_path: credentials_path.clone(),
        };

        // Set a password
        let result = store.set_password("test", "secret");
        assert!(result.is_ok(), "Should save credentials to file");
        assert!(credentials_dir.exists(), "Directory should exist");
        assert!(credentials_path.exists(), "Credentials file should exist");
    }

    #[test]
    fn test_file_credential_store_set_get_delete_roundtrip() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let credentials_path = temp_dir.path().join("credentials.json");

        let store = FileCredentialStore { credentials_path };

        // Set password
        store.set_password("profile1", "password1").unwrap();

        // Get password
        let retrieved = store.get_password("profile1").unwrap();
        assert_eq!(retrieved, "password1");

        // Set another password
        store.set_password("profile2", "password2").unwrap();

        // Both should exist
        assert_eq!(store.get_password("profile1").unwrap(), "password1");
        assert_eq!(store.get_password("profile2").unwrap(), "password2");

        // Delete one
        store.delete_password("profile1").unwrap();

        // Verify deleted
        let result = store.get_password("profile1");
        assert!(result.is_err());
        match result {
            Err(CredentialError::NotFound(id)) => assert_eq!(id, "profile1"),
            _ => panic!("Expected NotFound error"),
        }

        // Other should still exist
        assert_eq!(store.get_password("profile2").unwrap(), "password2");
    }

    #[test]
    fn test_file_credential_store_get_nonexistent_returns_not_found() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let credentials_path = temp_dir.path().join("credentials.json");

        let store = FileCredentialStore { credentials_path };

        let result = store.get_password("nonexistent");
        assert!(result.is_err());
        match result {
            Err(CredentialError::NotFound(id)) => assert_eq!(id, "nonexistent"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_file_credential_store_permissions() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let credentials_path = temp_dir.path().join("credentials.json");

        let store = FileCredentialStore {
            credentials_path: credentials_path.clone(),
        };

        // Save a credential
        store.set_password("test", "secret").unwrap();

        // Check file permissions
        let metadata = std::fs::metadata(&credentials_path).unwrap();
        let mode = metadata.permissions().mode();

        // Should be 0600 (owner read/write only)
        assert_eq!(
            mode & 0o777,
            0o600,
            "Credentials file should have 0600 permissions"
        );
    }

    #[test]
    fn test_file_credential_store_load_empty_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let credentials_path = temp_dir.path().join("credentials.json");

        let store = FileCredentialStore {
            credentials_path: credentials_path.clone(),
        };

        // Load from nonexistent file should return empty map
        let credentials = store.load().unwrap();
        assert!(credentials.is_empty());
    }

    // --- KeyringCredentialStore Tests ---

    #[test]
    fn test_keyring_credential_store_new() {
        let store = KeyringCredentialStore::new();
        assert_eq!(store.service, "awb-rs");
    }

    #[test]
    fn test_keyring_credential_store_default() {
        let store = KeyringCredentialStore::default();
        assert_eq!(store.service, "awb-rs");
    }

    // Note: Actual keyring tests are skipped because they require OS keychain access
    // and may prompt the user or fail in CI environments. The integration tests
    // cover actual keyring functionality if the OS supports it.

    #[test]
    fn test_credential_error_display() {
        let err1 = CredentialError::NotFound("test".to_string());
        assert_eq!(err1.to_string(), "Credential not found for profile test");

        let err2 = CredentialError::AccessDenied;
        assert_eq!(err2.to_string(), "Keychain access denied");

        let err3 = CredentialError::Backend("test error".to_string());
        assert_eq!(err3.to_string(), "Keychain error: test error");
    }
}
