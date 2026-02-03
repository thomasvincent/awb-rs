//! Namespace parsing, normalization, and enforcement for MediaWiki titles.
//!
//! Handles:
//! - Parsing "Namespace:Title" strings into (Namespace, normalized_name).
//! - Underscore/space normalization (MediaWiki treats them identically).
//! - First-letter case normalization (MediaWiki uppercases the first letter of titles).
//! - Namespace allowlist enforcement for unattended bot runs.
//! - Default: Main namespace only for unattended operation.

use awb_domain::types::Namespace;
use std::collections::HashSet;

/// Known namespace prefixes mapped to their IDs.
/// This covers English Wikipedia; a production bot would fetch these from siteinfo.
const NAMESPACE_MAP: &[(&str, Namespace)] = &[
    ("talk", Namespace::TALK),
    ("user", Namespace::USER),
    ("user talk", Namespace::USER_TALK),
    ("wikipedia", Namespace::PROJECT),
    ("project", Namespace::PROJECT),
    ("wikipedia talk", Namespace::PROJECT_TALK),
    ("project talk", Namespace::PROJECT_TALK),
    ("file", Namespace::FILE),
    ("image", Namespace::FILE), // alias
    ("file talk", Namespace::FILE_TALK),
    ("mediawiki", Namespace::MEDIAWIKI),
    ("template", Namespace::TEMPLATE),
    ("template talk", Namespace::TEMPLATE_TALK),
    ("help", Namespace::HELP),
    ("category", Namespace::CATEGORY),
    ("category talk", Namespace::CATEGORY_TALK),
];

/// A parsed and normalized title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTitle {
    pub namespace: Namespace,
    pub name: String,
}

/// Parse a title string into namespace + name.
///
/// - Normalizes underscores to spaces.
/// - Trims leading/trailing whitespace.
/// - Uppercases the first letter of the page name (MediaWiki convention).
/// - Recognizes standard English Wikipedia namespace prefixes.
/// - Unknown prefixes are treated as Main namespace (the colon becomes part of the title).
pub fn parse_title(raw: &str) -> ParsedTitle {
    let normalized = raw.replace('_', " ");
    let trimmed = normalized.trim();

    if let Some((prefix, rest)) = trimmed.split_once(':') {
        let prefix_lower = prefix.trim().to_ascii_lowercase();
        let rest = rest.trim();

        for &(name, ns) in NAMESPACE_MAP {
            if prefix_lower == name {
                return ParsedTitle {
                    namespace: ns,
                    name: normalize_first_letter(rest),
                };
            }
        }
    }

    // No recognized namespace prefix → Main namespace
    ParsedTitle {
        namespace: Namespace::MAIN,
        name: normalize_first_letter(trimmed),
    }
}

/// Uppercase the first letter of a title (MediaWiki convention).
fn normalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = String::with_capacity(s.len());
            for c in first.to_uppercase() {
                result.push(c);
            }
            result.extend(chars);
            result
        }
    }
}

/// Configuration for namespace enforcement.
#[derive(Debug, Clone)]
pub struct NamespacePolicy {
    /// Allowed namespace IDs. If empty, all namespaces are allowed.
    pub allowed: HashSet<Namespace>,
}

impl NamespacePolicy {
    /// Default policy for unattended bot runs: Main namespace only.
    pub fn unattended_default() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(Namespace::MAIN);
        Self { allowed }
    }

    /// Allow all namespaces (for supervised/interactive use).
    pub fn allow_all() -> Self {
        Self {
            allowed: HashSet::new(),
        }
    }

    /// Create a policy allowing specific namespaces.
    pub fn with_namespaces(namespaces: impl IntoIterator<Item = Namespace>) -> Self {
        Self {
            allowed: namespaces.into_iter().collect(),
        }
    }

    /// Check if a namespace is allowed under this policy.
    /// Empty allowed set means all namespaces are permitted.
    pub fn is_allowed(&self, ns: Namespace) -> bool {
        self.allowed.is_empty() || self.allowed.contains(&ns)
    }
}

