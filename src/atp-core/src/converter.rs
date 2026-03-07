//! # Format Converter
//!
//! Bidirectional conversion between JSON, YAML, TOML, CSV, XML-subset,
//! and INI. Operates on an intermediate `Document` representation so
//! every format can round-trip through any other.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported serialisation formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConvertFormat {
    Json,
    Yaml,
    Toml,
    Csv,
    Xml,
    Ini,
}

impl ConvertFormat {
    /// Common file extension (without dot).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Csv => "csv",
            Self::Xml => "xml",
            Self::Ini => "ini",
        }
    }

    /// Attempt to detect format from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "csv" => Some(Self::Csv),
            "xml" => Some(Self::Xml),
            "ini" | "cfg" => Some(Self::Ini),
            _ => None,
        }
    }
}

/// Intermediate document value – the universal pivot for conversions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DocValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<DocValue>),
    Map(BTreeMap<String, DocValue>),
}

impl DocValue {
    pub fn is_null(&self) -> bool {
        matches!(self, DocValue::Null)
    }

    pub fn as_str(&self) -> Option<&str> {
        if let DocValue::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let DocValue::Int(n) = self {
            Some(*n)
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DocValue::Float(f) => Some(*f),
            DocValue::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let DocValue::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&[DocValue]> {
        if let DocValue::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, DocValue>> {
        if let DocValue::Map(m) = self {
            Some(m)
        } else {
            None
        }
    }

    /// Deep-merge two maps; arrays are concatenated, scalars overwritten.
    pub fn merge(&mut self, other: DocValue) {
        match (self, other) {
            (DocValue::Map(a), DocValue::Map(b)) => {
                for (k, v) in b {
                    a.entry(k.clone())
                        .and_modify(|existing| existing.merge(v.clone()))
                        .or_insert(v);
                }
            }
            (DocValue::Array(a), DocValue::Array(b)) => a.extend(b),
            (s, o) => *s = o,
        }
    }
}

/// Converter options.
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Pretty-print output (indentation, newlines).
    pub pretty: bool,
    /// For CSV: header row present.
    pub csv_header: bool,
    /// For CSV: delimiter character.
    pub csv_delimiter: char,
    /// For XML: root element name.
    pub xml_root: String,
    /// For XML: item element name (array children).
    pub xml_item: String,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            pretty: true,
            csv_header: true,
            csv_delimiter: ',',
            xml_root: "root".into(),
            xml_item: "item".into(),
        }
    }
}

/// Conversion error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConvertError {
    #[error("parse error ({format:?}): {msg}")]
    Parse { format: ConvertFormat, msg: String },
    #[error("render error ({format:?}): {msg}")]
    Render { format: ConvertFormat, msg: String },
    #[error("unsupported conversion: {0}")]
    Unsupported(String),
}

/// A high-level converter that holds options.
#[derive(Debug, Clone)]
pub struct Converter {
    pub options: ConvertOptions,
}

// ---------------------------------------------------------------------------
// Parsing  (text → DocValue)
// ---------------------------------------------------------------------------

impl Converter {
    pub fn new() -> Self {
        Self {
            options: ConvertOptions::default(),
        }
    }

    pub fn with_options(options: ConvertOptions) -> Self {
        Self { options }
    }

    /// Parse text in `from` format into `DocValue`.
    pub fn parse(&self, text: &str, from: ConvertFormat) -> Result<DocValue, ConvertError> {
        match from {
            ConvertFormat::Json => parse_json(text),
            ConvertFormat::Yaml => parse_yaml(text),
            ConvertFormat::Toml => parse_toml(text),
            ConvertFormat::Csv => {
                parse_csv(text, self.options.csv_header, self.options.csv_delimiter)
            }
            ConvertFormat::Xml => parse_xml(text),
            ConvertFormat::Ini => parse_ini(text),
        }
    }

