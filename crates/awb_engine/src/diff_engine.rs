use awb_domain::diff::*;
use similar::{ChangeTag, TextDiff};

pub fn compute_diff(old: &str, new: &str) -> Vec<DiffOp> {
    let diff = TextDiff::from_lines(old, new);
    let estimated = (old.lines().count().max(new.lines().count()) / 2) + 10;
    let mut ops = Vec::with_capacity(estimated);
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for change in diff.iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                ops.push(DiffOp::Equal {
                    old_range: old_pos..old_pos + len,
                    new_range: new_pos..new_pos + len,
                    text: change.value().to_string(),
                });
                old_pos += len;
                new_pos += len;
            }
            ChangeTag::Delete => {
                ops.push(DiffOp::Delete {
                    old_range: old_pos..old_pos + len,
                    text: change.value().to_string(),
                });
                old_pos += len;
            }
            ChangeTag::Insert => {
                ops.push(DiffOp::Insert {
                    new_range: new_pos..new_pos + len,
                    text: change.value().to_string(),
                });
                new_pos += len;
            }
        }
    }
    ops
}

pub fn to_unified(ops: &[DiffOp], context_lines: usize) -> String {
    if ops.is_empty() {
        return String::new();
    }

    // Step 1: Flatten ops into tagged lines
    #[derive(Clone, Copy)]
    enum Tag { Context, Delete, Insert }

    struct TaggedLine<'a> {
        tag: Tag,
        text: &'a str,
    }

    let mut tagged: Vec<TaggedLine> = Vec::new();
    for op in ops {
        match op {
            DiffOp::Equal { text, .. } => {
                for line in text.split_inclusive('\n') {
                    tagged.push(TaggedLine { tag: Tag::Context, text: line });
                }
            }
            DiffOp::Delete { text, .. } => {
                for line in text.split_inclusive('\n') {
                    tagged.push(TaggedLine { tag: Tag::Delete, text: line });
                }
            }
            DiffOp::Insert { text, .. } => {
                for line in text.split_inclusive('\n') {
                    tagged.push(TaggedLine { tag: Tag::Insert, text: line });
                }
            }
            DiffOp::Replace {
                old_text, new_text, ..
            } => {
                for line in old_text.split_inclusive('\n') {
                    tagged.push(TaggedLine { tag: Tag::Delete, text: line });
                }
                for line in new_text.split_inclusive('\n') {
                    tagged.push(TaggedLine { tag: Tag::Insert, text: line });
                }
            }
        }
    }

    if tagged.is_empty() {
        return String::new();
    }

    // Step 2: Find change regions
    let is_change = |i: usize| !matches!(tagged[i].tag, Tag::Context);

    let mut change_ranges: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < tagged.len() {
        if is_change(i) {
            let start = i;
            while i < tagged.len() && is_change(i) {
                i += 1;
            }
            change_ranges.push((start, i - 1));
        } else {
            i += 1;
        }
    }

    if change_ranges.is_empty() {
        return String::new();
    }

    // Step 3: Build hunks by expanding context and merging overlaps
    struct Hunk {
        start: usize,
        end: usize,
    }

    let mut hunks: Vec<Hunk> = Vec::new();
    for &(cs, ce) in &change_ranges {
        let hunk_start = cs.saturating_sub(context_lines);
        let hunk_end = (ce + context_lines).min(tagged.len() - 1);

        if let Some(last) = hunks.last_mut() {
            if hunk_start <= last.end + 1 {
                last.end = hunk_end;
                continue;
            }
        }
        hunks.push(Hunk { start: hunk_start, end: hunk_end });
    }

    // Step 4: Emit output
    let mut output = String::from("--- a\n+++ b\n");

    for hunk in &hunks {
        let mut old_start = 1usize;
        let mut new_start = 1usize;
        for tl in tagged.iter().take(hunk.start) {
            match tl.tag {
                Tag::Context => { old_start += 1; new_start += 1; }
                Tag::Delete => { old_start += 1; }
                Tag::Insert => { new_start += 1; }
            }
        }

        let mut old_count = 0usize;
        let mut new_count = 0usize;
        for j in hunk.start..=hunk.end {
            match tagged[j].tag {
                Tag::Context => { old_count += 1; new_count += 1; }
                Tag::Delete => { old_count += 1; }
                Tag::Insert => { new_count += 1; }
            }
        }

        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start, old_count, new_start, new_count
        ));

        for j in hunk.start..=hunk.end {
            let prefix = match tagged[j].tag {
                Tag::Context => ' ',
                Tag::Delete => '-',
                Tag::Insert => '+',
            };
            let line_text = tagged[j].text;
            output.push(prefix);
            output.push_str(line_text);
            if !line_text.ends_with('\n') {
                output.push('\n');
            }
        }
    }

    output
}

