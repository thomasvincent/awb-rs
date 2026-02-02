use serde::Serialize;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize)]
pub enum TelemetryEvent {
    SessionStarted { profile: String, timestamp: DateTime<Utc> },
    PageProcessed { title: String, outcome: String, duration_ms: u64, timestamp: DateTime<Utc> },
    RuleApplied { rule_id: String, matches: usize, timestamp: DateTime<Utc> },
    ApiCall { endpoint: String, status: u16, duration_ms: u64, timestamp: DateTime<Utc> },
    Warning { message: String, timestamp: DateTime<Utc> },
    Error { message: String, context: String, timestamp: DateTime<Utc> },
    SessionCompleted { total: usize, saved: usize, skipped: usize, errors: usize, elapsed_secs: f64, timestamp: DateTime<Utc> },
}

impl TelemetryEvent {
    pub fn session_started(profile: impl Into<String>) -> Self {
        Self::SessionStarted { profile: profile.into(), timestamp: Utc::now() }
    }
    pub fn session_completed(total: usize, saved: usize, skipped: usize, errors: usize, elapsed_secs: f64) -> Self {
        Self::SessionCompleted { total, saved, skipped, errors, elapsed_secs, timestamp: Utc::now() }
    }
}
