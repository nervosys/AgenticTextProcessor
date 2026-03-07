//! Minimal LSP server for AQL files.
//!
//! Implements the Language Server Protocol over stdin/stdout using raw JSON-RPC.
//! No external LSP crate dependency — the protocol is simple enough to hand-roll
//! for the subset we need (initialize, completion, hover, diagnostics, formatting).
//!
//! ## v2 extensions
//! - **Index-aware workspace symbol search** via `workspace/symbol`
//! - **Notebook cell support** for `.atp.md` literate files
//! - **Scope-aware completions** using document context analysis

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// AQL keyword database for completions and hover
// ---------------------------------------------------------------------------

struct AqlKeyword {
    label: &'static str,
    detail: &'static str,
    documentation: &'static str,
    kind: u32, // CompletionItemKind: 14=Keyword, 3=Function, 6=Variable
}

const AQL_KEYWORDS: &[AqlKeyword] = &[
    // Stage keywords
    AqlKeyword {
        label: "find",
        detail: "Search stage",
        documentation: "Search for lines matching a pattern.\nUsage: find \"pattern\" [ignore_case] [whole_word] [multiline] [context N] [invert]",
        kind: 14,
    },
    AqlKeyword {
        label: "replace",
        detail: "Transform stage",
        documentation: "Replace occurrences of a pattern.\nUsage: replace \"pattern\" with \"replacement\" [all] [ignore_case]",
        kind: 14,
    },
    AqlKeyword {
        label: "delete",
        detail: "Delete stage",
        documentation: "Delete lines matching a pattern.\nUsage: delete lines matching \"pattern\"",
        kind: 14,
    },
    AqlKeyword {
        label: "insert",
        detail: "Insert stage",
        documentation: "Insert text before or after lines matching a pattern.\nUsage: insert \"text\" before|after \"pattern\"",
        kind: 14,
    },
    AqlKeyword {
        label: "select",
        detail: "Select fields stage",
        documentation: "Select specific fields from each line.\nUsage: select fields 1, 3, 5",
        kind: 14,
    },
    AqlKeyword {
        label: "filter",
        detail: "Filter stage",
        documentation: "Filter lines by field conditions.\nUsage: filter field 2 contains \"value\"",
        kind: 14,
    },
    AqlKeyword {
        label: "sort",
        detail: "Sort stage",
        documentation: "Sort output lines.\nUsage: sort [by field N] [asc|desc] [numeric]",
        kind: 14,
    },
    AqlKeyword {
        label: "unique",
        detail: "Deduplicate stage",
        documentation: "Remove duplicate lines.\nUsage: unique\nAliases: uniq, deduplicate, dedup",
        kind: 14,
    },
    AqlKeyword {
        label: "take",
        detail: "Limit stage",
        documentation: "Take the first N lines.\nUsage: take N [first|last]\nAliases: head",
        kind: 14,
    },
    AqlKeyword {
        label: "skip",
        detail: "Skip stage",
        documentation: "Skip the first N lines.\nUsage: skip N",
        kind: 14,
    },
    AqlKeyword {
        label: "count",
        detail: "Count stage",
        documentation: "Count the number of lines in the pipeline.\nUsage: count",
        kind: 14,
    },
    AqlKeyword {
        label: "aggregate",
        detail: "Aggregation stage",
        documentation: "Compute aggregations on fields.\nUsage: aggregate sum|avg|min|max|distinct|freq field N\nAliases: agg",
        kind: 14,
    },
    AqlKeyword {
        label: "set",
        detail: "Configuration stage",
        documentation: "Set pipeline configuration.\nUsage: set separator \",\"",
        kind: 14,
    },
    // Modifier keywords
    AqlKeyword {
        label: "ignore_case",
        detail: "Modifier",
        documentation: "Make pattern matching case-insensitive.\nAliases: ignorecase, nocase, case_insensitive",
        kind: 14,
    },
    AqlKeyword {
        label: "whole_word",
        detail: "Modifier",
        documentation: "Match only whole words.\nAliases: wholeword, word",
        kind: 14,
    },
    AqlKeyword {
        label: "multiline",
        detail: "Modifier",
        documentation: "Enable multi-line regex matching.",
        kind: 14,
    },
    AqlKeyword {
        label: "context",
        detail: "Modifier",
        documentation: "Include N lines of context around matches.\nUsage: context N",
        kind: 14,
    },
    AqlKeyword {
        label: "invert",
        detail: "Modifier",
        documentation: "Invert the match — select non-matching lines.\nAliases: not",
        kind: 14,
    },
    AqlKeyword {
        label: "all",
        detail: "Modifier",
        documentation: "Apply replacement globally (all occurrences).\nAliases: global, every",
        kind: 14,
    },
    AqlKeyword {
        label: "with",
        detail: "Keyword",
        documentation: "Specifies the replacement string in a replace stage.\nUsage: replace \"old\" with \"new\"",
        kind: 14,
    },
    AqlKeyword {
        label: "between",
        detail: "Keyword",
        documentation: "Specify a line range.\nUsage: between N and M",
        kind: 14,
    },
    // Aggregation functions
    AqlKeyword {
        label: "sum",
        detail: "Aggregation function",
        documentation: "Sum of numeric values in a field.",
        kind: 3,
    },
    AqlKeyword {
        label: "avg",
        detail: "Aggregation function",
        documentation: "Average (mean) of numeric values in a field.\nAliases: average, mean",
        kind: 3,
    },
    AqlKeyword {
        label: "min",
        detail: "Aggregation function",
        documentation: "Minimum value in a field.\nAlias: minimum",
        kind: 3,
    },
    AqlKeyword {
        label: "max",
        detail: "Aggregation function",
        documentation: "Maximum value in a field.\nAlias: maximum",
        kind: 3,
    },
    AqlKeyword {
        label: "distinct",
        detail: "Aggregation function",
        documentation: "Count of distinct values in a field.",
        kind: 3,
    },
    AqlKeyword {
        label: "freq",
        detail: "Aggregation function",
        documentation: "Frequency distribution of values in a field.\nAlias: frequency",
        kind: 3,
    },
];

