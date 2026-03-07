//! `atp mcp` subcommand — Model Context Protocol server.
//!
//! Runs ATP as an MCP server, exposing tools for:
//!   - `search` — grep-like search
//!   - `transform` — sed-like transformation
//!   - `query` — AQL query execution
//!   - `analyze` — awk-like analysis
//!
//! Protocol: JSON-RPC 2.0 over stdin/stdout (MCP spec).

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};

#[derive(Args, Debug)]
pub struct McpArgs {}

// ── MCP JSON-RPC types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Debug, Serialize)]
struct McpError {
    code: i64,
    message: String,
}

// ── Transport ───────────────────────────────────────────────────────────

fn read_message(reader: &mut impl BufRead) -> Result<Option<McpRequest>> {
    // MCP uses Content-Length framing (same as LSP)
    let mut header = String::new();
    let bytes = reader.read_line(&mut header)?;
    if bytes == 0 {
        return Ok(None); // EOF
    }

    let content_length: usize = if header.to_lowercase().starts_with("content-length:") {
        header
            .trim()
            .split(':')
            .nth(1)
            .unwrap_or("0")
            .trim()
            .parse()
            .unwrap_or(0)
    } else {
        return Ok(None);
    };

    // Read blank line separator
    let mut blank = String::new();
    reader.read_line(&mut blank)?;

    // Read body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    let request: McpRequest =
        serde_json::from_slice(&body).context("Failed to parse MCP request")?;
    Ok(Some(request))
}

fn send_response(writer: &mut impl Write, response: &McpResponse) -> Result<()> {
    let body = serde_json::to_string(response)?;
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()?;
    Ok(())
}

fn send_result(writer: &mut impl Write, id: Value, result: Value) -> Result<()> {
    send_response(
        writer,
        &McpResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        },
    )
}

fn send_error(writer: &mut impl Write, id: Value, code: i64, message: &str) -> Result<()> {
    send_response(
        writer,
        &McpResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.to_string(),
            }),
        },
    )
}

// ── Tool definitions ────────────────────────────────────────────────────

fn tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "search",
                "description": "Search for patterns in text or files (grep-like). Returns matching lines.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Search pattern (regex or literal)" },
                        "text": { "type": "string", "description": "Text to search in" },
                        "case_insensitive": { "type": "boolean", "description": "Case-insensitive search", "default": false },
                        "literal": { "type": "boolean", "description": "Treat pattern as literal text", "default": false }
                    },
                    "required": ["pattern", "text"]
                }
            },
            {
                "name": "transform",
                "description": "Transform text with pattern-based substitution (sed-like).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Pattern to match" },
                        "replacement": { "type": "string", "description": "Replacement text" },
                        "text": { "type": "string", "description": "Text to transform" },
                        "global": { "type": "boolean", "description": "Replace all occurrences", "default": true }
                    },
                    "required": ["pattern", "replacement", "text"]
                }
            },
            {
                "name": "query",
                "description": "Execute an AQL (ATP Query Language) query on text. AQL is a readable, keyword-based language for text processing.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "aql": { "type": "string", "description": "AQL query string, e.g. 'find \"TODO\" | sort | unique | count'" },
                        "text": { "type": "string", "description": "Input text to process" }
                    },
                    "required": ["aql", "text"]
                }
            },
            {
                "name": "analyze",
                "description": "Analyze text with field-based processing (awk-like).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Input text" },
                        "separator": { "type": "string", "description": "Field separator", "default": "\\t" },
                        "fields": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "description": "Field numbers to select (1-based)"
                        },
                        "condition": { "type": "string", "description": "Filter condition, e.g. '$2 > 10'" }
                    },
                    "required": ["text"]
                }
            }
        ]
    })
}

// ── Tool execution ──────────────────────────────────────────────────────

fn execute_tool(name: &str, args: &Value) -> Result<Value> {
    match name {
        "search" => execute_search(args),
        "transform" => execute_transform(args),
        "query" => execute_query(args),
        "analyze" => execute_analyze(args),
        _ => anyhow::bail!("Unknown tool: {name}"),
    }
}