impl Default for NamespacePolicy {
    fn default() -> Self {
        Self::unattended_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_title ---

    #[test]
    fn test_parse_main_namespace() {
        let result = parse_title("Example article");
        assert_eq!(result.namespace, Namespace::MAIN);
        assert_eq!(result.name, "Example article");
    }

    #[test]
    fn test_parse_first_letter_uppercase() {
        let result = parse_title("example article");
        assert_eq!(result.name, "Example article");
    }

    #[test]
    fn test_parse_underscore_normalization() {
        let result = parse_title("Example_article_name");
        assert_eq!(result.name, "Example article name");
    }

    #[test]
    fn test_parse_talk_namespace() {
        let result = parse_title("Talk:Example");
        assert_eq!(result.namespace, Namespace::TALK);
        assert_eq!(result.name, "Example");
    }

    #[test]
    fn test_parse_user_namespace() {
        let result = parse_title("User:SomeBot");
        assert_eq!(result.namespace, Namespace::USER);
        assert_eq!(result.name, "SomeBot");
    }

    #[test]
    fn test_parse_user_talk() {
        let result = parse_title("User talk:SomeBot");
        assert_eq!(result.namespace, Namespace::USER_TALK);
        assert_eq!(result.name, "SomeBot");
    }

    #[test]
    fn test_parse_user_talk_underscores() {
        let result = parse_title("User_talk:SomeBot");
        assert_eq!(result.namespace, Namespace::USER_TALK);
        assert_eq!(result.name, "SomeBot");
    }

    #[test]
    fn test_parse_template_namespace() {
        let result = parse_title("Template:Infobox");
        assert_eq!(result.namespace, Namespace::TEMPLATE);
        assert_eq!(result.name, "Infobox");
    }

    #[test]
    fn test_parse_category_namespace() {
        let result = parse_title("Category:Rust programming");
        assert_eq!(result.namespace, Namespace::CATEGORY);
        assert_eq!(result.name, "Rust programming");
    }

    #[test]
    fn test_parse_file_namespace() {
        let result = parse_title("File:Example.png");
        assert_eq!(result.namespace, Namespace::FILE);
        assert_eq!(result.name, "Example.png");
    }

    #[test]
    fn test_parse_image_alias() {
        let result = parse_title("Image:Old_photo.jpg");
        assert_eq!(result.namespace, Namespace::FILE);
        assert_eq!(result.name, "Old photo.jpg");
    }

    #[test]
    fn test_parse_wikipedia_alias() {
        let result = parse_title("Wikipedia:Bot policy");
        assert_eq!(result.namespace, Namespace::PROJECT);
        assert_eq!(result.name, "Bot policy");
    }

    #[test]
    fn test_parse_case_insensitive_prefix() {
        let result = parse_title("TALK:Example");
        assert_eq!(result.namespace, Namespace::TALK);
        assert_eq!(result.name, "Example");
    }

    #[test]
    fn test_parse_unknown_prefix_stays_main() {
        let result = parse_title("UnknownNS:SomePage");
        assert_eq!(result.namespace, Namespace::MAIN);
        // Unknown prefix becomes part of the title
        assert_eq!(result.name, "UnknownNS:SomePage");
    }

    #[test]
    fn test_parse_colon_in_title() {
        // "Foo: bar" — "Foo" is not a known namespace
        let result = parse_title("Foo: bar");
        assert_eq!(result.namespace, Namespace::MAIN);
        assert_eq!(result.name, "Foo: bar");
    }

    #[test]
    fn test_parse_trimming() {
        let result = parse_title("  Talk:  Example  ");
        assert_eq!(result.namespace, Namespace::TALK);
        assert_eq!(result.name, "Example");
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_title("");
        assert_eq!(result.namespace, Namespace::MAIN);
        assert_eq!(result.name, "");
    }

    #[test]
    fn test_parse_mediawiki_namespace() {
        let result = parse_title("MediaWiki:Common.css");
        assert_eq!(result.namespace, Namespace::MEDIAWIKI);
        assert_eq!(result.name, "Common.css");
    }

    // --- NamespacePolicy ---

    #[test]
    fn test_unattended_default_main_only() {
        let policy = NamespacePolicy::unattended_default();
        assert!(policy.is_allowed(Namespace::MAIN));
        assert!(!policy.is_allowed(Namespace::TALK));
        assert!(!policy.is_allowed(Namespace::USER));
        assert!(!policy.is_allowed(Namespace::TEMPLATE));
    }

    #[test]
    fn test_allow_all() {
        let policy = NamespacePolicy::allow_all();
        assert!(policy.is_allowed(Namespace::MAIN));
        assert!(policy.is_allowed(Namespace::TALK));
        assert!(policy.is_allowed(Namespace::USER));
        assert!(policy.is_allowed(Namespace::CATEGORY));
    }

    #[test]
    fn test_custom_policy() {
        let policy =
            NamespacePolicy::with_namespaces([Namespace::MAIN, Namespace::TALK]);
        assert!(policy.is_allowed(Namespace::MAIN));
        assert!(policy.is_allowed(Namespace::TALK));
        assert!(!policy.is_allowed(Namespace::USER));
    }

    #[test]
    fn test_default_is_unattended() {
        let policy = NamespacePolicy::default();
        assert!(policy.is_allowed(Namespace::MAIN));
        assert!(!policy.is_allowed(Namespace::USER));
    }
}
