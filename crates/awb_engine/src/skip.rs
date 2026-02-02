use awb_domain::session::{SkipCondition, SkipDecision};
use awb_domain::types::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkipError {
    #[error("Invalid regex in skip condition: {0}")]
    InvalidRegex(#[from] regex::Error),
}

pub struct SkipEngine {
    conditions: Vec<SkipCondition>,
    compiled_regexes: Vec<(usize, regex::Regex)>,
}

impl SkipEngine {
    pub fn new(conditions: Vec<SkipCondition>) -> Result<Self, SkipError> {
        let mut compiled = Vec::new();
        for (i, cond) in conditions.iter().enumerate() {
            if let SkipCondition::RegexMatch { pattern, .. } = cond {
                compiled.push((i, regex::Regex::new(pattern)?));
            }
        }
        Ok(Self { conditions, compiled_regexes: compiled })
    }

    pub fn evaluate(&self, page: &PageContent) -> SkipDecision {
        for (i, cond) in self.conditions.iter().enumerate() {
            match cond {
                SkipCondition::Namespace { allowed } => {
                    if !allowed.contains(&page.title.namespace) {
                        return SkipDecision::Skip("namespace filtered");
                    }
                }
                SkipCondition::RegexMatch { invert, .. } => {
                    if let Some((_, re)) = self.compiled_regexes.iter().find(|(idx, _)| *idx == i) {
                        let matches = re.is_match(&page.wikitext);
                        if *invert && matches {
                            return SkipDecision::Skip("regex match (inverted)");
                        }
                        if !invert && !matches {
                            return SkipDecision::Skip("regex no match");
                        }
                    }
                }
                SkipCondition::PageSize { min_bytes, max_bytes } => {
                    if let Some(min) = min_bytes {
                        if page.size_bytes < *min {
                            return SkipDecision::Skip("page too small");
                        }
                    }
                    if let Some(max) = max_bytes {
                        if page.size_bytes > *max {
                            return SkipDecision::Skip("page too large");
                        }
                    }
                }
                SkipCondition::Protection { max_level } => {
                    if let Some(level) = &page.protection.edit {
                        if protection_exceeds(level, max_level) {
                            return SkipDecision::Skip("protection too high");
                        }
                    }
                }
                SkipCondition::IsRedirect(skip_redirects) => {
                    if page.is_redirect && *skip_redirects {
                        return SkipDecision::Skip("is redirect");
                    }
                }
                SkipCondition::IsDisambig(skip_disambig) => {
                    if page.properties.is_disambig && *skip_disambig {
                        return SkipDecision::Skip("is disambiguation");
                    }
                }
            }
        }
        SkipDecision::Process
    }
}

fn protection_exceeds(actual: &ProtectionLevel, max: &ProtectionLevel) -> bool {
    let level = |p: &ProtectionLevel| match p {
        ProtectionLevel::Autoconfirmed => 1,
        ProtectionLevel::ExtendedConfirmed => 2,
        ProtectionLevel::Sysop => 3,
    };
    level(actual) > level(max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn create_test_page(namespace: Namespace, wikitext: &str, size: u64) -> PageContent {
        PageContent {
            page_id: PageId(1),
            title: Title::new(namespace, "Test"),
            revision: RevisionId(100),
            timestamp: chrono::Utc::now(),
            wikitext: wikitext.to_string(),
            size_bytes: size,
            is_redirect: false,
            protection: ProtectionInfo::default(),
            properties: PageProperties::default(),
        }
    }

    #[test]
    fn test_skip_engine_namespace_allowed() {
        let mut allowed = HashSet::new();
        allowed.insert(Namespace::MAIN);

        let conditions = vec![SkipCondition::Namespace { allowed }];
        let engine = SkipEngine::new(conditions).unwrap();

        let page = create_test_page(Namespace::MAIN, "test", 100);
        assert_eq!(engine.evaluate(&page), SkipDecision::Process);
    }

    #[test]
    fn test_skip_engine_namespace_filtered() {
        let mut allowed = HashSet::new();
        allowed.insert(Namespace::MAIN);

        let conditions = vec![SkipCondition::Namespace { allowed }];
        let engine = SkipEngine::new(conditions).unwrap();

        let page = create_test_page(Namespace::TALK, "test", 100);
        assert_eq!(engine.evaluate(&page), SkipDecision::Skip("namespace filtered"));
    }

    #[test]
    fn test_skip_engine_regex_match() {
        let conditions = vec![SkipCondition::RegexMatch {
            pattern: r"\d+".to_string(),
            invert: false,
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let page_with_digits = create_test_page(Namespace::MAIN, "test 123", 100);
        assert_eq!(engine.evaluate(&page_with_digits), SkipDecision::Process);

        let page_without_digits = create_test_page(Namespace::MAIN, "test", 100);
        assert_eq!(engine.evaluate(&page_without_digits), SkipDecision::Skip("regex no match"));
    }

    #[test]
    fn test_skip_engine_regex_inverted() {
        let conditions = vec![SkipCondition::RegexMatch {
            pattern: r"\d+".to_string(),
            invert: true,
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let page_with_digits = create_test_page(Namespace::MAIN, "test 123", 100);
        assert_eq!(engine.evaluate(&page_with_digits), SkipDecision::Skip("regex match (inverted)"));

        let page_without_digits = create_test_page(Namespace::MAIN, "test", 100);
        assert_eq!(engine.evaluate(&page_without_digits), SkipDecision::Process);
    }

    #[test]
    fn test_skip_engine_invalid_regex() {
        let conditions = vec![SkipCondition::RegexMatch {
            pattern: r"[invalid(".to_string(),
            invert: false,
        }];
        let result = SkipEngine::new(conditions);
        assert!(result.is_err());
    }

    #[test]
    fn test_skip_engine_page_size_min() {
        let conditions = vec![SkipCondition::PageSize {
            min_bytes: Some(100),
            max_bytes: None,
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let small_page = create_test_page(Namespace::MAIN, "x", 50);
        assert_eq!(engine.evaluate(&small_page), SkipDecision::Skip("page too small"));

        let large_page = create_test_page(Namespace::MAIN, "x", 150);
        assert_eq!(engine.evaluate(&large_page), SkipDecision::Process);
    }

    #[test]
    fn test_skip_engine_page_size_max() {
        let conditions = vec![SkipCondition::PageSize {
            min_bytes: None,
            max_bytes: Some(100),
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let small_page = create_test_page(Namespace::MAIN, "x", 50);
        assert_eq!(engine.evaluate(&small_page), SkipDecision::Process);

        let large_page = create_test_page(Namespace::MAIN, "x", 150);
        assert_eq!(engine.evaluate(&large_page), SkipDecision::Skip("page too large"));
    }

    #[test]
    fn test_skip_engine_page_size_range() {
        let conditions = vec![SkipCondition::PageSize {
            min_bytes: Some(50),
            max_bytes: Some(150),
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let too_small = create_test_page(Namespace::MAIN, "x", 30);
        assert_eq!(engine.evaluate(&too_small), SkipDecision::Skip("page too small"));

        let just_right = create_test_page(Namespace::MAIN, "x", 100);
        assert_eq!(engine.evaluate(&just_right), SkipDecision::Process);

        let too_large = create_test_page(Namespace::MAIN, "x", 200);
        assert_eq!(engine.evaluate(&too_large), SkipDecision::Skip("page too large"));
    }

    #[test]
    fn test_skip_engine_protection_level() {
        let conditions = vec![SkipCondition::Protection {
            max_level: ProtectionLevel::Autoconfirmed,
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let mut page = create_test_page(Namespace::MAIN, "test", 100);
        page.protection.edit = Some(ProtectionLevel::Sysop);

        assert_eq!(engine.evaluate(&page), SkipDecision::Skip("protection too high"));
    }

    #[test]
    fn test_skip_engine_protection_level_allowed() {
        let conditions = vec![SkipCondition::Protection {
            max_level: ProtectionLevel::Sysop,
        }];
        let engine = SkipEngine::new(conditions).unwrap();

        let mut page = create_test_page(Namespace::MAIN, "test", 100);
        page.protection.edit = Some(ProtectionLevel::Autoconfirmed);

        assert_eq!(engine.evaluate(&page), SkipDecision::Process);
    }

    #[test]
    fn test_skip_engine_redirect() {
        let conditions = vec![SkipCondition::IsRedirect(true)];
        let engine = SkipEngine::new(conditions).unwrap();

        let mut redirect_page = create_test_page(Namespace::MAIN, "#REDIRECT [[Target]]", 50);
        redirect_page.is_redirect = true;

        assert_eq!(engine.evaluate(&redirect_page), SkipDecision::Skip("is redirect"));

        let normal_page = create_test_page(Namespace::MAIN, "content", 100);
        assert_eq!(engine.evaluate(&normal_page), SkipDecision::Process);
    }

    #[test]
    fn test_skip_engine_disambig() {
        let conditions = vec![SkipCondition::IsDisambig(true)];
        let engine = SkipEngine::new(conditions).unwrap();

        let mut disambig_page = create_test_page(Namespace::MAIN, "test", 100);
        disambig_page.properties.is_disambig = true;

        assert_eq!(engine.evaluate(&disambig_page), SkipDecision::Skip("is disambiguation"));

        let normal_page = create_test_page(Namespace::MAIN, "content", 100);
        assert_eq!(engine.evaluate(&normal_page), SkipDecision::Process);
    }

    #[test]
    fn test_skip_engine_multiple_conditions() {
        let mut allowed = HashSet::new();
        allowed.insert(Namespace::MAIN);

        let conditions = vec![
            SkipCondition::Namespace { allowed },
            SkipCondition::PageSize {
                min_bytes: Some(10),
                max_bytes: Some(1000),
            },
        ];
        let engine = SkipEngine::new(conditions).unwrap();

        // Should pass all conditions
        let page = create_test_page(Namespace::MAIN, "test", 100);
        assert_eq!(engine.evaluate(&page), SkipDecision::Process);

        // Should fail namespace check
        let wrong_ns = create_test_page(Namespace::TALK, "test", 100);
        assert_eq!(engine.evaluate(&wrong_ns), SkipDecision::Skip("namespace filtered"));

        // Should fail size check
        let too_large = create_test_page(Namespace::MAIN, "test", 2000);
        assert_eq!(engine.evaluate(&too_large), SkipDecision::Skip("page too large"));
    }

    #[test]
    fn test_protection_exceeds_function() {
        assert!(!protection_exceeds(&ProtectionLevel::Autoconfirmed, &ProtectionLevel::Autoconfirmed));
        assert!(!protection_exceeds(&ProtectionLevel::Autoconfirmed, &ProtectionLevel::Sysop));
        assert!(protection_exceeds(&ProtectionLevel::Sysop, &ProtectionLevel::Autoconfirmed));
        assert!(protection_exceeds(&ProtectionLevel::ExtendedConfirmed, &ProtectionLevel::Autoconfirmed));
    }

    #[test]
    fn test_skip_engine_no_conditions() {
        let conditions = vec![];
        let engine = SkipEngine::new(conditions).unwrap();

        let page = create_test_page(Namespace::MAIN, "test", 100);
        assert_eq!(engine.evaluate(&page), SkipDecision::Process);
    }
}
