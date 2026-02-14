use awb_telemetry::events::TelemetryEvent;
use awb_telemetry::export::{ExportFormat, export_log};
use awb_telemetry::setup::TelemetryConfig;

#[test]
fn test_telemetry_event_creation() {
    let event = TelemetryEvent::session_started("enwiki");
    match &event {
        TelemetryEvent::SessionStarted { profile, .. } => {
            assert_eq!(profile, "enwiki");
        }
        _ => panic!("Expected SessionStarted"),
    }
}

#[test]
fn test_telemetry_session_completed() {
    let event = TelemetryEvent::session_completed(100, 80, 15, 5, 300.5);
    match &event {
        TelemetryEvent::SessionCompleted {
            total,
            saved,
            skipped,
            errors,
            elapsed_secs,
            ..
        } => {
            assert_eq!(*total, 100);
            assert_eq!(*saved, 80);
            assert_eq!(*skipped, 15);
            assert_eq!(*errors, 5);
            assert!(*elapsed_secs > 300.0);
        }
        _ => panic!("Expected SessionCompleted"),
    }
}

#[test]
fn test_telemetry_event_serialization() {
    let event = TelemetryEvent::session_started("test_profile");
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("SessionStarted"));
    assert!(json.contains("test_profile"));
}

#[test]
fn test_telemetry_all_event_variants_serialize() {
    let events = vec![
        TelemetryEvent::SessionStarted {
            profile: "test".into(),
            timestamp: chrono::Utc::now(),
        },
        TelemetryEvent::PageProcessed {
            title: "Test Page".into(),
            outcome: "saved".into(),
            duration_ms: 150,
            timestamp: chrono::Utc::now(),
        },
        TelemetryEvent::RuleApplied {
            rule_id: "rule_1".into(),
            matches: 3,
            timestamp: chrono::Utc::now(),
        },
        TelemetryEvent::ApiCall {
            endpoint: "/w/api.php".into(),
            status: 200,
            duration_ms: 50,
            timestamp: chrono::Utc::now(),
        },
        TelemetryEvent::Warning {
            message: "Large change detected".into(),
            timestamp: chrono::Utc::now(),
        },
        TelemetryEvent::Error {
            message: "Connection timeout".into(),
            context: "api_call".into(),
            timestamp: chrono::Utc::now(),
        },
        TelemetryEvent::session_completed(50, 40, 8, 2, 120.0),
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        assert!(!json.is_empty());
    }
}

#[test]
fn test_telemetry_export_json() {
    let events = vec![
        TelemetryEvent::session_started("enwiki"),
        TelemetryEvent::PageProcessed {
            title: "Page 1".into(),
            outcome: "saved".into(),
            duration_ms: 100,
            timestamp: chrono::Utc::now(),
        },
    ];

    let mut buf = Vec::new();
    export_log(&events, ExportFormat::Json, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("SessionStarted"));
    assert!(output.contains("enwiki"));
    assert!(output.contains("Page 1"));
}

#[test]
fn test_telemetry_export_csv() {
    let events = vec![
        TelemetryEvent::session_started("test"),
        TelemetryEvent::session_completed(10, 8, 1, 1, 60.0),
    ];

    let mut buf = Vec::new();
    export_log(&events, ExportFormat::Csv, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("type,timestamp,details"));
}

#[test]
fn test_telemetry_export_plain_text() {
    let events = vec![TelemetryEvent::session_started("test")];

    let mut buf = Vec::new();
    export_log(&events, ExportFormat::PlainText, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("SessionStarted"));
}

#[test]
fn test_telemetry_export_empty_events() {
    let events: Vec<TelemetryEvent> = vec![];
    let mut buf = Vec::new();
    export_log(&events, ExportFormat::Json, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.is_empty());
}

#[test]
fn test_telemetry_export_multiple_events_json() {
    let mut events = Vec::new();
    for i in 0..10 {
        events.push(TelemetryEvent::PageProcessed {
            title: format!("Page {}", i),
            outcome: if i % 2 == 0 { "saved" } else { "skipped" }.into(),
            duration_ms: i * 10,
            timestamp: chrono::Utc::now(),
        });
    }

    let mut buf = Vec::new();
    export_log(&events, ExportFormat::Json, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();

    for i in 0..10 {
        assert!(output.contains(&format!("Page {}", i)));
    }
}

#[test]
fn test_telemetry_config_default() {
    let config = TelemetryConfig::default();
    assert_eq!(config.level, tracing::Level::INFO);
    assert!(config.json_output);
    assert!(config.human_output);
}

#[test]
fn test_telemetry_init() {
    // Note: init_telemetry can only succeed once per process due to global subscriber.
    // We just test that TelemetryConfig can be constructed.
    let _config = TelemetryConfig {
        log_dir: std::path::PathBuf::from("/tmp/test_logs"),
        level: tracing::Level::DEBUG,
        json_output: true,
        human_output: false,
    };
}

#[test]
fn test_telemetry_event_timestamps_are_set() {
    let event = TelemetryEvent::session_started("test");
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("timestamp"));
}

#[test]
fn test_telemetry_export_preserves_order() {
    let mut events = Vec::new();
    for i in 1..=5 {
        events.push(TelemetryEvent::PageProcessed {
            title: format!("Page {}", i),
            outcome: "saved".into(),
            duration_ms: i * 100,
            timestamp: chrono::Utc::now(),
        });
    }

    let mut buf = Vec::new();
    export_log(&events, ExportFormat::PlainText, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();

    let pos1 = output.find("Page 1").unwrap();
    let pos2 = output.find("Page 2").unwrap();
    let pos3 = output.find("Page 3").unwrap();
    assert!(pos1 < pos2);
    assert!(pos2 < pos3);
}
