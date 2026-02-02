use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use crate::types::*;
use crate::rules::RuleSet;
use crate::diff::DiffOp;
use crate::warnings::Warning;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub schema_version: u32,
    pub session_id: String,
    pub profile_id: String,
    pub page_list: Vec<Title>,
    pub current_index: usize,
    pub rule_set: RuleSet,
    pub skip_conditions: Vec<SkipCondition>,
    pub general_fixes_enabled: Vec<String>,
    pub decisions: Vec<PageDecision>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SessionState {
    pub fn new(profile_id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            schema_version: 1,
            session_id: uuid::Uuid::new_v4().to_string(),
            profile_id: profile_id.into(),
            page_list: Vec::new(),
            current_index: 0,
            rule_set: RuleSet::new(),
            skip_conditions: Vec::new(),
            general_fixes_enabled: Vec::new(),
            decisions: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDecision {
    pub page_id: PageId,
    pub decision: EditDecision,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditDecision {
    Save,
    Skip,
    Pause,
    OpenInBrowser,
    ManualEdit(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPlan {
    pub page: PageContent,
    pub new_wikitext: String,
    pub rules_applied: Vec<uuid::Uuid>,
    pub fixes_applied: Vec<String>,
    pub diff_ops: Vec<DiffOp>,
    pub summary: String,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditResult {
    pub page_id: PageId,
    pub new_revision: Option<RevisionId>,
    pub outcome: EditOutcome,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditOutcome {
    Saved { revision: RevisionId },
    Skipped { reason: String },
    NoChange,
    Conflict { base_rev: RevisionId, current_rev: RevisionId },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkipCondition {
    Namespace { allowed: HashSet<Namespace> },
    RegexMatch { pattern: String, invert: bool },
    PageSize { min_bytes: Option<u64>, max_bytes: Option<u64> },
    Protection { max_level: ProtectionLevel },
    IsRedirect(bool),
    IsDisambig(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipDecision {
    Process,
    Skip(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_session_state_new() {
        let session = SessionState::new("test_profile");
        assert_eq!(session.schema_version, 1);
        assert_eq!(session.profile_id, "test_profile");
        assert_eq!(session.current_index, 0);
        assert_eq!(session.page_list.len(), 0);
        assert_eq!(session.decisions.len(), 0);
        assert!(session.created_at <= session.updated_at);
    }

    #[test]
    fn test_session_state_has_unique_id() {
        let session1 = SessionState::new("profile");
        let session2 = SessionState::new("profile");
        assert_ne!(session1.session_id, session2.session_id);
    }

    #[test]
    fn test_edit_decision_variants() {
        let decisions = vec![
            EditDecision::Save,
            EditDecision::Skip,
            EditDecision::Pause,
            EditDecision::OpenInBrowser,
            EditDecision::ManualEdit("custom text".to_string()),
        ];
        assert_eq!(decisions.len(), 5);
    }

    #[test]
    fn test_edit_outcome_saved() {
        let outcome = EditOutcome::Saved { revision: RevisionId(123) };
        match outcome {
            EditOutcome::Saved { revision } => assert_eq!(revision.0, 123),
            _ => panic!("Expected Saved outcome"),
        }
    }

    #[test]
    fn test_edit_outcome_conflict() {
        let outcome = EditOutcome::Conflict {
            base_rev: RevisionId(100),
            current_rev: RevisionId(101),
        };
        match outcome {
            EditOutcome::Conflict { base_rev, current_rev } => {
                assert_eq!(base_rev.0, 100);
                assert_eq!(current_rev.0, 101);
            }
            _ => panic!("Expected Conflict outcome"),
        }
    }

    #[test]
    fn test_skip_condition_namespace() {
        let mut allowed = HashSet::new();
        allowed.insert(Namespace::MAIN);
        allowed.insert(Namespace::USER);

        let condition = SkipCondition::Namespace { allowed: allowed.clone() };
        match condition {
            SkipCondition::Namespace { allowed: a } => {
                assert!(a.contains(&Namespace::MAIN));
                assert!(!a.contains(&Namespace::TALK));
            }
            _ => panic!("Expected Namespace condition"),
        }
    }

    #[test]
    fn test_skip_condition_page_size() {
        let condition = SkipCondition::PageSize {
            min_bytes: Some(100),
            max_bytes: Some(10000),
        };
        match condition {
            SkipCondition::PageSize { min_bytes, max_bytes } => {
                assert_eq!(min_bytes, Some(100));
                assert_eq!(max_bytes, Some(10000));
            }
            _ => panic!("Expected PageSize condition"),
        }
    }

    #[test]
    fn test_skip_condition_regex() {
        let condition = SkipCondition::RegexMatch {
            pattern: r"\d+".to_string(),
            invert: false,
        };
        match condition {
            SkipCondition::RegexMatch { pattern, invert } => {
                assert_eq!(pattern, r"\d+");
                assert!(!invert);
            }
            _ => panic!("Expected RegexMatch condition"),
        }
    }

    #[test]
    fn test_skip_decision_equality() {
        assert_eq!(SkipDecision::Process, SkipDecision::Process);
        assert_eq!(SkipDecision::Skip("reason"), SkipDecision::Skip("reason"));
        assert_ne!(SkipDecision::Process, SkipDecision::Skip("any"));
    }

    #[test]
    fn test_page_decision_serialization() {
        let decision = PageDecision {
            page_id: PageId(42),
            decision: EditDecision::Save,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&decision).unwrap();
        let deserialized: PageDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.page_id.0, 42);
    }

    #[test]
    fn test_session_state_serialization() {
        let session = SessionState::new("test");
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: SessionState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.session_id, session.session_id);
        assert_eq!(deserialized.profile_id, session.profile_id);
        assert_eq!(deserialized.schema_version, 1);
    }
}
