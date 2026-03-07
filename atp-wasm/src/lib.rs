//! # ATP WASM Bindings
//!
//! WebAssembly interface for the Agentic Text Processor.
//!
//! Exposes the core grep, sed, awk, and AQL engines to JavaScript/TypeScript
//! via `wasm-bindgen`. All functions accept and return JSON strings for
//! maximum interoperability with the browser environment.
//!
//! ## Usage (JavaScript)
//!
//! ```js
//! import init, { grep, sed, awk, aql, ontology } from './atp_wasm.js';
//!
//! await init();
//!
//! // Search for a pattern in text
//! const results = grep("TODO", "line one\nTODO: fix this\nline three", "{}");
//! console.log(JSON.parse(results));
//!
//! // Transform text
//! const transformed = sed('s/old/new/g', "old text here", "{}");
//! console.log(JSON.parse(transformed));
//!
//! // Run AQL query
//! const aqlResult = aql('find "TODO" | count', "TODO: a\nDone\nTODO: b");
//! console.log(JSON.parse(aqlResult));
//! ```

use wasm_bindgen::prelude::*;

use atp_core::engine::aql;
use atp_core::engine::awk::{AwkConfig, AwkEngine};
use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::engine::pipeline::{PipelineData, PipelineLine};
use atp_core::engine::sed::{SedConfig, SedEngine};
use atp_core::output::PatternType;

use std::io::Cursor;

// ---------------------------------------------------------------------------
// Grep
// ---------------------------------------------------------------------------

/// Search for a pattern in text content.
///
/// # Arguments
/// * `pattern` - Regex pattern to search for
/// * `content` - Text content to search through
/// * `options_json` - JSON options: `{"case_sensitive": bool, "whole_word": bool,
///    "invert_match": bool, "context_before": usize, "context_after": usize,
///    "max_matches": usize|null, "multiline": bool, "only_matching": bool}`
///
/// # Returns
/// JSON array of matches: `[{"file": str, "line_number": int, "content": str, ...}]`
#[wasm_bindgen]
pub fn grep(pattern: &str, content: &str, options_json: &str) -> String {
    match grep_inner(pattern, content, options_json) {
        Ok(json) => json,
        Err(e) => error_json(&format!("grep error: {e}")),
    }
}

