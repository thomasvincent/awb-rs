use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Warning {
    NoChange,
    LargeChange { added: usize, removed: usize, threshold: usize },
    RegexError { rule_id: Uuid, message: String },
    SuspiciousPattern { description: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warning_no_change() {
        let warning = Warning::NoChange;
        match warning {
            Warning::NoChange => (),
            _ => panic!("Expected NoChange warning"),
        }
    }

    #[test]
    fn test_warning_large_change() {
        let warning = Warning::LargeChange {
            added: 1000,
            removed: 500,
            threshold: 500,
        };

        match warning {
            Warning::LargeChange { added, removed, threshold } => {
                assert_eq!(added, 1000);
                assert_eq!(removed, 500);
                assert_eq!(threshold, 500);
            }
            _ => panic!("Expected LargeChange warning"),
        }
    }

    #[test]
    fn test_warning_regex_error() {
        let rule_id = Uuid::new_v4();
        let warning = Warning::RegexError {
            rule_id,
            message: "Invalid pattern".to_string(),
        };

        match warning {
            Warning::RegexError { rule_id: id, message } => {
                assert_eq!(id, rule_id);
                assert_eq!(message, "Invalid pattern");
            }
            _ => panic!("Expected RegexError warning"),
        }
    }

    #[test]
    fn test_warning_suspicious_pattern() {
        let warning = Warning::SuspiciousPattern {
            description: "Possible vandalism".to_string(),
        };

        match warning {
            Warning::SuspiciousPattern { description } => {
                assert_eq!(description, "Possible vandalism");
            }
            _ => panic!("Expected SuspiciousPattern warning"),
        }
    }

    #[test]
    fn test_warning_serialization() {
        let warning = Warning::LargeChange {
            added: 100,
            removed: 50,
            threshold: 75,
        };

        let json = serde_json::to_string(&warning).unwrap();
        let deserialized: Warning = serde_json::from_str(&json).unwrap();

        match deserialized {
            Warning::LargeChange { added, removed, threshold } => {
                assert_eq!(added, 100);
                assert_eq!(removed, 50);
                assert_eq!(threshold, 75);
            }
            _ => panic!("Deserialization changed warning type"),
        }
    }
}
