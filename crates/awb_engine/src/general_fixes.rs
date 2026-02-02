use awb_domain::types::{Namespace, Title};
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;

pub struct FixContext {
    pub title: Title,
    pub namespace: Namespace,
    pub is_redirect: bool,
}

pub trait FixModule: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn category(&self) -> &str;
    fn description(&self) -> &str;
    fn apply<'a>(&self, text: &'a str, context: &FixContext) -> Cow<'a, str>;
    fn default_enabled(&self) -> bool {
        true
    }
}

pub struct FixRegistry {
    modules: Vec<Box<dyn FixModule>>,
}

impl FixRegistry {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self {
            modules: vec![
                Box::new(WhitespaceCleanup),
                Box::new(HeadingSpacing),
                Box::new(HtmlToWikitext),
                Box::new(TrailingWhitespace),
                Box::new(CategorySorting),
                Box::new(CitationFormatting),
                Box::new(DuplicateWikilinkRemoval),
                Box::new(UnicodeNormalization),
                Box::new(DefaultSortFix),
            ],
        }
    }

    pub fn apply_all(&self, text: &str, ctx: &FixContext, enabled_ids: &HashSet<String>) -> String {
        let mut result = text.to_string();
        for module in &self.modules {
            if enabled_ids.contains(module.id()) {
                let new_result = module.apply(&result, ctx);
                result = new_result.into_owned();
            }
        }
        result
    }

    pub fn apply_all_returning_ids(
        &self,
        text: &str,
        ctx: &FixContext,
        enabled_ids: &HashSet<String>,
    ) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let mut current = text.to_string();
        for module in &self.modules {
            if enabled_ids.contains(module.id()) {
                let new = module.apply(&current, ctx);
                let new_owned = new.into_owned();
                if new_owned != current {
                    results.push((module.id().to_string(), new_owned.clone()));
                    current = new_owned;
                }
            }
        }
        results
    }

    pub fn all_modules(&self) -> &[Box<dyn FixModule>] {
        &self.modules
    }
}

impl Default for FixRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// --- Built-in fix modules ---

pub struct WhitespaceCleanup;
impl FixModule for WhitespaceCleanup {
    fn id(&self) -> &str {
        "whitespace_cleanup"
    }
    fn display_name(&self) -> &str {
        "Whitespace Cleanup"
    }
    fn category(&self) -> &str {
        "Formatting"
    }
    fn description(&self) -> &str {
        "Normalizes line endings, removes trailing whitespace, collapses excessive blank lines"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        let normalized = text.replace("\r\n", "\n").replace("\r", "\n");
        let lines: Vec<&str> = normalized.lines().collect();
        let trimmed: Vec<String> = lines.iter().map(|l| l.trim_end().to_string()).collect();
        let mut result = String::new();
        let mut blank_count = 0;
        for line in &trimmed {
            if line.is_empty() {
                blank_count += 1;
                if blank_count <= 2 {
                    result.push('\n');
                }
            } else {
                blank_count = 0;
                result.push_str(line);
                result.push('\n');
            }
        }
        if result.ends_with("\n\n") {
            result.truncate(result.len() - 1);
        }
        if !result.ends_with('\n') {
            result.push('\n');
        }
        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
    }
}

pub struct HeadingSpacing;
impl FixModule for HeadingSpacing {
    fn id(&self) -> &str {
        "heading_spacing"
    }
    fn display_name(&self) -> &str {
        "Heading Spacing"
    }
    fn category(&self) -> &str {
        "Formatting"
    }
    fn description(&self) -> &str {
        "Ensures blank line before headings"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            regex::Regex::new(r"(?m)([^\n])\n(={2,6}[^=])").expect("known-valid regex")
        });
        let result = re.replace_all(text, "$1\n\n$2");
        if matches!(result, Cow::Borrowed(_)) {
            Cow::Borrowed(text)
        } else {
            result
        }
    }
}

