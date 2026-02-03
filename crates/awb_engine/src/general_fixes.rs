use crate::fix_config::{ApplyResult, FixClassification, FixConfig, FixConfigError};
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
    /// The classification of changes this module makes.
    fn classification(&self) -> FixClassification {
        FixClassification::Maintenance
    }
    /// Minimum strictness tier required to run this module (0-3).
    fn min_tier(&self) -> u8 {
        1
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

    /// Apply all enabled fixes, returning the list of fix IDs that made changes
    /// and the final text after all fixes.
    pub fn apply_all_returning_ids(
        &self,
        text: &str,
        ctx: &FixContext,
        enabled_ids: &HashSet<String>,
    ) -> (Vec<String>, String) {
        if enabled_ids.is_empty() {
            return (Vec::new(), text.to_string());
        }
        let mut changed_ids = Vec::new();
        let mut current = text.to_string();
        for module in &self.modules {
            if enabled_ids.contains(module.id()) {
                let new = module.apply(&current, ctx);
                let new_owned = new.into_owned();
                if new_owned != current {
                    changed_ids.push(module.id().to_string());
                    current = new_owned;
                }
            }
        }
        (changed_ids, current)
    }

    pub fn all_modules(&self) -> &[Box<dyn FixModule>] {
        &self.modules
    }

    /// Returns the set of all known fix IDs.
    pub fn known_ids(&self) -> HashSet<&str> {
        self.modules.iter().map(|m| m.id()).collect()
    }

    /// Apply fixes filtered by a `FixConfig`.
    ///
    /// Returns an error if the config references unknown fix IDs.
    pub fn apply_all_with_config(
        &self,
        text: &str,
        ctx: &FixContext,
        config: &FixConfig,
    ) -> Result<ApplyResult, FixConfigError> {
        config.validate(&self.known_ids())?;

        let mut current = text.to_string();
        let mut changed_ids = Vec::new();
        let mut all_cosmetic = true;

        for module in &self.modules {
            // Tier gate
            if module.min_tier() > config.strictness_tier {
                continue;
            }
            // Explicit disable list
            if config.disabled_fixes.contains(module.id()) {
                continue;
            }
            // Explicit enable list (if non-empty, only listed IDs run)
            if !config.enabled_fixes.is_empty() && !config.enabled_fixes.contains(module.id()) {
                continue;
            }

            let new = module.apply(&current, ctx);
            let new_owned = new.into_owned();
            if new_owned != current {
                changed_ids.push(module.id().to_string());
                if module.classification() != FixClassification::Cosmetic {
                    all_cosmetic = false;
                }
                current = new_owned;
            }
        }

        let is_cosmetic_only = !changed_ids.is_empty() && all_cosmetic;

        Ok(ApplyResult {
            final_text: current,
            changed_ids,
            is_cosmetic_only,
        })
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
    fn classification(&self) -> FixClassification {
        FixClassification::Cosmetic
    }
    fn min_tier(&self) -> u8 {
        0
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        if text.is_empty() {
            return Cow::Borrowed(text);
        }

        // Single-pass: normalize line endings, trim trailing whitespace per line,
        // cap consecutive blank lines at 2, ensure single trailing newline.
        // SAFETY: We only split on ASCII bytes 0x0A (\n) and 0x0D (\r), which cannot
        // appear as UTF-8 continuation bytes (those are 0x80-0xBF), so byte-level
        // splitting is safe for UTF-8 text.
        let mut result = String::with_capacity(text.len());
        let mut consecutive_blanks: u32 = 0;
        let mut line_start = 0;
        let mut changed = false;
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // Find end of line
            let line_end;
            let newline_len;
            if bytes[i] == b'\r' {
                if i + 1 < len && bytes[i + 1] == b'\n' {
                    line_end = i;
                    newline_len = 2; // \r\n
                    changed = true;
                } else {
                    line_end = i;
                    newline_len = 1; // bare \r
                    changed = true;
                }
            } else if bytes[i] == b'\n' {
                line_end = i;
                newline_len = 1;
            } else {
                i += 1;
                continue;
            }

            // We have a complete line from line_start..line_end
            let line = &text[line_start..line_end];
            let trimmed = line.trim_end();
            if trimmed != line {
                changed = true;
            }

            if trimmed.is_empty() {
                consecutive_blanks += 1;
                if consecutive_blanks <= 2 {
                    result.push('\n');
                } else {
                    changed = true;
                }
            } else {
                consecutive_blanks = 0;
                result.push_str(trimmed);
                result.push('\n');
            }

            i = line_end + newline_len;
            line_start = i;
        }

        // Handle final line (no trailing newline)
        if line_start < len {
            let line = &text[line_start..len];
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                result.push_str(trimmed);
            }
            // Input didn't end with newline; we need to add one
            changed = true;
        }

        // Ensure exactly one trailing newline
        while result.ends_with("\n\n") {
            result.pop();
            changed = true;
        }
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
            changed = true;
        }

        if changed {
            Cow::Owned(result)
        } else {
            Cow::Borrowed(text)
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
    fn classification(&self) -> FixClassification {
        FixClassification::Cosmetic
    }
    fn min_tier(&self) -> u8 {
        0
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        // A blank line before a heading means pattern: ...\n\n==...
        // After split('\n'): [..., "", "==..."]
        // We need to ensure there's an empty line immediately before each heading.

        let lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            return Cow::Borrowed(text);
        }

        let mut result_lines = Vec::with_capacity(lines.len());
        let mut changed = false;

        for (i, line) in lines.iter().enumerate() {
            // Check if this is a heading line
            let is_heading = line.trim_start().len() >= 2
                && line.trim_start().starts_with("==")
                && line.trim_start().chars().take_while(|&c| c == '=').count() >= 2;

            if is_heading && i > 0 {
                // Check if there's any actual content before this heading
                let has_preceding_content = lines[..i].iter().any(|l| !l.trim().is_empty());

                if !has_preceding_content {
                    // Heading is at BOS (only whitespace before it) - don't add blank line
                    // This prevents cosmetic-only edits at beginning of string
                } else {
                    let prev_line = lines[i - 1];
                    if !prev_line.trim().is_empty() {
                        // Previous line has content, add blank line before heading
                        result_lines.push("");
                        changed = true;
                    }
                    // If prev_line is empty, blank line already exists - no action needed
                }
            }

            result_lines.push(line);
        }

        if changed {
            Cow::Owned(result_lines.join("\n"))
        } else {
            Cow::Borrowed(text)
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
    fn classification(&self) -> FixClassification {
        FixClassification::Maintenance
    }
    fn min_tier(&self) -> u8 {
        1
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        // Early return if no HTML tags present
        if !text.contains('<') {
            return Cow::Borrowed(text);
        }

        static BOLD_RE: OnceLock<regex::Regex> = OnceLock::new();
        static ITALIC_RE: OnceLock<regex::Regex> = OnceLock::new();
        static BR_RE: OnceLock<regex::Regex> = OnceLock::new();

        let bold_re = BOLD_RE
            .get_or_init(|| regex::Regex::new(r"(?i)<b>([^<]*)</b>").expect("known-valid regex"));
        let italic_re = ITALIC_RE
            .get_or_init(|| regex::Regex::new(r"(?i)<i>([^<]*)</i>").expect("known-valid regex"));
        let br_re =
            BR_RE.get_or_init(|| regex::Regex::new(r"(?i)<br\s*/?>").expect("known-valid regex"));

        // Check if any regex matches before allocating
        if !bold_re.is_match(text) && !italic_re.is_match(text) && !br_re.is_match(text) {
            return Cow::Borrowed(text);
        }

        // At least one match found, proceed with replacements
        let mut result = text.to_string();
        result = bold_re.replace_all(&result, "'''$1'''").into_owned();
        result = italic_re.replace_all(&result, "''$1''").into_owned();
        result = br_re.replace_all(&result, "<br />").into_owned();

        Cow::Owned(result)
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
    fn classification(&self) -> FixClassification {
        FixClassification::Cosmetic
    }
    fn min_tier(&self) -> u8 {
        0
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        let has_trailing = text.lines().any(|l| l != l.trim_end());
        if !has_trailing {
            Cow::Borrowed(text)
        } else {
            // Preserve original trailing newline status
            let had_trailing_newline = text.ends_with('\n');
            let mut result = text.lines()
                .map(|l| l.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            if had_trailing_newline {
                result.push('\n');
            }
            Cow::Owned(result)
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
    fn classification(&self) -> FixClassification {
        FixClassification::Maintenance
    }
    fn min_tier(&self) -> u8 {
        0
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        // PLACEHOLDER uses \x02 prefix to avoid collision with masking sentinels (\x00 prefix).
        // This is safe because masking runs at a higher level and category sorting operates
        // on already-masked text where sentinel regions are replaced with \x00\x01AWB_MASK_* tokens.
        const PLACEHOLDER: &str = "\x02AWB_SORT_PLACEHOLDER\x02";

        // Fail closed: if input already contains the placeholder, do not modify
        if text.contains(PLACEHOLDER) {
            return Cow::Borrowed(text);
        }

        static CAT_RE: OnceLock<regex::Regex> = OnceLock::new();
        static CAT_PARSE_RE: OnceLock<regex::Regex> = OnceLock::new();

        let cat_re = CAT_RE.get_or_init(|| {
            regex::Regex::new(r"\[\[Category:[^\]]+\]\]").expect("known-valid regex")
        });
        // Parse title and optional sort key: [[Category:Title|SortKey]] or [[Category:Title]]
        let cat_parse_re = CAT_PARSE_RE.get_or_init(|| {
            regex::Regex::new(r"\[\[Category:([^\]|]+)(?:\|([^\]]*))?\]\]").expect("known-valid regex")
        });

        let categories: Vec<String> = cat_re
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect();

        if categories.len() <= 1 {
            return Cow::Borrowed(text);
        }

        let original_order = categories.clone();

        // Build sort keys: (normalized_title, normalized_sort_key, original_text)
        let mut sort_entries: Vec<(String, String, &str)> = categories
            .iter()
            .map(|cat| {
                let (norm_title, norm_sort_key) = if let Some(caps) = cat_parse_re.captures(cat) {
                    let title = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let sort_key = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    (normalize_category_title(title), normalize_category_title(sort_key))
                } else {
                    (cat.to_lowercase(), String::new())
                };
                (norm_title, norm_sort_key, cat.as_str())
            })
            .collect();

        sort_entries.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(b.2))
        });

        let sorted_cats: Vec<&str> = sort_entries.iter().map(|e| e.2).collect();
        let original_refs: Vec<&str> = original_order.iter().map(|s| s.as_str()).collect();

        // Check if already sorted
        if sorted_cats == original_refs {
            return Cow::Borrowed(text);
        }

        // Replace categories with placeholders, then fill in sorted order
        let cleaned = cat_re.replace_all(text, PLACEHOLDER).to_string();
        let mut result = cleaned;
        for cat in &sorted_cats {
            // replacen with count=1 replaces the first occurrence only
            result = result.replacen(PLACEHOLDER, cat, 1);
        }

        // Fail closed: if any placeholder remains, something went wrong — return original
        if result.contains(PLACEHOLDER) {
            return Cow::Borrowed(text);
        }

        Cow::Owned(result)
    }
}

/// Citation template parameter formatting (normalize names, fix deprecated params).
///
/// # Design Note: Masking Interaction
///
/// This fix is **intentionally ineffective when masking is enabled**. Citation templates
/// (e.g., `{{cite web|...}}`) are masked as protected regions during high-level masking
/// operations, because their parameters require specialized understanding of citation syntax.
///
/// When masking runs:
/// - All `{{cite ...}}` templates are replaced with masking sentinels (`\x00\x01AWB_MASK_*`)
/// - This fix sees only the masked text, not the original template content
/// - Parameter renames like `accessdate` → `access-date` are **not applied**
/// - The fix returns the text unchanged
///
/// This is **by design**. Proper citation formatting requires:
/// 1. Unmasking templates to expose their parameters
/// 2. Specialized template parsing (not regex)
/// 3. Parameter-aware rewrites
/// 4. Re-masking the result
///
/// A future dedicated template-aware pass should handle citation reformatting. For now,
/// this fix only works on unmasked text (or when masking is disabled entirely).
///
/// # Tier Classification
///
/// Classified as **Tier 2 (StyleSensitive)** to prevent accidental use in unattended
/// bot mode (Tier 0-1), since the fix is currently ineffective on masked text.
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
    fn classification(&self) -> FixClassification {
        FixClassification::StyleSensitive
    }
    fn min_tier(&self) -> u8 {
        2
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
        result = cite_re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("{{{{{}", caps[1].to_lowercase())
            })
            .into_owned();

        // Fix deprecated parameter names
        // accessdate → access-date
        let accessdate_re = ACCESSDATE_RE.get_or_init(|| {
            regex::Regex::new(r"(?m)(\|\s*)accessdate(\s*=)").expect("known-valid regex")
        });
        result = accessdate_re
            .replace_all(&result, "${1}access-date${2}")
            .into_owned();

        // deadurl → url-status
        let deadurl_re = DEADURL_RE.get_or_init(|| {
            regex::Regex::new(r"(?m)(\|\s*)deadurl(\s*=\s*)(?:yes|true)")
                .expect("known-valid regex")
        });
        result = deadurl_re
            .replace_all(&result, "${1}url-status${2}dead")
            .into_owned();
        let deadurl_no_re = DEADURL_NO_RE.get_or_init(|| {
            regex::Regex::new(r"(?m)(\|\s*)deadurl(\s*=\s*)(?:no|false)")
                .expect("known-valid regex")
        });
        result = deadurl_no_re
            .replace_all(&result, "${1}url-status${2}live")
            .into_owned();

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
    fn classification(&self) -> FixClassification {
        FixClassification::Maintenance
    }
    fn min_tier(&self) -> u8 {
        1
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        // Early exit: if no wikilinks present, nothing to deduplicate
        if !text.contains("[[") {
            return Cow::Borrowed(text);
        }

        use std::collections::HashSet;

        static LINK_RE: OnceLock<regex::Regex> = OnceLock::new();
        static HEADING_RE: OnceLock<regex::Regex> = OnceLock::new();

        let link_re = LINK_RE.get_or_init(|| {
            regex::Regex::new(r"\[\[([^\|\]]+)(?:\|([^\]]+))?\]\]").expect("known-valid regex")
        });
        let heading_re = HEADING_RE.get_or_init(|| {
            regex::Regex::new(r"^={2,6}\s").expect("known-valid regex")
        });

        // Record the exact number of trailing newlines
        let trailing_newlines = text.chars().rev().take_while(|&c| c == '\n').count();

        let mut seen_targets = HashSet::new();
        let mut result = String::with_capacity(text.len());

        // Process line-by-line to detect section boundaries
        for line in text.lines() {
            // Reset seen_targets when encountering a heading
            if heading_re.is_match(line) {
                seen_targets.clear();
            }

            // Process wikilinks in this line
            let processed_line = link_re.replace_all(line, |caps: &regex::Captures| {
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
            });

            result.push_str(&processed_line);
            result.push('\n');
        }

        // Remove all trailing newlines
        while result.ends_with('\n') {
            result.pop();
        }

        // Re-append exactly the original count of trailing newlines
        for _ in 0..trailing_newlines {
            result.push('\n');
        }

        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
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
    fn classification(&self) -> FixClassification {
        FixClassification::StyleSensitive
    }
    fn min_tier(&self) -> u8 {
        2
    }
    fn apply<'a>(&self, text: &'a str, _ctx: &FixContext) -> Cow<'a, str> {
        static ENDASH_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();

        // Replace non-breaking spaces (U+00A0) with regular spaces
        // But preserve them before certain punctuation (;:!?»)
        if result.contains('\u{00A0}') {
            let chars: Vec<char> = result.chars().collect();
            let mut new_result = String::with_capacity(result.len());
            for (i, &c) in chars.iter().enumerate() {
                if c == '\u{00A0}' {
                    // Check if next char is punctuation to preserve
                    let next_is_punct = i + 1 < chars.len()
                        && matches!(chars[i + 1], ';' | ':' | '!' | '?' | '»');
                    if next_is_punct {
                        new_result.push('\u{00A0}');
                    } else {
                        new_result.push(' ');
                    }
                } else {
                    new_result.push(c);
                }
            }
            result = new_result;
        }

        // Normalize en-dash (–) in number ranges to consistent format
        // Match patterns like "2020–2021" or "pp. 10–15"
        let endash_re = ENDASH_RE
            .get_or_init(|| regex::Regex::new(r"(\d)\s*[–—]\s*(\d)").expect("known-valid regex"));
        result = endash_re.replace_all(&result, "$1–$2").into_owned();

        // Fix curly quotes to straight quotes INSIDE templates only (template-safe)
        // Use brace-depth tracking to avoid modifying prose quotes
        let chars: Vec<char> = result.chars().collect();
        let mut new_result = String::with_capacity(result.len());
        let mut brace_depth = 0;

        for &c in &chars {
            match c {
                '{' => {
                    brace_depth += 1;
                    new_result.push(c);
                }
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                    new_result.push(c);
                }
                '\u{201C}' | '\u{201D}' if brace_depth > 0 => {
                    // Left/right double curly quotes -> straight double quote (inside templates only)
                    new_result.push('"');
                }
                '\u{2018}' | '\u{2019}' if brace_depth > 0 => {
                    // Left/right single curly quotes -> straight single quote (inside templates only)
                    new_result.push('\'');
                }
                _ => {
                    new_result.push(c);
                }
            }
        }
        result = new_result;

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
    fn classification(&self) -> FixClassification {
        FixClassification::Maintenance
    }
    fn min_tier(&self) -> u8 {
        1
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
            Cow::Owned(format!(
                "{}\n{{{{DEFAULTSORT:{}}}}}\n",
                text.trim_end(),
                sort_key
            ))
        }
    }
}

