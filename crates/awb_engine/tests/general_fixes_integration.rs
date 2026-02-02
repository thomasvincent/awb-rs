use awb_domain::types::{Namespace, Title};
use awb_engine::general_fixes::{
    CitationFormatting, DefaultSortFix, DuplicateWikilinkRemoval, FixContext, FixModule,
    FixRegistry, UnicodeNormalization,
};
use std::collections::HashSet;

fn test_context(title_name: &str) -> FixContext {
    FixContext {
        title: Title::new(Namespace::MAIN, title_name),
        namespace: Namespace::MAIN,
        is_redirect: false,
    }
}

#[test]
fn test_citation_formatting_basic() {
    let fix = CitationFormatting;
    let ctx = test_context("Test");

    let input = "{{Cite Web|url=http://example.com|accessdate=2021-01-01}}";
    let result = fix.apply(input, &ctx);

    assert!(result.contains("{{cite web"));
    assert!(result.contains("access-date="));
    assert!(!result.contains("accessdate="));
}

#[test]
fn test_citation_formatting_deadurl() {
    let fix = CitationFormatting;
    let ctx = test_context("Test");

    let input = "{{cite news|url=http://example.com|deadurl=yes}}";
    let result = fix.apply(input, &ctx);

    assert!(result.contains("url-status=dead"));
    assert!(!result.contains("deadurl"));
}

#[test]
fn test_citation_formatting_deadurl_no() {
    let fix = CitationFormatting;
    let ctx = test_context("Test");

    let input = "{{cite news|url=http://example.com|deadurl=no}}";
    let result = fix.apply(input, &ctx);

    assert!(result.contains("url-status=live"));
}

#[test]
fn test_duplicate_wikilink_removal() {
    let fix = DuplicateWikilinkRemoval;
    let ctx = test_context("Test");

    let input = "The [[Python (programming language)|Python]] language and [[Python (programming language)|Python]] again.";
    let result = fix.apply(input, &ctx);

    // First link should remain, second should become plain text
    assert_eq!(
        result,
        "The [[Python (programming language)|Python]] language and Python again."
    );
}

#[test]
fn test_duplicate_wikilink_case_insensitive() {
    let fix = DuplicateWikilinkRemoval;
    let ctx = test_context("Test");

    let input = "[[Python]] and [[python]] and [[PYTHON]]";
    let result = fix.apply(input, &ctx);

    // Should only keep the first one
    assert_eq!(result, "[[Python]] and python and PYTHON");
}

#[test]
fn test_unicode_normalization_nbsp() {
    let fix = UnicodeNormalization;
    let ctx = test_context("Test");

    let input = "This\u{00A0}has\u{00A0}non-breaking\u{00A0}spaces";
    let result = fix.apply(input, &ctx);

    assert_eq!(result, "This has non-breaking spaces");
    assert!(!result.contains('\u{00A0}'));
}

#[test]
fn test_unicode_normalization_endash_ranges() {
    let fix = UnicodeNormalization;
    let ctx = test_context("Test");

    let input = "The years 2020 – 2021 and pages 10 — 15";
    let result = fix.apply(input, &ctx);

    assert_eq!(result, "The years 2020–2021 and pages 10–15");
}

#[test]
fn test_unicode_normalization_curly_quotes_in_template() {
    let fix = UnicodeNormalization;
    let ctx = test_context("Test");

    let input = "{{cite web|title=\u{201C}Quoted Title\u{201D}|author=\u{2018}John\u{2019}}}";
    let result = fix.apply(input, &ctx);

    assert!(result.contains("title=\"Quoted Title\""));
    assert!(result.contains("author='John'"));
}

#[test]
fn test_defaultsort_adds_for_diacritics() {
    let fix = DefaultSortFix;
    let ctx = test_context("Café");

    let input = "This is an article about cafés.\n[[Category:Restaurants]]";
    let result = fix.apply(input, &ctx);

    assert!(result.contains("{{DEFAULTSORT:Cafe}}"));
    assert!(result.contains("[[Category:Restaurants]]"));
}

#[test]
fn test_defaultsort_skips_if_already_present() {
    let fix = DefaultSortFix;
    let ctx = test_context("Café");

    let input = "{{DEFAULTSORT:Custom}}\n[[Category:Restaurants]]";
    let result = fix.apply(input, &ctx);

    // Should not add another DEFAULTSORT
    assert_eq!(result, input);
}

#[test]
fn test_defaultsort_skips_ascii_only() {
    let fix = DefaultSortFix;
    let ctx = test_context("Cafe");

    let input = "This is an article.\n[[Category:Restaurants]]";
    let result = fix.apply(input, &ctx);

    // Should not add DEFAULTSORT for ASCII-only title
    assert!(!result.contains("DEFAULTSORT"));
}

#[test]
fn test_registry_applies_all_new_fixes() {
    let registry = FixRegistry::with_defaults();
    let ctx = test_context("Test");

    let mut enabled = HashSet::new();
    enabled.insert("citation_formatting".to_string());
    enabled.insert("duplicate_wikilink_removal".to_string());
    enabled.insert("unicode_normalization".to_string());

    let input = "{{Cite Web|accessdate=2021-01-01}} [[Python]] and [[Python]] with\u{00A0}spaces";
    let result = registry.apply_all(input, &ctx, &enabled);

    assert!(result.contains("cite web"));
    assert!(result.contains("access-date="));
    assert!(!result.contains('\u{00A0}'));
    // Check duplicate link was removed (second occurrence becomes plain text)
    let python_count = result.matches("[[Python]]").count();
    assert_eq!(python_count, 1);
}

#[test]
fn test_all_modules_registered() {
    let registry = FixRegistry::with_defaults();
    let modules = registry.all_modules();

    let ids: Vec<&str> = modules.iter().map(|m| m.id()).collect();

    assert!(ids.contains(&"citation_formatting"));
    assert!(ids.contains(&"duplicate_wikilink_removal"));
    assert!(ids.contains(&"unicode_normalization"));
    assert!(ids.contains(&"defaultsort_fix"));

    // Also verify original modules still present
    assert!(ids.contains(&"whitespace_cleanup"));
    assert!(ids.contains(&"category_sorting"));
}