pub fn to_side_by_side(ops: &[DiffOp]) -> Vec<SideBySideLine> {
    let estimated = ops.len() + 10;
    let mut lines = Vec::with_capacity(estimated);
    let mut left_no = 1;
    let mut right_no = 1;

    for op in ops {
        match op {
            DiffOp::Equal { text, .. } => {
                for line_text in text.lines() {
                    lines.push(SideBySideLine {
                        left: Some(DiffLine {
                            line_no: left_no,
                            text: line_text.to_string(),
                            change_type: ChangeType::Equal,
                            inline_changes: vec![],
                        }),
                        right: Some(DiffLine {
                            line_no: right_no,
                            text: line_text.to_string(),
                            change_type: ChangeType::Equal,
                            inline_changes: vec![],
                        }),
                    });
                    left_no += 1;
                    right_no += 1;
                }
            }
            DiffOp::Delete { text, .. } => {
                for line_text in text.lines() {
                    lines.push(SideBySideLine {
                        left: Some(DiffLine {
                            line_no: left_no,
                            text: line_text.to_string(),
                            change_type: ChangeType::Removed,
                            inline_changes: vec![],
                        }),
                        right: None,
                    });
                    left_no += 1;
                }
            }
            DiffOp::Insert { text, .. } => {
                for line_text in text.lines() {
                    lines.push(SideBySideLine {
                        left: None,
                        right: Some(DiffLine {
                            line_no: right_no,
                            text: line_text.to_string(),
                            change_type: ChangeType::Added,
                            inline_changes: vec![],
                        }),
                    });
                    right_no += 1;
                }
            }
            DiffOp::Replace {
                old_text, new_text, ..
            } => {
                let old_lines: Vec<&str> = old_text.lines().collect();
                let new_lines: Vec<&str> = new_text.lines().collect();
                let max = old_lines.len().max(new_lines.len());
                for i in 0..max {
                    lines.push(SideBySideLine {
                        left: old_lines.get(i).map(|t| {
                            let l = DiffLine {
                                line_no: left_no,
                                text: t.to_string(),
                                change_type: ChangeType::Modified,
                                inline_changes: vec![],
                            };
                            left_no += 1;
                            l
                        }),
                        right: new_lines.get(i).map(|t| {
                            let l = DiffLine {
                                line_no: right_no,
                                text: t.to_string(),
                                change_type: ChangeType::Modified,
                                inline_changes: vec![],
                            };
                            right_no += 1;
                            l
                        }),
                    });
                }
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_no_change() {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nline 2\nline 3\n";
        let ops = compute_diff(old, new);

        // All ops should be Equal when texts are identical
        assert!(ops.iter().all(|op| matches!(op, DiffOp::Equal { .. })));
        let combined: String = ops
            .iter()
            .map(|op| match op {
                DiffOp::Equal { text, .. } => text.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(combined, old);
    }

    #[test]
    fn test_compute_diff_insert() {
        let old = "line 1\nline 3\n";
        let new = "line 1\nline 2\nline 3\n";
        let ops = compute_diff(old, new);

        let has_insert = ops.iter().any(|op| matches!(op, DiffOp::Insert { .. }));
        assert!(has_insert);
    }

    #[test]
    fn test_compute_diff_delete() {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nline 3\n";
        let ops = compute_diff(old, new);

        let has_delete = ops.iter().any(|op| matches!(op, DiffOp::Delete { .. }));
        assert!(has_delete);
    }

    #[test]
    fn test_compute_diff_replace() {
        let old = "old text\n";
        let new = "new text\n";
        let ops = compute_diff(old, new);

        assert!(ops.len() >= 2); // Should have delete and insert
    }

    #[test]
    fn test_compute_diff_empty_to_text() {
        let old = "";
        let new = "new content\n";
        let ops = compute_diff(old, new);

        match &ops[0] {
            DiffOp::Insert { text, .. } => {
                assert_eq!(text, "new content\n");
            }
            _ => panic!("Expected Insert op"),
        }
    }

    #[test]
    fn test_compute_diff_text_to_empty() {
        let old = "content to delete\n";
        let new = "";
        let ops = compute_diff(old, new);

        match &ops[0] {
            DiffOp::Delete { text, .. } => {
                assert_eq!(text, "content to delete\n");
            }
            _ => panic!("Expected Delete op"),
        }
    }

    #[test]
    fn test_to_unified_format() {
        let ops = vec![
            DiffOp::Equal {
                old_range: 0..10,
                new_range: 0..10,
                text: "same\n".to_string(),
            },
            DiffOp::Delete {
                old_range: 10..20,
                text: "deleted\n".to_string(),
            },
            DiffOp::Insert {
                new_range: 10..20,
                text: "added\n".to_string(),
            },
        ];

        let unified = to_unified(&ops, 3);
        assert!(unified.contains(" same"));
        assert!(unified.contains("-deleted"));
        assert!(unified.contains("+added"));
    }

    #[test]
    fn test_to_side_by_side() {
        let ops = vec![
            DiffOp::Equal {
                old_range: 0..5,
                new_range: 0..5,
                text: "same\n".to_string(),
            },
            DiffOp::Delete {
                old_range: 5..10,
                text: "old\n".to_string(),
            },
            DiffOp::Insert {
                new_range: 5..10,
                text: "new\n".to_string(),
            },
        ];

        let lines = to_side_by_side(&ops);
        assert!(lines.len() >= 3);

        // Check equal line has both sides
        assert!(lines[0].left.is_some());
        assert!(lines[0].right.is_some());

        // Check deleted line has only left
        let deleted_line = lines.iter().find(|l| {
            l.left
                .as_ref()
                .map(|left| left.change_type == ChangeType::Removed)
                .unwrap_or(false)
        });
        assert!(deleted_line.is_some());

        // Check added line has only right
        let added_line = lines.iter().find(|l| {
            l.right
                .as_ref()
                .map(|right| right.change_type == ChangeType::Added)
                .unwrap_or(false)
        });
        assert!(added_line.is_some());
    }

    #[test]
    fn test_to_side_by_side_replace() {
        let ops = vec![DiffOp::Replace {
            old_range: 0..10,
            new_range: 0..15,
            old_text: "old1\nold2\n".to_string(),
            new_text: "new1\nnew2\nnew3\n".to_string(),
        }];

        let lines = to_side_by_side(&ops);
        assert!(lines.len() >= 3);

        // Should have modified lines
        let has_modified = lines.iter().any(|l| {
            l.left
                .as_ref()
                .map(|left| left.change_type == ChangeType::Modified)
                .unwrap_or(false)
        });
        assert!(has_modified);
    }

    #[test]
    fn test_diff_preserves_ranges() {
        let old = "abc\n";
        let new = "xyz\n";
        let ops = compute_diff(old, new);

        for op in ops {
            match op {
                DiffOp::Equal {
                    old_range,
                    new_range,
                    ..
                } => {
                    assert!(old_range.start <= old_range.end);
                    assert!(new_range.start <= new_range.end);
                }
                DiffOp::Delete { old_range, .. } => {
                    assert!(old_range.start <= old_range.end);
                }
                DiffOp::Insert { new_range, .. } => {
                    assert!(new_range.start <= new_range.end);
                }
                DiffOp::Replace {
                    old_range,
                    new_range,
                    ..
                } => {
                    assert!(old_range.start <= old_range.end);
                    assert!(new_range.start <= new_range.end);
                }
            }
        }
    }

    #[test]
    fn test_unified_format_empty_ops() {
        let ops = vec![];
        let unified = to_unified(&ops, 3);
        assert_eq!(unified, "");
    }

    #[test]
    fn test_side_by_side_empty_ops() {
        let ops = vec![];
        let lines = to_side_by_side(&ops);
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_to_unified_has_headers() {
        let ops = vec![
            DiffOp::Equal {
                old_range: 0..6,
                new_range: 0..6,
                text: "same\n".to_string(),
            },
            DiffOp::Delete {
                old_range: 6..14,
                text: "deleted\n".to_string(),
            },
            DiffOp::Insert {
                new_range: 6..12,
                text: "added\n".to_string(),
            },
        ];
        let unified = to_unified(&ops, 3);
        assert!(unified.starts_with("--- a\n+++ b\n"), "Missing unified diff headers");
    }

    #[test]
    fn test_to_unified_has_hunk_markers() {
        let ops = vec![
            DiffOp::Delete {
                old_range: 0..4,
                text: "old\n".to_string(),
            },
            DiffOp::Insert {
                new_range: 0..4,
                text: "new\n".to_string(),
            },
        ];
        let unified = to_unified(&ops, 3);
        assert!(unified.contains("@@"), "Missing hunk marker");
    }

    #[test]
    fn test_to_unified_context_lines() {
        let ops = vec![
            DiffOp::Equal {
                old_range: 0..30,
                new_range: 0..30,
                text: "line1\nline2\nline3\nline4\nline5\n".to_string(),
            },
            DiffOp::Delete {
                old_range: 30..40,
                text: "removed\n".to_string(),
            },
            DiffOp::Equal {
                old_range: 40..70,
                new_range: 30..60,
                text: "line6\nline7\nline8\nline9\nline10\n".to_string(),
            },
        ];
        // With context_lines=1, should only show 1 context line before/after
        let unified = to_unified(&ops, 1);
        // The hunk should NOT include line1 (too far from the change)
        let lines: Vec<&str> = unified.lines().collect();
        // Should have --- a, +++ b, @@, then context + changes
        assert!(lines.len() < 12, "context_lines=1 should limit output, got {} lines", lines.len());
    }

    #[test]
    fn test_to_unified_no_changes() {
        let ops = vec![DiffOp::Equal {
            old_range: 0..5,
            new_range: 0..5,
            text: "same\n".to_string(),
        }];
        let unified = to_unified(&ops, 3);
        assert_eq!(unified, "", "No changes should produce empty output");
    }

    #[test]
    fn test_to_side_by_side_multiline_replace_alignment() {
        let ops = vec![DiffOp::Replace {
            old_range: 0..10,
            new_range: 0..20,
            old_text: "a\n".to_string(),
            new_text: "x\ny\nz\n".to_string(),
        }];
        let lines = to_side_by_side(&ops);
        assert_eq!(lines.len(), 3, "Should have 3 rows for max(1,3) lines");
        // First row: both sides present
        assert!(lines[0].left.is_some());
        assert!(lines[0].right.is_some());
        // Row 2,3: only right side
        assert!(lines[1].left.is_none());
        assert!(lines[1].right.is_some());
        assert!(lines[2].left.is_none());
        assert!(lines[2].right.is_some());
    }
}
