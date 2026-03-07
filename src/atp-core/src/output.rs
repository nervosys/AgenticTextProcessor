//! Output types and formatting for ATP.
//!
//! Every ATP operation produces strongly typed output wrapped in an [`AtpEnvelope`].
//! This ensures agents always receive structured, schema-validated, deterministic data.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ─── Envelope ────────────────────────────────────────────────────────────────

/// Universal output envelope wrapping every ATP result.
/// Agents can rely on this structure for *every* command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtpEnvelope<T: Serialize> {
    /// ATP semantic version
    pub version: String,
    /// Canonical command name that produced this output (e.g. "search", "transform")
    pub command: String,
    /// ISO-8601 UTC timestamp of execution
    pub timestamp: String,
    /// Whether the output is fully deterministic given the same inputs
    pub deterministic: bool,
    /// Reference to the JSON schema for `data`
    pub schema_ref: String,
    /// The actual result payload
    pub data: T,
    /// Execution metadata for provenance tracking
    pub metadata: ExecutionMetadata,
    /// Optional CUI / data classification marking (NIST SP 800-171)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_marking: Option<crate::compliance::DataMarking>,
}

impl<T: Serialize> AtpEnvelope<T> {
    pub fn new(command: &str, data: T, metadata: ExecutionMetadata) -> Self {
        let schema_ref = format!(
            "https://atp.nervosys.com/schemas/v{}/{}.json",
            crate::ATP_VERSION,
            command
        );
        Self {
            version: crate::ATP_VERSION.to_string(),
            command: command.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            deterministic: true,
            schema_ref,
            data,
            metadata,
            data_marking: None,
        }
    }

    /// Attach a CUI/data classification marking to this envelope.
    pub fn with_data_marking(mut self, marking: crate::compliance::DataMarking) -> Self {
        self.data_marking = Some(marking);
        self
    }
}

// ─── Metadata ────────────────────────────────────────────────────────────────

/// Execution metadata attached to every output for reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub files_scanned: usize,
    pub files_matched: usize,
    pub duration_ms: u64,
    pub provenance: Provenance,
}

/// Full provenance record enabling exact reproduction of a result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub tool: String,
    pub tool_version: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub input_hash: Option<String>,
}

impl Provenance {
    pub fn new(command: &str, args: Vec<String>) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self {
            tool: crate::ATP_TOOL_ID.to_string(),
            tool_version: crate::ATP_VERSION.to_string(),
            command: command.to_string(),
            args,
            working_directory: cwd,
            input_hash: None,
        }
    }

    /// Set the input content hash (FIPS 180-4 SHA-256) for provenance tracking.
    /// This enables chain-of-custody verification per NIST SP 800-171 / CMMC SI.L2-3.14.7.
    pub fn with_input_hash(mut self, hash: String) -> Self {
        self.input_hash = Some(hash);
        self
    }
}

// ─── Search types ────────────────────────────────────────────────────────────

/// Results from a search (grep-like) operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub pattern: String,
    pub pattern_type: PatternType,
    pub case_sensitive: bool,
    pub total_matches: usize,
    pub files_with_matches: usize,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternType {
    Regex,
    Literal,
    Glob,
    Semantic,
}

/// A single search match with full context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    /// Absolute or relative file path
    pub file: String,
    /// 1-based line number
    pub line_number: usize,
    /// 1-based column of match start
    pub column_start: usize,
    /// 1-based column of match end (exclusive)
    pub column_end: usize,
    /// The complete line containing the match
    pub line_content: String,
    /// The exact matched text
    pub matched_text: String,
    /// Lines of context before the match
    pub context_before: Vec<ContextLine>,
    /// Lines of context after the match
    pub context_after: Vec<ContextLine>,
    /// Byte offset from start of file
    pub byte_offset: usize,
}

/// A single line of context surrounding a match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextLine {
    pub line_number: usize,
    pub content: String,
}

// ─── Transform types ─────────────────────────────────────────────────────────

/// Results from a transform (sed-like) operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformResults {
    pub pattern: String,
    pub replacement: String,
    pub total_changes: usize,
    pub files_modified: usize,
    pub applied: bool,
    pub changes: Vec<FileChanges>,
}

/// Changes made to a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChanges {
    pub file: String,
    pub change_count: usize,
    pub changes: Vec<ChangeRecord>,
}

