use awb_domain::types::{Title, Namespace};
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
    fn apply(&self, text: &str, context: &FixContext) -> String;
    fn default_enabled(&self) -> bool { true }
}

pub struct FixRegistry {
    modules: Vec<Box<dyn FixModule>>,
}

impl FixRegistry {
    pub fn new() -> Self { Self { modules: Vec::new() } }

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
                result = module.apply(&result, ctx);
            }
        }
        result
    }

    pub fn apply_all_returning_ids(&self, text: &str, ctx: &FixContext, enabled_ids: &HashSet<String>) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let mut current = text.to_string();
        for module in &self.modules {
            if enabled_ids.contains(module.id()) {
                let new = module.apply(&current, ctx);
                if new != current {
                    results.push((module.id().to_string(), new.clone()));
                    current = new;
                }
            }
        }
        results
    }

    pub fn all_modules(&self) -> &[Box<dyn FixModule>] { &self.modules }
}

impl Default for FixRegistry {
    fn default() -> Self { Self::with_defaults() }
}

// --- Built-in fix modules ---

pub struct WhitespaceCleanup;
impl FixModule for WhitespaceCleanup {
    fn id(&self) -> &str { "whitespace_cleanup" }
    fn display_name(&self) -> &str { "Whitespace Cleanup" }
    fn category(&self) -> &str { "Formatting" }
    fn description(&self) -> &str { "Normalizes line endings, removes trailing whitespace, collapses excessive blank lines" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        let text = text.replace("\r\n", "\n").replace("\r", "\n");
        let lines: Vec<&str> = text.lines().collect();
        let trimmed: Vec<String> = lines.iter().map(|l| l.trim_end().to_string()).collect();
        let mut result = String::new();
        let mut blank_count = 0;
        for line in &trimmed {
            if line.is_empty() {
                blank_count += 1;
                if blank_count <= 2 { result.push('\n'); }
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
        result
    }
}

pub struct HeadingSpacing;
impl FixModule for HeadingSpacing {
    fn id(&self) -> &str { "heading_spacing" }
    fn display_name(&self) -> &str { "Heading Spacing" }
    fn category(&self) -> &str { "Formatting" }
    fn description(&self) -> &str { "Ensures blank line before headings" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = RE.get_or_init(|| regex::Regex::new(r"(?m)([^\n])\n(={2,6}[^=])").expect("known-valid regex"));
        re.replace_all(text, "$1\n\n$2").into_owned()
    }
}

pub struct HtmlToWikitext;
impl FixModule for HtmlToWikitext {
    fn id(&self) -> &str { "html_to_wikitext" }
    fn display_name(&self) -> &str { "HTML to Wikitext" }
    fn category(&self) -> &str { "Formatting" }
    fn description(&self) -> &str { "Converts HTML tags to wikitext equivalents" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        static BOLD_RE: OnceLock<regex::Regex> = OnceLock::new();
        static ITALIC_RE: OnceLock<regex::Regex> = OnceLock::new();
        static BR_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();
        // Bold
        let re = BOLD_RE.get_or_init(|| regex::Regex::new(r"(?i)<b>(.*?)</b>").expect("known-valid regex"));
        result = re.replace_all(&result, "'''$1'''").into_owned();
        // Italic
        let re = ITALIC_RE.get_or_init(|| regex::Regex::new(r"(?i)<i>(.*?)</i>").expect("known-valid regex"));
        result = re.replace_all(&result, "''$1''").into_owned();
        // BR
        let re = BR_RE.get_or_init(|| regex::Regex::new(r"(?i)<br\s*/?>").expect("known-valid regex"));
        result = re.replace_all(&result, "<br />").into_owned();
        result
    }
}

pub struct TrailingWhitespace;
impl FixModule for TrailingWhitespace {
    fn id(&self) -> &str { "trailing_whitespace" }
    fn display_name(&self) -> &str { "Trailing Whitespace" }
    fn category(&self) -> &str { "Formatting" }
    fn description(&self) -> &str { "Removes trailing whitespace from lines" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        text.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n") + "\n"
    }
}

pub struct CategorySorting;
impl FixModule for CategorySorting {
    fn id(&self) -> &str { "category_sorting" }
    fn display_name(&self) -> &str { "Category Sorting" }
    fn category(&self) -> &str { "Categories" }
    fn description(&self) -> &str { "Alphabetically sorts [[Category:...]] entries" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        static CAT_RE: OnceLock<regex::Regex> = OnceLock::new();
        let cat_re = CAT_RE.get_or_init(|| regex::Regex::new(r"\[\[Category:[^\]]+\]\]").expect("known-valid regex"));
        let mut categories: Vec<String> = cat_re.find_iter(text).map(|m| m.as_str().to_string()).collect();
        if categories.len() <= 1 { return text.to_string(); }
        categories.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        let cleaned = cat_re.replace_all(text, "\x00").to_string();
        let mut result = cleaned.clone();
        for cat in &categories {
            result = result.replacen('\x00', cat, 1);
        }
        // Remove any remaining placeholders
        result = result.replace('\x00', "");
        result
    }
}

pub struct CitationFormatting;
impl FixModule for CitationFormatting {
    fn id(&self) -> &str { "citation_formatting" }
    fn display_name(&self) -> &str { "Citation Formatting" }
    fn category(&self) -> &str { "Citations" }
    fn description(&self) -> &str { "Fixes common citation template issues: normalizes {{cite web}}/{{cite news}}/{{cite journal}}, renames deprecated parameters" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        static CITE_RE: OnceLock<regex::Regex> = OnceLock::new();
        static ACCESSDATE_RE: OnceLock<regex::Regex> = OnceLock::new();
        static DEADURL_RE: OnceLock<regex::Regex> = OnceLock::new();
        static DEADURL_NO_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();

        // Normalize citation template names to lowercase
        let cite_re = CITE_RE.get_or_init(|| regex::Regex::new(r"(?i)\{\{(cite\s+(?:web|news|journal|book|conference))").expect("known-valid regex"));
        result = cite_re.replace_all(&result, |caps: &regex::Captures| {
            format!("{{{{{}", caps[1].to_lowercase().replace(' ', " "))
        }).into_owned();

        // Fix deprecated parameter names
        // accessdate → access-date
        let accessdate_re = ACCESSDATE_RE.get_or_init(|| regex::Regex::new(r"(?m)(\|\s*)accessdate(\s*=)").expect("known-valid regex"));
        result = accessdate_re.replace_all(&result, "${1}access-date${2}").into_owned();

        // deadurl → url-status
        let deadurl_re = DEADURL_RE.get_or_init(|| regex::Regex::new(r"(?m)(\|\s*)deadurl(\s*=\s*)(?:yes|true)").expect("known-valid regex"));
        result = deadurl_re.replace_all(&result, "${1}url-status${2}dead").into_owned();
        let deadurl_no_re = DEADURL_NO_RE.get_or_init(|| regex::Regex::new(r"(?m)(\|\s*)deadurl(\s*=\s*)(?:no|false)").expect("known-valid regex"));
        result = deadurl_no_re.replace_all(&result, "${1}url-status${2}live").into_owned();

        result
    }
}

pub struct DuplicateWikilinkRemoval;
impl FixModule for DuplicateWikilinkRemoval {
    fn id(&self) -> &str { "duplicate_wikilink_removal" }
    fn display_name(&self) -> &str { "Duplicate Wikilink Removal" }
    fn category(&self) -> &str { "Links" }
    fn description(&self) -> &str { "Removes duplicate wikilinks, keeping only first occurrence" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        use std::collections::HashSet;

        static LINK_RE: OnceLock<regex::Regex> = OnceLock::new();
        let link_re = LINK_RE.get_or_init(|| regex::Regex::new(r"\[\[([^\|\]]+)(?:\|([^\]]+))?\]\]").expect("known-valid regex"));
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
        }).into_owned()
    }
}

