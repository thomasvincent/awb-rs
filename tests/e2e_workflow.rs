use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};
use awb_domain::rules::{Rule, RuleSet};
use awb_domain::types::*;
use awb_engine::general_fixes::FixRegistry;
use awb_engine::transform::TransformEngine;
use std::collections::HashSet;
use std::time::Duration;

fn create_realistic_wikipedia_article() -> &'static str {
    r#"{{short description|American actor}}
{{Infobox person
| name = John Doe
| birth_date = {{birth date|1980|01|15}}
| occupation = Actor
| years_active = 2000–present
}}

'''John Doe''' (born January 15, 1980) is an American actor known for his work in teh film industry.

== Early life ==
Doe was born in [[New York City]] and  raised in [[Los Angeles]].  He attended teh [[University of California]].

== Career ==
Doe began his career in 2000   with a role in ''Film Title''. He has since appeared in numerous productions.

<b>Notable works:</b>
* ''Film One'' (2005)
* ''Film Two'' (2010)
* ''Film Three'' (2015)

== Personal life ==
Doe is married to [[Jane Smith|Jane Doe]]   and has two children.

== References ==
{{reflist}}

== External links ==
* {{IMDb name|1234567}}

[[Category:1980 births]]
[[Category:Z American actors]]
[[Category:A Living people]]
[[Category:M Male actors]]

<!-- TODO: Add more references -->
"#
}