/// A single change within a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    pub line_number: usize,
    pub original: String,
    pub replacement: String,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Substitution,
    Deletion,
    Insertion,
    Transliteration,
}

// ─── Analysis types ──────────────────────────────────────────────────────────

/// Results from an analyze (awk-like) operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResults {
    pub records_processed: usize,
    pub output_records: Vec<AnalysisRecord>,
    pub aggregations: BTreeMap<String, AggregationValue>,
    pub field_separator: String,
}

/// A single output record from analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRecord {
    pub source_file: String,
    pub source_line: usize,
    /// Record number (NR) — 1-based sequential across all files
    pub nr: usize,
    /// Number of fields in this record (NF)
    pub nf: usize,
    pub fields: Vec<String>,
    pub computed: BTreeMap<String, serde_json::Value>,
}

/// Typed aggregation values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum AggregationValue {
    Count(usize),
    Sum(f64),
    Average(f64),
    Min(f64),
    Max(f64),
    Distinct(Vec<String>),
    Frequency(BTreeMap<String, usize>),
}

// ─── Pipeline types ──────────────────────────────────────────────────────────

/// Results from a pipeline operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResults {
    pub stages: Vec<PipelineStageResult>,
    pub final_output: serde_json::Value,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageResult {
    pub stage_index: usize,
    pub stage_type: String,
    pub duration_ms: u64,
    pub records_in: usize,
    pub records_out: usize,
}

// ─── Explain types ───────────────────────────────────────────────────────────

/// Explanation of what a command *would* do, without executing it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainResult {
    pub command: String,
    pub description: String,
    pub will_modify_files: bool,
    pub estimated_files_affected: usize,
    pub steps: Vec<ExplainStep>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainStep {
    pub order: usize,
    pub action: String,
    pub description: String,
}

// ─── Scope types ─────────────────────────────────────────────────────────────

/// Defines the scope of files to process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScope {
    /// Root paths to search from
    pub roots: Vec<PathBuf>,
    /// Glob patterns to include
    pub include_globs: Vec<String>,
    /// Glob patterns to exclude
    pub exclude_globs: Vec<String>,
    /// Maximum directory depth
    pub max_depth: Option<usize>,
    /// Whether to respect .gitignore
    pub respect_gitignore: bool,
    /// Whether to follow symlinks
    pub follow_symlinks: bool,
}

impl Default for FileScope {
    fn default() -> Self {
        Self {
            roots: vec![PathBuf::from(".")],
            include_globs: vec![],
            exclude_globs: vec![],
            max_depth: None,
            respect_gitignore: true,
            follow_symlinks: false,
        }
    }
}

// ─── Error types ─────────────────────────────────────────────────────────────

/// Typed error taxonomy for agent consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtpError {
    pub code: ErrorCode,
    pub message: String,
    pub context: Option<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    InvalidPattern,
    InvalidReplacement,
    FileNotFound,
    PermissionDenied,
    InvalidScope,
    PipelineError,
    IoError,
    EncodingError,
    InvalidArgument,
}

// ─── Output format ──────────────────────────────────────────────────────────

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    Json,
    JsonPretty,
    JsonLines,
    Yaml,
    Csv,
    Human,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Json => write!(f, "json"),
            Format::JsonPretty => write!(f, "json-pretty"),
            Format::JsonLines => write!(f, "jsonl"),
            Format::Yaml => write!(f, "yaml"),
            Format::Csv => write!(f, "csv"),
            Format::Human => write!(f, "human"),
        }
    }
}

impl std::str::FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Format::Json),
            "json-pretty" => Ok(Format::JsonPretty),
            "jsonl" | "jsonlines" | "json-lines" => Ok(Format::JsonLines),
            "yaml" | "yml" => Ok(Format::Yaml),
            "csv" => Ok(Format::Csv),
            "human" | "text" | "plain" => Ok(Format::Human),
            _ => Err(format!(
                "Unknown format: {s}. Use: json, json-pretty, jsonl, yaml, csv, human"
            )),
        }
    }
}

/// Formats an AtpEnvelope to the desired output format.
pub struct OutputFormatter;

