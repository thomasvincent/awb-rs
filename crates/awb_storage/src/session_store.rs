use crate::error::StorageError;
use async_trait::async_trait;
use awb_domain::session::SessionState;
use std::path::PathBuf;

/// Reject writes to symlink targets to prevent symlink swap attacks.
///
/// Note: This is a best-effort TOCTOU check. Between the symlink_metadata call
/// and the subsequent file operation, a race is theoretically possible. On Unix,
/// using O_NOFOLLOW at open time would be stronger, but tokio::fs doesn't expose
/// that. For our use case (bot checkpoint files in a controlled directory), this
/// is sufficient.
fn reject_symlink(path: &std::path::Path) -> Result<(), StorageError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Refusing to write to symlink: {}", path.display()),
        ))),
        _ => Ok(()),
    }
}

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
        reject_symlink(&final_path)?;
        reject_symlink(&temp)?;
        // Crash-safe: write to temp, fsync, then atomic rename
        tokio::fs::write(&temp, &json).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(&temp, perms).await?;
        }
        // fsync temp file to ensure data is durable before rename
        {
            #[cfg(windows)]
            let file = {
                use std::os::windows::fs::OpenOptionsExt;
                use std::fs::OpenOptions;
                // On Windows, we need to explicitly allow sharing to open the file we just wrote
                let std_file = OpenOptions::new()
                    .read(true)
                    .share_mode(0x00000001 | 0x00000002 | 0x00000004) // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
                    .open(&temp)?;
                tokio::fs::File::from_std(std_file)
            };
            #[cfg(not(windows))]
            let file = tokio::fs::File::open(&temp).await?;

            file.sync_all().await?;
        }
        tokio::fs::rename(&temp, &final_path).await?;
        // fsync parent directory to ensure the rename is durable (Unix only)
        #[cfg(unix)]
        {
            if let Some(parent) = final_path.parent() {
                if let Ok(dir) = tokio::fs::File::open(parent).await {
                    let _ = dir.sync_all().await;
                }
            }
        }
        Ok(())
    }

    async fn load(&self, id: &str) -> Result<SessionState, StorageError> {
        let path = self.session_path(id)?;
        reject_symlink(&path)?;
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
        reject_symlink(&path)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn test_symlink_rejected_on_save() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let store = JsonSessionStore::new(dir.path().join("sessions"));

        // Create a symlink target
        let target = dir.path().join("target.json");
        std::fs::write(&target, "{}").unwrap();

        // Create sessions dir and a symlink inside it
        let sessions_dir = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let link_path = sessions_dir.join("evil.json");
        std::os::unix::fs::symlink(&target, &link_path).unwrap();

        // Try to save a session with id "evil" — should be rejected
        let session = SessionState::new("test_profile");
        // Manually set session_id to "evil" to match the symlink filename
        let mut session = session;
        session.session_id = "evil".to_string();
        let result = store.save(&session).await;
        assert!(result.is_err(), "Symlink target should be rejected");
    }

    #[tokio::test]
    async fn test_session_roundtrip() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let store = JsonSessionStore::new(dir.path().join("sessions"));

        let mut session = SessionState::new("test_profile");
        session.session_id = "test123".to_string();
        store.save(&session).await.unwrap();
        let loaded = store.load("test123").await.unwrap();
        assert_eq!(loaded.session_id, "test123");
    }
}
