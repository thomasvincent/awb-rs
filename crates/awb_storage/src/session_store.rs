use crate::error::StorageError;
use async_trait::async_trait;
use awb_domain::session::SessionState;
use std::path::PathBuf;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, session: &SessionState) -> Result<(), StorageError>;
    async fn load(&self, id: &str) -> Result<SessionState, StorageError>;
    async fn list_sessions(&self) -> Result<Vec<String>, StorageError>;
    async fn delete(&self, id: &str) -> Result<(), StorageError>;
}

/// JSON file implementation with crash-safe write (write-to-temp + rename).
pub struct JsonSessionStore {
    dir: PathBuf,
}

impl JsonSessionStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Validate session ID to prevent path traversal attacks
    fn validate_session_id(id: &str) -> Result<(), StorageError> {
        // Only allow alphanumeric, hyphens, underscores, and periods
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(StorageError::InvalidSessionId(format!(
                "Session ID '{}' contains invalid characters. Only alphanumeric, hyphens, underscores, and periods are allowed.",
                id
            )));
        }
        // Prevent empty or hidden files
        if id.is_empty() || id.starts_with('.') {
            return Err(StorageError::InvalidSessionId(format!(
                "Session ID '{}' is invalid (empty or starts with '.')",
                id
            )));
        }
        Ok(())
    }

    fn session_path(&self, id: &str) -> Result<PathBuf, StorageError> {
        Self::validate_session_id(id)?;
        Ok(self.dir.join(format!("{}.json", id)))
    }

    fn temp_path(&self, id: &str) -> Result<PathBuf, StorageError> {
        Self::validate_session_id(id)?;
        Ok(self.dir.join(format!("{}.json.tmp", id)))
    }
}

#[async_trait]
impl SessionStore for JsonSessionStore {
    async fn save(&self, session: &SessionState) -> Result<(), StorageError> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| StorageError::Serialize(e.to_string()))?;
        let temp = self.temp_path(&session.session_id)?;
        let final_path = self.session_path(&session.session_id)?;
        // Crash-safe: write to temp, then atomic rename
        tokio::fs::write(&temp, &json).await?;
        tokio::fs::rename(&temp, &final_path).await?;
        Ok(())
    }

    async fn load(&self, id: &str) -> Result<SessionState, StorageError> {
        let path = self.session_path(id)?;
        if !path.exists() {
            // Try recovering from temp file
            let temp = self.temp_path(id)?;
            if temp.exists() {
                tokio::fs::rename(&temp, &path).await?;
            } else {
                return Err(StorageError::NotFound(id.to_string()));
            }
        }
        let data = tokio::fs::read_to_string(&path).await?;
        let session: SessionState = serde_json::from_str(&data)?;
        if session.schema_version != 1 {
            return Err(StorageError::SchemaMismatch {
                found: session.schema_version,
                expected: 1,
            });
        }
        Ok(session)
    }

    async fn list_sessions(&self) -> Result<Vec<String>, StorageError> {
        let mut sessions = Vec::new();
        if !self.dir.exists() {
            return Ok(sessions);
        }
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(stem.to_string());
                }
            }
        }
        Ok(sessions)
    }

    async fn delete(&self, id: &str) -> Result<(), StorageError> {
        let path = self.session_path(id)?;
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        let temp = self.temp_path(id)?;
        if temp.exists() {
            tokio::fs::remove_file(&temp).await?;
        }
        Ok(())
    }
}