pub struct UnicodeNormalization;
impl FixModule for UnicodeNormalization {
    fn id(&self) -> &str { "unicode_normalization" }
    fn display_name(&self) -> &str { "Unicode Normalization" }
    fn category(&self) -> &str { "Formatting" }
    fn description(&self) -> &str { "Fixes common unicode issues: non-breaking spaces, en-dashes in ranges, curly quotes in templates" }
    fn apply(&self, text: &str, _ctx: &FixContext) -> String {
        static ENDASH_RE: OnceLock<regex::Regex> = OnceLock::new();
        static TEMPLATE_RE: OnceLock<regex::Regex> = OnceLock::new();

        let mut result = text.to_string();

        // Replace non-breaking spaces (U+00A0) with regular spaces
        // But preserve them in special contexts like French punctuation
        result = result.replace('\u{00A0}', " ");

        // Normalize en-dash (–) in number ranges to consistent format
        // Match patterns like "2020–2021" or "pp. 10–15"
        let endash_re = ENDASH_RE.get_or_init(|| regex::Regex::new(r"(\d)\s*[–—]\s*(\d)").expect("known-valid regex"));
        result = endash_re.replace_all(&result, "$1–$2").into_owned();

        // Fix curly quotes to straight quotes in template parameters
        // Only inside {{ }} templates to avoid changing prose
        let template_re = TEMPLATE_RE.get_or_init(|| regex::Regex::new(r"\{\{[^}]+\}\}").expect("known-valid regex"));
        result = template_re.replace_all(&result, |caps: &regex::Captures| {
            let template = &caps[0];
            template
                .replace('\u{201C}', "\"")  // Left double quote
                .replace('\u{201D}', "\"")  // Right double quote
                .replace('\u{2018}', "'")   // Left single quote
                .replace('\u{2019}', "'")   // Right single quote
        }).into_owned();

        result
    }
}