#[test]
fn test_full_edit_workflow_without_network() {
    // Step 1: Create a Profile with rules
    let mut namespaces = HashSet::new();
    namespaces.insert(Namespace::MAIN);

    let _profile = Profile {
        id: "enwiki".to_string(),
        name: "English Wikipedia".to_string(),
        api_url: url::Url::parse("https://en.wikipedia.org/w/api.php").unwrap(),
        auth_method: AuthMethod::BotPassword {
            username: "TestBot".to_string(),
        },
        default_namespaces: namespaces,
        throttle_policy: ThrottlePolicy {
            min_edit_interval: Duration::from_secs(12),
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        },
    };

    // Step 2: Build TransformEngine from profile rules
    let mut ruleset = RuleSet::new();

    // Add typo fix rules
    let mut typo_rule = Rule::new_plain("teh", "the", true);
    typo_rule.comment_fragment = Some("fix typo: teh → the".to_string());
    ruleset.add(typo_rule);

    // Add whitespace normalization
    let mut ws_rule = Rule::new_regex(r"  +", " ", false);
    ws_rule.comment_fragment = Some("normalize whitespace".to_string());
    ruleset.add(ws_rule);

    // Enable general fixes
    let registry = FixRegistry::with_defaults();
    let mut enabled_fixes = HashSet::new();
    enabled_fixes.insert("whitespace_cleanup".to_string());
    enabled_fixes.insert("html_to_wikitext".to_string());
    enabled_fixes.insert("category_sorting".to_string());
    enabled_fixes.insert("trailing_whitespace".to_string());

    let engine = TransformEngine::new(&ruleset, registry, enabled_fixes).unwrap();

    // Step 3: Create mock page content (realistic Wikipedia article)
    let page = PageContent {
        page_id: PageId(12345),
        title: Title::new(Namespace::MAIN, "John Doe (actor)"),
        revision: RevisionId(98765),
        timestamp: chrono::Utc::now(),
        wikitext: create_realistic_wikipedia_article().to_string(),
        size_bytes: create_realistic_wikipedia_article().len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    // Step 4: Apply transform → get EditPlan
    let plan = engine.apply(&page);

    // Step 5: Verify diff shows expected changes
    assert!(!plan.diff_ops.is_empty(), "Diff should show changes");

    // Step 6: Verify summary describes changes
    assert!(plan.summary.contains("fix typo") || plan.summary.contains("normalize whitespace"));
    assert!(!plan.summary.is_empty());

    // Step 7: Check warnings are appropriate
    // Should not have "no change" warning since we made changes
    assert!(!plan.warnings.iter().any(|w| matches!(w, awb_domain::warnings::Warning::NoChange)));

    // Verify specific transformations
    assert!(!plan.new_wikitext.contains("teh "), "Typo 'teh' should be fixed");
    assert!(plan.new_wikitext.contains("the "), "Should contain correct 'the'");

    // HTML to wikitext conversion
    assert!(plan.new_wikitext.contains("'''Notable works:'''"), "HTML <b> should convert to wikitext");

    // Category sorting
    let cat_a_pos = plan.new_wikitext.find("[[Category:A Living people]]").unwrap();
    let cat_m_pos = plan.new_wikitext.find("[[Category:M Male actors]]").unwrap();
    let cat_z_pos = plan.new_wikitext.find("[[Category:Z American actors]]").unwrap();
    assert!(cat_a_pos < cat_m_pos, "Categories should be sorted alphabetically");
    assert!(cat_m_pos < cat_z_pos, "Categories should be sorted alphabetically");

    // HTML comments are preserved (whitespace_cleanup does not strip comments)
    // Just verify the engine ran without error

    // Verify rules were applied
    assert!(!plan.rules_applied.is_empty(), "Rules should have been applied");
    assert!(!plan.fixes_applied.is_empty(), "General fixes should have been applied");
}

#[test]
fn test_workflow_with_multiple_rule_types() {
    let mut ruleset = RuleSet::new();

    // Plain rule (case-sensitive)
    ruleset.add(Rule::new_plain("color", "colour", true));

    // Plain rule (case-insensitive)
    ruleset.add(Rule::new_plain("AMERICA", "United States", false));

    // Regex rule (date formatting)
    ruleset.add(Rule::new_regex(r"(\d{1,2})/(\d{1,2})/(\d{4})", "$3-$1-$2", false));

    // Regex rule (fix double spaces)
    ruleset.add(Rule::new_regex(r"  +", " ", false));

    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Test Article"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: "The color of AMERICA  was documented on 12/25/2024.  America and america.".to_string(),
        size_bytes: 100,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // Verify transformations
    assert!(plan.new_wikitext.contains("colour"), "Plain rule should replace color");
    assert!(plan.new_wikitext.contains("United States"), "Case-insensitive rule should work");
    assert!(plan.new_wikitext.contains("2024-12-25"), "Regex rule should format dates");
    assert!(!plan.new_wikitext.contains("  "), "Double spaces should be removed");

    // All rules should have been applied
    assert!(plan.rules_applied.len() >= 3, "Multiple rules should apply");
}

#[test]
fn test_workflow_with_general_fixes_enabled() {
    let ruleset = RuleSet::new(); // No custom rules

    let registry = FixRegistry::with_defaults();
    let mut enabled = HashSet::new();
    enabled.insert("html_to_wikitext".to_string());
    enabled.insert("trailing_whitespace".to_string());
    enabled.insert("unicode_normalization".to_string());
    enabled.insert("citation_formatting".to_string());

    let engine = TransformEngine::new(&ruleset, registry, enabled).unwrap();

    let content = r#"This is a test with <b>bold</b> and <i>italic</i> text.
Line with trailing spaces
{{cite web|accessdate=2020-01-01|deadurl=yes}}
2020–2021 was the range.
"#;

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Test"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.to_string(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // HTML to wikitext
    assert!(plan.new_wikitext.contains("'''bold'''"));
    assert!(plan.new_wikitext.contains("''italic''"));

    // Citation formatting
    assert!(plan.new_wikitext.contains("access-date"));
    assert!(plan.new_wikitext.contains("url-status"));

    // General fixes should have been applied
    assert!(!plan.fixes_applied.is_empty());
}

#[test]
fn test_workflow_with_typo_fix_rules() {
    // Simulate loading typo fix rules from inline TSV
    let typo_data = vec![
        ("teh", "the"),
        ("recieve", "receive"),
        ("seperate", "separate"),
        ("occured", "occurred"),
        ("untill", "until"),
    ];

    let mut ruleset = RuleSet::new();
    for (typo, correction) in typo_data {
        let mut rule = Rule::new_plain(typo, correction, true);
        rule.comment_fragment = Some(format!("typo fix: {} → {}", typo, correction));
        ruleset.add(rule);
    }

    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    let content = "This article contains teh word recieve, seperate instances of typos, and untill occured is fixed.";

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Typo Test"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.to_string(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // All typos should be fixed
    assert!(plan.new_wikitext.contains("the word"));
    assert!(plan.new_wikitext.contains("receive"));
    assert!(plan.new_wikitext.contains("separate"));
    assert!(plan.new_wikitext.contains("occurred"));
    assert!(plan.new_wikitext.contains("until"));

    // Should not contain any typos
    assert!(!plan.new_wikitext.contains("teh "));
    assert!(!plan.new_wikitext.contains("recieve"));
    assert!(!plan.new_wikitext.contains("seperate"));
    assert!(!plan.new_wikitext.contains("occured"));
    assert!(!plan.new_wikitext.contains("untill"));

    // Summary should mention typo fixes
    assert!(plan.summary.contains("typo fix"));
}

#[test]
fn test_workflow_no_changes_warning() {
    let ruleset = RuleSet::new(); // No rules
    let registry = FixRegistry::new(); // No fixes enabled
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    let content = "This content will not be changed.";

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "No Change Test"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.to_string(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // Should have no change warning
    assert!(plan.warnings.iter().any(|w| matches!(w, awb_domain::warnings::Warning::NoChange)));

    // Content should be unchanged
    assert_eq!(plan.new_wikitext, content);

    // No rules or fixes applied
    assert!(plan.rules_applied.is_empty());
    assert!(plan.fixes_applied.is_empty());
}

#[test]
fn test_workflow_large_change_warning() {
    let mut ruleset = RuleSet::new();

    // Create a rule that will add significant content
    let large_replacement = "x".repeat(600);
    ruleset.add(Rule::new_plain("REPLACE_ME", &large_replacement, true));

    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    let content = "Short text with REPLACE_ME marker.";

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Large Change Test"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.to_string(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // Should have large change warning
    assert!(plan.warnings.iter().any(|w| matches!(w, awb_domain::warnings::Warning::LargeChange { .. })));

    // Content should be significantly larger
    assert!(plan.new_wikitext.len() > content.len() + 500);
}

#[test]
fn test_workflow_with_disabled_rule() {
    let mut ruleset = RuleSet::new();

    // Add an enabled rule
    ruleset.add(Rule::new_plain("foo", "bar", true));

    // Add a disabled rule
    let mut disabled_rule = Rule::new_plain("baz", "qux", true);
    disabled_rule.enabled = false;
    ruleset.add(disabled_rule);

    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    let content = "Text with foo and baz words.";

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Disabled Rule Test"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.to_string(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // Enabled rule should apply
    assert!(plan.new_wikitext.contains("bar"));

    // Disabled rule should not apply
    assert!(plan.new_wikitext.contains("baz"));
    assert!(!plan.new_wikitext.contains("qux"));

    // Only one rule should be recorded as applied
    assert_eq!(plan.rules_applied.len(), 1);
}

#[test]
fn test_workflow_realistic_biography_cleanup() {
    let content = r#"
'''Jane Smith'''  (born  January 1, 1990) is an <i>American</i> scientist.

==Early Life==
Smith was born in [[New York]] and studied at teh [[Harvard University]].

==Career==
She has worked on various projects  since 2015.

==Publications==
* <b>Book One</b> (2018)
* <b>Book Two</b> (2020)

[[Category:Z Scientists]]
[[Category:A 1990 births]]
[[Category:M American people]]
"#;

    let mut ruleset = RuleSet::new();
    ruleset.add(Rule::new_plain("teh", "the", true));
    ruleset.add(Rule::new_regex(r"  +", " ", false));

    let registry = FixRegistry::with_defaults();
    let mut enabled = HashSet::new();
    enabled.insert("html_to_wikitext".to_string());
    enabled.insert("whitespace_cleanup".to_string());
    enabled.insert("category_sorting".to_string());
    enabled.insert("heading_spacing".to_string());

    let engine = TransformEngine::new(&ruleset, registry, enabled).unwrap();

    let page = PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Jane Smith"),
        revision: RevisionId(1),
        timestamp: chrono::Utc::now(),
        wikitext: content.to_string(),
        size_bytes: content.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    };

    let plan = engine.apply(&page);

    // Verify comprehensive cleanup
    assert!(!plan.new_wikitext.contains("teh "));
    assert!(plan.new_wikitext.contains("''American''"));
    assert!(plan.new_wikitext.contains("'''Book One'''"));
    assert!(!plan.new_wikitext.contains("  "));

    // Categories should be sorted
    let result = &plan.new_wikitext;
    let cat_a = result.find("[[Category:A").unwrap();
    let cat_m = result.find("[[Category:M").unwrap();
    let cat_z = result.find("[[Category:Z").unwrap();
    assert!(cat_a < cat_m && cat_m < cat_z);
}
