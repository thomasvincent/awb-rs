use awb_telemetry::events::{TelemetryEvent, EventData};
use awb_telemetry::export::{ExportFormat, export_log};
use awb_telemetry::setup::{TelemetryConfig, init_telemetry};
use tempfile::TempDir;

#[test]
fn test_telemetry_init_and_emit_events() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("telemetry.log");

    let config = TelemetryConfig {
        enabled: true,
        log_to_file: true,
        file_path: Some(log_path.clone()),
        log_level: "info".to_string(),
    };

    // Initialize telemetry
    init_telemetry(config).expect("Failed to initialize telemetry");

    // Emit various events
    tracing::info!("Test info event");
    tracing::warn!("Test warning event");
    tracing::error!("Test error event");

    // Give time for async logging
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Verify log file was created
    assert!(log_path.exists(), "Telemetry log file should exist");
}

#[test]
fn test_telemetry_event_creation() {
    let event = TelemetryEvent::new(
        "test_event",
        EventData::PageProcessed {
            title: "Test Page".to_string(),
            success: true,
            duration_ms: 150,
        },
    );

    assert_eq!(event.event_type, "test_event");
    assert!(matches!(event.data, EventData::PageProcessed { .. }));
}

#[test]
fn test_telemetry_event_serialization() {
    let event = TelemetryEvent::new(
        "page_edit",
        EventData::EditCompleted {
            page_id: 12345,
            revision_id: 98765,
            rules_applied: 5,
            fixes_applied: 3,
        },
    );

    // Serialize to JSON
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("page_edit"));
    assert!(json.contains("12345"));

    // Deserialize back
    let deserialized: TelemetryEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event_type, "page_edit");
}

#[test]
fn test_telemetry_export_json_format() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.log");
    let export_path = temp_dir.path().join("export.json");

    // Create test log content
    let events = vec![
        TelemetryEvent::new(
            "event1",
            EventData::SessionStarted {
                profile_id: "test".to_string(),
                rule_count: 10,
            },
        ),
        TelemetryEvent::new(
            "event2",
            EventData::PageProcessed {
                title: "Page1".to_string(),
                success: true,
                duration_ms: 100,
            },
        ),
    ];

    // Write events to log file as JSON lines
    let mut log_content = String::new();
    for event in &events {
        log_content.push_str(&serde_json::to_string(&event).unwrap());
        log_content.push('\n');
    }
    std::fs::write(&log_path, log_content).unwrap();

    // Export to JSON
    export_log(&log_path, &export_path, ExportFormat::Json).expect("Export should succeed");

    // Verify export file exists
    assert!(export_path.exists());

    // Verify content is valid JSON
    let exported_content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&exported_content).unwrap();
    assert!(parsed.is_array() || parsed.is_object());
}

#[test]
fn test_telemetry_export_csv_format() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.log");
    let export_path = temp_dir.path().join("export.csv");

    // Create test log content
    let events = vec![
        TelemetryEvent::new(
            "session_start",
            EventData::SessionStarted {
                profile_id: "enwiki".to_string(),
                rule_count: 5,
            },
        ),
        TelemetryEvent::new(
            "page_processed",
            EventData::PageProcessed {
                title: "Test Page".to_string(),
                success: true,
                duration_ms: 250,
            },
        ),
    ];

    // Write events to log file
    let mut log_content = String::new();
    for event in &events {
        log_content.push_str(&serde_json::to_string(&event).unwrap());
        log_content.push('\n');
    }
    std::fs::write(&log_path, log_content).unwrap();

    // Export to CSV
    export_log(&log_path, &export_path, ExportFormat::Csv).expect("Export should succeed");

    // Verify export file exists
    assert!(export_path.exists());

    // Verify CSV header exists
    let exported_content = std::fs::read_to_string(&export_path).unwrap();
    assert!(exported_content.contains("timestamp") || exported_content.contains("event_type"));
}