    /// Render `DocValue` into text in `to` format.
    pub fn render(&self, doc: &DocValue, to: ConvertFormat) -> Result<String, ConvertError> {
        match to {
            ConvertFormat::Json => render_json(doc, self.options.pretty),
            ConvertFormat::Yaml => render_yaml(doc),
            ConvertFormat::Toml => render_toml(doc),
            ConvertFormat::Csv => {
                render_csv(doc, self.options.csv_header, self.options.csv_delimiter)
            }
            ConvertFormat::Xml => render_xml(doc, &self.options.xml_root, &self.options.xml_item),
            ConvertFormat::Ini => render_ini(doc),
        }
    }

    /// One-shot: parse `from` → render `to`.
    pub fn convert(
        &self,
        text: &str,
        from: ConvertFormat,
        to: ConvertFormat,
    ) -> Result<String, ConvertError> {
        let doc = self.parse(text, from)?;
        self.render(&doc, to)
    }
}

impl Default for Converter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

fn serde_to_doc(v: serde_json::Value) -> DocValue {
    match v {
        serde_json::Value::Null => DocValue::Null,
        serde_json::Value::Bool(b) => DocValue::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                DocValue::Int(i)
            } else {
                DocValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => DocValue::Str(s),
        serde_json::Value::Array(a) => DocValue::Array(a.into_iter().map(serde_to_doc).collect()),
        serde_json::Value::Object(m) => {
            DocValue::Map(m.into_iter().map(|(k, v)| (k, serde_to_doc(v))).collect())
        }
    }
}

fn doc_to_serde(d: &DocValue) -> serde_json::Value {
    match d {
        DocValue::Null => serde_json::Value::Null,
        DocValue::Bool(b) => serde_json::Value::Bool(*b),
        DocValue::Int(n) => serde_json::json!(*n),
        DocValue::Float(f) => serde_json::json!(*f),
        DocValue::Str(s) => serde_json::Value::String(s.clone()),
        DocValue::Array(a) => serde_json::Value::Array(a.iter().map(doc_to_serde).collect()),
        DocValue::Map(m) => {
            let obj: serde_json::Map<String, serde_json::Value> = m
                .iter()
                .map(|(k, v)| (k.clone(), doc_to_serde(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

fn parse_json(text: &str) -> Result<DocValue, ConvertError> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| ConvertError::Parse {
        format: ConvertFormat::Json,
        msg: e.to_string(),
    })?;
    Ok(serde_to_doc(v))
}

fn render_json(doc: &DocValue, pretty: bool) -> Result<String, ConvertError> {
    let v = doc_to_serde(doc);
    let s = if pretty {
        serde_json::to_string_pretty(&v)
    } else {
        serde_json::to_string(&v)
    }
    .map_err(|e| ConvertError::Render {
        format: ConvertFormat::Json,
        msg: e.to_string(),
    })?;
    Ok(s)
}

// ---------------------------------------------------------------------------
// YAML
// ---------------------------------------------------------------------------

fn yaml_to_doc(v: serde_yaml::Value) -> DocValue {
    match v {
        serde_yaml::Value::Null => DocValue::Null,
        serde_yaml::Value::Bool(b) => DocValue::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                DocValue::Int(i)
            } else {
                DocValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_yaml::Value::String(s) => DocValue::Str(s),
        serde_yaml::Value::Sequence(a) => DocValue::Array(a.into_iter().map(yaml_to_doc).collect()),
        serde_yaml::Value::Mapping(m) => {
            let map: BTreeMap<String, DocValue> = m
                .into_iter()
                .map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => format!("{other:?}"),
                    };
                    (key, yaml_to_doc(v))
                })
                .collect();
            DocValue::Map(map)
        }
        serde_yaml::Value::Tagged(t) => yaml_to_doc(t.value),
    }
}

fn parse_yaml(text: &str) -> Result<DocValue, ConvertError> {
    let v: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| ConvertError::Parse {
        format: ConvertFormat::Yaml,
        msg: e.to_string(),
    })?;
    Ok(yaml_to_doc(v))
}

