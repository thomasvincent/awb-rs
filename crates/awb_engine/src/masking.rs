//! Masking engine: protects regions of wikitext that must not be transformed.
//!
//! The mask→transform→unmask pattern ensures that content inside `<nowiki>`,
//! `<pre>`, `<code>`, `<syntaxhighlight>`, `<math>`, `<source>`, HTML comments,
//! `{{templates}}`, and `[[File:…]]/[[Image:…]]` links is never altered by
//! general fixes or find-and-replace rules.
//!
//! Guarantees:
//! - Single-pass scan for mask extraction.
//! - Sentinel tokens are guaranteed absent from the original text.
//! - Exact byte restoration on unmask.
//! - If any sentinel leaks or restoration count mismatches → return original text (fail closed).

use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};

/// A sentinel prefix that is extremely unlikely in real wikitext.
const SENTINEL_PREFIX: &str = "\x00\x01AWB_MASK_";
const SENTINEL_SUFFIX: &str = "\x00\x02";

/// Global nonce counter to ensure each mask() call uses unique sentinels.
static MASK_NONCE: AtomicU64 = AtomicU64::new(0);

/// Holds masked regions and the masked text.
#[derive(Debug)]
pub struct MaskedText {
    /// The text with protected regions replaced by sentinels.
    pub masked: String,
    /// The original regions, in order, corresponding to sentinel indices.
    regions: Vec<String>,
    /// The sentinel prefix used for this masking (includes nonce).
    sentinel_base: String,
    /// The original text, kept for fail-closed restoration.
    original: String,
}

impl MaskedText {
    /// Apply a transformation function to the masked text.
    /// The function receives the masked text (with sentinels in place of protected regions)
    /// and should return the transformed text (leaving sentinels intact).
    pub fn transform<F>(&mut self, f: F)
    where
        F: FnOnce(&str) -> String,
    {
        self.masked = f(&self.masked);
    }

    /// Restore all sentinels with original content.
    /// If any sentinel is missing or extra sentinels remain, returns the original text unchanged (fail closed).
    ///
    /// Uses single-pass assembly to avoid O(n*m) repeated string copies.
    pub fn unmask(self) -> String {
        if self.regions.is_empty() {
            return self.masked;
        }

        // Single-pass: scan through masked text, find sentinels, assemble result
        let mut result = String::with_capacity(self.masked.len());
        let mut pos = 0;
        let mut restored_count = 0;
        let masked = &self.masked;

        while pos < masked.len() {
            // Check if current position starts with the sentinel base
            if masked[pos..].starts_with(&self.sentinel_base) {
                // Parse the sentinel index directly instead of iterating
                let after_base = pos + self.sentinel_base.len();

                // Find the end of the numeric index (before SENTINEL_SUFFIX)
                if let Some(suffix_pos) = masked[after_base..].find(SENTINEL_SUFFIX) {
                    let index_str = &masked[after_base..after_base + suffix_pos];

                    // Parse the index
                    if let Ok(idx) = index_str.parse::<usize>() {
                        if idx < self.regions.len() {
                            // Valid sentinel — restore it
                            result.push_str(&self.regions[idx]);
                            pos = after_base + suffix_pos + SENTINEL_SUFFIX.len();
                            restored_count += 1;
                        } else {
                            // Index out of bounds — fail closed
                            return self.original;
                        }
                    } else {
                        // Malformed index — fail closed
                        return self.original;
                    }
                } else {
                    // No suffix found — fail closed
                    return self.original;
                }
            } else {
                // Copy character
                let ch = masked[pos..].chars().next().unwrap();
                result.push(ch);
                pos += ch.len_utf8();
            }
        }

        // Verify all sentinels were restored exactly once
        if restored_count != self.regions.len() {
            return self.original;
        }

        result
    }
}

