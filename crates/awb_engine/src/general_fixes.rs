use awb_domain::types::{Title, Namespace};
use std::collections::HashSet;

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
        let re = regex::Regex::new(r"(?m)([^\n])\n(={2,6}[^=])").unwrap();
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
        let mut result = text.to_string();
        // Bold
        let re = regex::Regex::new(r"(?i)<b>(.*?)</b>").unwrap();
        result = re.replace_all(&result, "'''$1'''").into_owned();
        // Italic
        let re = regex::Regex::new(r"(?i)<i>(.*?)</i>").unwrap();
        result = re.replace_all(&result, "''$1''").into_owned();
        // BR
        let re = regex::Regex::new(r"(?i)<br\s*/?>").unwrap();
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
        let cat_re = regex::Regex::new(r"\[\[Category:[^\]]+\]\]").unwrap();
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
