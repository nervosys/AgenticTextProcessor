//! Lightweight template engine.
//!
//! Mustache-style `{{var}}` interpolation with `{{#if}}`, `{{#each}}`,
//! `{{#unless}}` blocks, partials via `{{> partial_name}}`, and
//! HTML/special-char escaping. Designed for report/output generation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A template context value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

impl Value {
    /// Check if the value is truthy (non-null, non-false, non-empty).
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(v) => !v.is_empty(),
            Value::Map(m) => !m.is_empty(),
        }
    }

    /// Render as string.
    pub fn to_string_value(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => {
                if *n == (*n as i64) as f64 {
                    (*n as i64).to_string()
                } else {
                    n.to_string()
                }
            }
            Value::Str(s) => s.clone(),
            Value::List(v) => {
                let items: Vec<String> = v.iter().map(|x| x.to_string_value()).collect();
                items.join(", ")
            }
            Value::Map(_) => "[object]".to_string(),
        }
    }

    /// Look up a dotted path: `"a.b.c"`.
    pub fn get_path(&self, path: &str) -> Option<&Value> {
        let mut current = self;
        for key in path.split('.') {
            match current {
                Value::Map(m) => {
                    current = m.get(key)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n as f64)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::List(v.into_iter().map(Into::into).collect())
    }
}

/// Template error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TemplateError {
    #[error("unclosed block: {{{{#{0}}}}}")]
    UnclosedBlock(String),
    #[error("unexpected closing tag: {{{{/{0}}}}}")]
    UnexpectedClose(String),
    #[error("partial not found: {0}")]
    PartialNotFound(String),
    #[error("parse error at position {pos}: {message}")]
    ParseError { pos: usize, message: String },
}

/// Parsed template token.
#[derive(Debug, Clone)]
enum Token {
    /// Literal text.
    Text(String),
    /// `{{var}}` interpolation.
    Var(String),
    /// `{{{var}}}` unescaped interpolation.
    RawVar(String),
    /// `{{#if var}}` block start.
    IfStart(String),
    /// `{{#unless var}}` block start.
    UnlessStart(String),
    /// `{{#each var}}` block start.
    EachStart(String),
    /// `{{/if}}`, `{{/unless}}`, `{{/each}}` block end.
    #[allow(dead_code)]
    BlockEnd(String),
    /// `{{> partial_name}}` partial include.
    Partial(String),
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

fn tokenize(template: &str) -> Result<Vec<Token>, TemplateError> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    let bytes = template.as_bytes();
    let len = bytes.len();

    while pos < len {
        if pos + 2 < len && bytes[pos] == b'{' && bytes[pos + 1] == b'{' {
            // Check for triple braces {{{var}}}
            if pos + 3 < len && bytes[pos + 2] == b'{' {
                let start = pos + 3;
                let end = template[start..]
                    .find("}}}")
                    .map(|i| start + i)
                    .ok_or_else(|| TemplateError::ParseError {
                        pos,
                        message: "unclosed {{{".into(),
                    })?;
                let name = template[start..end].trim().to_string();
                tokens.push(Token::RawVar(name));
                pos = end + 3;
                continue;
            }

            let start = pos + 2;
            let end = template[start..]
                .find("}}")
                .map(|i| start + i)
                .ok_or_else(|| TemplateError::ParseError {
                    pos,
                    message: "unclosed {{".into(),
                })?;
            let content = template[start..end].trim();

            if let Some(rest) = content.strip_prefix("#if ") {
                tokens.push(Token::IfStart(rest.trim().to_string()));
            } else if let Some(rest) = content.strip_prefix("#unless ") {
                tokens.push(Token::UnlessStart(rest.trim().to_string()));
            } else if let Some(rest) = content.strip_prefix("#each ") {
                tokens.push(Token::EachStart(rest.trim().to_string()));
            } else if let Some(rest) = content.strip_prefix('/') {
                tokens.push(Token::BlockEnd(rest.trim().to_string()));
            } else if let Some(rest) = content.strip_prefix("> ") {
                tokens.push(Token::Partial(rest.trim().to_string()));
            } else if let Some(rest) = content.strip_prefix('>') {
                tokens.push(Token::Partial(rest.trim().to_string()));
            } else {
                tokens.push(Token::Var(content.to_string()));
            }
            pos = end + 2;
        } else {
            // Literal text up to next {{ or end
            let next = template[pos..].find("{{").map(|i| pos + i).unwrap_or(len);
            tokens.push(Token::Text(template[pos..next].to_string()));
            pos = next;
        }
    }

