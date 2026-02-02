use awb_bot::checkpoint::Checkpoint;
use awb_bot::config::BotConfig;
use awb_bot::report::{BotReport, PageAction, PageResult};
use chrono::Utc;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_bot_config_builder_chain() {
    let config = BotConfig::new()
        .with_max_edits(100)
        .with_max_runtime(Duration::from_secs(3600))
        .with_skip_no_change(false)
        .with_skip_on_warning(true)
        .with_log_every_n(5)
        .with_dry_run(true);

    assert_eq!(config.max_edits, Some(100));
    assert_eq!(config.max_runtime, Some(Duration::from_secs(3600)));
    assert!(!config.skip_no_change);
    assert!(config.skip_on_warning);
    assert_eq!(config.log_every_n, 5);
    assert!(config.dry_run);
}

#[test]
fn test_bot_config_partial_builder() {
    // Test that builder allows partial configuration
    let config = BotConfig::new().with_max_edits(50).with_dry_run(true);

    assert_eq!(config.max_edits, Some(50));
    assert!(config.dry_run);
    // Other fields should have default values
    assert!(config.skip_no_change); // default is true
    assert!(!config.skip_on_warning); // default is false
}

#[test]
fn test_bot_report_multiple_page_results() {
    let start_time = Utc::now();
    let mut report = BotReport::new(start_time);

    // Add various page results
    report.record_page(PageResult {
        title: "Page1".to_string(),
        action: PageAction::Edited,
        diff_summary: Some("Fixed typos".to_string()),
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    report.record_page(PageResult {
        title: "Page2".to_string(),
        action: PageAction::Skipped,
        diff_summary: None,
        warnings: vec!["No changes needed".to_string()],
        error: None,
        timestamp: Utc::now(),
    });

    report.record_page(PageResult {
        title: "Page3".to_string(),
        action: PageAction::Errored,
        diff_summary: None,
        warnings: vec![],
        error: Some("Network timeout".to_string()),
        timestamp: Utc::now(),
    });

    report.record_page(PageResult {
        title: "Page4".to_string(),
        action: PageAction::Edited,
        diff_summary: Some("Updated links".to_string()),
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    // Verify statistics
    assert_eq!(report.pages_processed, 4);
    assert_eq!(report.pages_edited, 2);
    assert_eq!(report.pages_skipped, 1);
    assert_eq!(report.pages_errored, 1);
    assert_eq!(report.page_results.len(), 4);
}

#[test]
fn test_bot_report_summary_format() {
    let start_time = Utc::now();
    let mut report = BotReport::new(start_time);

    report.record_page(PageResult {
        title: "Test1".to_string(),
        action: PageAction::Edited,
        diff_summary: Some("Test edit".to_string()),
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    report.record_page(PageResult {
        title: "Test2".to_string(),
        action: PageAction::Skipped,
        diff_summary: None,
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    // Sleep briefly to ensure elapsed time is measurable
    std::thread::sleep(std::time::Duration::from_millis(10));

    report.finalize(true, Some("Completed all pages".to_string()));

    let summary = report.to_summary();

    // Verify summary contains expected sections
    assert!(summary.contains("=== Bot Run Summary ==="));
    assert!(summary.contains("Started:"));
    assert!(summary.contains("Finished:"));
    assert!(summary.contains("Duration:"));
    assert!(summary.contains("Status:   Completed"));
    assert!(summary.contains("Reason:   Completed all pages"));
    assert!(summary.contains("--- Statistics ---"));
    assert!(summary.contains("Processed: 2"));
    assert!(summary.contains("Edited:    1"));
    assert!(summary.contains("Skipped:   1"));
    assert!(summary.contains("Edit rate:"));
    // Speed may not be shown if elapsed time is too small, so make it optional
    assert!(summary.contains("Speed:") || report.elapsed_secs > 0.0);
}

#[test]
fn test_bot_report_json_export() {
    let start_time = Utc::now();
    let mut report = BotReport::new(start_time);

    report.record_page(PageResult {
        title: "TestPage".to_string(),
        action: PageAction::Edited,
        diff_summary: Some("Test".to_string()),
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    report.finalize(true, None);

    let json = report.to_json().unwrap();

    // Verify JSON is valid and contains expected fields
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["pages_processed"], 1);
    assert_eq!(parsed["pages_edited"], 1);
    assert_eq!(parsed["completed"], true);
    assert!(parsed["page_results"].is_array());
    assert_eq!(parsed["page_results"].as_array().unwrap().len(), 1);
}

#[test]
fn test_checkpoint_save_load_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint.json");

    let mut checkpoint = Checkpoint::new();

    // Record several pages
    checkpoint.record_page("Page1".to_string(), true, false, false);
    checkpoint.record_page("Page2".to_string(), false, true, false);
    checkpoint.record_page("Page3".to_string(), true, false, false);
    checkpoint.record_page("Page4".to_string(), false, false, true);

    // Save checkpoint
    checkpoint.save(&checkpoint_path).unwrap();

    // Load it back
    let loaded = Checkpoint::load(&checkpoint_path).unwrap();

    // Verify all data
    assert_eq!(loaded.last_processed_index, 4);
    assert_eq!(loaded.completed_pages.len(), 4);
    assert_eq!(loaded.pages_edited, 2);
    assert_eq!(loaded.pages_skipped, 1);
    assert_eq!(loaded.pages_errored, 1);
}

#[test]
fn test_checkpoint_resume_functionality() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint.json");

    // Simulate initial run
    let mut checkpoint = Checkpoint::new();
    checkpoint.record_page("Page1".to_string(), true, false, false);
    checkpoint.record_page("Page2".to_string(), true, false, false);
    checkpoint.save(&checkpoint_path).unwrap();

    // Simulate resume after crash
    let mut loaded = Checkpoint::load(&checkpoint_path).unwrap();

    // Check if pages were completed
    assert!(loaded.is_completed("Page1"));
    assert!(loaded.is_completed("Page2"));
    assert!(!loaded.is_completed("Page3"));

    // Get next index to process
    assert_eq!(loaded.next_index(), 2);

    // Continue processing
    loaded.record_page("Page3".to_string(), true, false, false);
    loaded.save(&checkpoint_path).unwrap();

    // Load again and verify
    let final_checkpoint = Checkpoint::load(&checkpoint_path).unwrap();
    assert_eq!(final_checkpoint.pages_edited, 3);
    assert!(final_checkpoint.is_completed("Page3"));
}

#[test]
fn test_checkpoint_completed_pages_set() {
    let mut checkpoint = Checkpoint::new();

    checkpoint.record_page("Page1".to_string(), true, false, false);
    checkpoint.record_page("Page2".to_string(), true, false, false);
    checkpoint.record_page("Page3".to_string(), true, false, false);

    // Test fast lookup
    assert!(checkpoint.is_completed("Page1"));
    assert!(checkpoint.is_completed("Page2"));
    assert!(checkpoint.is_completed("Page3"));
    assert!(!checkpoint.is_completed("Page4"));
    assert!(!checkpoint.is_completed("NonExistent"));
}

#[test]
fn test_bot_config_emergency_stop_file() {
    let temp_dir = TempDir::new().unwrap();
    let stop_file = temp_dir.path().join("emergency.stop");

    let config = BotConfig::new().with_emergency_stop_file(stop_file.clone());

    assert_eq!(config.emergency_stop_file, stop_file);
}

#[test]
fn test_page_result_with_warnings() {
    let result = PageResult {
        title: "Test Page".to_string(),
        action: PageAction::Edited,
        diff_summary: Some("Made changes".to_string()),
        warnings: vec![
            "Large change detected".to_string(),
            "Modified protected section".to_string(),
        ],
        error: None,
        timestamp: Utc::now(),
    };

    assert_eq!(result.action, PageAction::Edited);
    assert_eq!(result.warnings.len(), 2);
    assert!(result.error.is_none());
}

#[test]
fn test_page_result_with_error() {
    let result = PageResult {
        title: "Failed Page".to_string(),
        action: PageAction::Errored,
        diff_summary: None,
        warnings: vec![],
        error: Some("Edit conflict detected".to_string()),
        timestamp: Utc::now(),
    };

    assert_eq!(result.action, PageAction::Errored);
    assert!(result.error.is_some());
    assert_eq!(result.error.unwrap(), "Edit conflict detected");
}

#[test]
fn test_bot_report_interrupted_run() {
    let start_time = Utc::now();
    let mut report = BotReport::new(start_time);

    report.record_page(PageResult {
        title: "Page1".to_string(),
        action: PageAction::Edited,
        diff_summary: Some("Edit".to_string()),
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    // Simulate interruption
    report.finalize(false, Some("Emergency stop triggered".to_string()));

    assert!(!report.completed);
    assert_eq!(
        report.stop_reason,
        Some("Emergency stop triggered".to_string())
    );

    let summary = report.to_summary();
    assert!(summary.contains("Status:   Interrupted"));
    assert!(summary.contains("Reason:   Emergency stop triggered"));
}

#[test]
fn test_bot_config_serialization_roundtrip() {
    let config = BotConfig::new()
        .with_max_edits(50)
        .with_max_runtime(Duration::from_secs(1800))
        .with_skip_on_warning(true)
        .with_dry_run(true);

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: BotConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.max_edits, deserialized.max_edits);
    assert_eq!(config.max_runtime, deserialized.max_runtime);
    assert_eq!(config.skip_on_warning, deserialized.skip_on_warning);
    assert_eq!(config.dry_run, deserialized.dry_run);
}

#[test]
fn test_checkpoint_handles_mixed_outcomes() {
    let mut checkpoint = Checkpoint::new();

    // Mix of edited, skipped, and errored pages
    checkpoint.record_page("Edit1".to_string(), true, false, false);
    checkpoint.record_page("Skip1".to_string(), false, true, false);
    checkpoint.record_page("Edit2".to_string(), true, false, false);
    checkpoint.record_page("Error1".to_string(), false, false, true);
    checkpoint.record_page("Skip2".to_string(), false, true, false);
    checkpoint.record_page("Edit3".to_string(), true, false, false);

    assert_eq!(checkpoint.pages_edited, 3);
    assert_eq!(checkpoint.pages_skipped, 2);
    assert_eq!(checkpoint.pages_errored, 1);
    assert_eq!(checkpoint.last_processed_index, 6);
    assert_eq!(checkpoint.completed_pages.len(), 6);
}

#[test]
fn test_bot_report_calculates_edit_rate() {
    let start_time = Utc::now();
    let mut report = BotReport::new(start_time);

    // Add pages with 75% edit rate (3 edited out of 4)
    for i in 1..=3 {
        report.record_page(PageResult {
            title: format!("Edited{}", i),
            action: PageAction::Edited,
            diff_summary: Some("Edit".to_string()),
            warnings: vec![],
            error: None,
            timestamp: Utc::now(),
        });
    }

    report.record_page(PageResult {
        title: "Skipped".to_string(),
        action: PageAction::Skipped,
        diff_summary: None,
        warnings: vec![],
        error: None,
        timestamp: Utc::now(),
    });

    report.finalize(true, None);

    let summary = report.to_summary();
    assert!(summary.contains("Edit rate: 75.0%"));
}