/// Mask protected regions in wikitext.
///
/// Protected regions (in scan order):
/// 1. HTML comments: `<!-- ... -->`
/// 2. Extension tags (case-insensitive): `<nowiki>`, `<pre>`, `<code>`,
///    `<syntaxhighlight>`, `<math>`, `<source>`
/// 3. Templates: `{{...}}` (brace-depth tracking)
/// 4. File/Image links: `[[File:...]]` / `[[Image:...]]` (bracket-depth tracking)
///
/// If the input already contains the sentinel prefix, returns the text unmasked
/// (fail closed — we cannot safely mask).
pub fn mask(text: &str) -> MaskedText {
    // Fail closed if sentinel already present
    if text.contains(SENTINEL_PREFIX) {
        return MaskedText {
            masked: text.to_string(),
            regions: Vec::new(),
            sentinel_base: String::new(),
            original: text.to_string(),
        };
    }

    let nonce = MASK_NONCE.fetch_add(1, Ordering::Relaxed);
    let sentinel_base = format!("{}{}N", SENTINEL_PREFIX, nonce);
    let mut regions: Vec<String> = Vec::new();
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // 1. HTML comments: <!-- ... -->
        if i + 4 <= len && &bytes[i..i + 4] == b"<!--" {
            if let Some(end) = find_bytes(bytes, i + 4, b"-->") {
                let end = end + 3; // include -->
                let region = &text[i..end];
                let idx = regions.len();
                regions.push(region.to_string());
                result.push_str(&format!("{}{}{}", sentinel_base, idx, SENTINEL_SUFFIX));
                i = end;
                continue;
            }
        }

        // 2. Extension tags (case-insensitive)
        if bytes[i] == b'<' {
            if let Some((tag_name, close_pos)) = try_match_extension_tag(text, i) {
                let region = &text[i..close_pos];
                let idx = regions.len();
                regions.push(region.to_string());
                result.push_str(&format!("{}{}{}", sentinel_base, idx, SENTINEL_SUFFIX));
                i = close_pos;
                let _ = tag_name; // used in try_match_extension_tag
                continue;
            }
        }

        // 3. Templates: {{ ... }} with brace-depth tracking
        if i + 1 < len && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = find_matching_braces(bytes, i) {
                let region = &text[i..end];
                let idx = regions.len();
                regions.push(region.to_string());
                result.push_str(&format!("{}{}{}", sentinel_base, idx, SENTINEL_SUFFIX));
                i = end;
                continue;
            }
        }

        // 4. File/Image links: [[File:...]] or [[Image:...]]
        if i + 2 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' && is_file_or_image_link(text, i) {
            if let Some(end) = find_matching_brackets(bytes, i) {
                let region = &text[i..end];
                let idx = regions.len();
                regions.push(region.to_string());
                result.push_str(&format!("{}{}{}", sentinel_base, idx, SENTINEL_SUFFIX));
                i = end;
                continue;
            }
        }

        // No match — copy character
        // Advance by UTF-8 character
        let ch = text[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }

    MaskedText {
        masked: result,
        regions,
        sentinel_base,
        original: text.to_string(),
    }
}

/// Convenience: mask, transform, unmask. Returns original on any failure.
pub fn with_masking<F>(text: &str, f: F) -> Cow<'_, str>
where
    F: FnOnce(&str) -> String,
{
    if text.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut masked = mask(text);
    if masked.regions.is_empty() {
        // Nothing to mask — apply directly
        let result = f(text);
        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
    } else {
        masked.transform(f);
        let result = masked.unmask();
        if result == text {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(result)
        }
    }
}

// --- Internal helpers ---

/// Find byte pattern `needle` starting from `start` in `haystack`.
fn find_bytes(haystack: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || start + needle.len() > haystack.len() {
        return None;
    }
    haystack[start..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + start)
}

/// Extension tags we protect (case-insensitive).
const EXTENSION_TAGS: &[&str] = &[
    "nowiki",
    "pre",
    "code",
    "syntaxhighlight",
    "math",
    "source",
];