#[test]
fn test_telemetry_export_plain_format() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("test.log");
    let export_path = temp_dir.path().join("export.txt");

    // Create test log content
    let events = vec![
        TelemetryEvent::new(
            "test1",
            EventData::SessionStarted {
                profile_id: "test".to_string(),
                rule_count: 1,
            },
        ),
    ];

    // Write events to log file
    let mut log_content = String::new();
    for event in &events {
        log_content.push_str(&serde_json::to_string(&event).unwrap());
        log_content.push('\n');
    }
    std::fs::write(&log_path, log_content).unwrap();

    // Export to plain text
    export_log(&log_path, &export_path, ExportFormat::Plain).expect("Export should succeed");

    // Verify export file exists
    assert!(export_path.exists());

    // Verify content is human-readable
    let exported_content = std::fs::read_to_string(&export_path).unwrap();
    assert!(!exported_content.is_empty());
}

#[test]
fn test_telemetry_event_types() {
    // Test all event data variants
    let events = vec![
        TelemetryEvent::new(
            "session_started",
            EventData::SessionStarted {
                profile_id: "test".to_string(),
                rule_count: 10,
            },
        ),
        TelemetryEvent::new(
            "page_processed",
            EventData::PageProcessed {
                title: "Page".to_string(),
                success: true,
                duration_ms: 100,
            },
        ),
        TelemetryEvent::new(
            "edit_completed",
            EventData::EditCompleted {
                page_id: 123,
                revision_id: 456,
                rules_applied: 3,
                fixes_applied: 2,
            },
        ),
        TelemetryEvent::new(
            "error_occurred",
            EventData::ErrorOccurred {
                error_type: "NetworkError".to_string(),
                message: "Connection timeout".to_string(),
            },
        ),
        TelemetryEvent::new(
            "session_ended",
            EventData::SessionEnded {
                pages_processed: 50,
                pages_edited: 40,
                duration_secs: 300.5,
            },
        ),
    ];

    // All events should serialize without error
    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.is_empty());
    }
}

#[test]
fn test_telemetry_config_disabled() {
    let config = TelemetryConfig {
        enabled: false,
        log_to_file: false,
        file_path: None,
        log_level: "info".to_string(),
    };

    // Should succeed even when disabled
    let result = init_telemetry(config);
    assert!(result.is_ok());
}

#[test]
fn test_telemetry_log_levels() {
    let temp_dir = TempDir::new().unwrap();

    let levels = vec!["trace", "debug", "info", "warn", "error"];

    for level in levels {
        let log_path = temp_dir.path().join(format!("{}.log", level));
        let config = TelemetryConfig {
            enabled: true,
            log_to_file: true,
            file_path: Some(log_path.clone()),
            log_level: level.to_string(),
        };

        let result = init_telemetry(config);
        assert!(result.is_ok(), "Failed to initialize with log level: {}", level);
    }
}

#[test]
fn test_telemetry_multiple_events_export() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("multi.log");
    let export_path = temp_dir.path().join("multi_export.json");

    // Create multiple events
    let mut log_content = String::new();
    for i in 0..10 {
        let event = TelemetryEvent::new(
            &format!("event_{}", i),
            EventData::PageProcessed {
                title: format!("Page {}", i),
                success: i % 2 == 0,
                duration_ms: i * 10,
            },
        );
        log_content.push_str(&serde_json::to_string(&event).unwrap());
        log_content.push('\n');
    }
    std::fs::write(&log_path, log_content).unwrap();

    // Export all events
    export_log(&log_path, &export_path, ExportFormat::Json).expect("Export should succeed");

    // Verify all events are in export
    let exported = std::fs::read_to_string(&export_path).unwrap();
    for i in 0..10 {
        assert!(exported.contains(&format!("event_{}", i)) || exported.contains(&format!("Page {}", i)));
    }
}