// Helper function to convert diacritics to ASCII equivalents
fn ascii_fold(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => result.push('a'),
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => result.push('A'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => result.push('e'),
            'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => result.push('E'),
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => result.push('i'),
            'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => result.push('I'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => result.push('o'),
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => result.push('O'),
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => result.push('u'),
            'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => result.push('U'),
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => result.push('c'),
            'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => result.push('C'),
            'ñ' | 'ń' | 'ņ' | 'ň' => result.push('n'),
            'Ñ' | 'Ń' | 'Ņ' | 'Ň' => result.push('N'),
            'ý' | 'ÿ' | 'ŷ' => result.push('y'),
            'Ý' | 'Ÿ' | 'Ŷ' => result.push('Y'),
            // Polish
            'ł' => result.push('l'),
            'Ł' => result.push('L'),
            'ś' => result.push('s'),
            'Ś' => result.push('S'),
            'ź' => result.push('z'),
            'Ź' => result.push('Z'),
            'ż' => result.push('z'),
            'Ż' => result.push('Z'),
            // Turkish
            'ğ' => result.push('g'),
            'Ğ' => result.push('G'),
            'ş' => result.push('s'),
            'Ş' => result.push('S'),
            // Other extended Latin
            'đ' => result.push('d'),
            'Đ' => result.push('D'),
            'þ' => result.push_str("th"),
            'Þ' => result.push_str("Th"),
            'ð' => result.push('d'),
            'Ð' => result.push('D'),
            'ř' => result.push('r'),
            'Ř' => result.push('R'),
            'ž' => result.push('z'),
            'Ž' => result.push('Z'),
            'š' => result.push('s'),
            'Š' => result.push('S'),
            // Ligatures and special
            'ß' => result.push_str("ss"),
            'æ' => result.push_str("ae"),
            'Æ' => result.push_str("Ae"),
            'œ' => result.push_str("oe"),
            'Œ' => result.push_str("Oe"),
            _ => result.push(c),
        }
    }
    result
}