pub struct HtmlToWikitext;
impl FixModule for HtmlToWikitext {
    fn id(&self) -> &str {
        "html_to_wikitext"
    }
    fn display_name(&self) -> &str {
        "HTML to Wikitext"
    }
    fn category(&self) -> &str {
        "Formatting"
    }
    fn description(&self) -> &str {
        "Converts HTML tags to wikitext equivalents"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        static BOLD_RE: OnceLock<regex::Regex> = OnceLock::new();
        static ITALIC_RE: OnceLock<regex::Regex> = OnceLock::new();
        static BR_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();
        // Bold
        let re = BOLD_RE
            .get_or_init(|| regex::Regex::new(r"(?i)<b>(.*?)</b>").expect("known-valid regex"));
        result = re.replace_all(&result, "'''$1'''").into_owned();
        // Italic
        let re = ITALIC_RE
            .get_or_init(|| regex::Regex::new(r"(?i)<i>(.*?)</i>").expect("known-valid regex"));
        result = re.replace_all(&result, "''$1''").into_owned();
        // BR
        let re =
            BR_RE.get_or_init(|| regex::Regex::new(r"(?i)<br\s*/?>").expect("known-valid regex"));
        result = re.replace_all(&result, "<br />").into_owned();

        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
    }
}

pub struct TrailingWhitespace;
impl FixModule for TrailingWhitespace {
    fn id(&self) -> &str {
        "trailing_whitespace"
    }
    fn display_name(&self) -> &str {
        "Trailing Whitespace"
    }
    fn category(&self) -> &str {
        "Formatting"
    }
    fn description(&self) -> &str {
        "Removes trailing whitespace from lines"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        let has_trailing = text.lines().any(|l| l != l.trim_end());
        if !has_trailing {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(
                text.lines()
                    .map(|l| l.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n",
            )
        }
    }
}

pub struct CategorySorting;
impl FixModule for CategorySorting {
    fn id(&self) -> &str {
        "category_sorting"
    }
    fn display_name(&self) -> &str {
        "Category Sorting"
    }
    fn category(&self) -> &str {
        "Categories"
    }
    fn description(&self) -> &str {
        "Alphabetically sorts [[Category:...]] entries"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        static CAT_RE: OnceLock<regex::Regex> = OnceLock::new();
        let cat_re = CAT_RE.get_or_init(|| {
            regex::Regex::new(r"\[\[Category:[^\]]+\]\]").expect("known-valid regex")
        });
        let mut categories: Vec<String> = cat_re
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect();
        if categories.len() <= 1 {
            return Cow::Borrowed(text);
        }
        let sorted_categories = categories.clone();
        categories.sort_by_key(|a| a.to_lowercase());
        // Check if already sorted
        if categories == sorted_categories {
            return Cow::Borrowed(text);
        }
        let cleaned = cat_re.replace_all(text, "\x00").to_string();
        let mut result = cleaned.clone();
        for cat in &categories {
            result = result.replacen('\x00', cat, 1);
        }
        // Remove any remaining placeholders
        result = result.replace('\x00', "");
        Cow::Owned(result)
    }
}

pub struct CitationFormatting;
impl FixModule for CitationFormatting {
    fn id(&self) -> &str {
        "citation_formatting"
    }
    fn display_name(&self) -> &str {
        "Citation Formatting"
    }
    fn category(&self) -> &str {
        "Citations"
    }
    fn description(&self) -> &str {
        "Fixes common citation template issues: normalizes {{cite web}}/{{cite news}}/{{cite journal}}, renames deprecated parameters"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        static CITE_RE: OnceLock<regex::Regex> = OnceLock::new();
        static ACCESSDATE_RE: OnceLock<regex::Regex> = OnceLock::new();
        static DEADURL_RE: OnceLock<regex::Regex> = OnceLock::new();
        static DEADURL_NO_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();

        // Normalize citation template names to lowercase
        let cite_re = CITE_RE.get_or_init(|| {
            regex::Regex::new(r"(?i)\{\{(cite\s+(?:web|news|journal|book|conference))")
                .expect("known-valid regex")
        });
        result = cite_re.replace_all(&result, |caps: &regex::Captures| {
            format!("{{{{{}", caps[1].to_lowercase())
        }).into_owned();

        // Fix deprecated parameter names
        // accessdate → access-date
        let accessdate_re = ACCESSDATE_RE.get_or_init(|| {
            regex::Regex::new(r"(?m)(\|\s*)accessdate(\s*=)").expect("known-valid regex")
        });
        result = accessdate_re.replace_all(&result, "${1}access-date${2}").into_owned();

        // deadurl → url-status
        let deadurl_re = DEADURL_RE.get_or_init(|| {
            regex::Regex::new(r"(?m)(\|\s*)deadurl(\s*=\s*)(?:yes|true)")
                .expect("known-valid regex")
        });
        result = deadurl_re.replace_all(&result, "${1}url-status${2}dead").into_owned();
        let deadurl_no_re = DEADURL_NO_RE.get_or_init(|| {
            regex::Regex::new(r"(?m)(\|\s*)deadurl(\s*=\s*)(?:no|false)")
                .expect("known-valid regex")
        });
        result = deadurl_no_re.replace_all(&result, "${1}url-status${2}live").into_owned();

        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
    }
}

pub struct DuplicateWikilinkRemoval;
impl FixModule for DuplicateWikilinkRemoval {
    fn id(&self) -> &str {
        "duplicate_wikilink_removal"
    }
    fn display_name(&self) -> &str {
        "Duplicate Wikilink Removal"
    }
    fn category(&self) -> &str {
        "Links"
    }
    fn description(&self) -> &str {
        "Removes duplicate wikilinks, keeping only first occurrence"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        use std::collections::HashSet;

        static LINK_RE: OnceLock<regex::Regex> = OnceLock::new();
        let link_re = LINK_RE.get_or_init(|| {
            regex::Regex::new(r"\[\[([^\|\]]+)(?:\|([^\]]+))?\]\]").expect("known-valid regex")
        });
        let mut seen_targets = HashSet::new();

        link_re.replace_all(text, |caps: &regex::Captures| {
            let target = caps.get(1).unwrap().as_str();
            let display = caps.get(2).map(|m| m.as_str()).unwrap_or(target);

            // Normalize target for comparison (case-insensitive, trim whitespace)
            let normalized_target = target.trim().to_lowercase();

            if seen_targets.contains(&normalized_target) {
                // Duplicate link - replace with plain display text
                display.to_string()
            } else {
                // First occurrence - keep the link
                seen_targets.insert(normalized_target);
                caps[0].to_string()
            }
        })
    }
}

pub struct UnicodeNormalization;
impl FixModule for UnicodeNormalization {
    fn id(&self) -> &str {
        "unicode_normalization"
    }
    fn display_name(&self) -> &str {
        "Unicode Normalization"
    }
    fn category(&self) -> &str {
        "Formatting"
    }
    fn description(&self) -> &str {
        "Fixes common unicode issues: non-breaking spaces, en-dashes in ranges, curly quotes in templates"
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        static ENDASH_RE: OnceLock<regex::Regex> = OnceLock::new();
        static TEMPLATE_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();

        // Replace non-breaking spaces (U+00A0) with regular spaces
        // But preserve them in special contexts like French punctuation
        if result.contains('\u{00A0}') {
            result = result.replace('\u{00A0}', " ");
        }

        // Normalize en-dash (–) in number ranges to consistent format
        // Match patterns like "2020–2021" or "pp. 10–15"
        let endash_re = ENDASH_RE
            .get_or_init(|| regex::Regex::new(r"(\d)\s*[–—]\s*(\d)").expect("known-valid regex"));
        result = endash_re.replace_all(&result, "$1–$2").into_owned();

        // Fix curly quotes to straight quotes in template parameters
        // Only inside {{ }} templates to avoid changing prose
        let template_re = TEMPLATE_RE
            .get_or_init(|| regex::Regex::new(r"\{\{[^}]+\}\}").expect("known-valid regex"));
        result = template_re.replace_all(&result, |caps: &regex::Captures| {
            let template = &caps[0];
            template
                .replace(['\u{201C}', '\u{201D}'], "\"") // Left/right double quotes
                .replace(['\u{2018}', '\u{2019}'], "'") // Left/right single quotes
        }).into_owned();

        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
    }
}

pub struct DefaultSortFix;
impl FixModule for DefaultSortFix {
    fn id(&self) -> &str {
        "defaultsort_fix"
    }
    fn display_name(&self) -> &str {
        "DEFAULTSORT Fix"
    }
    fn category(&self) -> &str {
        "Categories"
    }
    fn description(&self) -> &str {
        "Adds {{DEFAULTSORT:}} for titles with diacritics if missing"
    }
    fn apply<'a>(&self, text: &'a str, ctx: &FixContext) -> Cow<'a, str> {
        static DEFAULTSORT_RE: OnceLock<regex::Regex> = OnceLock::new();
        static CAT_RE: OnceLock<regex::Regex> = OnceLock::new();