    Ok(tokens)
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

fn render_tokens(
    tokens: &[Token],
    context: &Value,
    partials: &BTreeMap<String, String>,
    pos: &mut usize,
) -> Result<String, TemplateError> {
    let mut output = String::new();

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Text(t) => {
                output.push_str(t);
                *pos += 1;
            }
            Token::Var(name) => {
                if let Some(val) = context.get_path(name) {
                    output.push_str(&escape_html(&val.to_string_value()));
                }
                *pos += 1;
            }
            Token::RawVar(name) => {
                if let Some(val) = context.get_path(name) {
                    output.push_str(&val.to_string_value());
                }
                *pos += 1;
            }
            Token::IfStart(var) => {
                *pos += 1;
                let body = render_tokens(tokens, context, partials, pos)?;
                let truthy = context
                    .get_path(var)
                    .map(|v| v.is_truthy())
                    .unwrap_or(false);
                if truthy {
                    output.push_str(&body);
                }
            }
            Token::UnlessStart(var) => {
                *pos += 1;
                let body = render_tokens(tokens, context, partials, pos)?;
                let truthy = context
                    .get_path(var)
                    .map(|v| v.is_truthy())
                    .unwrap_or(false);
                if !truthy {
                    output.push_str(&body);
                }
            }
            Token::EachStart(var) => {
                *pos += 1;
                // We need to snapshot the start position of the loop body
                let body_start = *pos;

                let items = context.get_path(var).cloned();

                match items {
                    Some(Value::List(list)) => {
                        let mut _first_body = String::new();
                        for (idx, item) in list.iter().enumerate() {
                            *pos = body_start;
                            // Merge item into context: if item is a Map, merge; else set as "this"
                            let iter_context = match item {
                                Value::Map(m) => {
                                    let mut merged = if let Value::Map(ctx) = context {
                                        ctx.clone()
                                    } else {
                                        BTreeMap::new()
                                    };
                                    merged.extend(m.clone());
                                    merged.insert("@index".to_string(), Value::Number(idx as f64));
                                    Value::Map(merged)
                                }
                                _ => {
                                    let mut m = if let Value::Map(ctx) = context {
                                        ctx.clone()
                                    } else {
                                        BTreeMap::new()
                                    };
                                    m.insert("this".to_string(), item.clone());
                                    m.insert("@index".to_string(), Value::Number(idx as f64));
                                    Value::Map(m)
                                }
                            };
                            let rendered = render_tokens(tokens, &iter_context, partials, pos)?;
                            if idx == 0 {
                                _first_body = rendered.clone();
                            }
                            output.push_str(&rendered);
                        }
                        if list.is_empty() {
                            // Still need to skip past the body
                            let rendered = render_tokens(tokens, context, partials, pos)?;
                            let _ = rendered; // discard
                        }
                    }
                    _ => {
                        // Skip the body
                        let _ = render_tokens(tokens, context, partials, pos)?;
                    }
                }
            }
            Token::BlockEnd(_) => {
                *pos += 1;
                return Ok(output);
            }
            Token::Partial(name) => {
                if let Some(partial_tmpl) = partials.get(name) {
                    let rendered = render(partial_tmpl, context, partials)?;
                    output.push_str(&rendered);
                }
                *pos += 1;
            }
        }
    }

    Ok(output)
}

/// Render a template string with the given context and partials.
pub fn render(
    template: &str,
    context: &Value,
    partials: &BTreeMap<String, String>,
) -> Result<String, TemplateError> {
    let tokens = tokenize(template)?;
    let mut pos = 0;
    render_tokens(&tokens, context, partials, &mut pos)
}

// ---------------------------------------------------------------------------
// TemplateEngine
// ---------------------------------------------------------------------------

/// Template engine with registered partials.
#[derive(Debug, Clone)]
pub struct TemplateEngine {
    partials: BTreeMap<String, String>,
}

impl TemplateEngine {
    /// Create a new engine with no partials.
    pub fn new() -> Self {
        Self {
            partials: BTreeMap::new(),
        }
    }

    /// Register a named partial template.
    pub fn register_partial(&mut self, name: &str, template: &str) {
        self.partials.insert(name.to_string(), template.to_string());
    }

    /// Render a template with the given context.
    pub fn render(&self, template: &str, context: &Value) -> Result<String, TemplateError> {
        render(template, context, &self.partials)
    }

    /// Convenience: build a context map.
    pub fn context() -> BTreeMap<String, Value> {
        BTreeMap::new()
    }