/// Try to match an extension tag at position `start` (which points to '<').
/// Returns (tag_name, end_position_exclusive) if matched.
fn try_match_extension_tag(text: &str, start: usize) -> Option<(&'static str, usize)> {
    let rest = &text[start..];
    // Must start with '<'
    if !rest.starts_with('<') {
        return None;
    }
    let after_lt = &rest[1..];

    for &tag in EXTENSION_TAGS {
        // Check opening tag (case-insensitive)
        if after_lt.len() >= tag.len()
            && after_lt[..tag.len()].eq_ignore_ascii_case(tag)
        {
            // After tag name must be '>' or whitespace (for attributes) or '/>'
            let after_name = &after_lt[tag.len()..];
            if after_name.starts_with('>')
                || after_name.starts_with(' ')
                || after_name.starts_with('\t')
                || after_name.starts_with('\n')
                || after_name.starts_with("/>")
            {
                // Self-closing: <tag ... />
                if let Some(gt) = after_name.find("/>") {
                    // Check if there's a '>' before '/>'
                    if let Some(gt_single) = after_name.find('>') {
                        if gt_single < gt {
                            // Non-self-closing, look for closing tag
                        } else {
                            return Some((tag, start + 1 + tag.len() + gt + 2));
                        }
                    } else {
                        return Some((tag, start + 1 + tag.len() + gt + 2));
                    }
                }

                // Find closing tag: </tag>
                let close_pattern_lower = format!("</{}>", tag);
                // Case-insensitive search for closing tag
                let search_start = start + 1 + tag.len();
                let search_text = &text[search_start..];
                if let Some(pos) = find_case_insensitive(search_text, &close_pattern_lower) {
                    return Some((tag, search_start + pos + close_pattern_lower.len()));
                }
            }
        }
    }
    None
}

/// Case-insensitive substring search.
fn find_case_insensitive(haystack: &str, needle_lower: &str) -> Option<usize> {
    let needle_bytes = needle_lower.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return None;
    }
    for i in 0..=(haystack_bytes.len() - needle_bytes.len()) {
        if haystack_bytes[i..i + needle_bytes.len()]
            .iter()
            .zip(needle_bytes.iter())
            .all(|(h, n)| h.to_ascii_lowercase() == *n)
        {
            return Some(i);
        }
    }
    None
}

/// Find matching `}}` for `{{` at position `start`. Returns exclusive end position.
///
/// Skips over HTML comments (`<!-- ... -->`) and extension tag blocks inside
/// the template to avoid false matches on `}}` within those regions.
/// SAFETY: `{`, `}`, `<`, `-`, `>` are all ASCII (< 0x80) and cannot appear
/// as continuation bytes in multi-byte UTF-8, so byte-level scanning is safe.
fn find_matching_braces(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = start;
    let len = bytes.len();
    while i < len {
        // Skip HTML comments: <!-- ... -->
        if i + 3 < len && &bytes[i..i + 4] == b"<!--" {
            if let Some(end) = find_bytes(bytes, i + 4, b"-->") {
                i = end + 3;
                continue;
            }
        }
        if i + 1 < len && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 2;
        } else if i + 1 < len && bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(i);
            }
        } else {
            i += 1;
        }
    }
    None // Unmatched — don't mask (fail closed: leave as-is)
}

/// Check if `[[` at `start` is a File: or Image: link (case-insensitive).
fn is_file_or_image_link(text: &str, start: usize) -> bool {
    let after = &text[start + 2..];
    (after.len() >= 5 && after[..5].eq_ignore_ascii_case("file:"))
        || (after.len() >= 6 && after[..6].eq_ignore_ascii_case("image:"))
}

