use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization: {0}")]
    Serialize(String),
    #[error("Deserialization: {0}")]
    Deserialize(String),
    #[error("Schema version mismatch: found v{found}, expected v{expected}")]
    SchemaMismatch { found: u32, expected: u32 },
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid session ID: {0}")]
    InvalidSessionId(String),
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self::Deserialize(e.to_string())
    }
}

impl From<toml::de::Error> for StorageError {
    fn from(e: toml::de::Error) -> Self {
        Self::Deserialize(e.to_string())
    }
}

impl From<toml::ser::Error> for StorageError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Serialize(e.to_string())
    }
}