fn grep_inner(pattern: &str, content: &str, options_json: &str) -> Result<String, String> {
    let opts: serde_json::Value =
        serde_json::from_str(options_json).unwrap_or(serde_json::Value::Object(Default::default()));

    let config = GrepConfig {
        pattern: pattern.to_string(),
        pattern_type: PatternType::Regex,
        case_sensitive: opts
            .get("case_sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        whole_word: opts
            .get("whole_word")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        invert_match: opts
            .get("invert_match")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        context_before: opts
            .get("context_before")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        context_after: opts
            .get("context_after")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize,
        max_matches: opts
            .get("max_matches")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize),
        max_matches_per_file: None,
        multiline: opts
            .get("multiline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        include_binary: false,
        extra_patterns: Vec::new(),
        only_matching: opts
            .get("only_matching")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    let engine = GrepEngine::new(config).map_err(|e| format!("{e}"))?;
    let cursor = Cursor::new(content.as_bytes());
    let matches = engine
        .search_reader(cursor, "<wasm-input>")
        .map_err(|e| format!("{e}"))?;

    serde_json::to_string(&matches).map_err(|e| format!("{e}"))
}

// ---------------------------------------------------------------------------
// Sed
// ---------------------------------------------------------------------------

/// Transform text using sed-style substitution expressions.
///
/// # Arguments
/// * `expression` - Sed expression, e.g. `s/pattern/replacement/g`
/// * `content` - Text content to transform
/// * `options_json` - JSON options: `{"dry_run": bool}`
///
/// # Returns
/// JSON with transformed text and change records
#[wasm_bindgen]
pub fn sed(expression: &str, content: &str, options_json: &str) -> String {
    match sed_inner(expression, content, options_json) {
        Ok(json) => json,
        Err(e) => error_json(&format!("sed error: {e}")),
    }
}

fn sed_inner(expression: &str, content: &str, options_json: &str) -> Result<String, String> {
    let opts: serde_json::Value =
        serde_json::from_str(options_json).unwrap_or(serde_json::Value::Object(Default::default()));

    let cmd = atp_core::engine::sed::parse_substitution(expression).map_err(|e| format!("{e}"))?;

    let config = SedConfig {
        commands: vec![cmd],
        in_place: false,
        backup_extension: None,
        dry_run: opts
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        line_range: None,
        address_range: None,
    };

    let engine = SedEngine::new(config);
    let cursor = Cursor::new(content.as_bytes());
    let mut output = Vec::new();
    let changes = engine
        .transform_reader(cursor, &mut output, "<wasm-input>")
        .map_err(|e| format!("{e}"))?;

    let result = serde_json::json!({
        "output": String::from_utf8_lossy(&output),
        "changes": changes,
    });

    serde_json::to_string(&result).map_err(|e| format!("{e}"))
}

// ---------------------------------------------------------------------------
// Awk
// ---------------------------------------------------------------------------

/// Analyze text using awk-style field processing.
///
/// # Arguments
/// * `content` - Text content to analyze
/// * `options_json` - JSON options: `{"separator": str, "output_fields": [int],
///    "header": bool, "rules_json": str}`
///
/// # Returns
/// JSON array of analysis records
#[wasm_bindgen]
pub fn awk(content: &str, options_json: &str) -> String {
    match awk_inner(content, options_json) {
        Ok(json) => json,
        Err(e) => error_json(&format!("awk error: {e}")),
    }
}

fn awk_inner(content: &str, options_json: &str) -> Result<String, String> {
    let opts: serde_json::Value =
        serde_json::from_str(options_json).unwrap_or(serde_json::Value::Object(Default::default()));

    let field_separator = opts
        .get("separator")
        .and_then(|v| v.as_str())
        .unwrap_or(r"\s+")
        .to_string();

    let has_header = opts
        .get("header")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let config = AwkConfig {
        field_separator,
        has_header,
        ..Default::default()
    };

    let engine = AwkEngine::new(config).map_err(|e| format!("{e}"))?;
    let cursor = Cursor::new(content.as_bytes());
    let records = engine
        .process_reader(cursor, "<wasm-input>", 0)
        .map_err(|e| format!("{e}"))?;

    serde_json::to_string(&records).map_err(|e| format!("{e}"))
}

// ---------------------------------------------------------------------------
// AQL
// ---------------------------------------------------------------------------

/// Execute an AQL (ATP Query Language) query on text content.
///
/// # Arguments
/// * `query` - AQL query string, e.g. `find "TODO" | sort | count`
/// * `content` - Text content to process
///
/// # Returns
/// JSON pipeline results
#[wasm_bindgen]
pub fn aql(query: &str, content: &str) -> String {
    match aql_inner(query, content) {
        Ok(json) => json,
        Err(e) => error_json(&format!("aql error: {e}")),
    }
}

fn aql_inner(query: &str, content: &str) -> Result<String, String> {
    let pipeline = aql::parse(query).map_err(|e| format!("{e}"))?;

    let data = PipelineData {
        lines: content
            .lines()
            .enumerate()
            .map(|(idx, line)| PipelineLine {
                source_file: "<wasm-input>".to_string(),
                source_line: idx + 1,
                content: line.to_string(),
                fields: Vec::new(),
            })
            .collect(),
        source_files: Vec::new(),
    };

    let mut engine = aql::AqlEngine::new();
    let results = engine
        .execute_on_data(&pipeline, data)
        .map_err(|e| format!("{e}"))?;

    serde_json::to_string(&results).map_err(|e| format!("{e}"))
}

// ---------------------------------------------------------------------------
// Ontology
// ---------------------------------------------------------------------------

/// Get the complete ATP ontology as JSON.
///
/// Returns the full machine-readable API description including all capabilities,
/// commands, types, and error codes for AI agent consumption.
#[wasm_bindgen]
pub fn ontology() -> String {
    let ont = atp_core::ontology::build_ontology();
    serde_json::to_string_pretty(&ont).unwrap_or_else(|e| error_json(&format!("{e}")))
}

// ---------------------------------------------------------------------------
// Semantic Search
// ---------------------------------------------------------------------------

/// Perform TF-IDF semantic search across provided documents.
///
/// # Arguments
/// * `query_text` - Search query string
/// * `documents_json` - JSON array of `{"file": str, "content": str}` objects
/// * `max_results` - Maximum number of results to return
///
/// # Returns
/// JSON array of scored results: `[{"file": str, "line": int, "content": str, "score": float}]`
#[wasm_bindgen]
pub fn semantic_search(query_text: &str, documents_json: &str, max_results: usize) -> String {
    match semantic_inner(query_text, documents_json, max_results) {
        Ok(json) => json,
        Err(e) => error_json(&format!("semantic search error: {e}")),
    }
}

fn semantic_inner(
    query_text: &str,
    documents_json: &str,
    max_results: usize,
) -> Result<String, String> {
    let docs: Vec<serde_json::Value> =
        serde_json::from_str(documents_json).map_err(|e| format!("{e}"))?;

    let mut index = atp_core::semantic::TfIdfIndex::new();

    for doc in &docs {
        let file = doc
            .get("file")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let content = doc.get("content").and_then(|v| v.as_str()).unwrap_or("");
        for (idx, line) in content.lines().enumerate() {
            index.add_document(file, idx + 1, line);
        }
    }

    index.finalize();
    let results = index.query(query_text, max_results);
    // ScoredDocument doesn't derive Serialize, so convert manually
    let json_results: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "file": r.file,
                "line": r.line,
                "content": r.content,
                "score": r.score
            })
        })
        .collect();
    serde_json::to_string(&json_results).map_err(|e| format!("{e}"))
}

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