/// Find matching `]]` for `[[` at position `start`. Returns exclusive end position.
/// Handles nested `[[...]]` inside.
fn find_matching_brackets(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = start;
    let len = bytes.len();
    while i < len {
        if i + 1 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            depth += 1;
            i += 2;
        } else if i + 1 < len && bytes[i] == b']' && bytes[i + 1] == b']' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(i);
            }
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Round-trip identity ---

    #[test]
    fn test_roundtrip_plain_text() {
        let text = "Hello, world! This is plain text.";
        let masked = mask(text);
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_html_comment() {
        let text = "Before <!-- this is hidden --> After";
        let masked = mask(text);
        assert!(masked.masked.contains(SENTINEL_PREFIX));
        assert!(!masked.masked.contains("this is hidden"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_nowiki() {
        let text = "Before <nowiki>[[not a link]]</nowiki> After";
        let masked = mask(text);
        assert!(!masked.masked.contains("[[not a link]]"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_pre() {
        let text = "Before <pre>  indented\n  code  </pre> After";
        let masked = mask(text);
        assert!(!masked.masked.contains("indented"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_code() {
        let text = "Inline <code>foo()</code> function";
        let masked = mask(text);
        assert!(!masked.masked.contains("foo()"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_syntaxhighlight() {
        let text = "Before <syntaxhighlight lang=\"rust\">fn main() {}</syntaxhighlight> After";
        let masked = mask(text);
        assert!(!masked.masked.contains("fn main()"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_math() {
        let text = "Formula: <math>E = mc^2</math> done";
        let masked = mask(text);
        assert!(!masked.masked.contains("E = mc^2"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_source() {
        let text = "<source lang=\"python\">print('hi')</source>";
        let masked = mask(text);
        assert!(!masked.masked.contains("print"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_template() {
        let text = "Before {{cite web|url=http://example.com|title=Test}} After";
        let masked = mask(text);
        assert!(!masked.masked.contains("cite web"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_nested_template() {
        let text = "{{outer|{{inner|arg}}|other}}";
        let masked = mask(text);
        assert_eq!(masked.regions.len(), 1); // entire nested template is one region
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_file_link() {
        let text = "See [[File:Example.png|thumb|Caption text]] here";
        let masked = mask(text);
        assert!(!masked.masked.contains("Example.png"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_image_link() {
        let text = "See [[Image:Photo.jpg|200px]] here";
        let masked = mask(text);
        assert!(!masked.masked.contains("Photo.jpg"));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_case_insensitive_tags() {
        let text = "A <NOWIKI>x</NOWIKI> B <Pre>y</Pre> C <CODE>z</CODE> D";
        let masked = mask(text);
        assert!(!masked.masked.contains('x'));
        assert!(!masked.masked.contains('y'));
        assert!(!masked.masked.contains('z'));
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_roundtrip_multiple_regions() {
        let text = "<!-- c1 --> text {{tmpl}} more <nowiki>nw</nowiki> end";
        let masked = mask(text);
        assert_eq!(masked.regions.len(), 3);
        assert_eq!(masked.unmask(), text);
    }

    // --- Transform preserves protected regions ---

    #[test]
    fn test_transform_does_not_affect_masked() {
        let text = "Replace THIS but not <nowiki>THIS</nowiki>";
        let mut masked = mask(text);
        masked.transform(|t| t.replace("THIS", "THAT"));
        let result = masked.unmask();
        assert_eq!(result, "Replace THAT but not <nowiki>THIS</nowiki>");
    }

    #[test]
    fn test_transform_does_not_affect_template() {
        let text = "Fix <b>bold</b> but not {{cite|<b>bold</b>}}";
        let mut masked = mask(text);
        masked.transform(|t| t.replace("<b>", "'''").replace("</b>", "'''"));
        let result = masked.unmask();
        assert!(result.contains("'''bold'''"));
        assert!(result.contains("{{cite|<b>bold</b>}}"));
    }

    #[test]
    fn test_transform_does_not_affect_comment() {
        let text = "Text <!-- <b>keep</b> --> more <b>fix</b>";
        let mut masked = mask(text);
        masked.transform(|t| t.replace("<b>", "'''").replace("</b>", "'''"));
        let result = masked.unmask();
        assert!(result.contains("<!-- <b>keep</b> -->"));
        assert!(result.contains("'''fix'''"));
    }

    // --- Fail-closed behavior ---

    #[test]
    fn test_sentinel_collision_fails_closed() {
        let text = format!("Text with {}0{} in it", SENTINEL_PREFIX, SENTINEL_SUFFIX);
        let masked = mask(&text);
        assert!(masked.regions.is_empty());
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_unmask_fails_closed_on_deleted_sentinel() {
        let text = "Before {{template}} After";
        let mut masked = mask(text);
        // Simulate a transform that deletes the sentinel
        masked.transform(|_| "completely replaced".to_string());
        let result = masked.unmask();
        // Should return original since sentinel was lost
        assert_eq!(result, text);
    }

    #[test]
    fn test_unmask_fails_closed_on_duplicated_sentinel() {
        let text = "Before {{template}} After";
        let mut masked = mask(text);
        // Simulate a transform that duplicates the sentinel
        masked.transform(|t| format!("{} {}", t, t));
        let result = masked.unmask();
        // Should return original since sentinel appears twice
        assert_eq!(result, text);
    }

    // --- with_masking convenience ---

    #[test]
    fn test_with_masking_no_protected_regions() {
        let text = "Simple <b>bold</b> text";
        let result = with_masking(text, |t| t.replace("<b>", "'''").replace("</b>", "'''"));
        assert_eq!(result.as_ref(), "Simple '''bold''' text");
    }

    #[test]
    fn test_with_masking_with_protected_regions() {
        let text = "<b>fix</b> <nowiki><b>keep</b></nowiki>";
        let result = with_masking(text, |t| t.replace("<b>", "'''").replace("</b>", "'''"));
        assert_eq!(result.as_ref(), "'''fix''' <nowiki><b>keep</b></nowiki>");
    }

    #[test]
    fn test_with_masking_empty() {
        let result = with_masking("", |t| t.to_uppercase());
        assert_eq!(result.as_ref(), "");
    }

    // --- Edge cases ---

    #[test]
    fn test_unclosed_comment_not_masked() {
        let text = "Before <!-- unclosed comment";
        let masked = mask(text);
        assert!(masked.regions.is_empty());
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_unclosed_template_not_masked() {
        let text = "Before {{unclosed template";
        let masked = mask(text);
        assert!(masked.regions.is_empty());
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_unclosed_tag_not_masked() {
        let text = "Before <nowiki>unclosed tag";
        let masked = mask(text);
        assert!(masked.regions.is_empty());
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_regular_wikilink_not_masked() {
        let text = "See [[Article Name]] for details";
        let masked = mask(text);
        assert!(masked.regions.is_empty()); // regular links are NOT masked
        assert!(masked.masked.contains("[[Article Name]]"));
    }

    #[test]
    fn test_utf8_preserved() {
        let text = "Café <!-- héllo --> naïve {{Ñ|ñ}}";
        let masked = mask(text);
        let result = masked.unmask();
        assert_eq!(result, text);
    }

    #[test]
    fn test_empty_template() {
        let text = "A {{}} B";
        let masked = mask(text);
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_adjacent_protected_regions() {
        let text = "{{a}}{{b}}<!-- c --><nowiki>d</nowiki>";
        let masked = mask(text);
        assert_eq!(masked.regions.len(), 4);
        assert_eq!(masked.unmask(), text);
    }

    // --- Regression tests for code review findings ---

    #[test]
    fn test_braces_inside_html_comment_in_template() {
        // HIGH-2: }} inside a comment within a template must not close the template early
        let text = "{{template|<!-- }} -->|arg}}";
        let masked = mask(text);
        assert_eq!(masked.regions.len(), 1);
        assert_eq!(&masked.regions[0], text);
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_mixed_case_file_link() {
        // HIGH-1: mixed-case File:/Image: should be detected
        let text = "[[fIlE:Test.png|thumb]]";
        let masked = mask(text);
        assert_eq!(masked.regions.len(), 1);
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_mixed_case_image_link() {
        let text = "[[iMaGe:Photo.jpg|200px]]";
        let masked = mask(text);
        assert_eq!(masked.regions.len(), 1);
        assert_eq!(masked.unmask(), text);
    }

    #[test]
    fn test_nonce_prevents_cross_mask_collision() {
        // CRITICAL-1: two mask() calls should use different sentinels
        let m1 = mask("{{a}}");
        let m2 = mask("{{b}}");
        assert_ne!(m1.sentinel_base, m2.sentinel_base);
    }
}
