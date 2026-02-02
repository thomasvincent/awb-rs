use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use awb_domain::rules::{Rule, RuleSet};
use awb_domain::types::*;
use awb_engine::general_fixes::{FixRegistry, FixContext};
use awb_engine::transform::TransformEngine;
use awb_engine::diff_engine::compute_diff;
use std::collections::HashSet;

fn create_test_page(wikitext: &str) -> PageContent {
    PageContent {
        page_id: PageId(1),
        title: Title::new(Namespace::MAIN, "Test Page"),
        revision: RevisionId(100),
        timestamp: chrono::Utc::now(),
        wikitext: wikitext.to_string(),
        size_bytes: wikitext.len() as u64,
        is_redirect: false,
        protection: ProtectionInfo::default(),
        properties: PageProperties::default(),
    }
}

fn bench_plain_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("plain_rules");

    // Test with 10 rules
    let mut ruleset_10 = RuleSet::new();
    for i in 0..10 {
        ruleset_10.add(Rule::new_plain(format!("pattern{}", i), format!("replacement{}", i), false));
    }

    // Test with 50 rules
    let mut ruleset_50 = RuleSet::new();
    for i in 0..50 {
        ruleset_50.add(Rule::new_plain(format!("pattern{}", i), format!("replacement{}", i), false));
    }

    let sample_text = "This is a test page with pattern0 and pattern5 and pattern10 repeated multiple times.\n\
                       Pattern0 Pattern5 Pattern10 should all be replaced in this benchmark test.\n\
                       The quick brown fox jumps over the lazy dog. Pattern1 pattern2 pattern3.\n";

    group.bench_with_input(BenchmarkId::new("10_rules", "sample"), &ruleset_10, |b, ruleset| {
        let engine = TransformEngine::new(ruleset, FixRegistry::new(), HashSet::new()).unwrap();
        let page = create_test_page(sample_text);
        b.iter(|| {
            black_box(engine.apply(&page));
        });
    });

    group.bench_with_input(BenchmarkId::new("50_rules", "sample"), &ruleset_50, |b, ruleset| {
        let engine = TransformEngine::new(ruleset, FixRegistry::new(), HashSet::new()).unwrap();
        let page = create_test_page(sample_text);
        b.iter(|| {
            black_box(engine.apply(&page));
        });
    });

    group.finish();
}

fn bench_fix_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("fix_chain");

    let sample_text = "{{Cite Web|title=Test|accessdate=2021-01-01}}\n\
                       Line with trailing spaces   \n\
                       [[Python]] is great and [[Python]] is awesome.\n\
                       Word\u{00A0}with\u{00A0}nbsp spaces.\n\
                       [[Category:Zebra]]\n[[Category:Apple]]\n";

    let registry = FixRegistry::with_defaults();
    let ctx = FixContext {
        title: Title::new(Namespace::MAIN, "Test"),
        namespace: Namespace::MAIN,
        is_redirect: false,
    };

    let mut enabled = HashSet::new();
    enabled.insert("citation_formatting".to_string());
    enabled.insert("trailing_whitespace".to_string());
    enabled.insert("duplicate_wikilink_removal".to_string());
    enabled.insert("unicode_normalization".to_string());
    enabled.insert("category_sorting".to_string());

    group.bench_function("apply_all_fixes", |b| {
        b.iter(|| {
            black_box(registry.apply_all(sample_text, &ctx, &enabled));
        });
    });

    group.finish();
}

fn bench_diff_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_computation");

    let old_small = "line 1\nline 2\nline 3\n";
    let new_small = "line 1\nline 2 modified\nline 3\nline 4\n";

    let old_large = (0..100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n") + "\n";
    let new_large = (0..100).map(|i| {
        if i % 10 == 0 {
            format!("line {} modified", i)
        } else {
            format!("line {}", i)
        }
    }).collect::<Vec<_>>().join("\n") + "\n";

    group.bench_function("small_diff", |b| {
        b.iter(|| {
            black_box(compute_diff(old_small, new_small));
        });
    });

    group.bench_function("large_diff", |b| {
        b.iter(|| {
            black_box(compute_diff(&old_large, &new_large));
        });
    });

    group.finish();
}

fn bench_case_insensitive_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("case_insensitive");

    let mut ruleset = RuleSet::new();
    for i in 0..20 {
        ruleset.add(Rule::new_plain(format!("WORD{}", i), format!("replacement{}", i), false));
    }

    let sample_text = "This text has WORD0 and word1 and WoRd2 in various cases.\n\
                       More text with WORD5 word10 WoRd15 repeated.\n";

    let registry = FixRegistry::new();
    let enabled = HashSet::new();
    let engine = TransformEngine::new(&ruleset, registry, enabled).unwrap();
    let page = create_test_page(sample_text);

    group.bench_function("case_insensitive_matching", |b| {
        b.iter(|| {
            black_box(engine.apply(&page));
        });
    });

    group.finish();
}

criterion_group!(benches, bench_plain_rules, bench_fix_chain, bench_diff_computation, bench_case_insensitive_rules);
criterion_main!(benches);
