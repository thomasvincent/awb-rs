use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffOp {
    Equal {
        old_range: Range<usize>,
        new_range: Range<usize>,
        text: String,
    },
    Insert {
        new_range: Range<usize>,
        text: String,
    },
    Delete {
        old_range: Range<usize>,
        text: String,
    },
    Replace {
        old_range: Range<usize>,
        new_range: Range<usize>,
        old_text: String,
        new_text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDiffStat {
    pub rule_id: uuid::Uuid,
    pub matches: usize,
    pub chars_added: i64,
    pub chars_removed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideBySideLine {
    pub left: Option<DiffLine>,
    pub right: Option<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub line_no: usize,
    pub text: String,
    pub change_type: ChangeType,
    pub inline_changes: Vec<Range<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Equal,
    Added,
    Removed,
    Modified,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_op_equal() {
        let op = DiffOp::Equal {
            old_range: 0..10,
            new_range: 0..10,
            text: "unchanged".to_string(),
        };

        match op {
            DiffOp::Equal {
                old_range,
                new_range,
                text,
            } => {
                assert_eq!(old_range, 0..10);
                assert_eq!(new_range, 0..10);
                assert_eq!(text, "unchanged");
            }
            _ => panic!("Expected Equal op"),
        }
    }

    #[test]
    fn test_diff_op_insert() {
        let op = DiffOp::Insert {
            new_range: 5..15,
            text: "inserted".to_string(),
        };

        match op {
            DiffOp::Insert { new_range, text } => {
                assert_eq!(new_range, 5..15);
                assert_eq!(text, "inserted");
            }
            _ => panic!("Expected Insert op"),
        }
    }

    #[test]
    fn test_diff_op_delete() {
        let op = DiffOp::Delete {
            old_range: 10..20,
            text: "deleted".to_string(),
        };

        match op {
            DiffOp::Delete { old_range, text } => {
                assert_eq!(old_range, 10..20);
                assert_eq!(text, "deleted");
            }
            _ => panic!("Expected Delete op"),
        }
    }

    #[test]
    fn test_diff_op_replace() {
        let op = DiffOp::Replace {
            old_range: 0..5,
            new_range: 0..8,
            old_text: "old".to_string(),
            new_text: "new text".to_string(),
        };

        match op {
            DiffOp::Replace {
                old_range,
                new_range,
                old_text,
                new_text,
            } => {
                assert_eq!(old_range, 0..5);
                assert_eq!(new_range, 0..8);
                assert_eq!(old_text, "old");
                assert_eq!(new_text, "new text");
            }
            _ => panic!("Expected Replace op"),
        }
    }

    #[test]
    fn test_rule_diff_stat() {
        let stat = RuleDiffStat {
            rule_id: uuid::Uuid::new_v4(),
            matches: 5,
            chars_added: 100,
            chars_removed: 50,
        };

        assert_eq!(stat.matches, 5);
        assert_eq!(stat.chars_added, 100);
        assert_eq!(stat.chars_removed, 50);
    }

    #[test]
    fn test_change_type_equality() {
        assert_eq!(ChangeType::Equal, ChangeType::Equal);
        assert_eq!(ChangeType::Added, ChangeType::Added);
        assert_ne!(ChangeType::Added, ChangeType::Removed);
    }

    #[test]
    fn test_diff_line() {
        let line = DiffLine {
            line_no: 42,
            text: "test line".to_string(),
            change_type: ChangeType::Modified,
            inline_changes: vec![5..10],
        };

        assert_eq!(line.line_no, 42);
        assert_eq!(line.text, "test line");
        assert_eq!(line.change_type, ChangeType::Modified);
        assert_eq!(line.inline_changes.len(), 1);
    }

    #[test]
    fn test_side_by_side_line_both() {
        let sbs = SideBySideLine {
            left: Some(DiffLine {
                line_no: 1,
                text: "left".to_string(),
                change_type: ChangeType::Equal,
                inline_changes: vec![],
            }),
            right: Some(DiffLine {
                line_no: 1,
                text: "left".to_string(),
                change_type: ChangeType::Equal,
                inline_changes: vec![],
            }),
        };

        assert!(sbs.left.is_some());
        assert!(sbs.right.is_some());
    }

    #[test]
    fn test_side_by_side_line_left_only() {
        let sbs = SideBySideLine {
            left: Some(DiffLine {
                line_no: 1,
                text: "deleted".to_string(),
                change_type: ChangeType::Removed,
                inline_changes: vec![],
            }),
            right: None,
        };

        assert!(sbs.left.is_some());
        assert!(sbs.right.is_none());
    }

    #[test]
    fn test_diff_op_serialization() {
        let op = DiffOp::Insert {
            new_range: 0..5,
            text: "test".to_string(),
        };

        let json = serde_json::to_string(&op).unwrap();
        let deserialized: DiffOp = serde_json::from_str(&json).unwrap();

        match deserialized {
            DiffOp::Insert { new_range, text } => {
                assert_eq!(new_range, 0..5);
                assert_eq!(text, "test");
            }
            _ => panic!("Deserialization changed op type"),
        }
    }
}