fn render_yaml(doc: &DocValue) -> Result<String, ConvertError> {
    let v = doc_to_serde(doc);
    serde_yaml::to_string(&v).map_err(|e| ConvertError::Render {
        format: ConvertFormat::Yaml,
        msg: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// TOML
// ---------------------------------------------------------------------------

fn toml_to_doc(v: toml::Value) -> DocValue {
    match v {
        toml::Value::Boolean(b) => DocValue::Bool(b),
        toml::Value::Integer(i) => DocValue::Int(i),
        toml::Value::Float(f) => DocValue::Float(f),
        toml::Value::String(s) => DocValue::Str(s),
        toml::Value::Datetime(d) => DocValue::Str(d.to_string()),
        toml::Value::Array(a) => DocValue::Array(a.into_iter().map(toml_to_doc).collect()),
        toml::Value::Table(t) => {
            DocValue::Map(t.into_iter().map(|(k, v)| (k, toml_to_doc(v))).collect())
        }
    }
}

fn doc_to_toml(d: &DocValue) -> toml::Value {
    match d {
        DocValue::Null => toml::Value::String("null".into()),
        DocValue::Bool(b) => toml::Value::Boolean(*b),
        DocValue::Int(n) => toml::Value::Integer(*n),
        DocValue::Float(f) => toml::Value::Float(*f),
        DocValue::Str(s) => toml::Value::String(s.clone()),
        DocValue::Array(a) => toml::Value::Array(a.iter().map(doc_to_toml).collect()),
        DocValue::Map(m) => {
            let t: toml::map::Map<String, toml::Value> =
                m.iter().map(|(k, v)| (k.clone(), doc_to_toml(v))).collect();
            toml::Value::Table(t)
        }
    }
}

fn parse_toml(text: &str) -> Result<DocValue, ConvertError> {
    let v: toml::Value = text
        .parse()
        .map_err(|e: toml::de::Error| ConvertError::Parse {
            format: ConvertFormat::Toml,
            msg: e.to_string(),
        })?;
    Ok(toml_to_doc(v))
}

fn render_toml(doc: &DocValue) -> Result<String, ConvertError> {
    let v = doc_to_toml(doc);
    // toml::to_string needs a table at the root
    match &v {
        toml::Value::Table(_) => {}
        _ => {
            return Err(ConvertError::Render {
                format: ConvertFormat::Toml,
                msg: "TOML root must be a table/map".into(),
            });
        }
    }
    toml::to_string_pretty(&v).map_err(|e| ConvertError::Render {
        format: ConvertFormat::Toml,
        msg: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// CSV  (array-of-maps or array-of-arrays)
// ---------------------------------------------------------------------------

fn parse_csv(text: &str, header: bool, delim: char) -> Result<DocValue, ConvertError> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(DocValue::Array(vec![]));
    }

    let split_row = |line: &str| -> Vec<String> {
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        for ch in line.chars() {
            if ch == '"' {
                in_quotes = !in_quotes;
            } else if ch == delim && !in_quotes {
                fields.push(current.trim().to_string());
                current = String::new();
            } else {
                current.push(ch);
            }
        }
        fields.push(current.trim().to_string());
        fields
    };

    let infer_value = |s: &str| -> DocValue {
        if s.is_empty() {
            return DocValue::Null;
        }
        if let Ok(i) = s.parse::<i64>() {
            return DocValue::Int(i);
        }
        if let Ok(f) = s.parse::<f64>() {
            return DocValue::Float(f);
        }
        match s {
            "true" => DocValue::Bool(true),
            "false" => DocValue::Bool(false),
            _ => DocValue::Str(s.to_string()),
        }
    };

    if header && lines.len() > 1 {
        let headers = split_row(lines[0]);
        let rows: Vec<DocValue> = lines[1..]
            .iter()
            .map(|line| {
                let fields = split_row(line);
                let map: BTreeMap<String, DocValue> = headers
                    .iter()
                    .zip(fields.iter())
                    .map(|(h, f)| (h.clone(), infer_value(f)))
                    .collect();
                DocValue::Map(map)
            })
            .collect();
        Ok(DocValue::Array(rows))
    } else {
        let rows: Vec<DocValue> = lines
            .iter()
            .map(|line| {
                let fields = split_row(line);
                DocValue::Array(fields.iter().map(|f| infer_value(f)).collect())
            })
            .collect();
        Ok(DocValue::Array(rows))
    }
}

fn render_csv(doc: &DocValue, header: bool, delim: char) -> Result<String, ConvertError> {
    let arr = doc.as_array().ok_or_else(|| ConvertError::Render {
        format: ConvertFormat::Csv,
        msg: "CSV requires an array at root".into(),
    })?;
    if arr.is_empty() {
        return Ok(String::new());
    }

    let escape_field = |s: &str| -> String {
        if s.contains(delim) || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };

    let value_to_str = |v: &DocValue| -> String {
        match v {
            DocValue::Null => String::new(),
            DocValue::Bool(b) => b.to_string(),
            DocValue::Int(n) => n.to_string(),
            DocValue::Float(f) => format!("{f}"),
            DocValue::Str(s) => s.clone(),
            _ => String::new(),
        }
    };

    let mut out = String::new();

    // If rows are maps, gather column names from first row
    if let Some(DocValue::Map(first)) = arr.first() {
        let keys: Vec<&String> = first.keys().collect();
        if header {
            let hdr: Vec<String> = keys.iter().map(|k| escape_field(k)).collect();
            out.push_str(&hdr.join(&delim.to_string()));
            out.push('\n');
        }
        for row in arr {
            if let DocValue::Map(m) = row {
                let vals: Vec<String> = keys
                    .iter()
                    .map(|k| escape_field(&value_to_str(m.get(*k).unwrap_or(&DocValue::Null))))
                    .collect();
                out.push_str(&vals.join(&delim.to_string()));
                out.push('\n');
            }
        }
    } else {
        // rows are arrays
        for row in arr {
            if let DocValue::Array(cols) = row {
                let vals: Vec<String> = cols
                    .iter()
                    .map(|c| escape_field(&value_to_str(c)))
                    .collect();
                out.push_str(&vals.join(&delim.to_string()));
                out.push('\n');
            }
        }
    }

    // trim trailing newline
    if out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// XML  (simple subset)
// ---------------------------------------------------------------------------

fn parse_xml(text: &str) -> Result<DocValue, ConvertError> {
    // Minimal XML parser: handles <tag>value</tag> and nested elements.
    // Does NOT handle attributes, CDATA, namespaces etc.
    let text = text.trim();
    if text.is_empty() {
        return Ok(DocValue::Null);
    }
    parse_xml_element(text)
        .map(|(_tag, val)| val)
        .ok_or_else(|| ConvertError::Parse {
            format: ConvertFormat::Xml,
            msg: "invalid XML structure".into(),
        })
}

fn parse_xml_element(s: &str) -> Option<(String, DocValue)> {
    let s = s.trim();
    if !s.starts_with('<') {
        return None;
    }
    let close_bracket = s.find('>')?;
    let tag = &s[1..close_bracket];
    // skip attributes for simplicity
    let tag_name = tag.split_whitespace().next().unwrap_or(tag);
    let end_tag = format!("</{tag_name}>");
    let end_pos = s.rfind(&end_tag)?;
    let inner = &s[close_bracket + 1..end_pos];
    let inner = inner.trim();

    // Check if inner contains child elements
    if inner.contains('<') {
        let mut map: BTreeMap<String, Vec<DocValue>> = BTreeMap::new();
        let mut pos = 0;
        let bytes = inner.as_bytes();
        while pos < inner.len() {
            // skip whitespace
            while pos < inner.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= inner.len() {
                break;
            }
            if bytes[pos] != b'<' {
                break;
            }
            let remaining = &inner[pos..];
            if let Some((child_tag, child_val)) = parse_xml_element(remaining) {
                let child_end = format!("</{child_tag}>");
                if let Some(ce) = remaining.find(&child_end) {
                    pos += ce + child_end.len();
                } else {
                    break;
                }
                map.entry(child_tag).or_default().push(child_val);
            } else {
                break;
            }
        }
        // Flatten single-element arrays
        let result: BTreeMap<String, DocValue> = map
            .into_iter()
            .map(|(k, v)| {
                if v.len() == 1 {
                    (k, v.into_iter().next().unwrap())
                } else {
                    (k, DocValue::Array(v))
                }
            })
            .collect();
        Some((tag_name.to_string(), DocValue::Map(result)))
    } else {
        // leaf
        let val = infer_scalar(inner);
        Some((tag_name.to_string(), val))
    }
}

fn infer_scalar(s: &str) -> DocValue {
    if s.is_empty() {
        return DocValue::Null;
    }
    if let Ok(i) = s.parse::<i64>() {
        return DocValue::Int(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return DocValue::Float(f);
    }
    if s == "true" {
        return DocValue::Bool(true);
    }
    if s == "false" {
        return DocValue::Bool(false);
    }
    DocValue::Str(s.to_string())
}

fn render_xml(doc: &DocValue, root: &str, item: &str) -> Result<String, ConvertError> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<{root}>\n"));
    render_xml_value(&mut out, doc, item, 1);
    out.push_str(&format!("</{root}>\n"));
    Ok(out)
}

fn render_xml_value(out: &mut String, doc: &DocValue, item: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    match doc {
        DocValue::Map(m) => {
            for (k, v) in m {
                match v {
                    DocValue::Map(_) => {
                        out.push_str(&format!("{indent}<{k}>\n"));
                        render_xml_value(out, v, item, depth + 1);
                        out.push_str(&format!("{indent}</{k}>\n"));
                    }
                    DocValue::Array(arr) => {
                        for child in arr {
                            out.push_str(&format!("{indent}<{k}>\n"));
                            render_xml_value(out, child, item, depth + 1);
                            out.push_str(&format!("{indent}</{k}>\n"));
                        }
                    }
                    _ => {
                        let text = scalar_to_string(v);
                        out.push_str(&format!("{indent}<{k}>{text}</{k}>\n"));
                    }
                }
            }
        }
        DocValue::Array(arr) => {
            for child in arr {
                out.push_str(&format!("{indent}<{item}>\n"));
                render_xml_value(out, child, item, depth + 1);
                out.push_str(&format!("{indent}</{item}>\n"));
            }
        }
        _ => {
            let text = scalar_to_string(doc);
            out.push_str(&format!("{indent}{text}\n"));
        }
    }
}

fn scalar_to_string(v: &DocValue) -> String {
    match v {
        DocValue::Null => String::new(),
        DocValue::Bool(b) => b.to_string(),
        DocValue::Int(n) => n.to_string(),
        DocValue::Float(f) => format!("{f}"),
        DocValue::Str(s) => xml_escape(s),
        _ => String::new(),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// INI
// ---------------------------------------------------------------------------

fn parse_ini(text: &str) -> Result<DocValue, ConvertError> {
    let mut root: BTreeMap<String, DocValue> = BTreeMap::new();
    let mut current_section: Option<String> = None;
    let mut section_map: BTreeMap<String, DocValue> = BTreeMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            // flush previous section
            if let Some(sec) = current_section.take() {
                root.insert(sec, DocValue::Map(std::mem::take(&mut section_map)));
            }
            current_section = Some(line[1..line.len() - 1].trim().to_string());
        } else if let Some(eq) = line.find('=') {
            let key = line[..eq].trim().to_string();
            let val = line[eq + 1..].trim();
            let dv = infer_scalar(val);
            if current_section.is_some() {
                section_map.insert(key, dv);
            } else {
                root.insert(key, dv);
            }
        }
    }
    // flush last section
    if let Some(sec) = current_section {
        root.insert(sec, DocValue::Map(section_map));
    }
    Ok(DocValue::Map(root))
}

fn render_ini(doc: &DocValue) -> Result<String, ConvertError> {
    let map = doc.as_map().ok_or_else(|| ConvertError::Render {
        format: ConvertFormat::Ini,
        msg: "INI requires a map at root".into(),
    })?;

    let mut out = String::new();
    let mut sections = Vec::new();

    // First: top-level scalars
    for (k, v) in map {
        match v {
            DocValue::Map(_) => sections.push((k, v)),
            _ => {
                out.push_str(&format!("{k} = {}\n", scalar_to_string(v)));
            }
        }
    }

    for (sec, v) in sections {
        out.push_str(&format!("\n[{sec}]\n"));
        if let DocValue::Map(m) = v {
            for (k, val) in m {
                out.push_str(&format!("{k} = {}\n", scalar_to_string(val)));
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Detect format from first non-whitespace characters.
pub fn detect_format(text: &str) -> Option<ConvertFormat> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        Some(ConvertFormat::Json)
    } else if trimmed.starts_with("<?xml") || trimmed.starts_with('<') {
        Some(ConvertFormat::Xml)
    } else if trimmed.contains("\n[") && trimmed.contains('=') {
        Some(ConvertFormat::Ini)
    } else if trimmed.starts_with("---") || trimmed.contains(": ") {
        Some(ConvertFormat::Yaml)
    } else {
        None
    }
}

/// List all supported format pairs.
pub fn supported_conversions() -> Vec<(ConvertFormat, ConvertFormat)> {
    let fmts = [
        ConvertFormat::Json,
        ConvertFormat::Yaml,
        ConvertFormat::Toml,
        ConvertFormat::Csv,
        ConvertFormat::Xml,
        ConvertFormat::Ini,
    ];
    let mut pairs = Vec::new();
    for &a in &fmts {
        for &b in &fmts {
            if a != b {
                pairs.push((a, b));
            }
        }
    }
    pairs
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_roundtrip() {
        let json = r#"{"name":"atp","version":1,"active":true}"#;
        let c = Converter::new();
        let doc = c.parse(json, ConvertFormat::Json).unwrap();
        let out = c.render(&doc, ConvertFormat::Json).unwrap();
        let doc2 = c.parse(&out, ConvertFormat::Json).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_yaml_roundtrip() {
        let yaml = "name: atp\nversion: 1\nactive: true\n";
        let c = Converter::new();
        let doc = c.parse(yaml, ConvertFormat::Yaml).unwrap();
        assert_eq!(doc.as_map().unwrap()["name"], DocValue::Str("atp".into()));
        let out = c.render(&doc, ConvertFormat::Yaml).unwrap();
        let doc2 = c.parse(&out, ConvertFormat::Yaml).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_toml_roundtrip() {
        let toml_text = "name = \"atp\"\nversion = 1\nactive = true\n";
        let c = Converter::new();
        let doc = c.parse(toml_text, ConvertFormat::Toml).unwrap();
        let out = c.render(&doc, ConvertFormat::Toml).unwrap();
        let doc2 = c.parse(&out, ConvertFormat::Toml).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_csv_with_header() {
        let csv = "name,age,active\nalice,30,true\nbob,25,false";
        let c = Converter::new();
        let doc = c.parse(csv, ConvertFormat::Csv).unwrap();
        let arr = doc.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(
            arr[0].as_map().unwrap()["name"],
            DocValue::Str("alice".into())
        );
        assert_eq!(arr[0].as_map().unwrap()["age"], DocValue::Int(30));
    }

    #[test]
    fn test_csv_roundtrip() {
        let csv = "name,age\nalice,30\nbob,25";
        let c = Converter::new();
        let doc = c.parse(csv, ConvertFormat::Csv).unwrap();
        let out = c.render(&doc, ConvertFormat::Csv).unwrap();
        assert!(out.contains("alice"));
        assert!(out.contains("bob"));
    }

    #[test]
    fn test_xml_parse() {
        let xml = "<root><name>atp</name><version>1</version></root>";
        let c = Converter::new();
        let doc = c.parse(xml, ConvertFormat::Xml).unwrap();
        let m = doc.as_map().unwrap();
        assert_eq!(m["name"], DocValue::Str("atp".into()));
        assert_eq!(m["version"], DocValue::Int(1));
    }

    #[test]
    fn test_xml_render() {
        let mut m = BTreeMap::new();
        m.insert("tool".to_string(), DocValue::Str("atp".into()));
        m.insert("ver".to_string(), DocValue::Int(2));
        let doc = DocValue::Map(m);
        let c = Converter::new();
        let out = c.render(&doc, ConvertFormat::Xml).unwrap();
        assert!(out.contains("<tool>atp</tool>"));
        assert!(out.contains("<ver>2</ver>"));
    }

    #[test]
    fn test_ini_roundtrip() {
        let ini = "[database]\nhost = localhost\nport = 5432\n\n[app]\ndebug = true\n";
        let c = Converter::new();
        let doc = c.parse(ini, ConvertFormat::Ini).unwrap();
        let m = doc.as_map().unwrap();
        let db = m["database"].as_map().unwrap();
        assert_eq!(db["host"], DocValue::Str("localhost".into()));
        assert_eq!(db["port"], DocValue::Int(5432));
        let out = c.render(&doc, ConvertFormat::Ini).unwrap();
        assert!(out.contains("[database]"));
        assert!(out.contains("port = 5432"));
    }

    #[test]
    fn test_json_to_yaml() {
        let json = r#"{"a":1,"b":"hello"}"#;
        let c = Converter::new();
        let yaml = c
            .convert(json, ConvertFormat::Json, ConvertFormat::Yaml)
            .unwrap();
        assert!(yaml.contains("a:"));
        assert!(yaml.contains("hello"));
    }

    #[test]
    fn test_json_to_toml() {
        let json = r#"{"name":"atp","count":42}"#;
        let c = Converter::new();
        let toml = c
            .convert(json, ConvertFormat::Json, ConvertFormat::Toml)
            .unwrap();
        assert!(toml.contains("name = \"atp\""));
        assert!(toml.contains("count = 42"));
    }

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format("{\"a\":1}"), Some(ConvertFormat::Json));
        assert_eq!(detect_format("[1,2]"), Some(ConvertFormat::Json));
        assert_eq!(
            detect_format("<?xml version=\"1.0\"?>"),
            Some(ConvertFormat::Xml)
        );
        assert_eq!(detect_format("---\na: 1"), Some(ConvertFormat::Yaml));
    }

    #[test]
    fn test_from_extension() {
        assert_eq!(
            ConvertFormat::from_extension("json"),
            Some(ConvertFormat::Json)
        );
        assert_eq!(
            ConvertFormat::from_extension("yml"),
            Some(ConvertFormat::Yaml)
        );
        assert_eq!(
            ConvertFormat::from_extension("cfg"),
            Some(ConvertFormat::Ini)
        );
        assert_eq!(ConvertFormat::from_extension("rs"), None);
    }

    #[test]
    fn test_doc_value_merge() {
        let mut a = DocValue::Map(BTreeMap::from([("x".into(), DocValue::Int(1))]));
        let b = DocValue::Map(BTreeMap::from([("y".into(), DocValue::Int(2))]));
        a.merge(b);
        let m = a.as_map().unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m["x"], DocValue::Int(1));
        assert_eq!(m["y"], DocValue::Int(2));
    }

    #[test]
    fn test_supported_conversions() {
        let pairs = supported_conversions();
        // 6 formats, each can convert to 5 others = 30
        assert_eq!(pairs.len(), 30);
    }

    #[test]
    fn test_yaml_to_json() {
        let yaml = "items:\n  - a\n  - b\n";
        let c = Converter::new();
        let json = c
            .convert(yaml, ConvertFormat::Yaml, ConvertFormat::Json)
            .unwrap();
        assert!(json.contains("\"items\""));
        assert!(json.contains("\"a\""));
    }

    #[test]
    fn test_csv_quoted_fields() {
        let csv = "name,desc\n\"Alice, Jr.\",\"She said \"\"hi\"\"\"";
        let c = Converter::new();
        let doc = c.parse(csv, ConvertFormat::Csv).unwrap();
        let arr = doc.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let m = arr[0].as_map().unwrap();
        assert_eq!(m["name"], DocValue::Str("Alice, Jr.".into()));
    }

    #[test]
    fn test_converter_default() {
        let c = Converter::default();
        assert!(c.options.pretty);
        assert!(c.options.csv_header);
    }
}