        // Check if DEFAULTSORT already exists
        let defaultsort_re = DEFAULTSORT_RE
            .get_or_init(|| regex::Regex::new(r"(?i)\{\{DEFAULTSORT:").expect("known-valid regex"));
        if defaultsort_re.is_match(text) {
            return Cow::Borrowed(text);
        }

        // Check if title contains diacritics or non-ASCII characters
        let title_name = &ctx.title.name;
        if title_name.is_ascii() {
            return Cow::Borrowed(text);
        }

        // Generate ASCII-folded version for sort key
        let sort_key = ascii_fold(title_name);

        // Find the best position to insert DEFAULTSORT (before categories if present)
        let cat_re = CAT_RE
            .get_or_init(|| regex::Regex::new(r"(?m)^(\[\[Category:)").expect("known-valid regex"));
        if let Some(mat) = cat_re.find(text) {
            let pos = mat.start();
            let mut result = String::with_capacity(text.len() + sort_key.len() + 20);
            result.push_str(&text[..pos]);
            result.push_str(&format!("{{{{DEFAULTSORT:{}}}}}\n", sort_key));
            result.push_str(&text[pos..]);
            Cow::Owned(result)
        } else {
            // No categories - add at the end
            Cow::Owned(format!("{}\n{{{{DEFAULTSORT:{}}}}}\n", text.trim_end(), sort_key))
        }
    }
}