    /// Convenience: wrap a map as a Value.
    pub fn map_value(map: BTreeMap<String, Value>) -> Value {
        Value::Map(map)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(pairs: &[(&str, Value)]) -> Value {
        Value::Map(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn test_simple_interpolation() {
        let eng = TemplateEngine::new();
        let c = ctx(&[("name", Value::Str("World".into()))]);
        let result = eng.render("Hello, {{name}}!", &c).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_html_escaping() {
        let eng = TemplateEngine::new();
        let c = ctx(&[("val", Value::Str("<b>bold</b>".into()))]);
        let result = eng.render("{{val}}", &c).unwrap();
        assert_eq!(result, "&lt;b&gt;bold&lt;/b&gt;");
    }

    #[test]
    fn test_raw_interpolation() {
        let eng = TemplateEngine::new();
        let c = ctx(&[("val", Value::Str("<b>bold</b>".into()))]);
        let result = eng.render("{{{val}}}", &c).unwrap();
        assert_eq!(result, "<b>bold</b>");
    }

    #[test]
    fn test_if_truthy() {
        let eng = TemplateEngine::new();
        let c = ctx(&[("show", Value::Bool(true))]);
        let result = eng.render("{{#if show}}visible{{/if}}", &c).unwrap();
        assert_eq!(result, "visible");
    }

    #[test]
    fn test_if_falsy() {
        let eng = TemplateEngine::new();
        let c = ctx(&[("show", Value::Bool(false))]);
        let result = eng.render("{{#if show}}visible{{/if}}", &c).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_unless() {
        let eng = TemplateEngine::new();
        let c = ctx(&[("hidden", Value::Bool(false))]);
        let result = eng
            .render("{{#unless hidden}}shown{{/unless}}", &c)
            .unwrap();
        assert_eq!(result, "shown");
    }

    #[test]
    fn test_each_list() {
        let eng = TemplateEngine::new();
        let items = Value::List(vec![
            Value::Str("a".into()),
            Value::Str("b".into()),
            Value::Str("c".into()),
        ]);
        let c = ctx(&[("items", items)]);
        let result = eng
            .render("{{#each items}}[{{this}}]{{/each}}", &c)
            .unwrap();
        assert_eq!(result, "[a][b][c]");
    }

    #[test]
    fn test_each_map_items() {
        let eng = TemplateEngine::new();
        let items = Value::List(vec![
            Value::Map(
                [("name".to_string(), Value::Str("Alice".into()))]
                    .into_iter()
                    .collect(),
            ),
            Value::Map(
                [("name".to_string(), Value::Str("Bob".into()))]
                    .into_iter()
                    .collect(),
            ),
        ]);
        let c = ctx(&[("users", items)]);
        let result = eng.render("{{#each users}}{{name}} {{/each}}", &c).unwrap();
        assert_eq!(result, "Alice Bob ");
    }

    #[test]
    fn test_nested_path() {
        let eng = TemplateEngine::new();
        let inner = Value::Map(
            [("city".to_string(), Value::Str("NYC".into()))]
                .into_iter()
                .collect(),
        );
        let c = ctx(&[("user", inner)]);
        let result = eng.render("City: {{user.city}}", &c).unwrap();
        assert_eq!(result, "City: NYC");
    }

    #[test]
    fn test_partials() {
        let mut eng = TemplateEngine::new();
        eng.register_partial("header", "=== {{title}} ===\n");
        let c = ctx(&[("title", Value::Str("Report".into()))]);
        let result = eng.render("{{> header}}Body here", &c).unwrap();
        assert_eq!(result, "=== Report ===\nBody here");
    }

    #[test]
    fn test_missing_var() {
        let eng = TemplateEngine::new();
        let c = ctx(&[]);
        let result = eng.render("Hello {{name}}", &c).unwrap();
        assert_eq!(result, "Hello ");
    }

    #[test]
    fn test_number_rendering() {
        let eng = TemplateEngine::new();
        let c = ctx(&[
            ("int_val", Value::Number(42.0)),
            ("float_val", Value::Number(3.14)),
        ]);
        let result = eng.render("{{int_val}} and {{float_val}}", &c).unwrap();
        assert_eq!(result, "42 and 3.14");
    }

    #[test]
    fn test_value_truthiness() {
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Str(String::new()).is_truthy());
        assert!(Value::Str("x".into()).is_truthy());
        assert!(!Value::List(vec![]).is_truthy());
        assert!(Value::List(vec![Value::Null]).is_truthy());
        assert!(!Value::Number(0.0).is_truthy());
        assert!(Value::Number(1.0).is_truthy());
    }

    #[test]
    fn test_complex_template() {
        let eng = TemplateEngine::new();
        let items = Value::List(vec![
            Value::Map(
                [
                    ("name".to_string(), Value::Str("File A".into())),
                    ("size".to_string(), Value::Number(1024.0)),
                ]
                .into_iter()
                .collect(),
            ),
            Value::Map(
                [
                    ("name".to_string(), Value::Str("File B".into())),
                    ("size".to_string(), Value::Number(2048.0)),
                ]
                .into_iter()
                .collect(),
            ),
        ]);
        let c = ctx(&[
            ("title", Value::Str("Report".into())),
            ("files", items),
            ("show_footer", Value::Bool(true)),
        ]);
        let tmpl = "# {{title}}\n{{#each files}}- {{name}} ({{size}} bytes)\n{{/each}}{{#if show_footer}}---\nEnd{{/if}}";
        let result = eng.render(tmpl, &c).unwrap();
        assert!(result.contains("# Report"));
        assert!(result.contains("- File A (1024 bytes)"));
        assert!(result.contains("- File B (2048 bytes)"));
        assert!(result.contains("End"));
    }
}
