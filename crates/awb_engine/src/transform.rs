use awb_domain::rules::{RuleKind, RuleSet};
use awb_domain::session::EditPlan;
use awb_domain::types::PageContent;
use awb_domain::warnings::Warning;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("Rule {rule_id} has invalid regex: {source}")]
    InvalidRegex {
        rule_id: uuid::Uuid,
        source: regex::Error,
    },
}

enum CompiledRule {
    Plain {
        find: String,
        replace: String,
        case_sensitive: bool,
        case_insensitive_regex: Option<regex::Regex>,
        id: uuid::Uuid,
        comment: Option<String>,
    },
    Regex {
        regex: regex::Regex,
        replacement: String,
        id: uuid::Uuid,
        comment: Option<String>,
    },
}

pub struct TransformEngine {
    compiled_rules: Vec<CompiledRule>,
    fix_registry: crate::general_fixes::FixRegistry,
    enabled_fixes: std::collections::HashSet<String>,
}

impl TransformEngine {
    pub fn new(
        rule_set: &RuleSet,
        fix_registry: crate::general_fixes::FixRegistry,
        enabled_fixes: std::collections::HashSet<String>,
    ) -> Result<Self, TransformError> {
        // Compile each enabled rule
        let compiled = rule_set
            .enabled_rules()
            .map(|rule| match &rule.kind {
                RuleKind::Plain {
                    find,
                    replace,
                    case_sensitive,
                } => {
                    let case_insensitive_regex = if !case_sensitive {
                        Some(
                            regex::RegexBuilder::new(&regex::escape(find))
                                .case_insensitive(true)
                                .build()
                                .expect("known-valid escaped regex"),
                        )
                    } else {
                        None
                    };
                    Ok(CompiledRule::Plain {
                        find: find.clone(),
                        replace: replace.clone(),
                        case_sensitive: *case_sensitive,
                        case_insensitive_regex,
                        id: rule.id,
                        comment: rule.comment_fragment.clone(),
                    })
                }
                RuleKind::Regex {
                    pattern,
                    replacement,
                    case_insensitive,
                } => {
                    let regex = regex::RegexBuilder::new(pattern)
                        .case_insensitive(*case_insensitive)
                        .size_limit(1 << 20)
                        .dfa_size_limit(1 << 20)
                        .build()
                        .map_err(|e| TransformError::InvalidRegex {
                            rule_id: rule.id,
                            source: e,
                        })?;
                    Ok(CompiledRule::Regex {
                        regex,
                        replacement: replacement.clone(),
                        id: rule.id,
                        comment: rule.comment_fragment.clone(),
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            compiled_rules: compiled,
            fix_registry,
            enabled_fixes,
        })
    }

    pub fn apply(&self, page: &PageContent) -> EditPlan {
        let mut text = page.wikitext.clone();
        let mut rules_applied = Vec::new();
        let mut summaries = Vec::new();
        let mut warnings = Vec::new();

        // Apply rules
        for rule in &self.compiled_rules {
            let (new_text, id, comment) = match rule {
                CompiledRule::Plain {
                    find,
                    replace,
                    case_sensitive,
                    case_insensitive_regex,
                    id,
                    comment,
                } => {
                    let new = if *case_sensitive {
                        text.replace(find.as_str(), replace.as_str())
                    } else {
                        // Use pre-compiled case-insensitive regex
                        case_insensitive_regex
                            .as_ref()
                            .expect(
                                "case_insensitive_regex must be Some when case_sensitive is false",
                            )
                            .replace_all(&text, replace.as_str())
                            .into_owned()
                    };
                    (new, *id, comment)
                }
                CompiledRule::Regex {
                    regex,
                    replacement,
                    id,
                    comment,
                } => {
                    let new = regex.replace_all(&text, replacement.as_str()).into_owned();
                    (new, *id, comment)
                }
            };
            if new_text != text {
                rules_applied.push(id);
                if let Some(c) = comment {
                    summaries.push(c.clone());
                }
                text = new_text;
            }
        }

        // Apply general fixes
        let ctx = crate::general_fixes::FixContext {
            title: page.title.clone(),
            namespace: page.title.namespace,
            is_redirect: page.is_redirect,
        };

        let mut fixes_applied = Vec::new();
        let mut current_text = text.clone();
        for (id, new_text) in
            self.fix_registry
                .apply_all_returning_ids(&text, &ctx, &self.enabled_fixes)
        {
            if new_text != current_text {
                fixes_applied.push(id);
                current_text = new_text;
            }
        }
        text = current_text;

        // Check for warnings
        if text == page.wikitext {
            warnings.push(Warning::NoChange);
        } else {
            let added = text.len().saturating_sub(page.wikitext.len());
            let removed = page.wikitext.len().saturating_sub(text.len());
            if added + removed > 500 {
                warnings.push(Warning::LargeChange {
                    added,
                    removed,
                    threshold: 500,
                });
            }
        }

        // Compute diff
        let diff_ops = crate::diff_engine::compute_diff(&page.wikitext, &text);

        // Build summary
        let summary = if summaries.is_empty() {
            "AWB-RS ([[WP:AWB]]) automated edit".to_string()
        } else {
            format!("AWB-RS ([[WP:AWB]]): {}", summaries.join(", "))
        };

        EditPlan {
            page: page.clone(),
            new_wikitext: text,
            rules_applied,
            fixes_applied,
            diff_ops,
            summary,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use awb_domain::rules::Rule;
    use awb_domain::types::*;
    use std::collections::HashSet;

    fn create_test_page(wikitext: &str) -> PageContent {
        PageContent {
            page_id: PageId(1),
            title: Title::new(Namespace::MAIN, "Test"),
            revision: RevisionId(100),
            timestamp: chrono::Utc::now(),
            wikitext: wikitext.to_string(),
            size_bytes: wikitext.len() as u64,
            is_redirect: false,
            protection: ProtectionInfo::default(),
            properties: PageProperties::default(),
        }
    }

    #[test]
    fn test_transform_engine_plain_rule() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new_plain("hello", "goodbye", true));

        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("hello world");
        let plan = engine.apply(&page);

        assert_eq!(plan.new_wikitext, "goodbye world");
        assert_eq!(plan.rules_applied.len(), 1);
    }

    #[test]
    fn test_transform_engine_case_insensitive() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new_plain("HELLO", "goodbye", false));

        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("hello HELLO HeLLo");
        let plan = engine.apply(&page);

        assert!(plan.new_wikitext.contains("goodbye"));
        assert!(!plan.new_wikitext.contains("hello"));
    }

    #[test]
    fn test_transform_engine_regex_rule() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new_regex(r"\d+", "NUM", false));

        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("test 123 and 456");
        let plan = engine.apply(&page);