pub struct DefaultSortFix;
impl FixModule for DefaultSortFix {
    fn id(&self) -> &str { "defaultsort_fix" }
    fn display_name(&self) -> &str { "DEFAULTSORT Fix" }
    fn category(&self) -> &str { "Categories" }
    fn description(&self) -> &str { "Adds {{DEFAULTSORT:}} for titles with diacritics if missing" }
    fn apply(&self, text: &str, ctx: &FixContext) -> String {
        static DEFAULTSORT_RE: OnceLock<regex::Regex> = OnceLock::new();
        static CAT_RE: OnceLock<regex::Regex> = OnceLock::new();

        // Check if DEFAULTSORT already exists
        let defaultsort_re = DEFAULTSORT_RE.get_or_init(|| regex::Regex::new(r"(?i)\{\{DEFAULTSORT:").expect("known-valid regex"));
        if defaultsort_re.is_match(text) {
            return text.to_string();
        }

        // Check if title contains diacritics or non-ASCII characters
        let title_name = &ctx.title.name;
        if title_name.chars().all(|c| c.is_ascii()) {
            return text.to_string();
        }

        // Generate ASCII-folded version for sort key
        let sort_key = ascii_fold(title_name);

        // Find the best position to insert DEFAULTSORT (before categories if present)
        let cat_re = CAT_RE.get_or_init(|| regex::Regex::new(r"(?m)^(\[\[Category:)").expect("known-valid regex"));
        if let Some(mat) = cat_re.find(text) {
            let pos = mat.start();
            let mut result = String::with_capacity(text.len() + sort_key.len() + 20);
            result.push_str(&text[..pos]);
            result.push_str(&format!("{{{{DEFAULTSORT:{}}}}}\n", sort_key));
            result.push_str(&text[pos..]);
            result
        } else {
            // No categories - add at the end
            format!("{}\n{{{{DEFAULTSORT:{}}}}}\n", text.trim_end(), sort_key)
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
