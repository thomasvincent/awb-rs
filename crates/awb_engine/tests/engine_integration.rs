use awb_domain::profile::ThrottlePolicy;
use awb_domain::rules::{Rule, RuleSet};
use awb_domain::session::{EditDecision, SkipCondition, SkipDecision};
use awb_domain::types::*;
use awb_domain::warnings::Warning;
use awb_engine::general_fixes::FixRegistry;
use awb_engine::review::{ReviewEvent, ReviewSideEffect, ReviewState, ReviewStateMachine};
use awb_engine::skip::SkipEngine;
use awb_engine::transform::TransformEngine;
use std::collections::HashSet;
use std::time::Duration;

fn create_test_page(namespace: Namespace, title: &str, wikitext: &str, size: u64) -> PageContent {
    PageContent {
        page_id: PageId(1),
        title: Title::new(namespace, title),
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
fn test_full_transform_pipeline() {
    // Create a RuleSet with multiple rules
    let mut ruleset = RuleSet::new();
    ruleset.add(Rule::new_plain("teh", "the", true));
    ruleset.add(Rule::new_regex(r"\s+", " ", false)); // Normalize whitespace

    let registry = FixRegistry::with_defaults();
    let mut enabled = HashSet::new();
    enabled.insert("trailing_whitespace".to_string());

    let engine = TransformEngine::new(&ruleset, registry, enabled).unwrap();

    // Apply transform to page content
    let page = create_test_page(
        Namespace::MAIN,
        "Test Article",
        "This is teh   test   content with  spaces   \n",
        100,
    );

    let plan = engine.apply(&page);

    // Verify the transformation
    assert!(plan.new_wikitext.contains("the"));
    assert!(!plan.new_wikitext.contains("teh"));
    assert!(!plan.new_wikitext.contains("   ")); // Multiple spaces should be normalized
    assert_eq!(plan.rules_applied.len(), 2); // Both rules should have applied
    assert!(plan.fixes_applied.len() > 0); // Trailing whitespace fix should apply
    assert!(!plan.diff_ops.is_empty()); // Should have diff operations
}

#[test]
fn test_skip_engine_with_transform_engine() {
    // Setup skip conditions
    let mut allowed_namespaces = HashSet::new();
    allowed_namespaces.insert(Namespace::MAIN);

    let skip_conditions = vec![
        SkipCondition::Namespace {
            allowed: allowed_namespaces,
        },
        SkipCondition::PageSize {
            min_bytes: Some(50),
            max_bytes: Some(500),
        },
    ];

    let skip_engine = SkipEngine::new(skip_conditions).unwrap();

    // Setup transform engine
    let mut ruleset = RuleSet::new();
    ruleset.add(Rule::new_plain("old", "new", true));
    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    // Test page that should be skipped (wrong namespace)
    let skip_page = create_test_page(Namespace::TALK, "Discussion", "old content", 100);
    assert_eq!(
        skip_engine.evaluate(&skip_page),
        SkipDecision::Skip("namespace filtered")
    );

    // Test page that should be skipped (too small)
    let too_small = create_test_page(Namespace::MAIN, "Small", "old", 10);
    assert_eq!(
        skip_engine.evaluate(&too_small),
        SkipDecision::Skip("page too small")
    );

    // Test page that should be processed
    let good_page = create_test_page(Namespace::MAIN, "Good", "old content", 100);
    assert_eq!(skip_engine.evaluate(&good_page), SkipDecision::Process);

    // Apply transform to the good page
    let plan = engine.apply(&good_page);
    assert_eq!(plan.new_wikitext, "new content");
}

#[test]
fn test_review_state_machine_complete_cycle() {
    let mut machine = ReviewStateMachine::new();

    // Start the review
    let effects = machine.transition(ReviewEvent::Start);
    assert!(matches!(machine.state(), ReviewState::LoadingList));
    assert_eq!(effects.len(), 0);

    // Load a list of pages
    let titles = vec![
        Title::new(Namespace::MAIN, "Page1"),
        Title::new(Namespace::MAIN, "Page2"),
        Title::new(Namespace::MAIN, "Page3"),
    ];
    let effects = machine.transition(ReviewEvent::ListLoaded(titles.clone()));
    assert!(matches!(
        machine.state(),
        ReviewState::FetchingPage { index: 0 }
    ));
    assert!(matches!(effects[0], ReviewSideEffect::FetchPage(_)));

    // Process each page through the full cycle
    for (i, title) in titles.iter().enumerate() {
        // Fetch page
        let page = create_test_page(Namespace::MAIN, &title.name, "test content", 100);
        let effects = machine.transition(ReviewEvent::PageFetched(page.clone()));
        assert!(matches!(machine.state(), ReviewState::ApplyingRules { .. }));
        assert!(matches!(effects[0], ReviewSideEffect::ApplyRules(_)));

        // Apply rules (simulated)
        let plan = awb_domain::session::EditPlan {
            page: page.clone(),
            new_wikitext: "modified content".to_string(),
            rules_applied: vec![],
            fixes_applied: vec![],
            diff_ops: vec![],
            summary: format!("Edit {}", i + 1),
            warnings: vec![],
        };
        let effects = machine.transition(ReviewEvent::RulesApplied(plan.clone()));
        assert!(matches!(
            machine.state(),
            ReviewState::AwaitingDecision { .. }
        ));
        assert!(matches!(effects[0], ReviewSideEffect::PresentForReview(_)));

        // User decides to save
        let effects = machine.transition(ReviewEvent::UserDecision(EditDecision::Save));
        assert!(matches!(machine.state(), ReviewState::Saving { .. }));
        assert!(matches!(effects[0], ReviewSideEffect::ExecuteEdit { .. }));

        // Save completes
        let result = awb_domain::session::EditResult {
            page_id: page.page_id,
            new_revision: Some(RevisionId(101 + i as u64)),
            outcome: awb_domain::session::EditOutcome::Saved {
                revision: RevisionId(101 + i as u64),
            },
            timestamp: chrono::Utc::now(),
        };
        let _effects = machine.transition(ReviewEvent::SaveComplete(result));

        if i < titles.len() - 1 {
            // Should advance to next page
            assert!(matches!(machine.state(), ReviewState::FetchingPage { .. }));
        } else {
            // Should complete
            assert!(matches!(machine.state(), ReviewState::Completed { .. }));
        }
    }
}

#[test]
fn test_fix_registry_with_all_default_fixes() {
    let registry = FixRegistry::with_defaults();
    let mut enabled = HashSet::new();

    // Enable all default fixes
    for module in registry.all_modules() {
        enabled.insert(module.id().to_string());
    }

    let context = awb_engine::general_fixes::FixContext {
        title: Title::new(Namespace::MAIN, "Test Article"),
        namespace: Namespace::MAIN,
        is_redirect: false,
    };

    // Test content with various issues
    let content = r#"
This is   a test   article.

<b>Bold text</b> and <i>italic text</i>
Line with trailing spaces

[[Category:Z Test]]
[[Category:A First]]
[[Category:M Middle]]
"#;

    let result = registry.apply_all(content, &context, &enabled);

    // Verify fixes were applied
    assert!(result.contains("'''Bold text'''")); // HTML to wikitext
    assert!(result.contains("''italic text''")); // HTML to wikitext

    // Verify content was actually transformed (not identical to input)
    assert_ne!(result, content, "Fixes should have modified the content");

    // Categories should be sorted
    let cat_positions: Vec<_> = result.match_indices("[[Category:").collect();
    assert!(
        cat_positions.len() >= 3,
        "Should have at least 3 categories"
    );

    // Check that categories appear in alphabetical order
    let cat_a_pos = result
        .find("[[Category:A First]]")
        .expect("Category A should exist");
    let cat_m_pos = result
        .find("[[Category:M Middle]]")
        .expect("Category M should exist");
    let cat_z_pos = result
        .find("[[Category:Z Test]]")
        .expect("Category Z should exist");

    assert!(cat_a_pos < cat_m_pos, "Category A should come before M");
    assert!(cat_m_pos < cat_z_pos, "Category M should come before Z");
}

#[test]
fn test_transform_with_regex_and_plain_rules() {
    let mut ruleset = RuleSet::new();

    // Add plain rule
    ruleset.add(Rule::new_plain("colour", "color", true));

    // Add regex rule to fix date formats
    ruleset.add(Rule::new_regex(
        r"(\d{1,2})/(\d{1,2})/(\d{4})",
        "$3-$1-$2",
        false,
    ));

    // Add case-insensitive plain rule
    ruleset.add(Rule::new_plain("WIKI", "wiki", false));

    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    let page = create_test_page(
        Namespace::MAIN,
        "Test",
        "The colour of WIKI pages. Date: 12/25/2024. wiki and Wiki.",
        100,
    );

    let plan = engine.apply(&page);

    assert!(plan.new_wikitext.contains("color")); // Plain rule
    assert!(plan.new_wikitext.contains("2024-12-25")); // Regex rule
    assert!(!plan.new_wikitext.contains("WIKI")); // Case-insensitive replacement
    assert!(plan.new_wikitext.contains("wiki")); // All variants replaced
    assert_eq!(plan.rules_applied.len(), 3);
}

#[test]
fn test_warnings_generation() {
    let ruleset = RuleSet::new(); // No rules
    let registry = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset, registry, HashSet::new()).unwrap();

    // Test no change warning
    let page = create_test_page(Namespace::MAIN, "Test", "unchanged", 100);
    let plan = engine.apply(&page);
    assert!(plan.warnings.iter().any(|w| matches!(w, Warning::NoChange)));

    // Test large change warning
    let mut ruleset2 = RuleSet::new();
    let large_text = "x".repeat(600);
    ruleset2.add(Rule::new_plain("small", &large_text, true));
    let registry2 = FixRegistry::new();
    let engine = TransformEngine::new(&ruleset2, registry2, HashSet::new()).unwrap();

    let page = create_test_page(Namespace::MAIN, "Test", "small", 10);
    let plan = engine.apply(&page);
    assert!(
        plan.warnings
            .iter()
            .any(|w| matches!(w, Warning::LargeChange { .. }))
    );
}

#[test]
fn test_throttle_policy_serialization() {
    let policy = ThrottlePolicy {
        min_edit_interval: Duration::from_secs(10),
        maxlag: 5,
        max_retries: 3,
        backoff_base: Duration::from_secs(2),
    };

    let json = serde_json::to_string(&policy).unwrap();
    let deserialized: ThrottlePolicy = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.min_edit_interval, Duration::from_secs(10));
    assert_eq!(deserialized.maxlag, 5);
    assert_eq!(deserialized.max_retries, 3);
    assert_eq!(deserialized.backoff_base, Duration::from_secs(2));
}