fn execute_search(args: &Value) -> Result<Value> {
    let pattern = args["pattern"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'pattern'"))?;
    let text = args["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'text'"))?;
    let case_insensitive = args["case_insensitive"].as_bool().unwrap_or(false);
    let literal = args["literal"].as_bool().unwrap_or(false);

    let pattern_type = if literal {
        atp_core::output::PatternType::Literal
    } else {
        atp_core::output::PatternType::Regex
    };

    let config = atp_core::engine::grep::GrepConfig {
        pattern: pattern.to_string(),
        pattern_type,
        case_sensitive: !case_insensitive,
        ..Default::default()
    };
    let engine = atp_core::GrepEngine::new(config)?;
    let cursor = std::io::Cursor::new(text.as_bytes());
    let results = engine.search_reader(cursor, "<mcp-input>")?;

    let matches: Vec<Value> = results
        .iter()
        .map(|m| {
            json!({
                "line": m.line_number,
                "text": m.line_content,
            })
        })
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&matches)?
        }]
    }))
}

fn execute_transform(args: &Value) -> Result<Value> {
    let pattern = args["pattern"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'pattern'"))?;
    let replacement = args["replacement"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'replacement'"))?;
    let text = args["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'text'"))?;
    let global = args["global"].as_bool().unwrap_or(true);

    let cmd = atp_core::engine::sed::TransformCommand::Substitute {
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
        global,
        case_insensitive: false,
    };
    let config = atp_core::engine::sed::SedConfig {
        commands: vec![cmd],
        ..Default::default()
    };
    let engine = atp_core::SedEngine::new(config);
    let cursor = std::io::Cursor::new(text.as_bytes());
    let mut output_buf = Vec::new();
    let _results = engine.transform_reader(cursor, &mut output_buf, "<mcp-input>")?;

    // Use the writer output which has the transformed text
    let output_text = String::from_utf8_lossy(&output_buf);

    Ok(json!({
        "content": [{
            "type": "text",
            "text": output_text.trim_end()
        }]
    }))
}

fn execute_query(args: &Value) -> Result<Value> {
    let aql = args["aql"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'aql'"))?;
    let text = args["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'text'"))?;

    let pipeline = atp_core::engine::aql::parse(aql)?;
    let mut engine = atp_core::AqlEngine::new();

    // Write text to a temp file for the AQL engine
    let dir = tempfile::tempdir()?;
    let file_path = dir.path().join("input.txt");
    std::fs::write(&file_path, text)?;

    let results = engine.execute(&pipeline, &[&file_path])?;

    // Use the final_output field which contains the result
    let output = match &results.final_output {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        other => serde_json::to_string_pretty(other)?,
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": output
        }]
    }))
}

fn execute_analyze(args: &Value) -> Result<Value> {
    let text = args["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'text'"))?;
    let separator = args["separator"].as_str().unwrap_or("\t");
    let fields: Option<Vec<usize>> = args["fields"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_u64().map(|n| n as usize))
            .collect()
    });

    let mut output_lines = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split(separator).collect();
        if let Some(ref fields) = fields {
            let selected: Vec<&str> = fields
                .iter()
                .filter_map(|&f| {
                    if f >= 1 && f <= parts.len() {
                        Some(parts[f - 1])
                    } else {
                        None
                    }
                })
                .collect();
            output_lines.push(selected.join(separator));
        } else {
            output_lines.push(line.to_string());
        }
    }

    Ok(json!({
        "content": [{
            "type": "text",
            "text": output_lines.join("\n")
        }]
    }))
}

// ── Server ──────────────────────────────────────────────────────────────

pub fn execute(_args: &McpArgs) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();

    loop {
        let request = match read_message(&mut reader)? {
            Some(r) => r,
            None => break, // EOF
        };

        let id = request.id.clone().unwrap_or(Value::Null);

        match request.method.as_str() {
            "initialize" => {
                send_result(
                    &mut writer,
                    id,
                    json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": { "listChanged": false }
                        },
                        "serverInfo": {
                            "name": "atp",
                            "version": atp_core::ATP_VERSION
                        }
                    }),
                )?;
            }

            "notifications/initialized" | "initialized" => {
                // No response needed for notifications
            }

            "tools/list" => {
                send_result(&mut writer, id, tool_definitions())?;
            }

            "tools/call" => {
                let tool_name = request.params["name"].as_str().unwrap_or("").to_string();
                let arguments = request
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(json!({}));

                match execute_tool(&tool_name, &arguments) {
                    Ok(result) => send_result(&mut writer, id, result)?,
                    Err(e) => {
                        send_result(
                            &mut writer,
                            id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error: {e:#}")
                                }],
                                "isError": true
                            }),
                        )?;
                    }
                }
            }

            "ping" => {
                send_result(&mut writer, id, json!({}))?;
            }

            "shutdown" | "exit" => break,

            _ => {
                send_error(
                    &mut writer,
                    id,
                    -32601,
                    &format!("Method not found: {}", request.method),
                )?;
            }
        }
    }

    Ok(())
}