/// Normalize a category title for sorting: underscores to spaces,
/// first character uppercase (MediaWiki convention), then lowercase for comparison.
fn normalize_category_title(title: &str) -> String {
    let s = title.trim().replace('_', " ");
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = String::with_capacity(s.len());
            for c in first.to_uppercase() {
                result.push(c);
            }
            for c in chars {
                result.extend(c.to_lowercase());
            }
            result
        }
    }
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

    // --- HeadingSpacing Tests ---

    #[test]
    fn test_heading_spacing_adds_blank_line() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");

        let input = "Some text\n== Heading ==\nMore text";
        let result = fix.apply(input, &ctx);

        assert_eq!(result.as_ref(), "Some text\n\n== Heading ==\nMore text");
    }

    #[test]
    fn test_heading_spacing_at_page_start() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");

        let input = "\n== Heading ==\nContent";
        let result = fix.apply(input, &ctx);

        // No cosmetic edit at BOS - input already has blank line, leave unchanged
        assert_eq!(result.as_ref(), input);
    }

    #[test]
    fn test_heading_spacing_already_has_blank_line() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");

        let input = "Some text\n\n== Heading ==\nMore text";
        let result = fix.apply(input, &ctx);

        // Should not change if already has blank line
        assert_eq!(result.as_ref(), input);
    }

    #[test]
    fn test_heading_spacing_multiple_headings() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");

        let input = "Text\n== H1 ==\nMore\n=== H2 ===\nEven more";
        let result = fix.apply(input, &ctx);

        assert_eq!(result.as_ref(), "Text\n\n== H1 ==\nMore\n\n=== H2 ===\nEven more");
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

        assert!(
            result
                .as_ref()
                .starts_with("[[Python (programming language)|Python]]")
        );
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
    }

    #[test]
    fn test_ascii_fold_polish() {
        assert_eq!(ascii_fold("Łódź"), "Lodz");
        assert_eq!(ascii_fold("Kraków"), "Krakow");
        assert_eq!(ascii_fold("Ślązak"), "Slazak");
        assert_eq!(ascii_fold("Żółć"), "Zolc");
    }

    #[test]
    fn test_ascii_fold_turkish() {
        assert_eq!(ascii_fold("İstanbul"), "Istanbul");
        assert_eq!(ascii_fold("Dağ"), "Dag");
        assert_eq!(ascii_fold("Şişli"), "Sisli");
        assert_eq!(ascii_fold("ışık"), "isik");
    }

    #[test]
    fn test_ascii_fold_extended_latin() {
        assert_eq!(ascii_fold("Øresund"), "Oresund");
        assert_eq!(ascii_fold("Đorđe"), "Dorde");
        assert_eq!(ascii_fold("Þórr"), "Thorr");
        assert_eq!(ascii_fold("Ðað"), "Dad");
        assert_eq!(ascii_fold("Řeka"), "Reka");
        assert_eq!(ascii_fold("Žižek"), "Zizek");
        assert_eq!(ascii_fold("Čech"), "Cech");
        assert_eq!(ascii_fold("Šíp"), "Sip");
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

    // --- WhitespaceCleanup Tests ---

    #[test]
    fn test_whitespace_cleanup_empty_input() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let result = fix.apply("", &ctx);
        assert_eq!(result.as_ref(), "");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_whitespace_cleanup_crlf_normalized() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let result = fix.apply("line1\r\nline2\r\n", &ctx);
        assert_eq!(result.as_ref(), "line1\nline2\n");
    }

    #[test]
    fn test_whitespace_cleanup_bare_cr() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let result = fix.apply("line1\rline2\r", &ctx);
        assert_eq!(result.as_ref(), "line1\nline2\n");
    }

    #[test]
    fn test_whitespace_cleanup_excessive_blank_lines() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let input = "line1\n\n\n\n\nline2\n";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), "line1\n\n\nline2\n");
    }

    #[test]
    fn test_whitespace_cleanup_trailing_spaces() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let input = "line1   \nline2\t\n";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), "line1\nline2\n");
    }

    #[test]
    fn test_whitespace_cleanup_no_trailing_newline() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let input = "line1\nline2";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), "line1\nline2\n");
    }

    #[test]
    fn test_whitespace_cleanup_already_clean() {
        let fix = WhitespaceCleanup;
        let ctx = test_context("Test");
        let input = "line1\nline2\n";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), input);
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    // --- HeadingSpacing additional tests ---

    #[test]
    fn test_heading_spacing_at_absolute_bos_unchanged() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");
        // Heading at very start of text (i==0) should be left alone
        let input = "== Heading ==\nContent";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), input);
    }

    #[test]
    fn test_heading_spacing_multiple_blank_lines_stable() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");
        let input = "Text\n\n\n== Heading ==\nMore";
        let result = fix.apply(input, &ctx);
        // Already has blank line(s), should not add more
        assert_eq!(result.as_ref(), input);
    }

    #[test]
    fn test_heading_spacing_bos_no_leading_newline() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");
        // Heading at absolute BOS - should NOT add a leading newline
        let input = "== Heading ==";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), input, "Should not add newline at BOS");
    }

    #[test]
    fn test_heading_spacing_bos_with_preceding_content() {
        let fix = HeadingSpacing;
        let ctx = test_context("Test");
        // Actual content before heading - SHOULD add blank line
        let input = "Some text\n== Heading ==";
        let result = fix.apply(input, &ctx);
        assert_eq!(result.as_ref(), "Some text\n\n== Heading ==");
    }

    // --- CategorySorting additional tests ---

    #[test]
    fn test_category_sorting_with_sort_keys() {
        let fix = CategorySorting;
        let ctx = test_context("Test");
        let input = "text\n[[Category:Zebra|Aaa]]\n[[Category:Apple|Zzz]]\n";
        let result = fix.apply(input, &ctx);
        // Should sort by normalized title: Apple < Zebra
        assert!(result.as_ref().find("[[Category:Apple|Zzz]]").unwrap()
            < result.as_ref().find("[[Category:Zebra|Aaa]]").unwrap());
    }

    #[test]
    fn test_category_sorting_placeholder_collision() {
        let fix = CategorySorting;
        let ctx = test_context("Test");
        let input = "text with \x02AWB_SORT_PLACEHOLDER\x02 in it\n[[Category:B]]\n[[Category:A]]\n";
        let result = fix.apply(input, &ctx);
        // Should return original text unchanged (fail closed)
        assert_eq!(result.as_ref(), input);
    }

    #[test]
    fn test_category_sorting_no_silent_deletion() {
        let fix = CategorySorting;
        let ctx = test_context("Test");
        let input = "text\n[[Category:B]]\n[[Category:A]]\n";
        let result = fix.apply(input, &ctx);
        // Both categories must still be present
        assert!(result.as_ref().contains("[[Category:A]]"));
        assert!(result.as_ref().contains("[[Category:B]]"));
    }

    #[test]
    fn test_category_sorting_underscore_normalization() {
        let fix = CategorySorting;
        let ctx = test_context("Test");
        let input = "[[Category:Foo_bar]]\n[[Category:Aaa]]\n";
        let result = fix.apply(input, &ctx);
        assert!(result.as_ref().find("[[Category:Aaa]]").unwrap()
            < result.as_ref().find("[[Category:Foo_bar]]").unwrap());
    }

    // --- Property-based tests for idempotency ---

    // --- apply_all_with_config Tests ---

    #[test]
    fn test_tier0_only_runs_tier0_fixes() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 0,
            allow_cosmetic_only: true,
            ..Default::default()
        };
        // Input has HTML that tier-1 HtmlToWikitext would fix, and trailing whitespace (tier 0)
        let input = "line   \n<b>bold</b>\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        // Tier 0 fixes should run (whitespace cleanup removes trailing spaces)
        // Tier 1 HtmlToWikitext should NOT run
        assert!(result.final_text.contains("<b>bold</b>"), "HtmlToWikitext should not run at tier 0");
        assert!(!result.changed_ids.contains(&"html_to_wikitext".to_string()));
    }

    #[test]
    fn test_tier1_includes_tier0_and_tier1() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 1,
            allow_cosmetic_only: true,
            ..Default::default()
        };
        let input = "line   \n<b>bold</b>\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        // Both tier 0 and tier 1 should run
        assert!(result.final_text.contains("'''bold'''"), "HtmlToWikitext should run at tier 1");
    }

    #[test]
    fn test_tier2_includes_citations() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 2,
            allow_cosmetic_only: true,
            ..Default::default()
        };
        let input = "{{Cite Web|accessdate=2021-01-01}}\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        assert!(result.final_text.contains("access-date="));
        assert!(result.changed_ids.contains(&"citation_formatting".to_string()));
    }

    #[test]
    fn test_unknown_id_in_config_errors() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            enabled_fixes: ["nonexistent_fix".to_string()].into_iter().collect(),
            ..Default::default()
        };
        let result = registry.apply_all_with_config("text\n", &ctx, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_disabled_fixes_skipped() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 2,
            disabled_fixes: ["citation_formatting".to_string()].into_iter().collect(),
            allow_cosmetic_only: true,
            ..Default::default()
        };
        let input = "{{Cite Web|accessdate=2021-01-01}}\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        // Citation formatting disabled, so accessdate should remain
        assert!(result.final_text.contains("accessdate="));
    }

    #[test]
    fn test_enabled_fixes_whitelist() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 2,
            enabled_fixes: ["whitespace_cleanup".to_string()].into_iter().collect(),
            allow_cosmetic_only: true,
            ..Default::default()
        };
        let input = "line   \n<b>bold</b>\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        // Only whitespace_cleanup should run
        assert!(result.final_text.contains("<b>bold</b>"));
        for id in &result.changed_ids {
            assert_eq!(id, "whitespace_cleanup");
        }
    }

    #[test]
    fn test_cosmetic_only_detection() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 0,
            allow_cosmetic_only: true,
            ..Default::default()
        };
        // Only cosmetic changes (trailing whitespace)
        let input = "line   \n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        assert!(result.is_cosmetic_only);
    }

    #[test]
    fn test_non_cosmetic_with_maintenance() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 1,
            allow_cosmetic_only: true,
            ..Default::default()
        };
        let input = "<b>bold</b>\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        // HtmlToWikitext is Maintenance, not Cosmetic
        assert!(!result.is_cosmetic_only);
    }

    #[test]
    fn test_no_changes_not_cosmetic_only() {
        let registry = FixRegistry::with_defaults();
        let ctx = test_context("Test");
        let config = FixConfig {
            strictness_tier: 0,
            allow_cosmetic_only: true,
            ..Default::default()
        };
        let input = "clean text\n";
        let result = registry.apply_all_with_config(input, &ctx, &config).unwrap();
        assert!(!result.is_cosmetic_only);
        assert!(result.changed_ids.is_empty());
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_wikitext() -> impl Strategy<Value = String> {
            // Generate wikitext with various elements that the fixes handle
            // Include HTML tags, categories, headings, diacritics
            prop::string::string_regex(
                r"([A-Za-z0-9 \n\[\]{}|:=/.\-àáâãäåéèêëíìîïóòôõöúùûüçñß<>/]|==[^=]+==|\[\[Category:[A-Za-z ]+\]\]|<b>[^<]*</b>|<i>[^<]*</i>|\u{00A0}){0,300}"
            ).unwrap()
        }

        fn test_ctx() -> FixContext {
            FixContext {
                title: Title::new(Namespace::MAIN, "Test Article"),
                namespace: Namespace::MAIN,
                is_redirect: false,
            }
        }

        proptest! {
            #[test]
            fn whitespace_cleanup_idempotent(input in arb_wikitext()) {
                let fix = WhitespaceCleanup;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "WhitespaceCleanup not idempotent");
            }

            #[test]
            fn heading_spacing_idempotent(input in arb_wikitext()) {
                let fix = HeadingSpacing;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "HeadingSpacing not idempotent");
            }

            #[test]
            fn html_to_wikitext_idempotent(input in arb_wikitext()) {
                let fix = HtmlToWikitext;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "HtmlToWikitext not idempotent");
            }

            #[test]
            fn trailing_whitespace_idempotent(input in arb_wikitext()) {
                let fix = TrailingWhitespace;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "TrailingWhitespace not idempotent");
            }

            #[test]
            fn category_sorting_idempotent(input in arb_wikitext()) {
                let fix = CategorySorting;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "CategorySorting not idempotent");
            }

            #[test]
            fn citation_formatting_idempotent(input in arb_wikitext()) {
                let fix = CitationFormatting;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "CitationFormatting not idempotent");
            }

            #[test]
            fn unicode_normalization_idempotent(input in arb_wikitext()) {
                let fix = UnicodeNormalization;
                let ctx = test_ctx();
                let once = fix.apply(&input, &ctx).into_owned();
                let twice = fix.apply(&once, &ctx).into_owned();
                prop_assert_eq!(&once, &twice, "UnicodeNormalization not idempotent");
            }
        }
    }
}