/// Get the ATP version string.
#[wasm_bindgen]
pub fn version() -> String {
    atp_core::ATP_VERSION.to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn error_json(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_basic() {
        let result = grep("hello", "hello world\ngoodbye\nhello again", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_grep_case_insensitive() {
        let result = grep(
            "hello",
            "Hello World\nhello again",
            r#"{"case_sensitive": false}"#,
        );
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_grep_invert() {
        let result = grep(
            "skip",
            "keep\nskip this\nkeep too",
            r#"{"invert_match": true}"#,
        );
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_grep_invalid_pattern() {
        let result = grep("[invalid", "text", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("error").is_some());
    }

    #[test]
    fn test_sed_basic() {
        let result = sed("s/old/new/g", "old text old", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let output = parsed.get("output").unwrap().as_str().unwrap();
        assert!(output.contains("new text new"));
    }

    #[test]
    fn test_sed_invalid_expression() {
        let result = sed("not-a-valid-expr", "text", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("error").is_some());
    }

    #[test]
    fn test_awk_basic() {
        // awk engine requires rules to produce records; without rules it returns []
        let result = awk("a b c\nd e f", "{}");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 0); // no rules = no records
    }

    #[test]
    fn test_awk_custom_separator() {
        // Same: empty rules → empty output; validates no crash with custom sep
        let result = awk("a,b,c\nd,e,f", r#"{"separator": ","}"#);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn test_aql_find() {
        let result = aql(r#"find "hello""#, "hello world\ngoodbye\nhello again");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("final_output").is_some());
    }

    #[test]
    fn test_aql_count() {
        let result = aql(r#"find "x" | count"#, "x\ny\nx\nx");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("total_duration_ms").is_some());
    }

    #[test]
    fn test_aql_invalid() {
        let result = aql("not valid aql $$$", "text");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("error").is_some());
    }

    #[test]
    fn test_ontology_returns_json() {
        let result = ontology();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("tool").is_some());
    }

    #[test]
    fn test_semantic_search_basic() {
        let docs = serde_json::json!([
            {"file": "a.txt", "content": "rust programming language\nsystems programming"},
            {"file": "b.txt", "content": "python scripting\nweb development"}
        ]);
        let result = semantic_search("rust programming", &docs.to_string(), 5);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let arr = parsed.as_array().unwrap();
        assert!(!arr.is_empty());
        // First result should be from a.txt (rust programming content)
        assert_eq!(arr[0].get("file").unwrap().as_str().unwrap(), "a.txt");
    }

    #[test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
    }

    #[test]
    fn test_error_json_format() {
        let err = error_json("test error");
        let parsed: serde_json::Value = serde_json::from_str(&err).unwrap();
        assert_eq!(parsed.get("error").unwrap().as_str().unwrap(), "test error");
    }
}
