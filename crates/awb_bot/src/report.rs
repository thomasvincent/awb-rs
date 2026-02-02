use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Action taken on a page
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PageAction {
    /// Page was edited successfully
    Edited,
    /// Page was skipped (no changes or warnings)
    Skipped,
    /// Page processing resulted in an error
    Errored,
}

/// Result for a single page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResult {
    /// Page title
    pub title: String,

    /// Action taken
    pub action: PageAction,

    /// Brief summary of changes (for edited pages)
    pub diff_summary: Option<String>,

    /// Warnings encountered
    pub warnings: Vec<String>,

    /// Error message (for errored pages)
    pub error: Option<String>,

    /// Processing timestamp
    pub timestamp: DateTime<Utc>,
}

/// Complete bot run report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotReport {
    /// Total pages processed
    pub pages_processed: usize,

    /// Pages successfully edited
    pub pages_edited: usize,

    /// Pages skipped
    pub pages_skipped: usize,

    /// Pages with errors
    pub pages_errored: usize,

    /// Start timestamp
    pub start_time: DateTime<Utc>,

    /// End timestamp
    pub end_time: DateTime<Utc>,

    /// Total elapsed seconds
    pub elapsed_secs: f64,

    /// Per-page results
    pub page_results: Vec<PageResult>,

    /// Whether the run was completed or interrupted
    pub completed: bool,

    /// Reason for stopping
    pub stop_reason: Option<String>,
}

impl BotReport {
    /// Create a new bot report
    pub fn new(start_time: DateTime<Utc>) -> Self {
        Self {
            pages_processed: 0,
            pages_edited: 0,
            pages_skipped: 0,
            pages_errored: 0,
            start_time,
            end_time: start_time,
            elapsed_secs: 0.0,
            page_results: Vec::new(),
            completed: false,
            stop_reason: None,
        }
    }

    /// Record a page result
    pub fn record_page(&mut self, result: PageResult) {
        self.pages_processed += 1;
        match result.action {
            PageAction::Edited => self.pages_edited += 1,
            PageAction::Skipped => self.pages_skipped += 1,
            PageAction::Errored => self.pages_errored += 1,
        }
        self.page_results.push(result);
    }

    /// Finalize the report
    pub fn finalize(&mut self, completed: bool, stop_reason: Option<String>) {
        self.end_time = Utc::now();
        self.elapsed_secs = (self.end_time - self.start_time).num_milliseconds() as f64 / 1000.0;
        self.completed = completed;
        self.stop_reason = stop_reason;
    }

    /// Generate human-readable summary
    pub fn to_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Bot Run Summary ===\n");
        summary.push_str(&format!("Started:  {}\n", self.start_time.format("%Y-%m-%d %H:%M:%S UTC")));
        summary.push_str(&format!("Finished: {}\n", self.end_time.format("%Y-%m-%d %H:%M:%S UTC")));
        summary.push_str(&format!("Duration: {:.2} seconds\n", self.elapsed_secs));
        summary.push_str(&format!("Status:   {}\n", if self.completed { "Completed" } else { "Interrupted" }));
        if let Some(reason) = &self.stop_reason {
            summary.push_str(&format!("Reason:   {}\n", reason));
        }
        summary.push_str("\n--- Statistics ---\n");
        summary.push_str(&format!("Processed: {}\n", self.pages_processed));
        summary.push_str(&format!("Edited:    {}\n", self.pages_edited));
        summary.push_str(&format!("Skipped:   {}\n", self.pages_skipped));
        summary.push_str(&format!("Errors:    {}\n", self.pages_errored));

        if self.pages_processed > 0 {
            let edit_rate = (self.pages_edited as f64 / self.pages_processed as f64) * 100.0;
            summary.push_str(&format!("Edit rate: {:.1}%\n", edit_rate));
        }

        if self.elapsed_secs > 0.0 {
            let pages_per_sec = self.pages_processed as f64 / self.elapsed_secs;
            summary.push_str(&format!("Speed:     {:.2} pages/sec\n", pages_per_sec));
        }

        summary
    }

    /// Generate JSON report
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_result(title: &str, action: PageAction) -> PageResult {
        PageResult {
            title: title.to_string(),
            action,
            diff_summary: None,
            warnings: vec![],
            error: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_bot_report_new() {
        let start = Utc::now();
        let report = BotReport::new(start);

        assert_eq!(report.pages_processed, 0);
        assert_eq!(report.pages_edited, 0);
        assert_eq!(report.start_time, start);
        assert!(!report.completed);
    }

    #[test]
    fn test_bot_report_record_page() {
        let mut report = BotReport::new(Utc::now());

        report.record_page(create_test_result("Page1", PageAction::Edited));
        report.record_page(create_test_result("Page2", PageAction::Skipped));
        report.record_page(create_test_result("Page3", PageAction::Errored));

        assert_eq!(report.pages_processed, 3);
        assert_eq!(report.pages_edited, 1);
        assert_eq!(report.pages_skipped, 1);
        assert_eq!(report.pages_errored, 1);
    }

    #[test]
    fn test_bot_report_finalize() {
        let start = Utc::now();
        let mut report = BotReport::new(start);

        report.finalize(true, Some("Completed successfully".to_string()));

        assert!(report.completed);
        assert_eq!(report.stop_reason, Some("Completed successfully".to_string()));
        assert!(report.elapsed_secs >= 0.0);
    }

    #[test]
    fn test_bot_report_summary() {
        let mut report = BotReport::new(Utc::now());
        report.record_page(create_test_result("Page1", PageAction::Edited));
        report.record_page(create_test_result("Page2", PageAction::Skipped));
        report.finalize(true, None);

        let summary = report.to_summary();
        assert!(summary.contains("Processed: 2"));
        assert!(summary.contains("Edited:    1"));
        assert!(summary.contains("Skipped:   1"));
    }

    #[test]
    fn test_bot_report_json() {
        let mut report = BotReport::new(Utc::now());
        report.record_page(create_test_result("Test", PageAction::Edited));

        let json = report.to_json().unwrap();
        // Pretty-printed JSON has spaces, so check for both formats
        assert!(json.contains("\"pages_processed\": 1") || json.contains("\"pages_processed\":1"));
        assert!(json.contains("\"pages_edited\": 1") || json.contains("\"pages_edited\":1"));
    }
}
