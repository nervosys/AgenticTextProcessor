//! Property-based tests for ATP core engines using proptest.
//!
//! These tests verify invariants that should hold for all inputs,
//! not just hand-picked examples.

use proptest::prelude::*;
use std::io::Cursor;

use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::engine::sed::{SedConfig, SedEngine, TransformCommand};
use atp_core::output::PatternType;

// ── Strategies ──────────────────────────────────────────────────────────

/// Generate arbitrary multi-line text.
fn arb_text() -> impl Strategy<Value = String> {
    prop::collection::vec("[^\x00]{0,120}", 0..50).prop_map(|lines| lines.join("\n") + "\n")
}

/// Generate a valid regex-safe literal pattern (no regex metacharacters).
fn arb_literal() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_ ]{1,20}"
}

// ── Grep invariants ─────────────────────────────────────────────────────

proptest! {
    /// Grep never crashes on arbitrary text.
    #[test]
    fn grep_no_crash(text in arb_text(), pattern in "[a-zA-Z0-9]{1,10}") {
        let config = GrepConfig {
            pattern,
            pattern_type: PatternType::Regex,
            ..Default::default()
        };
        if let Ok(engine) = GrepEngine::new(config) {
            let cursor = Cursor::new(text.as_bytes());
            let _ = engine.search_reader(cursor, "prop");
        }
    }

    /// Grep match count is always <= line count.
    #[test]
    fn grep_matches_leq_lines(text in arb_text(), pattern in "[a-zA-Z]{1,5}") {
        let line_count = text.lines().count();
        let config = GrepConfig {
            pattern,
            pattern_type: PatternType::Regex,
            ..Default::default()
        };
        if let Ok(engine) = GrepEngine::new(config) {
            let cursor = Cursor::new(text.as_bytes());
            if let Ok(results) = engine.search_reader(cursor, "prop") {
                prop_assert!(results.len() <= line_count);
            }
        }
    }

    /// Grep with invert_match: matched lines + inverted lines cover all lines.
    #[test]
    fn grep_invert_partition(text in arb_text(), pattern in "[a-zA-Z]{1,4}") {
        let line_count = text.lines().count();

        let config_normal = GrepConfig {
            pattern: pattern.clone(),
            pattern_type: PatternType::Regex,
            ..Default::default()
        };
        let config_invert = GrepConfig {
            pattern,
            pattern_type: PatternType::Regex,
            invert_match: true,
            ..Default::default()
        };

        if let (Ok(engine_n), Ok(engine_i)) = (GrepEngine::new(config_normal), GrepEngine::new(config_invert)) {
            let cursor_n = Cursor::new(text.as_bytes());
            let cursor_i = Cursor::new(text.as_bytes());
            if let (Ok(normal), Ok(inverted)) = (
                engine_n.search_reader(cursor_n, "prop"),
                engine_i.search_reader(cursor_i, "prop"),
            ) {
                // Count unique lines (grep may return multiple matches per line)
                use std::collections::HashSet;
                let normal_lines: HashSet<usize> = normal.iter().map(|m| m.line_number).collect();
                let inverted_lines: HashSet<usize> = inverted.iter().map(|m| m.line_number).collect();
                // Normal and inverted should be disjoint
                prop_assert!(normal_lines.is_disjoint(&inverted_lines));
                // Together they should cover all lines
                prop_assert_eq!(normal_lines.len() + inverted_lines.len(), line_count);
            }
        }
    }

    /// Grep with max_matches: result count is finite and doesn't crash.
    #[test]
    fn grep_max_matches(text in arb_text(), pattern in "[a-z]{1,3}", max in 1usize..20) {
        let config = GrepConfig {
            pattern,
            pattern_type: PatternType::Regex,
            max_matches: Some(max),
            ..Default::default()
        };
        if let Ok(engine) = GrepEngine::new(config) {
            let cursor = Cursor::new(text.as_bytes());
            // Just verify it doesn't crash and returns a finite result
            let _ = engine.search_reader(cursor, "prop");
        }
    }

    /// Literal search finds only exact substrings.
    #[test]
    fn grep_literal_exact(text in arb_text(), needle in arb_literal()) {
        let config = GrepConfig {
            pattern: needle.clone(),
            pattern_type: PatternType::Literal,
            ..Default::default()
        };
        if let Ok(engine) = GrepEngine::new(config) {
            let cursor = Cursor::new(text.as_bytes());
            if let Ok(results) = engine.search_reader(cursor, "prop") {
                for m in &results {
                    prop_assert!(m.line_content.contains(&needle),
                        "Literal match on line that doesn't contain needle");
                }
            }
        }
    }
}