#[test]
fn test_telemetry_event_timestamps() {
    let event = TelemetryEvent::new(
        "test",
        EventData::SessionStarted {
            profile_id: "test".to_string(),
            rule_count: 1,
        },
    );

    // Timestamp should be set automatically
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("timestamp"));
}

#[test]
fn test_telemetry_export_empty_log() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("empty.log");
    let export_path = temp_dir.path().join("empty_export.json");

    // Create empty log file
    std::fs::write(&log_path, "").unwrap();

    // Export should handle empty log gracefully
    let result = export_log(&log_path, &export_path, ExportFormat::Json);

    // Should either succeed with empty output or return an appropriate error
    if result.is_ok() {
        assert!(export_path.exists());
    }
}

#[test]
fn test_telemetry_export_nonexistent_log() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("nonexistent.log");
    let export_path = temp_dir.path().join("export.json");

    // Export should fail for nonexistent file
    let result = export_log(&log_path, &export_path, ExportFormat::Json);
    assert!(result.is_err());
}

#[test]
fn test_telemetry_event_data_variants() {
    // Test PageProcessed
    let event1 = EventData::PageProcessed {
        title: "Test".to_string(),
        success: true,
        duration_ms: 100,
    };
    let json1 = serde_json::to_string(&event1).unwrap();
    assert!(json1.contains("Test"));

    // Test EditCompleted
    let event2 = EventData::EditCompleted {
        page_id: 123,
        revision_id: 456,
        rules_applied: 5,
        fixes_applied: 3,
    };
    let json2 = serde_json::to_string(&event2).unwrap();
    assert!(json2.contains("123"));

    // Test ErrorOccurred
    let event3 = EventData::ErrorOccurred {
        error_type: "TestError".to_string(),
        message: "Test message".to_string(),
    };
    let json3 = serde_json::to_string(&event3).unwrap();
    assert!(json3.contains("TestError"));
}

#[test]
fn test_telemetry_session_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("session.log");

    let config = TelemetryConfig {
        enabled: true,
        log_to_file: true,
        file_path: Some(log_path.clone()),
        log_level: "info".to_string(),
    };

    init_telemetry(config).expect("Failed to initialize");

    // Simulate session lifecycle
    let start_event = TelemetryEvent::new(
        "session_start",
        EventData::SessionStarted {
            profile_id: "test".to_string(),
            rule_count: 10,
        },
    );

    let process_event = TelemetryEvent::new(
        "page_process",
        EventData::PageProcessed {
            title: "Test Page".to_string(),
            success: true,
            duration_ms: 150,
        },
    );

    let end_event = TelemetryEvent::new(
        "session_end",
        EventData::SessionEnded {
            pages_processed: 1,
            pages_edited: 1,
            duration_secs: 1.5,
        },
    );

    // Events should be created without errors
    assert_eq!(start_event.event_type, "session_start");
    assert_eq!(process_event.event_type, "page_process");
    assert_eq!(end_event.event_type, "session_end");
}

#[test]
fn test_telemetry_export_preserves_order() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("ordered.log");
    let export_path = temp_dir.path().join("ordered_export.json");

    // Create events in specific order
    let mut log_content = String::new();
    for i in 1..=5 {
        let event = TelemetryEvent::new(
            &format!("event_{}", i),
            EventData::PageProcessed {
                title: format!("Page {}", i),
                success: true,
                duration_ms: i * 100,
            },
        );
        log_content.push_str(&serde_json::to_string(&event).unwrap());
        log_content.push('\n');
    }
    std::fs::write(&log_path, log_content).unwrap();

    // Export and verify order is preserved
    export_log(&log_path, &export_path, ExportFormat::Plain).expect("Export should succeed");

    let exported = std::fs::read_to_string(&export_path).unwrap();
    let pos1 = exported.find("event_1").unwrap_or(0);
    let pos2 = exported.find("event_2").unwrap_or(0);
    let pos3 = exported.find("event_3").unwrap_or(0);

    assert!(pos1 < pos2);
    assert!(pos2 < pos3);
}