        assert_eq!(plan.new_wikitext, "test NUM and NUM");
        assert_eq!(plan.rules_applied.len(), 1);
    }

    #[test]
    fn test_transform_engine_invalid_regex() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new_regex(r"[invalid(", "replacement", false));

        let registry = crate::general_fixes::FixRegistry::new();
        let result = TransformEngine::new(&ruleset, registry, HashSet::new());

        assert!(result.is_err());
        match result {
            Err(TransformError::InvalidRegex { .. }) => (),
            _ => panic!("Expected InvalidRegex error"),
        }
    }

    #[test]
    fn test_transform_engine_multiple_rules() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new_plain("a", "A", true));
        ruleset.add(Rule::new_plain("b", "B", true));

        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("a b a b");
        let plan = engine.apply(&page);

        assert_eq!(plan.new_wikitext, "A B A B");
        assert_eq!(plan.rules_applied.len(), 2);
    }

    #[test]
    fn test_transform_engine_no_change_warning() {
        let ruleset = RuleSet::new();
        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("unchanged text");
        let plan = engine.apply(&page);

        assert_eq!(plan.new_wikitext, "unchanged text");
        assert!(plan.warnings.iter().any(|w| matches!(w, Warning::NoChange)));
    }

    #[test]
    fn test_transform_engine_large_change_warning() {
        let mut ruleset = RuleSet::new();
        let large_replacement = "x".repeat(600);
        ruleset.add(Rule::new_plain("small", &large_replacement, true));

        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("small text");
        let plan = engine.apply(&page);

        assert!(
            plan.warnings
                .iter()
                .any(|w| matches!(w, Warning::LargeChange { .. }))
        );
    }

    #[test]
    fn test_transform_engine_with_fixes() {
        let ruleset = RuleSet::new();
        let registry = crate::general_fixes::FixRegistry::with_defaults();
        let mut enabled = HashSet::new();
        enabled.insert("trailing_whitespace".to_string());

        let engine = TransformEngine::new(&ruleset, registry, enabled).unwrap();

        let page = create_test_page("line with spaces   \nanother line  ");
        let plan = engine.apply(&page);

        assert!(!plan.new_wikitext.contains("   "));
        assert!(plan.fixes_applied.len() > 0);
    }

    #[test]
    fn test_transform_engine_disabled_rule() {
        let mut ruleset = RuleSet::new();
        let mut rule = Rule::new_plain("test", "result", true);
        rule.enabled = false;
        ruleset.add(rule);

        let registry = crate::general_fixes::FixRegistry::new();
        let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

        let page = create_test_page("test text");
        let plan = engine.apply(&page);

        assert_eq!(plan.new_wikitext, "test text");
        assert_eq!(plan.rules_applied.len(), 0);
    }
}