impl OutputFormatter {
    pub fn format<T: Serialize>(envelope: &AtpEnvelope<T>, fmt: Format) -> anyhow::Result<String> {
        match fmt {
            Format::Json => Ok(serde_json::to_string(envelope)?),
            Format::JsonPretty => Ok(serde_json::to_string_pretty(envelope)?),
            Format::Yaml => Ok(serde_yaml::to_string(envelope)?),
            Format::JsonLines => {
                // JSONL: output each data item on its own line
                let data_val = serde_json::to_value(&envelope.data)?;
                match data_val {
                    serde_json::Value::Array(items) => {
                        let lines: Vec<String> = items
                            .iter()
                            .map(serde_json::to_string)
                            .collect::<Result<_, _>>()?;
                        Ok(lines.join("\n"))
                    }
                    _ => {
                        // Single item — output as one JSON line
                        Ok(serde_json::to_string(&data_val)?)
                    }
                }
            }
            Format::Csv => {
                // Simple CSV serialization of the data portion
                let val = serde_json::to_value(&envelope.data)?;
                Ok(Self::value_to_csv(&val))
            }
            Format::Human => {
                // Delegate to Display-style formatting
                Ok(serde_json::to_string_pretty(&envelope.data)?)
            }
        }
    }

    fn value_to_csv(val: &serde_json::Value) -> String {
        match val {
            serde_json::Value::Array(arr) => {
                let mut lines = Vec::new();
                // Header from first object
                if let Some(serde_json::Value::Object(obj)) = arr.first() {
                    lines.push(obj.keys().cloned().collect::<Vec<_>>().join(","));
                }
                for item in arr {
                    if let serde_json::Value::Object(obj) = item {
                        let row: Vec<String> = obj
                            .values()
                            .map(|v| match v {
                                serde_json::Value::String(s) => {
                                    format!("\"{}\"", s.replace('"', "\"\""))
                                }
                                other => other.to_string(),
                            })
                            .collect();
                        lines.push(row.join(","));
                    }
                }
                lines.join("\n")
            }
            other => other.to_string(),
        }
    }
}

/// Format search results for human-readable terminal output.
pub fn format_search_human(results: &SearchResults, colorize: bool) -> String {
    let mut out = String::new();
    for m in &results.matches {
        for ctx in &m.context_before {
            out.push_str(&format!(
                "{}:{}: {}\n",
                m.file, ctx.line_number, ctx.content
            ));
        }
        if colorize {
            let highlighted = m.line_content.replace(
                &m.matched_text,
                &format!("\x1b[1;31m{}\x1b[0m", m.matched_text),
            );
            out.push_str(&format!(
                "\x1b[35m{}\x1b[0m:\x1b[32m{}\x1b[0m: {}\n",
                m.file, m.line_number, highlighted
            ));
        } else {
            out.push_str(&format!(
                "{}:{}: {}\n",
                m.file, m.line_number, m.line_content
            ));
        }
        for ctx in &m.context_after {
            out.push_str(&format!(
                "{}:{}: {}\n",
                m.file, ctx.line_number, ctx.content
            ));
        }
        if !m.context_before.is_empty() || !m.context_after.is_empty() {
            out.push_str("--\n");
        }
    }
    if !results.matches.is_empty() {
        out.push_str(&format!(
            "\n{} matches in {} files\n",
            results.total_matches, results.files_with_matches
        ));
    }
    out
}

