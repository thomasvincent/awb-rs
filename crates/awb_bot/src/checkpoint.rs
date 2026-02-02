use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckpointError {
    #[error("Failed to read checkpoint file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse checkpoint: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Checkpoint data for resuming bot runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Index of the last processed page
    pub last_processed_index: usize,

    /// List of completed page titles
    pub completed_pages: Vec<String>,

    /// Total pages edited so far
    pub pages_edited: usize,

    /// Total pages skipped so far
    pub pages_skipped: usize,

    /// Total pages with errors so far
    pub pages_errored: usize,

    /// Timestamp of last checkpoint save
    pub last_save_time: chrono::DateTime<chrono::Utc>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new() -> Self {
        Self {
            last_processed_index: 0,
            completed_pages: Vec::new(),
            pages_edited: 0,
            pages_skipped: 0,
            pages_errored: 0,
            last_save_time: chrono::Utc::now(),
        }
    }

    /// Save checkpoint to file
    pub fn save(&self, path: &Path) -> Result<(), CheckpointError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load checkpoint from file
    pub fn load(path: &Path) -> Result<Self, CheckpointError> {
        let json = std::fs::read_to_string(path)?;
        let checkpoint = serde_json::from_str(&json)?;
        Ok(checkpoint)
    }

    /// Update checkpoint with page completion
    pub fn record_page(&mut self, title: String, edited: bool, skipped: bool, errored: bool) {
        self.completed_pages.push(title);
        self.last_processed_index += 1;

        if edited {
            self.pages_edited += 1;
        } else if skipped {
            self.pages_skipped += 1;
        } else if errored {
            self.pages_errored += 1;
        }

        self.last_save_time = chrono::Utc::now();
    }

    /// Check if a page has been completed
    pub fn is_completed(&self, title: &str) -> bool {
        self.completed_pages.contains(&title.to_string())
    }

    /// Get the next page index to process
    pub fn next_index(&self) -> usize {
        self.last_processed_index
    }
}

impl Default for Checkpoint {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_checkpoint_new() {
        let checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.last_processed_index, 0);
        assert_eq!(checkpoint.completed_pages.len(), 0);
        assert_eq!(checkpoint.pages_edited, 0);
    }

    #[test]
    fn test_checkpoint_record_page() {
        let mut checkpoint = Checkpoint::new();

        checkpoint.record_page("Page1".to_string(), true, false, false);
        checkpoint.record_page("Page2".to_string(), false, true, false);
        checkpoint.record_page("Page3".to_string(), false, false, true);

        assert_eq!(checkpoint.last_processed_index, 3);
        assert_eq!(checkpoint.pages_edited, 1);
        assert_eq!(checkpoint.pages_skipped, 1);
        assert_eq!(checkpoint.pages_errored, 1);
        assert_eq!(checkpoint.completed_pages.len(), 3);
    }

    #[test]
    fn test_checkpoint_is_completed() {
        let mut checkpoint = Checkpoint::new();
        checkpoint.record_page("TestPage".to_string(), true, false, false);

        assert!(checkpoint.is_completed("TestPage"));
        assert!(!checkpoint.is_completed("OtherPage"));
    }

    #[test]
    fn test_checkpoint_next_index() {
        let mut checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.next_index(), 0);

        checkpoint.record_page("Page1".to_string(), true, false, false);
        assert_eq!(checkpoint.next_index(), 1);
    }

    #[test]
    fn test_checkpoint_save_load() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let checkpoint_path = temp_dir.path().join("checkpoint.json");

        let mut checkpoint = Checkpoint::new();
        checkpoint.record_page("TestPage".to_string(), true, false, false);
        checkpoint.save(&checkpoint_path)?;

        let loaded = Checkpoint::load(&checkpoint_path)?;
        assert_eq!(loaded.last_processed_index, checkpoint.last_processed_index);
        assert_eq!(loaded.pages_edited, checkpoint.pages_edited);
        assert_eq!(loaded.completed_pages, checkpoint.completed_pages);

        Ok(())
    }

    #[test]
    fn test_checkpoint_load_nonexistent() {
        let result = Checkpoint::load(Path::new("/nonexistent/checkpoint.json"));
        assert!(result.is_err());
    }
}