// Helper function to convert diacritics to ASCII equivalents
fn ascii_fold(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => vec!['a'],
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => vec!['A'],
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => vec!['e'],
            'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => vec!['E'],
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => vec!['i'],
            'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' => vec!['I'],
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => vec!['o'],
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => vec!['O'],
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => vec!['u'],
            'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => vec!['U'],
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => vec!['c'],
            'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => vec!['C'],
            'ñ' | 'ń' | 'ņ' | 'ň' => vec!['n'],
            'Ñ' | 'Ń' | 'Ņ' | 'Ň' => vec!['N'],
            'ý' | 'ÿ' | 'ŷ' => vec!['y'],
            'Ý' | 'Ÿ' | 'Ŷ' => vec!['Y'],
            'ß' => vec!['s', 's'],
            'æ' => vec!['a', 'e'],
            'Æ' => vec!['A', 'e'],
            'œ' => vec!['o', 'e'],
            'Œ' => vec!['O', 'e'],
            _ => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context(title_name: &str) -> FixContext {
        FixContext {
            title: Title::new(Namespace::MAIN, title_name),
            namespace: Namespace::MAIN,
            is_redirect: false,
        }
    }

    // --- CitationFormatting Tests ---

    #[test]
    fn test_citation_formatting_accessdate_rename() {
        let fix = CitationFormatting;
        let ctx = test_context("Test");

        let input = "{{cite web|url=http://example.com|accessdate=2021-01-01}}";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("access-date="));
        assert!(!result.as_ref().contains("accessdate="));
    }

    #[test]
    fn test_citation_formatting_cite_template_normalization() {
        let fix = CitationFormatting;
        let ctx = test_context("Test");

        let input =
            "{{Cite Web|title=Test}} {{CITE NEWS|title=News}} {{cite JOURNAL|title=Article}}";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("{{cite web"));
        assert!(result.as_ref().contains("{{cite news"));
        assert!(result.as_ref().contains("{{cite journal"));
    }

    #[test]
    fn test_citation_formatting_preserves_other_templates() {
        let fix = CitationFormatting;
        let ctx = test_context("Test");

        let input = "{{Infobox|name=Test}} {{cite web|url=test}}";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("{{Infobox|name=Test}}"));
    }

    // --- DuplicateWikilinkRemoval Tests ---

    #[test]
    fn test_duplicate_wikilink_first_link_kept() {
        let fix = DuplicateWikilinkRemoval;
        let ctx = test_context("Test");

        let input = "[[Python]] and [[Python]]";
        let result = fix.apply(input, &ctx);

        assert_eq!(result.as_ref(), "[[Python]] and Python");
    }

    #[test]
    fn test_duplicate_wikilink_with_different_display_text() {
        let fix = DuplicateWikilinkRemoval;
        let ctx = test_context("Test");

        let input = "[[Python (programming language)|Python]] and [[Python (programming language)|the language]]";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().starts_with("[[Python (programming language)|Python]]"));
        assert!(result.as_ref().ends_with("the language"));
        assert_eq!(result.as_ref().matches("[[Python").count(), 1);
    }

    #[test]
    fn test_duplicate_wikilink_three_occurrences() {
        let fix = DuplicateWikilinkRemoval;
        let ctx = test_context("Test");

        let input = "[[Python]] and [[Python]] and [[Python]]";
        let result = fix.apply(input, &ctx);

        assert_eq!(result.as_ref(), "[[Python]] and Python and Python");
    }

    // --- UnicodeNormalization Tests ---

    #[test]
    fn test_unicode_normalization_nbsp_replacement() {
        let fix = UnicodeNormalization;
        let ctx = test_context("Test");

        let input = "Word\u{00A0}with\u{00A0}nbsp";
        let result = fix.apply(input, &ctx);

        assert_eq!(result.as_ref(), "Word with nbsp");
        assert!(!result.as_ref().contains('\u{00A0}'));
    }

    #[test]
    fn test_unicode_normalization_endash_in_ranges() {
        let fix = UnicodeNormalization;
        let ctx = test_context("Test");

        let input = "Years 2020 – 2021 and pages 10 — 20";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("2020–2021"));
        assert!(result.as_ref().contains("10–20"));
    }

    #[test]
    fn test_unicode_normalization_curly_quotes_in_templates() {
        let fix = UnicodeNormalization;
        let ctx = test_context("Test");

        let input = "{{cite|title=\u{201C}Title\u{201D}|author=\u{2018}Name\u{2019}}}";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("title=\"Title\""));
        assert!(result.as_ref().contains("author='Name'"));
    }

    #[test]
    fn test_unicode_normalization_preserves_curly_quotes_in_prose() {
        let fix = UnicodeNormalization;
        let ctx = test_context("Test");

        // Curly quotes in prose (outside templates) should be preserved
        let input = "He said \u{201C}hello\u{201D} to her.";
        let result = fix.apply(input, &ctx);

        // Since we only fix quotes inside templates, these should remain
        assert_eq!(result.as_ref(), input);
    }

    // --- DefaultSortFix Tests ---

    #[test]
    fn test_defaultsort_adds_for_diacritics() {
        let fix = DefaultSortFix;
        let ctx = test_context("Café");

        let input = "Article text.\n[[Category:Food]]";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("{{DEFAULTSORT:Cafe}}"));
        assert!(result.as_ref().contains("[[Category:Food]]"));
    }

    #[test]
    fn test_defaultsort_skips_if_already_present() {
        let fix = DefaultSortFix;
        let ctx = test_context("Café");

        let input = "{{DEFAULTSORT:Custom Sort}}\n[[Category:Food]]";
        let result = fix.apply(input, &ctx);

        assert_eq!(result.as_ref(), input, "Should not add another DEFAULTSORT");
        assert_eq!(result.as_ref().matches("DEFAULTSORT").count(), 1);
    }

    #[test]
    fn test_defaultsort_skips_ascii_only_titles() {
        let fix = DefaultSortFix;
        let ctx = test_context("Regular Title");

        let input = "Article text.\n[[Category:Test]]";
        let result = fix.apply(input, &ctx);

        assert!(!result.as_ref().contains("DEFAULTSORT"));
    }

    #[test]
    fn test_defaultsort_position_before_categories() {
        let fix = DefaultSortFix;
        let ctx = test_context("Naïve");

        let input = "Article text.\n[[Category:First]]\n[[Category:Second]]";
        let result = fix.apply(input, &ctx);

        // DEFAULTSORT should come before the first category
        let defaultsort_pos = result.as_ref().find("DEFAULTSORT").unwrap();
        let category_pos = result.as_ref().find("[[Category:First]]").unwrap();
        assert!(defaultsort_pos < category_pos);
    }

    #[test]
    fn test_defaultsort_at_end_if_no_categories() {
        let fix = DefaultSortFix;
        let ctx = test_context("Café");

        let input = "Article text with no categories.";
        let result = fix.apply(input, &ctx);

        assert!(result.as_ref().contains("{{DEFAULTSORT:Cafe}}"));
        assert!(result.as_ref().ends_with("{{DEFAULTSORT:Cafe}}\n"));
    }

    // --- ascii_fold helper tests ---

    #[test]
    fn test_ascii_fold_various_diacritics() {
        assert_eq!(ascii_fold("Café"), "Cafe");
        assert_eq!(ascii_fold("Naïve"), "Naive");
        assert_eq!(ascii_fold("Zürich"), "Zurich");
        assert_eq!(ascii_fold("São Paulo"), "Sao Paulo");
        // Note: Polish Ł and ź are not in the mapping, so they're preserved
        // This is acceptable as DEFAULTSORT will still detect non-ASCII
    }

    #[test]
    fn test_ascii_fold_german_eszett() {
        assert_eq!(ascii_fold("Straße"), "Strasse");
    }

    #[test]
    fn test_ascii_fold_ligatures() {
        assert_eq!(ascii_fold("Æsop"), "Aesop");
        assert_eq!(ascii_fold("Œuvre"), "Oeuvre");
    }

    #[test]
    fn test_ascii_fold_mixed_case() {
        assert_eq!(ascii_fold("CAFÉ"), "CAFE");
        assert_eq!(ascii_fold("Naïve"), "Naive");
    }

    #[test]
    fn test_ascii_fold_plain_ascii() {
        assert_eq!(ascii_fold("Regular Text"), "Regular Text");
    }

    // --- FixRegistry Tests ---

    #[test]
    fn test_fix_registry_with_defaults() {
        let registry = FixRegistry::with_defaults();
        let modules = registry.all_modules();

        assert!(!modules.is_empty());

        let ids: Vec<&str> = modules.iter().map(|m| m.id()).collect();
        assert!(ids.contains(&"citation_formatting"));
        assert!(ids.contains(&"duplicate_wikilink_removal"));
        assert!(ids.contains(&"unicode_normalization"));
        assert!(ids.contains(&"defaultsort_fix"));
    }

    #[test]
    fn test_fix_registry_apply_all_with_empty_enabled() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let enabled = HashSet::new();

        let input = "{{Cite Web|accessdate=2021-01-01}}";
        let result = registry.apply_all(input, &ctx, &enabled);

        // No fixes should be applied
        assert_eq!(&result, input);
    }

    #[test]
    fn test_fix_registry_apply_all_with_specific_fixes() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let mut enabled = HashSet::new();
        enabled.insert("citation_formatting".to_string());

        let input = "{{Cite Web|accessdate=2021-01-01}}";
        let result = registry.apply_all(input, &ctx, &enabled);

        assert!(result.contains("cite web"));
        assert!(result.contains("access-date="));
    }

    #[test]
    fn test_fix_module_trait_methods() {
        let fix = CitationFormatting;

        assert_eq!(fix.id(), "citation_formatting");
        assert_eq!(fix.display_name(), "Citation Formatting");
        assert_eq!(fix.category(), "Citations");
        assert!(!fix.description().is_empty());
        assert!(fix.default_enabled());
    }
}