/// Format transform results for human-readable terminal output.
pub fn format_transform_human(results: &TransformResults, colorize: bool) -> String {
    let mut out = String::new();
    let status = if results.applied {
        "APPLIED"
    } else {
        "DRY RUN"
    };
    out.push_str(&format!(
        "[{}] {} changes in {} files\n\n",
        status, results.total_changes, results.files_modified
    ));
    for fc in &results.changes {
        out.push_str(&format!("--- {}\n", fc.file));
        for c in &fc.changes {
            if colorize {
                out.push_str(&format!(
                    "  L{}: \x1b[31m- {}\x1b[0m\n  L{}: \x1b[32m+ {}\x1b[0m\n",
                    c.line_number, c.original, c.line_number, c.replacement
                ));
            } else {
                out.push_str(&format!(
                    "  L{}: - {}\n  L{}: + {}\n",
                    c.line_number, c.original, c.line_number, c.replacement
                ));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metadata() -> ExecutionMetadata {
        ExecutionMetadata {
            files_scanned: 0,
            files_matched: 0,
            duration_ms: 0,
            provenance: Provenance::new("test", vec![]),
        }
    }

    #[test]
    fn test_provenance_new() {
        let p = Provenance::new("test.cmd", vec!["arg1".into()]);
        assert_eq!(p.command, "test.cmd");
        assert_eq!(p.tool, crate::ATP_TOOL_ID);
        assert_eq!(p.args, vec!["arg1"]);
        assert!(p.input_hash.is_none());
    }

    #[test]
    fn test_provenance_with_input_hash() {
        let p = Provenance::new("cmd", vec![]).with_input_hash("abc123".to_string());
        assert_eq!(p.input_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_envelope_new() {
        let data = "data".to_string();
        let env = AtpEnvelope::new(
            "search",
            &data,
            ExecutionMetadata {
                files_scanned: 10,
                files_matched: 3,
                duration_ms: 42,
                provenance: Provenance::new("search", vec![]),
            },
        );
        assert_eq!(env.version, crate::ATP_VERSION);
        assert_eq!(env.command, "search");
        assert_eq!(env.data, &data);
        assert_eq!(env.metadata.files_scanned, 10);
        assert_eq!(env.metadata.files_matched, 3);
        assert!(env.data_marking.is_none());
    }

    #[test]
    fn test_format_json() {
        let data = vec!["hello".to_string()];
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let json = OutputFormatter::format(&env, Format::Json).unwrap();
        assert!(json.contains("\"hello\""));
        assert!(json.contains("\"command\":\"test\""));
    }

    #[test]
    fn test_format_json_pretty() {
        let data = "val".to_string();
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let json = OutputFormatter::format(&env, Format::JsonPretty).unwrap();
        assert!(json.contains('\n'));
        assert!(json.contains("\"version\""));
    }

    #[test]
    fn test_format_yaml() {
        let data = "val".to_string();
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let yaml = OutputFormatter::format(&env, Format::Yaml).unwrap();
        assert!(yaml.contains("command: test"));
    }

    #[test]
    fn test_format_jsonl_array() {
        let data = vec!["a", "b", "c"];
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let jsonl = OutputFormatter::format(&env, Format::JsonLines).unwrap();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_format_jsonl_single() {
        let data = "single".to_string();
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let jsonl = OutputFormatter::format(&env, Format::JsonLines).unwrap();
        assert_eq!(jsonl.lines().count(), 1);
    }

    #[test]
    fn test_format_from_str() {
        assert_eq!("json".parse::<Format>().unwrap(), Format::Json);
        assert_eq!("json-pretty".parse::<Format>().unwrap(), Format::JsonPretty);
        assert_eq!("yaml".parse::<Format>().unwrap(), Format::Yaml);
        assert_eq!("jsonl".parse::<Format>().unwrap(), Format::JsonLines);
        assert_eq!("csv".parse::<Format>().unwrap(), Format::Csv);
        assert_eq!("human".parse::<Format>().unwrap(), Format::Human);
        assert!("invalid".parse::<Format>().is_err());
    }

    #[test]
    fn test_format_display() {
        assert_eq!(format!("{}", Format::Json), "json");
        assert_eq!(format!("{}", Format::Human), "human");
    }

    #[test]
    fn test_file_scope_default() {
        let scope = FileScope::default();
        assert!(!scope.roots.is_empty()); // default root is "."
        assert!(scope.include_globs.is_empty());
        assert!(scope.exclude_globs.is_empty());
        assert!(scope.max_depth.is_none());
    }

    #[test]
    fn test_data_marking() {
        let data = "val".to_string();
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let marked = env.with_data_marking(crate::compliance::DataMarking::default());
        assert!(marked.data_marking.is_some());
    }

    #[test]
    fn test_format_csv() {
        let data = vec![
            serde_json::json!({"name": "alice", "age": 30}),
            serde_json::json!({"name": "bob", "age": 25}),
        ];
        let env = AtpEnvelope::new("test", &data, make_metadata());
        let csv = OutputFormatter::format(&env, Format::Csv).unwrap();
        assert!(csv.contains("name"));
        assert!(csv.contains("alice"));
    }

    #[test]
    fn test_envelope_deterministic() {
        let data = "x".to_string();
        let env = AtpEnvelope::new("test", &data, make_metadata());
        assert!(env.deterministic);
    }
}
