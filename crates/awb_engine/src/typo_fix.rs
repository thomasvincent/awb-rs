use crate::general_fixes::{FixContext, FixModule};
use regex::Regex;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct TypoRule {
    pub find: Regex,
    pub replace: String,
}

impl TypoRule {
    pub fn new(pattern: &str, replacement: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            find: Regex::new(pattern)?,
            replace: replacement.to_string(),
        })
    }

    pub fn apply(&self, text: &str) -> String {
        self.find
            .replace_all(text, self.replace.as_str())
            .into_owned()
    }
}

#[derive(Debug, Clone)]
pub struct TypoFixer {
    rules: Vec<TypoRule>,
}

impl TypoFixer {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: TypoRule) {
        self.rules.push(rule);
    }

    /// Parse typo rules from TSV format (tab-separated: regex\treplacement)
    pub fn from_tsv(tsv_content: &str) -> Result<Self, Box<dyn Error>> {
        let mut fixer = Self::new();

        for (line_num, line) in tsv_content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                return Err(format!(
                    "Line {}: Invalid TSV format, expected 'pattern<TAB>replacement', got: {}",
                    line_num + 1,
                    line
                )
                .into());
            }

            let pattern = parts[0];
            let replacement = parts[1];

            match TypoRule::new(pattern, replacement) {
                Ok(rule) => fixer.add_rule(rule),
                Err(e) => {
                    return Err(format!(
                        "Line {}: Invalid regex pattern '{}': {}",
                        line_num + 1,
                        pattern,
                        e
                    )
                    .into());
                }
            }
        }

        Ok(fixer)
    }

    /// Parse typo rules from AWB's XML-style format:
    /// <Typo word="..." find="..." replace="..." />
    pub fn from_awb_xml(xml_content: &str) -> Result<Self, Box<dyn Error>> {
        let mut fixer = Self::new();

        // Simple regex-based XML parsing for <Typo> elements
        let typo_re =
            Regex::new(r#"<Typo\s+(?:word="[^"]*"\s+)?find="([^"]*)"\s+replace="([^"]*)"\s*/>"#)?;

        for (line_num, caps) in typo_re.captures_iter(xml_content).enumerate() {
            let pattern = &caps[1];
            let replacement = &caps[2];

            // Unescape XML entities
            let pattern = unescape_xml(pattern);
            let replacement = unescape_xml(replacement);

            match TypoRule::new(&pattern, &replacement) {
                Ok(rule) => fixer.add_rule(rule),
                Err(e) => {
                    return Err(format!(
                        "Typo entry {}: Invalid regex pattern '{}': {}",
                        line_num + 1,
                        pattern,
                        e
                    )
                    .into());
                }
            }
        }

        Ok(fixer)
    }

    /// Auto-detect format and parse
    pub fn parse_str(content: &str) -> Result<Self, Box<dyn Error>> {
        let trimmed = content.trim();

        if trimmed.contains("<Typo") {
            Self::from_awb_xml(content)
        } else {
            Self::from_tsv(content)
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for TypoFixer {
    fn default() -> Self {
        Self::new()
    }
}

impl FixModule for TypoFixer {
    fn id(&self) -> &str {
        "typo_fixer"
    }

    fn display_name(&self) -> &str {
        "Typo Fixer"
    }

    fn category(&self) -> &str {
        "Typos"
    }

    fn description(&self) -> &str {
        "Applies regex-based typo correction rules"
    }

    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        let mut result = text.to_string();
        for rule in &self.rules {
            result = rule.apply(&result);
        }
        result
    }

    fn default_enabled(&self) -> bool {
        // Only enable if rules are loaded
        !self.rules.is_empty()
    }
}

/// Unescape basic XML entities
fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::general_fixes::FixContext;
    use awb_domain::types::{Namespace, Title};

    fn test_context() -> FixContext {
        FixContext {
            title: Title::new(Namespace::MAIN, "Test"),
            namespace: Namespace::MAIN,
            is_redirect: false,
        }
    }

    #[test]
    fn test_typo_rule_creation() {
        let rule = TypoRule::new(r"\bcolour\b", "color").unwrap();
        let result = rule.apply("The colour is blue");
        assert_eq!(result, "The color is blue");
    }

    #[test]
    fn test_typo_rule_multiple_matches() {
        let rule = TypoRule::new(r"\bcolour\b", "color").unwrap();
        let result = rule.apply("The colour and another colour");
        assert_eq!(result, "The color and another color");
    }

    #[test]
    fn test_tsv_parsing_basic() {
        let tsv = "\\bcolour\\b\tcolor\n\\bcentre\\b\tcenter";
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        assert_eq!(fixer.rule_count(), 2);
    }

    #[test]
    fn test_tsv_parsing_with_comments() {
        let tsv = r#"
# Comment line
\bcolour\b	color
# Another comment
\bcentre\b	center
"#;
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        assert_eq!(fixer.rule_count(), 2);
    }

    #[test]
    fn test_tsv_parsing_empty_lines() {
        let tsv = "\n\\bcolour\\b\tcolor\n\n\\bcentre\\b\tcenter\n";
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        assert_eq!(fixer.rule_count(), 2);
    }

    #[test]
    fn test_tsv_parsing_invalid_format() {
        let tsv = "invalid line without tab";
        let result = TypoFixer::from_tsv(tsv);
        assert!(result.is_err());
    }

    #[test]
    fn test_tsv_parsing_invalid_regex() {
        let tsv = "[invalid(regex\treplacement";
        let result = TypoFixer::from_tsv(tsv);
        assert!(result.is_err());
    }

    #[test]
    fn test_awb_xml_parsing_basic() {
        let xml = r#"<Typo word="colour" find="\bcolour\b" replace="color" />"#;
        let fixer = TypoFixer::from_awb_xml(xml).unwrap();
        assert_eq!(fixer.rule_count(), 1);
    }

    #[test]
    fn test_awb_xml_parsing_multiple() {
        let xml = r#"
<Typo word="colour" find="\bcolour\b" replace="color" />
<Typo word="centre" find="\bcentre\b" replace="center" />
"#;
        let fixer = TypoFixer::from_awb_xml(xml).unwrap();
        assert_eq!(fixer.rule_count(), 2);
    }

    #[test]
    fn test_awb_xml_parsing_with_entities() {
        let xml = r#"<Typo word="test" find="&lt;test&gt;" replace="&amp;" />"#;
        let fixer = TypoFixer::from_awb_xml(xml).unwrap();
        assert_eq!(fixer.rule_count(), 1);
    }

    #[test]
    fn test_awb_xml_parsing_no_word_attribute() {
        let xml = r#"<Typo find="\bcolour\b" replace="color" />"#;
        let fixer = TypoFixer::from_awb_xml(xml).unwrap();
        assert_eq!(fixer.rule_count(), 1);
    }

    #[test]
    fn test_auto_detect_tsv() {
        let content = "\\bcolour\\b\tcolor";
        let fixer = TypoFixer::parse_str(content).unwrap();
        assert_eq!(fixer.rule_count(), 1);
    }

    #[test]
    fn test_auto_detect_xml() {
        let content = r#"<Typo find="\bcolour\b" replace="color" />"#;
        let fixer = TypoFixer::parse_str(content).unwrap();
        assert_eq!(fixer.rule_count(), 1);
    }

    #[test]
    fn test_fix_module_apply() {
        let tsv = "\\bcolour\\b\tcolor\n\\bcentre\\b\tcenter";
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        let ctx = test_context();

        let text = "The colour of the centre";
        let result = fixer.apply(text, &ctx);
        assert_eq!(result, "The color of the center");
    }

    #[test]
    fn test_fix_module_metadata() {
        let fixer = TypoFixer::new();
        assert_eq!(fixer.id(), "typo_fixer");
        assert_eq!(fixer.display_name(), "Typo Fixer");
        assert_eq!(fixer.category(), "Typos");
    }

    #[test]
    fn test_default_enabled_empty() {
        let fixer = TypoFixer::new();
        assert!(!fixer.default_enabled());
    }

    #[test]
    fn test_default_enabled_with_rules() {
        let tsv = "\\bcolour\\b\tcolor";
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        assert!(fixer.default_enabled());
    }

    #[test]
    fn test_unescape_xml() {
        assert_eq!(unescape_xml("&lt;test&gt;"), "<test>");
        assert_eq!(unescape_xml("&amp;&quot;&apos;"), "&\"'");
    }

    #[test]
    fn test_complex_regex_patterns() {
        let tsv = r"(?i)\bcan not\b	cannot";
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        let ctx = test_context();

        let text = "I can not do this";
        let result = fixer.apply(text, &ctx);
        assert_eq!(result, "I cannot do this");
    }

    #[test]
    fn test_preserve_case_sensitivity() {
        let tsv = "\\bcolour\\b\tcolor";
        let fixer = TypoFixer::from_tsv(tsv).unwrap();
        let ctx = test_context();

        // Should not match Colour (capital C)
        let text = "Colour is different from colour";
        let result = fixer.apply(text, &ctx);
        assert_eq!(result, "Colour is different from color");
    }
}
