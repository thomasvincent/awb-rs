use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("Credential not found for profile {0}")]
    NotFound(String),
    #[error("Keychain access denied")]
    AccessDenied,
    #[error("Keychain error: {0}")]
    Backend(String),
}

/// Trait for OS-specific credential storage.
pub trait CredentialPort: Send + Sync {
    fn get_password(&self, profile_id: &str) -> Result<String, CredentialError>;
    fn set_password(&self, profile_id: &str, password: &str) -> Result<(), CredentialError>;
    fn delete_password(&self, profile_id: &str) -> Result<(), CredentialError>;
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
