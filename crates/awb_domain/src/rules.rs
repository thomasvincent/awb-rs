use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: Uuid,
    pub enabled: bool,
    pub order: u32,
    pub kind: RuleKind,
    pub comment_fragment: Option<String>,
}

impl Rule {
    pub fn new_plain(
        find: impl Into<String>,
        replace: impl Into<String>,
        case_sensitive: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            order: 0,
            kind: RuleKind::Plain {
                find: find.into(),
                replace: replace.into(),
                case_sensitive,
            },
            comment_fragment: None,
        }
    }

    pub fn new_regex(
        pattern: impl Into<String>,
        replacement: impl Into<String>,
        case_insensitive: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            order: 0,
            kind: RuleKind::Regex {
                pattern: pattern.into(),
                replacement: replacement.into(),
                case_insensitive,
            },
            comment_fragment: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleKind {
    Plain {
        find: String,
        replace: String,
        case_sensitive: bool,
    },
    Regex {
        pattern: String,
        replacement: String,
        case_insensitive: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn enabled_rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter().filter(|r| r.enabled)
    }

    pub fn add(&mut self, mut rule: Rule) {
        rule.order = self.rules.len() as u32;
        self.rules.push(rule);
    }

    pub fn reorder(&mut self, from: usize, to: usize) {
        if from < self.rules.len() && to < self.rules.len() {
            let rule = self.rules.remove(from);
            self.rules.insert(to, rule);
            for (i, r) in self.rules.iter_mut().enumerate() {
                r.order = i as u32;
            }
        }
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_new_plain() {
        let rule = Rule::new_plain("find", "replace", true);
        assert!(rule.enabled);
        assert_eq!(rule.order, 0);
        match rule.kind {
            RuleKind::Plain {
                find,
                replace,
                case_sensitive,
            } => {
                assert_eq!(find, "find");
                assert_eq!(replace, "replace");
                assert!(case_sensitive);
            }
            _ => panic!("Expected Plain rule"),
        }
    }

    #[test]
    fn test_rule_new_regex() {
        let rule = Rule::new_regex(r"\d+", "NUMBER", true);
        assert!(rule.enabled);
        match rule.kind {
            RuleKind::Regex {
                pattern,
                replacement,
                case_insensitive,
            } => {
                assert_eq!(pattern, r"\d+");
                assert_eq!(replacement, "NUMBER");
                assert!(case_insensitive);
            }
            _ => panic!("Expected Regex rule"),
        }
    }

    #[test]
    fn test_ruleset_new() {
        let ruleset = RuleSet::new();
        assert_eq!(ruleset.rules.len(), 0);
    }

    #[test]
    fn test_ruleset_add() {
        let mut ruleset = RuleSet::new();
        let rule1 = Rule::new_plain("a", "b", true);
        let rule2 = Rule::new_plain("c", "d", false);

        ruleset.add(rule1);
        ruleset.add(rule2);

        assert_eq!(ruleset.rules.len(), 2);
        assert_eq!(ruleset.rules[0].order, 0);
        assert_eq!(ruleset.rules[1].order, 1);
    }

    #[test]
    fn test_ruleset_enabled_rules() {
        let mut ruleset = RuleSet::new();
        let mut rule1 = Rule::new_plain("a", "b", true);
        rule1.enabled = true;
        let mut rule2 = Rule::new_plain("c", "d", true);
        rule2.enabled = false;

        ruleset.add(rule1);
        ruleset.add(rule2);

        let enabled: Vec<_> = ruleset.enabled_rules().collect();
        assert_eq!(enabled.len(), 1);
    }

    #[test]
    fn test_ruleset_reorder() {
        let mut ruleset = RuleSet::new();
        let rule1 = Rule::new_plain("first", "1", true);
        let rule2 = Rule::new_plain("second", "2", true);
        let rule3 = Rule::new_plain("third", "3", true);

        ruleset.add(rule1);
        ruleset.add(rule2);
        ruleset.add(rule3);

        // Move index 2 to index 0
        ruleset.reorder(2, 0);

        assert_eq!(ruleset.rules.len(), 3);
        match &ruleset.rules[0].kind {
            RuleKind::Plain { find, .. } => assert_eq!(find, "third"),
            _ => panic!("Expected Plain rule"),
        }
        match &ruleset.rules[1].kind {
            RuleKind::Plain { find, .. } => assert_eq!(find, "first"),
            _ => panic!("Expected Plain rule"),
        }

        // Verify orders are updated
        assert_eq!(ruleset.rules[0].order, 0);
        assert_eq!(ruleset.rules[1].order, 1);
        assert_eq!(ruleset.rules[2].order, 2);
    }

    #[test]
    fn test_ruleset_reorder_out_of_bounds() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new_plain("a", "b", true));

        // Should not panic
        ruleset.reorder(0, 10);
        ruleset.reorder(10, 0);

        assert_eq!(ruleset.rules.len(), 1);
    }

    #[test]
    fn test_rule_serialization() {
        let rule = Rule::new_plain("test", "result", true);
        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: Rule = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.enabled, rule.enabled);
        match (rule.kind, deserialized.kind) {
            (
                RuleKind::Plain {
                    find: f1,
                    replace: r1,
                    case_sensitive: c1,
                },
                RuleKind::Plain {
                    find: f2,
                    replace: r2,
                    case_sensitive: c2,
                },
            ) => {
                assert_eq!(f1, f2);
                assert_eq!(r1, r2);
                assert_eq!(c1, c2);
            }
            _ => panic!("Serialization changed rule kind"),
        }
    }
}