// ── Sed invariants ──────────────────────────────────────────────────────

proptest! {
    /// Sed never crashes on arbitrary text.
    #[test]
    fn sed_no_crash(text in arb_text(), from in "[a-z]{1,5}", to in "[A-Z]{0,5}") {
        let cmd = TransformCommand::Substitute {
            pattern: from,
            replacement: to,
            global: true,
            case_insensitive: false,
        };
        let config = SedConfig {
            commands: vec![cmd],
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let cursor = Cursor::new(text.as_bytes());
        let mut out = Vec::new();
        let _ = engine.transform_reader(cursor, &mut out, "prop");
    }

    /// Sed substitution preserves line count.
    #[test]
    fn sed_preserves_line_count(text in arb_text(), from in "[a-z]{1,3}", to in "[A-Z]{0,3}") {
        let original_lines = text.lines().count();
        let cmd = TransformCommand::Substitute {
            pattern: from,
            replacement: to,
            global: true,
            case_insensitive: false,
        };
        let config = SedConfig {
            commands: vec![cmd],
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let cursor = Cursor::new(text.as_bytes());
        let mut out = Vec::new();
        if engine.transform_reader(cursor, &mut out, "prop").is_ok() {
            let output_text = String::from_utf8_lossy(&out);
            let result_lines = output_text.lines().count();
            prop_assert_eq!(original_lines, result_lines,
                "Substitution should not change line count");
        }
    }

    /// Sed no-op: replacing "X" with "X" yields same output as input (modulo line endings).
    #[test]
    fn sed_identity(text in arb_text(), pat in "[a-z]{1,3}") {
        let cmd = TransformCommand::Substitute {
            pattern: pat.clone(),
            replacement: pat,
            global: true,
            case_insensitive: false,
        };
        let config = SedConfig {
            commands: vec![cmd],
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let cursor = Cursor::new(text.as_bytes());
        let mut out = Vec::new();
        if engine.transform_reader(cursor, &mut out, "prop").is_ok() {
            let output = String::from_utf8_lossy(&out);
            // Normalize all CR/CRLF for comparison (sed normalizes line endings)
            let expected = text.replace('\r', "");
            let actual = output.replace('\r', "");
            prop_assert_eq!(expected.trim_end(), actual.trim_end(),
                "Replacing pattern with itself should be identity");
        }
    }
}

// ── AQL invariants ──────────────────────────────────────────────────────

proptest! {
    /// AQL parse never crashes on arbitrary strings.
    #[test]
    fn aql_parse_no_crash(query in "[ a-zA-Z0-9|\"'_.><=!,/]{0,100}") {
        let _ = atp_core::engine::aql::parse(&query);
    }

    /// AQL count always yields exactly 1 output record.
    #[test]
    fn aql_count_is_one(text in arb_text()) {
        let pipeline = atp_core::engine::aql::parse("count").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input.txt");
        std::fs::write(&file, &text).unwrap();
        let mut engine = atp_core::AqlEngine::new();
        let results = engine.execute(&pipeline, &[file.as_path()]).unwrap();
        prop_assert_eq!(results.stages.last().unwrap().records_out, 1);
    }

    /// AQL sort | unique never increases record count.
    #[test]
    fn aql_unique_shrinks(text in arb_text()) {
        let line_count = text.lines().count();
        let pipeline = atp_core::engine::aql::parse("sort | unique").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input.txt");
        std::fs::write(&file, &text).unwrap();
        let mut engine = atp_core::AqlEngine::new();
        let results = engine.execute(&pipeline, &[file.as_path()]).unwrap();
        let unique_count = results.stages.last().unwrap().records_out;
        prop_assert!(unique_count <= line_count);
    }
}