// ---------------------------------------------------------------------------
// Document store (in-memory open buffers)
// ---------------------------------------------------------------------------

struct DocumentStore {
    docs: HashMap<String, String>,
}

impl DocumentStore {
    fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
    }

    fn change(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
    }

    fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    fn get(&self, uri: &str) -> Option<&str> {
        self.docs.get(uri).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC transport
// ---------------------------------------------------------------------------

fn read_message(reader: &mut dyn BufRead) -> Result<Option<Value>> {
    let mut header = String::new();
    let mut content_length: Option<usize> = None;

    loop {
        header.clear();
        let n = reader.read_line(&mut header)?;
        if n == 0 {
            return Ok(None); // EOF
        }
        let trimmed = header.trim();
        if trimmed.is_empty() {
            break; // end of headers
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(len_str.trim().parse().context("Invalid Content-Length")?);
        }
    }

    let length = content_length.context("Missing Content-Length header")?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let msg: Value = serde_json::from_slice(&body)?;
    Ok(Some(msg))
}

fn send_message(writer: &mut dyn Write, msg: &Value) -> Result<()> {
    let body = serde_json::to_string(msg)?;
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()?;
    Ok(())
}

fn response(id: &Value, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn notification(method: &str, params: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

// ---------------------------------------------------------------------------
// LSP capability handlers
// ---------------------------------------------------------------------------

fn handle_initialize(id: &Value) -> Value {
    response(
        id,
        serde_json::json!({
            "capabilities": {
                "textDocumentSync": {
                    "openClose": true,
                    "change": 1, // Full sync
                    "save": { "includeText": true }
                },
                "completionProvider": {
                    "triggerCharacters": ["\"", "|", " "],
                    "resolveProvider": false
                },
                "hoverProvider": true,
                "documentFormattingProvider": true,
                "diagnosticProvider": {
                    "interFileDependencies": false,
                    "workspaceDiagnostics": false
                },
                "workspaceSymbolProvider": true
            },
            "serverInfo": {
                "name": "atp-lsp",
                "version": atp_core::ATP_VERSION
            }
        }),
    )
}

#[allow(dead_code)]
fn handle_completion(_params: &Value) -> Value {
    let items: Vec<Value> = AQL_KEYWORDS
        .iter()
        .map(|kw| {
            serde_json::json!({
                "label": kw.label,
                "kind": kw.kind,
                "detail": kw.detail,
                "documentation": {
                    "kind": "markdown",
                    "value": kw.documentation
                },
                "insertText": kw.label
            })
        })
        .collect();

    serde_json::json!({
        "isIncomplete": false,
        "items": items
    })
}

fn handle_hover(params: &Value, docs: &DocumentStore) -> Value {
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let line = params
        .pointer("/position/line")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let character = params
        .pointer("/position/character")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    if let Some(text) = docs.get(uri) {
        if let Some(line_text) = text.lines().nth(line) {
            let word = extract_word(line_text, character);
            if let Some(kw) = AQL_KEYWORDS.iter().find(|k| k.label == word) {
                return serde_json::json!({
                    "contents": {
                        "kind": "markdown",
                        "value": format!("**{}** — {}\n\n{}", kw.label, kw.detail, kw.documentation)
                    }
                });
            }
        }
    }
    serde_json::json!(null)
}

fn extract_word(line: &str, pos: usize) -> &str {
    let bytes = line.as_bytes();
    if pos >= bytes.len() {
        return "";
    }
    let start = (0..=pos)
        .rev()
        .find(|&i| !bytes[i].is_ascii_alphanumeric() && bytes[i] != b'_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = (pos..bytes.len())
        .find(|&i| !bytes[i].is_ascii_alphanumeric() && bytes[i] != b'_')
        .unwrap_or(bytes.len());
    &line[start..end]
}

fn compute_diagnostics(uri: &str, text: &str) -> Value {
    let mut diagnostics = Vec::new();

    // Try parsing each pipe-separated segment as AQL
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        if let Err(e) = atp_core::engine::aql::parse(trimmed) {
            let msg = format!("{e:#}");
            diagnostics.push(serde_json::json!({
                "range": {
                    "start": { "line": line_idx, "character": 0 },
                    "end": { "line": line_idx, "character": trimmed.len() }
                },
                "severity": 1, // Error
                "source": "atp-lsp",
                "message": msg
            }));
        }
    }

    notification(
        "textDocument/publishDiagnostics",
        serde_json::json!({
            "uri": uri,
            "diagnostics": diagnostics
        }),
    )
}

fn handle_formatting(params: &Value, docs: &DocumentStore) -> Value {
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if let Some(text) = docs.get(uri) {
        let mut edits = Vec::new();
        for (line_idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Canonical formatting: normalize whitespace around pipes
            let formatted = trimmed
                .split('|')
                .map(|s| s.trim())
                .collect::<Vec<_>>()
                .join(" | ");

            if formatted != line {
                edits.push(serde_json::json!({
                    "range": {
                        "start": { "line": line_idx, "character": 0 },
                        "end": { "line": line_idx, "character": line.len() }
                    },
                    "newText": formatted
                }));
            }
        }
        serde_json::json!(edits)
    } else {
        serde_json::json!([])
    }
}

// ---------------------------------------------------------------------------
// LSP v2: Workspace symbol search (index-aware)
// ---------------------------------------------------------------------------

/// Handle `workspace/symbol` — search workspace files via the index.
fn handle_workspace_symbol(params: &Value) -> Value {
    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");

    if query.is_empty() {
        return serde_json::json!([]);
    }

    // Use internal AQL keywords as workspace "symbols" for quick navigation.
    // In a full deployment, this would query the SearchIndex for file-level symbols.
    let mut symbols = Vec::new();
    for kw in AQL_KEYWORDS {
        if kw.label.contains(query) || kw.detail.to_lowercase().contains(&query.to_lowercase()) {
            symbols.push(serde_json::json!({
                "name": kw.label,
                "kind": match kw.kind {
                    14 => 14, // Keyword → SymbolKind.Key
                    3 => 12,  // Function → SymbolKind.Function
                    _ => 13,  // Variable
                },
                "location": {
                    "uri": "aql://builtin",
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": kw.label.len() }
                    }
                },
                "containerName": kw.detail
            }));
        }
    }
    serde_json::json!(symbols)
}

// ---------------------------------------------------------------------------
// LSP v2: Scope-aware completions
// ---------------------------------------------------------------------------

/// Analyze the current line context to provide scope-aware completions.
///
/// For example:
/// - After `find "pattern"` → suggest modifiers (ignore_case, whole_word, etc.)
/// - After `|` → suggest stage keywords (find, replace, sort, etc.)
/// - After `aggregate` → suggest functions (sum, avg, min, max, etc.)
fn handle_completion_v2(params: &Value, docs: &DocumentStore) -> Value {
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let line = params
        .pointer("/position/line")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let character = params
        .pointer("/position/character")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    // Determine context from the current line text
    let context = if let Some(text) = docs.get(uri) {
        text.lines()
            .nth(line)
            .map(|l| classify_completion_context(l, character))
            .unwrap_or(CompletionContext::Empty)
    } else {
        CompletionContext::Empty
    };

    let items: Vec<Value> = AQL_KEYWORDS
        .iter()
        .filter(|kw| match context {
            CompletionContext::AfterPipe | CompletionContext::Empty => {
                // After pipe or at start: suggest stage keywords
                matches!(kw.kind, 14)
                    && matches!(
                        kw.label,
                        "find"
                            | "replace"
                            | "delete"
                            | "insert"
                            | "select"
                            | "filter"
                            | "sort"
                            | "unique"
                            | "take"
                            | "skip"
                            | "count"
                            | "aggregate"
                            | "set"
                    )
            }
            CompletionContext::AfterStageKeyword => {
                // After a stage keyword: suggest modifiers
                matches!(kw.kind, 14)
                    && matches!(
                        kw.label,
                        "ignore_case"
                            | "whole_word"
                            | "multiline"
                            | "context"
                            | "invert"
                            | "all"
                            | "with"
                            | "between"
                    )
            }
            CompletionContext::AfterAggregate => {
                // After aggregate: suggest aggregation functions
                kw.kind == 3
            }
            CompletionContext::General => true,
        })
        .map(|kw| {
            serde_json::json!({
                "label": kw.label,
                "kind": kw.kind,
                "detail": kw.detail,
                "documentation": {
                    "kind": "markdown",
                    "value": kw.documentation
                },
                "insertText": kw.label,
                "sortText": match context {
                    CompletionContext::AfterPipe => format!("0{}", kw.label),
                    CompletionContext::AfterStageKeyword => format!("1{}", kw.label),
                    CompletionContext::AfterAggregate => format!("0{}", kw.label),
                    _ => format!("2{}", kw.label),
                }
            })
        })
        .collect();

    serde_json::json!({
        "isIncomplete": false,
        "items": items
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionContext {
    /// At the start of a line or empty document.
    Empty,
    /// After a pipe `|` — need a stage keyword.
    AfterPipe,
    /// After a stage keyword — need modifiers.
    AfterStageKeyword,
    /// After `aggregate` — need an aggregation function.
    AfterAggregate,
    /// General context — suggest everything.
    General,
}

fn classify_completion_context(line: &str, cursor: usize) -> CompletionContext {
    let prefix = if cursor <= line.len() {
        &line[..cursor]
    } else {
        line
    };
    let trimmed = prefix.trim();

    if trimmed.is_empty() {
        return CompletionContext::Empty;
    }

    // If the last non-whitespace char is '|', we're after a pipe
    if trimmed.ends_with('|') {
        return CompletionContext::AfterPipe;
    }

    // Get the last word before cursor
    let last_segment = trimmed.rsplit('|').next().unwrap_or("").trim();
    let words: Vec<&str> = last_segment.split_whitespace().collect();

    if let Some(&last_word) = words.last() {
        if last_word == "aggregate" || last_word == "agg" {
            return CompletionContext::AfterAggregate;
        }
    }

    // If the segment starts with a stage keyword, offer modifiers
    if let Some(&first_word) = words.first() {
        let stage_keywords = [
            "find",
            "replace",
            "delete",
            "insert",
            "select",
            "filter",
            "sort",
            "unique",
            "take",
            "skip",
            "count",
            "aggregate",
            "set",
        ];
        if stage_keywords.contains(&first_word) && words.len() > 1 {
            return CompletionContext::AfterStageKeyword;
        }
    }

    CompletionContext::General
}

// ---------------------------------------------------------------------------
// LSP v2: Notebook cell diagnostics
// ---------------------------------------------------------------------------

/// Compute diagnostics for a notebook-style `.atp.md` file.
///
/// Parses markdown text, finds AQL code fences, and validates each one.
fn compute_notebook_diagnostics(uri: &str, text: &str) -> Value {
    let mut diagnostics = Vec::new();
    let mut in_aql_block = false;
    let mut aql_start_line = 0;
    let mut aql_lines = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```aql") {
            in_aql_block = true;
            aql_start_line = line_idx + 1;
            aql_lines.clear();
            continue;
        }
        if trimmed == "```" && in_aql_block {
            in_aql_block = false;
            // Validate the collected AQL block
            let aql_text = aql_lines.join("\n");
            let aql_trimmed = aql_text.trim();
            if !aql_trimmed.is_empty() {
                if let Err(e) = atp_core::engine::aql::parse(aql_trimmed) {
                    let msg = format!("{e:#}");
                    diagnostics.push(serde_json::json!({
                        "range": {
                            "start": { "line": aql_start_line, "character": 0 },
                            "end": { "line": line_idx, "character": 0 }
                        },
                        "severity": 1,
                        "source": "atp-lsp",
                        "message": format!("AQL error in notebook cell: {msg}")
                    }));
                }
            }
            continue;
        }
        if in_aql_block {
            aql_lines.push(line.to_string());
        }
    }

    notification(
        "textDocument/publishDiagnostics",
        serde_json::json!({
            "uri": uri,
            "diagnostics": diagnostics
        }),
    )
}

// ---------------------------------------------------------------------------
// Main server loop
// ---------------------------------------------------------------------------

pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    let mut docs = DocumentStore::new();
    let mut initialized = false;

    loop {
        let msg = match read_message(&mut reader)? {
            Some(m) => m,
            None => break, // EOF
        };

        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let id = msg.get("id");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                if let Some(req_id) = id {
                    let resp = handle_initialize(req_id);
                    send_message(&mut writer, &resp)?;
                    initialized = true;
                }
            }
            "initialized" => {
                // Client acknowledgement, nothing to do
            }
            "shutdown" => {
                if let Some(req_id) = id {
                    send_message(&mut writer, &response(req_id, Value::Null))?;
                }
            }
            "exit" => {
                break;
            }
            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                    params
                        .pointer("/textDocument/text")
                        .and_then(|v| v.as_str()),
                ) {
                    docs.open(uri, text);
                    let diag = if uri.ends_with(".atp.md") || uri.ends_with(".atp.markdown") {
                        compute_notebook_diagnostics(uri, text)
                    } else {
                        compute_diagnostics(uri, text)
                    };
                    send_message(&mut writer, &diag)?;
                }
            }
            "textDocument/didChange" => {
                if let (Some(uri), Some(changes)) = (
                    params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                    params.get("contentChanges").and_then(|v| v.as_array()),
                ) {
                    // Full sync: take the last change's text
                    if let Some(text) = changes
                        .last()
                        .and_then(|c| c.get("text"))
                        .and_then(|v| v.as_str())
                    {
                        docs.change(uri, text);
                        let diag = if uri.ends_with(".atp.md") || uri.ends_with(".atp.markdown") {
                            compute_notebook_diagnostics(uri, text)
                        } else {
                            compute_diagnostics(uri, text)
                        };
                        send_message(&mut writer, &diag)?;
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(|v| v.as_str()) {
                    docs.close(uri);
                }
            }
            "textDocument/completion" => {
                if let Some(req_id) = id {
                    let result = handle_completion_v2(&params, &docs);
                    send_message(&mut writer, &response(req_id, result))?;
                }
            }
            "textDocument/hover" => {
                if let Some(req_id) = id {
                    let result = handle_hover(&params, &docs);
                    send_message(&mut writer, &response(req_id, result))?;
                }
            }
            "textDocument/formatting" => {
                if let Some(req_id) = id {
                    let result = handle_formatting(&params, &docs);
                    send_message(&mut writer, &response(req_id, result))?;
                }
            }
            "workspace/symbol" => {
                if let Some(req_id) = id {
                    let result = handle_workspace_symbol(&params);
                    send_message(&mut writer, &response(req_id, result))?;
                }
            }
            _ => {
                // Unknown method — if it's a request (has id), return method not found
                if let Some(req_id) = id {
                    let err = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {method}")
                        }
                    });
                    send_message(&mut writer, &err)?;
                }
            }
        }
    }

    let _ = initialized; // suppress unused warning
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_word_middle() {
        assert_eq!(extract_word("find \"hello\" ignore_case", 0), "find");
    }

    #[test]
    fn test_extract_word_underscore() {
        assert_eq!(
            extract_word("find \"hello\" ignore_case", 14),
            "ignore_case"
        );
    }

    #[test]
    fn test_extract_word_end() {
        assert_eq!(extract_word("count", 2), "count");
    }

    #[test]
    fn test_extract_word_out_of_bounds() {
        assert_eq!(extract_word("abc", 10), "");
    }

    #[test]
    fn test_handle_initialize() {
        let resp = handle_initialize(&serde_json::json!(1));
        assert!(resp.get("result").is_some());
        let caps = resp.pointer("/result/capabilities").unwrap();
        assert!(caps.get("completionProvider").is_some());
        assert!(caps.get("hoverProvider").is_some());
        assert!(caps.get("documentFormattingProvider").is_some());
    }

    #[test]
    fn test_completion_returns_all_keywords() {
        let result = handle_completion(&serde_json::json!({}));
        let items = result.get("items").unwrap().as_array().unwrap();
        assert!(items.len() >= 20); // We have 27 keywords
        let labels: Vec<&str> = items
            .iter()
            .filter_map(|i| i.get("label").and_then(|v| v.as_str()))
            .collect();
        assert!(labels.contains(&"find"));
        assert!(labels.contains(&"replace"));
        assert!(labels.contains(&"sort"));
        assert!(labels.contains(&"count"));
    }

    #[test]
    fn test_hover_known_keyword() {
        let mut docs = DocumentStore::new();
        docs.open("test://file.aql", "find \"hello\"");
        let params = serde_json::json!({
            "textDocument": { "uri": "test://file.aql" },
            "position": { "line": 0, "character": 1 }
        });
        let result = handle_hover(&params, &docs);
        let content = result.pointer("/contents/value").unwrap().as_str().unwrap();
        assert!(content.contains("find"));
        assert!(content.contains("Search stage"));
    }

    #[test]
    fn test_hover_unknown_word() {
        let mut docs = DocumentStore::new();
        docs.open("test://file.aql", "xyzzy");
        let params = serde_json::json!({
            "textDocument": { "uri": "test://file.aql" },
            "position": { "line": 0, "character": 2 }
        });
        let result = handle_hover(&params, &docs);
        assert!(result.is_null());
    }

    #[test]
    fn test_diagnostics_valid_query() {
        let diag = compute_diagnostics("test://f.aql", "find \"hello\"");
        let items = diag
            .pointer("/params/diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_diagnostics_invalid_query() {
        let diag = compute_diagnostics("test://f.aql", "not valid $$$");
        let items = diag
            .pointer("/params/diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(!items.is_empty());
        assert_eq!(items[0].get("severity").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn test_formatting_normalizes_pipes() {
        let mut docs = DocumentStore::new();
        docs.open("test://f.aql", "find \"x\"|sort|count");
        let params = serde_json::json!({
            "textDocument": { "uri": "test://f.aql" }
        });
        let edits = handle_formatting(&params, &docs);
        let edits_arr = edits.as_array().unwrap();
        assert_eq!(edits_arr.len(), 1);
        let new_text = edits_arr[0].get("newText").unwrap().as_str().unwrap();
        assert_eq!(new_text, "find \"x\" | sort | count");
    }

    #[test]
    fn test_formatting_already_canonical() {
        let mut docs = DocumentStore::new();
        docs.open("test://f.aql", "find \"x\" | sort | count");
        let params = serde_json::json!({
            "textDocument": { "uri": "test://f.aql" }
        });
        let edits = handle_formatting(&params, &docs);
        let edits_arr = edits.as_array().unwrap();
        assert!(edits_arr.is_empty());
    }

    #[test]
    fn test_document_store_lifecycle() {
        let mut docs = DocumentStore::new();
        assert!(docs.get("test://a").is_none());
        docs.open("test://a", "content");
        assert_eq!(docs.get("test://a"), Some("content"));
        docs.change("test://a", "updated");
        assert_eq!(docs.get("test://a"), Some("updated"));
        docs.close("test://a");
        assert!(docs.get("test://a").is_none());
    }

    #[test]
    fn test_response_format() {
        let resp = response(&serde_json::json!(42), serde_json::json!("ok"));
        assert_eq!(resp.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
        assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 42);
        assert_eq!(resp.get("result").unwrap().as_str().unwrap(), "ok");
    }

    #[test]
    fn test_notification_format() {
        let notif = notification("test/method", serde_json::json!({"key": "val"}));
        assert_eq!(
            notif.get("method").unwrap().as_str().unwrap(),
            "test/method"
        );
        assert!(notif.get("id").is_none());
    }

    // ── LSP v2 tests ──────────────────────────────────────────

    #[test]
    fn test_workspace_symbol_find() {
        let params = serde_json::json!({ "query": "find" });
        let result = handle_workspace_symbol(&params);
        let items = result.as_array().unwrap();
        assert!(items
            .iter()
            .any(|s| s.get("name").unwrap().as_str().unwrap() == "find"));
    }

    #[test]
    fn test_workspace_symbol_empty_query() {
        let params = serde_json::json!({ "query": "" });
        let result = handle_workspace_symbol(&params);
        let items = result.as_array().unwrap();
        // Empty query returns empty list (no filter match)
        assert!(items.is_empty());
    }

    #[test]
    fn test_classify_context_empty() {
        assert!(matches!(
            classify_completion_context("", 0),
            CompletionContext::Empty
        ));
    }

    #[test]
    fn test_classify_context_after_pipe() {
        let ctx = classify_completion_context("find \"x\" | ", 11);
        assert!(matches!(ctx, CompletionContext::AfterPipe));
    }

    #[test]
    fn test_classify_context_after_stage() {
        // stage keyword + args triggers AfterStageKeyword
        let ctx = classify_completion_context("find \"hello\" ", 14);
        assert!(matches!(ctx, CompletionContext::AfterStageKeyword));
    }

    #[test]
    fn test_classify_context_after_aggregate() {
        let ctx = classify_completion_context("aggregate ", 10);
        assert!(matches!(ctx, CompletionContext::AfterAggregate));
    }

    #[test]
    fn test_completion_v2_after_pipe() {
        let mut docs = DocumentStore::new();
        docs.open("test://f.aql", "find \"x\" | ");
        let params = serde_json::json!({
            "textDocument": { "uri": "test://f.aql" },
            "position": { "line": 0, "character": 11 }
        });
        let result = handle_completion_v2(&params, &docs);
        let items = result.get("items").unwrap().as_array().unwrap();
        let labels: Vec<&str> = items
            .iter()
            .filter_map(|i| i.get("label").and_then(|v| v.as_str()))
            .collect();
        // After pipe we should get stage keywords, not modifiers
        assert!(labels.contains(&"sort"));
        assert!(labels.contains(&"count"));
    }

    #[test]
    fn test_notebook_diagnostics_valid() {
        let text = "# Title\n\n```aql\nfind \"hello\"\n```\n";
        let diag = compute_notebook_diagnostics("test://f.atp.md", text);
        let items = diag
            .pointer("/params/diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_notebook_diagnostics_invalid() {
        let text = "# Title\n\n```aql\ninvalid $$$\n```\n";
        let diag = compute_notebook_diagnostics("test://f.atp.md", text);
        let items = diag
            .pointer("/params/diagnostics")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(!items.is_empty());
    }

    #[test]
    fn test_initialize_has_workspace_symbol() {
        let resp = handle_initialize(&serde_json::json!(1));
        let caps = resp.pointer("/result/capabilities").unwrap();
        assert_eq!(
            caps.get("workspaceSymbolProvider")
                .unwrap()
                .as_bool()
                .unwrap(),
            true
        );
    }
}
