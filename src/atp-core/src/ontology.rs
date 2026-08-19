//! Ontology module — machine-readable self-description of ATP capabilities.
//!
//! The ontology enables AI agents to discover, understand, and correctly invoke
//! every ATP command without human documentation. It provides:
//! - Complete command specifications with typed parameters
//! - Output schemas for every command
//! - Capability declarations
//! - Usage examples
//! - Semantic tags for intent matching

use serde::{Deserialize, Serialize};

/// The complete ATP ontology — a self-describing manifest of all capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ontology {
    pub tool: String,
    pub version: String,
    pub description: String,
    pub homepage: String,
    pub capabilities: Vec<Capability>,
    pub commands: Vec<CommandSpec>,
    pub types: Vec<TypeSpec>,
    pub output_formats: Vec<String>,
    pub error_codes: Vec<ErrorCodeSpec>,
    pub examples: Vec<UsageExample>,
}

/// A high-level capability declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub description: String,
    pub semantic_tags: Vec<String>,
}

/// Full specification of a CLI command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub long_description: String,
    pub parameters: Vec<ParameterSpec>,
    pub output_type: String,
    pub output_schema: serde_json::Value,
    pub deterministic: bool,
    pub modifies_files: bool,
    pub supports_dry_run: bool,
    pub supports_explain: bool,
    pub examples: Vec<UsageExample>,
    pub semantic_tags: Vec<String>,
    pub related_commands: Vec<String>,
}

/// Specification of a command parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub name: String,
    pub short: Option<String>,
    pub description: String,
    pub param_type: ParamType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub valid_values: Option<Vec<String>>,
    pub semantic_role: Option<String>,
}

/// Parameter types for the ontology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    String,
    Integer,
    Float,
    Boolean,
    Path,
    Pattern,
    Replacement,
    Enum(Vec<String>),
    Array(Box<ParamType>),
}

/// Specification of a named type in the output schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
}

/// Specification of an error code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCodeSpec {
    pub code: String,
    pub description: String,
    pub recoverable: bool,
    pub suggestion: String,
}

/// A usage example.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageExample {
    pub description: String,
    pub command: String,
    pub expected_output_summary: String,
}

/// Build the complete ATP ontology.
pub fn build_ontology() -> Ontology {
    Ontology {
        tool: "atp".to_string(),
        version: crate::ATP_VERSION.to_string(),
        description: "Agentic Text Processor — an agentic-first successor to grep, sed, and awk. \
            Provides strongly typed, deterministic text processing with full machine-readable \
            ontology for AI agent discoverability and operation."
            .to_string(),
        homepage: "https://github.com/nervosys/AgenticTextProcessor".to_string(),
        capabilities: build_capabilities(),
        commands: build_command_specs(),
        types: build_type_specs(),
        output_formats: vec![
            "json".into(),
            "json-pretty".into(),
            "jsonl".into(),
            "yaml".into(),
            "csv".into(),
            "human".into(),
        ],
        error_codes: build_error_codes(),
        examples: build_global_examples(),
    }
}

fn build_capabilities() -> Vec<Capability> {
    vec![
        Capability {
            id: "search".into(),
            name: "Pattern Search".into(),
            description: "Search for patterns across files and directories with regex, literal, or glob matching. Returns strongly typed match results with context.".into(),
            semantic_tags: vec!["grep".into(), "find".into(), "search".into(), "match".into(), "pattern".into(), "regex".into()],
        },
        Capability {
            id: "transform".into(),
            name: "Text Transformation".into(),
            description: "Transform text with substitution, deletion, insertion, and transliteration. Supports dry-run preview and atomic multi-file application.".into(),
            semantic_tags: vec!["sed".into(), "replace".into(), "substitute".into(), "transform".into(), "edit".into(), "modify".into()],
        },
        Capability {
            id: "analyze".into(),
            name: "Record Analysis".into(),
            description: "Analyze structured and semi-structured text with field extraction, filtering, and aggregation. Returns typed records with computed fields.".into(),
            semantic_tags: vec!["awk".into(), "analyze".into(), "aggregate".into(), "fields".into(), "statistics".into(), "tabular".into()],
        },
        Capability {
            id: "pipeline".into(),
            name: "Pipeline Composition".into(),
            description: "Compose multi-stage processing pipelines combining search, transform, and analyze operations with typed intermediates.".into(),
            semantic_tags: vec!["pipeline".into(), "chain".into(), "compose".into(), "workflow".into(), "multi-stage".into()],
        },
        Capability {
            id: "scope".into(),
            name: "Scope Control".into(),
            description: "Define precise file and directory scopes with glob patterns, gitignore awareness, depth limits, and file type filtering.".into(),
            semantic_tags: vec!["files".into(), "directories".into(), "scope".into(), "filter".into(), "glob".into(), "gitignore".into()],
        },
        Capability {
            id: "ontology".into(),
            name: "Self-Description".into(),
            description: "Machine-readable self-description of all capabilities, commands, parameters, types, and examples for AI agent discovery.".into(),
            semantic_tags: vec!["ontology".into(), "discovery".into(), "schema".into(), "metadata".into(), "capabilities".into()],
        },
        Capability {
            id: "explain".into(),
            name: "Command Explanation".into(),
            description: "Explain what any command will do before execution. Returns structured step-by-step description with impact analysis.".into(),
            semantic_tags: vec!["explain".into(), "preview".into(), "dry-run".into(), "impact".into(), "safety".into()],
        },
        Capability {
            id: "provenance".into(),
            name: "Provenance Tracking".into(),
            description: "Every output includes provenance metadata enabling exact reproduction: tool version, command, arguments, working directory, input hash.".into(),
            semantic_tags: vec!["provenance".into(), "reproducibility".into(), "lineage".into(), "audit".into(), "traceability".into()],
        },
        Capability {
            id: "context_extraction".into(),
            name: "Smart Context Extraction".into(),
            description: "Extract meaningful context windows around matches: function-level, block-level, or custom line ranges for code-aware processing.".into(),
            semantic_tags: vec!["context".into(), "code".into(), "function".into(), "block".into(), "scope".into()],
        },
        Capability {
            id: "multi_file".into(),
            name: "Multi-File Processing".into(),
            description: "Process operations across multiple files and directories atomically, with consistent ordering and aggregate results.".into(),
            semantic_tags: vec!["multi-file".into(), "directory".into(), "recursive".into(), "batch".into(), "atomic".into()],
        },
        Capability {
            id: "regulatory_compliance".into(),
            name: "Regulatory Compliance".into(),
            description: "NIST FIPS 180-4 SHA-256 hashing, CMMC 2.0 control mapping, CUI data markings, structured audit logging, and compliance reporting for DoD-relevant regulations.".into(),
            semantic_tags: vec!["compliance".into(), "fips".into(), "cmmc".into(), "nist".into(), "dod".into(), "cui".into(), "audit".into(), "sha256".into(), "integrity".into()],
        },
        Capability {
            id: "backwards_compatibility".into(),
            name: "POSIX Tool Compatibility".into(),
            description: "Standalone atp-grep, atp-sed, and atp-awk binaries that accept traditional POSIX flag syntax (grep -i, sed -e, awk -F) while producing ATP's strongly typed structured output. Drop-in replacements for grep/sed/awk with typed JSON/YAML/CSV output and full provenance metadata.".into(),
            semantic_tags: vec!["compat".into(), "grep".into(), "sed".into(), "awk".into(), "posix".into(), "drop-in".into(), "backwards-compatible".into()],
        },
    ]
}

fn build_command_specs() -> Vec<CommandSpec> {
    let mut specs = vec![
        CommandSpec {
            name: "search".into(),
            aliases: vec!["s".into(), "grep".into(), "find".into()],
            description: "Search for patterns in files".into(),
            long_description: "Search for regex, literal, or glob patterns across one or more files and directories. Returns strongly typed match results including line numbers, columns, byte offsets, and configurable context windows. Supports case-insensitive, whole-word, inverted, and multiline matching.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "pattern".into(), short: Some("p".into()),
                    description: "The search pattern (regex by default)".into(),
                    param_type: ParamType::Pattern, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: Some("search_pattern".into()),
                },
                ParameterSpec {
                    name: "paths".into(), short: None,
                    description: "Files or directories to search".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)), required: false,
                    default_value: Some(serde_json::json!(["."])), valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
                ParameterSpec {
                    name: "literal".into(), short: Some("l".into()),
                    description: "Treat pattern as literal string, not regex".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "case-insensitive".into(), short: Some("i".into()),
                    description: "Case-insensitive matching".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "context".into(), short: Some("C".into()),
                    description: "Number of context lines before and after each match".into(),
                    param_type: ParamType::Integer, required: false,
                    default_value: Some(serde_json::json!(0)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "whole-word".into(), short: Some("w".into()),
                    description: "Match whole words only".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "invert".into(), short: Some("v".into()),
                    description: "Invert match (show non-matching lines)".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "max-matches".into(), short: Some("m".into()),
                    description: "Maximum total matches to return".into(),
                    param_type: ParamType::Integer, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "include".into(), short: None,
                    description: "Glob pattern for files to include".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("file_filter".into()),
                },
                ParameterSpec {
                    name: "exclude".into(), short: None,
                    description: "Glob pattern for files to exclude".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("file_filter".into()),
                },
            ],
            output_type: "SearchResults".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "pattern_type": {"type": "string", "enum": ["Regex", "Literal", "Glob", "Semantic"]},
                    "case_sensitive": {"type": "boolean"},
                    "total_matches": {"type": "integer"},
                    "files_with_matches": {"type": "integer"},
                    "matches": {
                        "type": "array",
                        "items": {"$ref": "#/types/SearchMatch"}
                    }
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: true,
            examples: vec![
                UsageExample {
                    description: "Search for a function definition".into(),
                    command: "atp search 'fn\\s+\\w+' src/ --include '*.rs'".into(),
                    expected_output_summary: "JSON with all matching function definitions, file locations, and context".into(),
                },
                UsageExample {
                    description: "Case-insensitive literal search".into(),
                    command: "atp search --literal --case-insensitive 'TODO' .".into(),
                    expected_output_summary: "All TODO comments with file paths and line numbers".into(),
                },
            ],
            semantic_tags: vec!["search".into(), "grep".into(), "find".into(), "match".into()],
            related_commands: vec!["transform".into(), "analyze".into(), "pipeline".into()],
        },
        CommandSpec {
            name: "transform".into(),
            aliases: vec!["t".into(), "sed".into(), "replace".into()],
            description: "Transform text with pattern-based substitution".into(),
            long_description: "Apply text transformations using regex substitution, deletion, insertion, or transliteration. Supports dry-run preview, in-place editing with backups, and multi-file atomic application. Returns a diff of all changes for verification.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "expression".into(), short: Some("e".into()),
                    description: "Sed-style expression (e.g. s/pattern/replacement/flags)".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("transform_expression".into()),
                },
                ParameterSpec {
                    name: "pattern".into(), short: Some("p".into()),
                    description: "Pattern to match for substitution".into(),
                    param_type: ParamType::Pattern, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("search_pattern".into()),
                },
                ParameterSpec {
                    name: "replacement".into(), short: Some("r".into()),
                    description: "Replacement string (supports capture groups)".into(),
                    param_type: ParamType::Replacement, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("replacement_text".into()),
                },
                ParameterSpec {
                    name: "global".into(), short: Some("g".into()),
                    description: "Replace all occurrences per line (not just first)".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "in-place".into(), short: None,
                    description: "Modify files in place".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "backup".into(), short: None,
                    description: "Backup extension for in-place editing (e.g. 'bak')".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "dry-run".into(), short: Some("n".into()),
                    description: "Preview changes without applying them".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(true)), valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "TransformResults".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "replacement": {"type": "string"},
                    "total_changes": {"type": "integer"},
                    "files_modified": {"type": "integer"},
                    "applied": {"type": "boolean"},
                    "changes": {"type": "array"}
                }
            }),
            deterministic: true,
            modifies_files: true,
            supports_dry_run: true,
            supports_explain: true,
            examples: vec![
                UsageExample {
                    description: "Replace all occurrences of 'foo' with 'bar'".into(),
                    command: "atp transform -e 's/foo/bar/g' src/".into(),
                    expected_output_summary: "Dry-run diff showing all substitutions".into(),
                },
                UsageExample {
                    description: "Rename a function across a codebase".into(),
                    command: "atp transform -p 'old_function' -r 'new_function' -g --in-place --backup bak src/".into(),
                    expected_output_summary: "Applied changes with backup files created".into(),
                },
            ],
            semantic_tags: vec!["transform".into(), "sed".into(), "replace".into(), "substitute".into()],
            related_commands: vec!["search".into(), "pipeline".into()],
        },
        CommandSpec {
            name: "analyze".into(),
            aliases: vec!["a".into(), "awk".into(), "fields".into()],
            description: "Analyze text with field-based processing and aggregation".into(),
            long_description: "Process text as structured records with configurable field separators. Extract, filter, and aggregate fields. Compute statistics, frequencies, and custom derived fields. Returns strongly typed analysis results.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "separator".into(), short: Some("F".into()),
                    description: "Field separator (regex). Default: whitespace".into(),
                    param_type: ParamType::Pattern, required: false,
                    default_value: Some(serde_json::json!("\\s+")), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "fields".into(), short: Some("f".into()),
                    description: "Fields to select (1-based, comma-separated)".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "pattern".into(), short: Some("p".into()),
                    description: "Pattern to filter lines before processing".into(),
                    param_type: ParamType::Pattern, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("search_pattern".into()),
                },
                ParameterSpec {
                    name: "aggregate".into(), short: None,
                    description: "Aggregation: count, sum:N, avg:N, min:N, max:N, distinct:N, freq:N".into(),
                    param_type: ParamType::Array(Box::new(ParamType::String)), required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "AnalysisResults".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "records_processed": {"type": "integer"},
                    "output_records": {"type": "array"},
                    "aggregations": {"type": "object"},
                    "field_separator": {"type": "string"}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: true,
            examples: vec![
                UsageExample {
                    description: "Extract specific columns from CSV".into(),
                    command: "atp analyze -F ',' -f '1,3,5' data.csv".into(),
                    expected_output_summary: "Extracted fields 1, 3, and 5 from each CSV row".into(),
                },
                UsageExample {
                    description: "Count word frequencies".into(),
                    command: "atp analyze --aggregate 'freq:1' wordlist.txt".into(),
                    expected_output_summary: "Frequency distribution of the first field".into(),
                },
            ],
            semantic_tags: vec!["analyze".into(), "awk".into(), "fields".into(), "aggregate".into()],
            related_commands: vec!["search".into(), "pipeline".into()],
        },
        CommandSpec {
            name: "pipeline".into(),
            aliases: vec!["pipe".into(), "chain".into()],
            description: "Execute multi-stage processing pipelines".into(),
            long_description: "Compose and execute multi-stage processing pipelines that chain search, transform, and analyze operations. Supports DSL syntax (stage1 | stage2 | ...), YAML definitions, and inline JSON. Pipeline data flows between stages with type-checked intermediates.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "expression".into(), short: Some("e".into()),
                    description: "Pipeline DSL expression: 'search:pattern | filter:pattern | head:10'".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("pipeline_expression".into()),
                },
                ParameterSpec {
                    name: "file".into(), short: Some("f".into()),
                    description: "Path to a pipeline definition file (YAML/JSON)".into(),
                    param_type: ParamType::Path, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "explain".into(), short: None,
                    description: "Explain what the pipeline will do without executing".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "PipelineResults".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stages": {"type": "array"},
                    "final_output": {},
                    "total_duration_ms": {"type": "integer"}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: true,
            supports_explain: true,
            examples: vec![
                UsageExample {
                    description: "Search, filter, and count".into(),
                    command: "atp pipeline -e 'search:TODO | filter:FIXME | count' src/".into(),
                    expected_output_summary: "Count of TODO lines that also contain FIXME".into(),
                },
                UsageExample {
                    description: "Extract and sort unique imports".into(),
                    command: "atp pipeline -e 'search:^import | sort | unique' src/".into(),
                    expected_output_summary: "Sorted, deduplicated list of import statements".into(),
                },
            ],
            semantic_tags: vec!["pipeline".into(), "chain".into(), "compose".into(), "workflow".into()],
            related_commands: vec!["search".into(), "transform".into(), "analyze".into()],
        },
        CommandSpec {
            name: "ontology".into(),
            aliases: vec!["onto".into(), "capabilities".into(), "schema".into()],
            description: "Output the complete ATP ontology for agent discovery".into(),
            long_description: "Output the complete machine-readable ontology describing all ATP capabilities, commands, parameters, types, output schemas, error codes, and usage examples. This is the primary discovery mechanism for AI agents.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "command".into(), short: Some("c".into()),
                    description: "Show ontology for a specific command only".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "section".into(), short: Some("s".into()),
                    description: "Show a specific section: capabilities, commands, types, errors, examples".into(),
                    param_type: ParamType::Enum(vec!["capabilities".into(), "commands".into(), "types".into(), "errors".into(), "examples".into()]),
                    required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "Ontology".into(),
            output_schema: serde_json::json!({"type": "object"}),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Get full ontology".into(),
                    command: "atp ontology".into(),
                    expected_output_summary: "Complete JSON ontology".into(),
                },
                UsageExample {
                    description: "Get search command schema".into(),
                    command: "atp ontology -c search".into(),
                    expected_output_summary: "JSON spec for the search command".into(),
                },
            ],
            semantic_tags: vec!["ontology".into(), "discovery".into(), "schema".into(), "help".into()],
            related_commands: vec![],
        },
        CommandSpec {
            name: "explain".into(),
            aliases: vec!["x".into(), "preview".into()],
            description: "Explain what a command will do before execution".into(),
            long_description: "Provide a detailed, structured explanation of what a command will do without actually executing it. Includes step-by-step description, impact analysis, estimated scope, and warnings. Essential for agent safety and verification.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "command-string".into(), short: None,
                    description: "The full atp command to explain".into(),
                    param_type: ParamType::String, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "ExplainResult".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "description": {"type": "string"},
                    "will_modify_files": {"type": "boolean"},
                    "estimated_files_affected": {"type": "integer"},
                    "steps": {"type": "array"},
                    "warnings": {"type": "array"}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Explain a transform command".into(),
                    command: "atp explain 'atp transform -e s/old/new/g src/'".into(),
                    expected_output_summary: "Structured explanation of what the transform will do".into(),
                },
            ],
            semantic_tags: vec!["explain".into(), "preview".into(), "safety".into()],
            related_commands: vec!["search".into(), "transform".into(), "analyze".into(), "pipeline".into()],
        },
        CommandSpec {
            name: "scope".into(),
            aliases: vec!["ls".into(), "files".into()],
            description: "List files matching the current scope configuration".into(),
            long_description: "Preview which files would be processed by a given scope configuration. Useful for verifying file selection before running search, transform, or analyze operations.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "paths".into(), short: None,
                    description: "Root paths to scan".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)), required: false,
                    default_value: Some(serde_json::json!(["."])), valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
                ParameterSpec {
                    name: "include".into(), short: None,
                    description: "Glob pattern for files to include".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("file_filter".into()),
                },
                ParameterSpec {
                    name: "exclude".into(), short: None,
                    description: "Glob pattern for files to exclude".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("file_filter".into()),
                },
                ParameterSpec {
                    name: "max-depth".into(), short: Some("d".into()),
                    description: "Maximum directory traversal depth".into(),
                    param_type: ParamType::Integer, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "ScopeResults".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "files": {"type": "array", "items": {"type": "string"}},
                    "total_files": {"type": "integer"},
                    "total_size_bytes": {"type": "integer"}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "List Rust source files".into(),
                    command: "atp scope --include '*.rs' src/".into(),
                    expected_output_summary: "JSON list of all .rs files under src/".into(),
                },
            ],
            semantic_tags: vec!["scope".into(), "files".into(), "list".into(), "directory".into()],
            related_commands: vec!["search".into(), "transform".into(), "analyze".into()],
        },
        CommandSpec {
            name: "query".into(),
            aliases: vec!["q".into(), "aql".into(), "run".into()],
            description: "Execute AQL (ATP Query Language) queries — unified syntax for agents and humans".into(),
            long_description: "AQL replaces grep/sed/awk regex syntax with a readable, unambiguous, keyword-based language optimized for both AI agents and humans. Supports composable pipeline stages, literal and regex patterns, stdin piping, and shell pipe chaining.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "query".into(), short: None,
                    description: "AQL query expression (e.g. 'find \"error\" ignore_case | count')".into(),
                    param_type: ParamType::String, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: Some("aql_expression".into()),
                },
                ParameterSpec {
                    name: "paths".into(), short: None,
                    description: "Files or directories to process (reads stdin if omitted and piped)".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)), required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
                ParameterSpec {
                    name: "explain".into(), short: None,
                    description: "Explain what the query will do without executing".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "validate".into(), short: None,
                    description: "Validate the query syntax without executing".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "include".into(), short: None,
                    description: "Glob pattern for files to include".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("file_filter".into()),
                },
                ParameterSpec {
                    name: "exclude".into(), short: None,
                    description: "Glob pattern for files to exclude".into(),
                    param_type: ParamType::String, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("file_filter".into()),
                },
                ParameterSpec {
                    name: "max-depth".into(), short: Some("d".into()),
                    description: "Maximum directory traversal depth".into(),
                    param_type: ParamType::Integer, required: false,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "PipelineResults".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stages": {"type": "array"},
                    "final_output": {"type": "array"},
                    "total_duration_ms": {"type": "integer"}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: true,
            examples: vec![
                UsageExample {
                    description: "Find all TODO comments".into(),
                    command: "atp query 'find \"TODO\" ignore_case' src/".into(),
                    expected_output_summary: "All lines containing TODO with file locations".into(),
                },
                UsageExample {
                    description: "Replace and count".into(),
                    command: "atp query 'replace \"old\" with \"new\" all | count' src/".into(),
                    expected_output_summary: "Count of lines where replacement was applied".into(),
                },
                UsageExample {
                    description: "Stdin piping".into(),
                    command: "cat data.csv | atp query 'set separator \",\" | select fields 1, 3 | sort'".into(),
                    expected_output_summary: "Sorted output of columns 1 and 3 from CSV data".into(),
                },
            ],
            semantic_tags: vec!["query".into(), "aql".into(), "unified".into(), "pipeline".into(), "agent".into()],
            related_commands: vec!["search".into(), "transform".into(), "analyze".into(), "pipeline".into()],
        },
        CommandSpec {
            name: "validate".into(),
            aliases: vec!["check".into()],
            description: "Validate a pattern, expression, pipeline, or AQL query without executing".into(),
            long_description: "Check the syntactic validity of a regex pattern, sed expression, pipeline DSL, or AQL query. Returns validation status, any error details, and a normalized representation of the validated input.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "input".into(), short: None,
                    description: "The input string to validate".into(),
                    param_type: ParamType::String, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "input-type".into(), short: Some("t".into()),
                    description: "Type of input to validate".into(),
                    param_type: ParamType::Enum(vec!["pattern".into(), "expression".into(), "pipeline".into(), "aql".into()]),
                    required: false,
                    default_value: Some(serde_json::json!("pattern")), valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "ValidationResult".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "valid": {"type": "boolean"},
                    "input_type": {"type": "string"},
                    "input": {"type": "string"},
                    "error": {"type": ["string", "null"]},
                    "normalized": {"type": ["string", "null"]}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Validate a regex pattern".into(),
                    command: "atp validate -t pattern 'fn\\s+\\w+'".into(),
                    expected_output_summary: "Validation result with normalized pattern".into(),
                },
                UsageExample {
                    description: "Validate an AQL query".into(),
                    command: "atp validate -t aql 'find \"x\" | sort | unique'".into(),
                    expected_output_summary: "Validation result with stage explanations".into(),
                },
            ],
            semantic_tags: vec!["validate".into(), "check".into(), "syntax".into(), "safety".into()],
            related_commands: vec!["query".into(), "search".into(), "transform".into(), "pipeline".into()],
        },
        CommandSpec {
            name: "context".into(),
            aliases: vec!["ctx".into()],
            description: "Extract smart context windows around a specific line".into(),
            long_description: "Go beyond line-based context to extract meaningful code regions around a target line. Supports function-level, block-level, indentation-based, and fixed line-range context modes. Useful for providing code-aware context to AI agents.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "file".into(), short: None,
                    description: "File to extract context from".into(),
                    param_type: ParamType::Path, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: Some("input_path".into()),
                },
                ParameterSpec {
                    name: "line".into(), short: Some("L".into()),
                    description: "Target line number (1-based)".into(),
                    param_type: ParamType::Integer, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "mode".into(), short: Some("m".into()),
                    description: "Context extraction mode".into(),
                    param_type: ParamType::Enum(vec!["function".into(), "block".into(), "indent".into(), "lines".into(), "file".into()]),
                    required: false,
                    default_value: Some(serde_json::json!("function")), valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "count".into(), short: Some("n".into()),
                    description: "Number of context lines (for 'lines' mode)".into(),
                    param_type: ParamType::Integer, required: false,
                    default_value: Some(serde_json::json!(10)), valid_values: None,
                    semantic_role: None,
                },
            ],
            output_type: "ContextResult".into(),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file": {"type": "string"},
                    "target_line": {"type": "integer"},
                    "mode": {"type": "string"},
                    "start_line": {"type": "integer"},
                    "end_line": {"type": "integer"},
                    "lines": {"type": "array", "items": {"type": "object"}}
                }
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Get the enclosing function".into(),
                    command: "atp context src/main.rs -L 42 -m function".into(),
                    expected_output_summary: "The complete function containing line 42".into(),
                },
                UsageExample {
                    description: "Get indentation-based scope".into(),
                    command: "atp context src/lib.py -L 100 -m indent".into(),
                    expected_output_summary: "Lines at the same or deeper indentation level".into(),
                },
            ],
            semantic_tags: vec!["context".into(), "code".into(), "function".into(), "scope".into()],
            related_commands: vec!["search".into()],
        },
        // ── Compliance ──────────────────────────────────────────────────────
        CommandSpec {
            name: "compliance".into(),
            aliases: vec!["cmmc".into(), "fips".into()],
            description: "Compliance reporting — NIST FIPS, CMMC 2.0, DoD regulatory posture.".into(),
            long_description: "Generate compliance reports, list CMMC 2.0 / NIST SP 800-171 controls \
                and their implementation status, and verify file integrity using FIPS 180-4 SHA-256 \
                hashing. Supports CUI data classification markings and structured audit event logging \
                for SIEM integration.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "report".into(), short: Some("r".into()),
                    description: "Show the full compliance posture report".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(true)),
                    valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "controls".into(), short: Some("c".into()),
                    description: "List all CMMC 2.0 / NIST SP 800-171 controls and their status".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)),
                    valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "integrity".into(), short: Some("i".into()),
                    description: "Verify file integrity using SHA-256 (FIPS 180-4)".into(),
                    param_type: ParamType::Boolean, required: false,
                    default_value: Some(serde_json::json!(false)),
                    valid_values: None,
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "status-filter".into(), short: Some("s".into()),
                    description: "Filter controls by status".into(),
                    param_type: ParamType::Enum(vec!["implemented".into(), "partial".into(), "na".into(), "policy".into()]),
                    required: false,
                    default_value: None,
                    valid_values: Some(vec!["implemented".into(), "partial".into(), "na".into(), "policy".into()]),
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "domain-filter".into(), short: Some("d".into()),
                    description: "Filter controls by CMMC domain prefix".into(),
                    param_type: ParamType::Enum(vec!["AU".into(), "CM".into(), "IA".into(), "MP".into(), "SC".into(), "SI".into(), "AC".into()]),
                    required: false,
                    default_value: None,
                    valid_values: Some(vec!["AU".into(), "CM".into(), "IA".into(), "MP".into(), "SC".into(), "SI".into(), "AC".into()]),
                    semantic_role: None,
                },
                ParameterSpec {
                    name: "FILE".into(), short: None,
                    description: "Files to hash for integrity verification (used with --integrity)".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)),
                    required: false,
                    default_value: None,
                    valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
            ],
            output_type: "ComplianceReport | CmmcControl[] | FileIntegrityManifest".into(),
            output_schema: serde_json::json!({
                "oneOf": [
                    {"$ref": "#/definitions/ComplianceReport"},
                    {"type": "array", "items": {"$ref": "#/definitions/CmmcControl"}},
                    {"$ref": "#/definitions/FileIntegrityManifest"}
                ]
            }),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Full compliance report".into(),
                    command: "atp compliance".into(),
                    expected_output_summary: "Complete NIST/CMMC compliance posture report".into(),
                },
                UsageExample {
                    description: "List implemented CMMC controls".into(),
                    command: "atp compliance --controls -s implemented".into(),
                    expected_output_summary: "Controls with status 'Implemented'".into(),
                },
                UsageExample {
                    description: "SHA-256 integrity hash".into(),
                    command: "atp compliance -i Cargo.toml".into(),
                    expected_output_summary: "FIPS 180-4 SHA-256 hash of the file".into(),
                },
            ],
            semantic_tags: vec!["compliance".into(), "fips".into(), "cmmc".into(), "nist".into(), "audit".into(), "integrity".into(), "sha256".into(), "cui".into()],
            related_commands: vec!["ontology".into()],
        },
        // ── Backwards-Compatible Binaries ───────────────────────────────
        CommandSpec {
            name: "atp-grep".into(),
            aliases: vec![],
            description: "POSIX grep-compatible binary with typed structured output.".into(),
            long_description: "Standalone binary accepting traditional grep command-line flags \
                (-i, -v, -w, -n, -c, -l, -r, -e, -F, -o, -m, -A/-B/-C, --include/--exclude) \
                while producing ATP's strongly typed JSON/YAML/CSV output. Supports stdin piping. \
                Auto-detects format: JSON when piped, human-readable when in a terminal.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "PATTERN".into(), short: None,
                    description: "Search pattern (regex by default, literal with -F)".into(),
                    param_type: ParamType::Pattern, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: Some("search_pattern".into()),
                },
                ParameterSpec {
                    name: "FILE".into(), short: None,
                    description: "Files or directories to search (stdin if omitted)".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)), required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
            ],
            output_type: "SearchResults".into(),
            output_schema: serde_json::json!({"$ref": "#/definitions/SearchResults"}),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Case-insensitive recursive search".into(),
                    command: "atp-grep -i -r 'TODO' src/".into(),
                    expected_output_summary: "Typed SearchResults with all matches".into(),
                },
                UsageExample {
                    description: "Count matches per file".into(),
                    command: "atp-grep -c 'error' *.log".into(),
                    expected_output_summary: "Per-file match counts".into(),
                },
                UsageExample {
                    description: "Stdin piping with JSON output".into(),
                    command: "cat log.txt | atp-grep 'ERROR'".into(),
                    expected_output_summary: "JSON SearchResults from stdin".into(),
                },
            ],
            semantic_tags: vec!["grep".into(), "search".into(), "compat".into(), "posix".into()],
            related_commands: vec!["search".into()],
        },
        CommandSpec {
            name: "atp-sed".into(),
            aliases: vec![],
            description: "POSIX sed-compatible binary with typed structured output.".into(),
            long_description: "Standalone binary accepting traditional sed command-line flags \
                (-e, -i, -n) and commands (s/pat/repl/flags, /pat/d, y/from/to/) while producing \
                ATP's strongly typed JSON/YAML/CSV output. Dry-run by default (use -i for in-place). \
                Supports stdin piping. Auto-detects format.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "COMMAND".into(), short: None,
                    description: "Sed command (e.g. 's/old/new/g', '/pattern/d')".into(),
                    param_type: ParamType::String, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: Some("transform_expression".into()),
                },
                ParameterSpec {
                    name: "FILE".into(), short: None,
                    description: "Files to transform (stdin if omitted)".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)), required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
            ],
            output_type: "TransformResults".into(),
            output_schema: serde_json::json!({"$ref": "#/definitions/TransformResults"}),
            deterministic: true,
            modifies_files: true,
            supports_dry_run: true,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Global substitution (dry-run)".into(),
                    command: "atp-sed 's/old/new/g' file.txt".into(),
                    expected_output_summary: "Typed TransformResults showing what would change".into(),
                },
                UsageExample {
                    description: "In-place with backup".into(),
                    command: "atp-sed -i.bak 's/debug/release/g' *.rs".into(),
                    expected_output_summary: "Files modified in-place with .bak backups".into(),
                },
            ],
            semantic_tags: vec!["sed".into(), "transform".into(), "compat".into(), "posix".into()],
            related_commands: vec!["transform".into()],
        },
        CommandSpec {
            name: "atp-awk".into(),
            aliases: vec![],
            description: "POSIX awk-compatible binary with typed structured output.".into(),
            long_description: "Standalone binary accepting traditional awk command-line flags \
                (-F, -v var=val) and program strings ('{print $1, $3}', '/pattern/ {action}', \
                'BEGIN {FS=\",\"} {print $1}') while producing ATP's strongly typed JSON/YAML/CSV \
                output. Supports stdin piping. Auto-detects format.".into(),
            parameters: vec![
                ParameterSpec {
                    name: "PROGRAM".into(), short: None,
                    description: "Awk program string".into(),
                    param_type: ParamType::String, required: true,
                    default_value: None, valid_values: None,
                    semantic_role: Some("awk_program".into()),
                },
                ParameterSpec {
                    name: "FILE".into(), short: None,
                    description: "Files to process (stdin if omitted)".into(),
                    param_type: ParamType::Array(Box::new(ParamType::Path)), required: false,
                    default_value: None, valid_values: None,
                    semantic_role: Some("input_paths".into()),
                },
            ],
            output_type: "AnalysisResults".into(),
            output_schema: serde_json::json!({"$ref": "#/definitions/AnalysisResults"}),
            deterministic: true,
            modifies_files: false,
            supports_dry_run: false,
            supports_explain: false,
            examples: vec![
                UsageExample {
                    description: "Select CSV fields".into(),
                    command: "atp-awk -F ',' '{print $1, $3}' data.csv".into(),
                    expected_output_summary: "Typed AnalysisResults with selected fields".into(),
                },
                UsageExample {
                    description: "Filter by pattern".into(),
                    command: "atp-awk '/error/' log.txt".into(),
                    expected_output_summary: "Records matching the pattern".into(),
                },
                UsageExample {
                    description: "Sum a column".into(),
                    command: "atp-awk '{sum += $2} END {print sum}' data.txt".into(),
                    expected_output_summary: "Aggregation result".into(),
                },
            ],
            semantic_tags: vec!["awk".into(), "analyze".into(), "compat".into(), "posix".into()],
            related_commands: vec!["analyze".into()],
        },
    ];
    specs.extend(remaining_command_specs());
    specs
}

/// Build a command spec from the fields that always differ.
///
/// The specs above carry full parameter schemas and worked examples. The ones
/// built through this do not, and that is deliberate staging: an agent's first
/// questions are whether a command exists, whether it rewrites files, and
/// whether it can be asked what it would do first. Answering those for every
/// command beats answering everything for two thirds of them.
#[allow(clippy::too_many_arguments)]
fn spec(
    name: &str,
    description: &str,
    output_type: &str,
    deterministic: bool,
    modifies_files: bool,
    supports_dry_run: bool,
    supports_explain: bool,
    related: &[&str],
) -> CommandSpec {
    CommandSpec {
        name: name.into(),
        aliases: vec![],
        description: description.into(),
        long_description: description.into(),
        parameters: vec![],
        output_type: output_type.into(),
        output_schema: serde_json::json!({}),
        deterministic,
        modifies_files,
        supports_dry_run,
        supports_explain,
        examples: vec![],
        semantic_tags: vec![],
        related_commands: related.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// The commands that had no spec at all.
///
/// The ontology described eleven of atp's seventeen subcommands while being
/// the document an agent reads to decide what atp can do. `config` changes
/// defaults that alter every later invocation, and `mcp` re-exposes the whole
/// tool surface over JSON-RPC; neither was discoverable here.
fn remaining_command_specs() -> Vec<CommandSpec> {
    vec![
        spec(
            "repl",
            "Interactive AQL shell with history.",
            "Interactive",
            false,
            false,
            false,
            false,
            &["query"],
        ),
        spec(
            "watch",
            "Re-run an AQL query whenever the watched files change. Runs until stopped.",
            "Stream",
            false,
            false,
            false,
            false,
            &["query", "search"],
        ),
        spec(
            "completions",
            "Print shell completion scripts to stdout.",
            "Text",
            true,
            false,
            false,
            false,
            &[],
        ),
        spec(
            "manpage",
            "Print a man page to stdout, or write all man pages to a directory.",
            "Text",
            true,
            true,
            false,
            false,
            &[],
        ),
        spec(
            "config",
            "View and manage ATP configuration. Changes here alter the defaults every later command runs under.",
            "Config",
            true,
            true,
            false,
            false,
            &[],
        ),
        spec(
            "mcp",
            "Serve the ATP tool surface over JSON-RPC for agents. Runs until stopped, and re-exposes search, transform, query and analyze to whatever connects.",
            "Stream",
            false,
            true,
            false,
            false,
            &["search", "transform", "query", "analyze"],
        ),
        spec(
            "ai",
            "Natural language to AQL, explanation and suggestion via an LLM. Not deterministic, and sends the prompt to whichever model is configured.",
            "Text",
            false,
            false,
            false,
            false,
            &["query", "explain"],
        ),
        spec(
            "remote",
            "Run ATP commands on remote hosts over SSH. Declared as modifying because what it runs remotely may be transform.",
            "Json",
            false,
            true,
            false,
            false,
            &["transform", "pipeline"],
        ),
        spec(
            "symbols",
            "Extract and search symbols in source code.",
            "Json",
            true,
            false,
            false,
            false,
            &["search"],
        ),
        spec(
            "plugin",
            "Install, remove, scaffold and validate ATP plugins. Installing puts new code where ATP will load it.",
            "Json",
            true,
            true,
            false,
            false,
            &[],
        ),
        spec(
            "index",
            "Build, search and manage the file index. Building writes the index to disk.",
            "Json",
            true,
            true,
            false,
            false,
            &["search"],
        ),
        spec(
            "debug",
            "Step through an AQL pipeline, inspecting each stage.",
            "Interactive",
            false,
            false,
            false,
            false,
            &["pipeline", "explain"],
        ),
        spec(
            "notebook",
            "Execute a literate AQL notebook (Markdown plus AQL). Declared as modifying because a notebook may contain transform stages.",
            "Json",
            false,
            true,
            false,
            false,
            &["pipeline"],
        ),
        spec(
            "distributed",
            "Scatter/gather pipeline execution across workers. Declared as modifying because the pipeline it runs may contain transform stages.",
            "Json",
            false,
            true,
            false,
            false,
            &["pipeline"],
        ),
    ]
}

fn build_type_specs() -> Vec<TypeSpec> {
    vec![
        TypeSpec {
            name: "SearchMatch".into(),
            description: "A single search match with full context and location information".into(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["file", "line_number", "column_start", "column_end", "line_content", "matched_text"],
                "properties": {
                    "file": {"type": "string", "description": "File path containing the match"},
                    "line_number": {"type": "integer", "description": "1-based line number"},
                    "column_start": {"type": "integer", "description": "1-based column start of match"},
                    "column_end": {"type": "integer", "description": "1-based column end (exclusive)"},
                    "line_content": {"type": "string", "description": "Complete line text"},
                    "matched_text": {"type": "string", "description": "Exact matched text"},
                    "context_before": {"type": "array", "items": {"$ref": "#/types/ContextLine"}},
                    "context_after": {"type": "array", "items": {"$ref": "#/types/ContextLine"}},
                    "byte_offset": {"type": "integer", "description": "Byte offset from file start"}
                }
            }),
        },
        TypeSpec {
            name: "ContextLine".into(),
            description: "A single line of context surrounding a match".into(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["line_number", "content"],
                "properties": {
                    "line_number": {"type": "integer"},
                    "content": {"type": "string"}
                }
            }),
        },
        TypeSpec {
            name: "ChangeRecord".into(),
            description: "A single text change within a file".into(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["line_number", "original", "replacement", "change_type"],
                "properties": {
                    "line_number": {"type": "integer"},
                    "original": {"type": "string"},
                    "replacement": {"type": "string"},
                    "change_type": {"type": "string", "enum": ["Substitution", "Deletion", "Insertion", "Transliteration"]}
                }
            }),
        },
        TypeSpec {
            name: "AtpEnvelope".into(),
            description: "Universal output envelope wrapping every ATP result".into(),
            schema: serde_json::json!({
                "type": "object",
                "required": ["version", "command", "timestamp", "deterministic", "schema_ref", "data", "metadata"],
                "properties": {
                    "version": {"type": "string"},
                    "command": {"type": "string"},
                    "timestamp": {"type": "string", "format": "date-time"},
                    "deterministic": {"type": "boolean"},
                    "schema_ref": {"type": "string", "format": "uri"},
                    "data": {"description": "Command-specific result data"},
                    "metadata": {"$ref": "#/types/ExecutionMetadata"}
                }
            }),
        },
    ]
}

fn build_error_codes() -> Vec<ErrorCodeSpec> {
    vec![
        ErrorCodeSpec {
            code: "INVALID_PATTERN".into(),
            description: "The provided regex or glob pattern is syntactically invalid".into(),
            recoverable: true,
            suggestion: "Check regex syntax. Use --literal for literal string matching.".into(),
        },
        ErrorCodeSpec {
            code: "INVALID_REPLACEMENT".into(),
            description: "The replacement string contains invalid capture group references".into(),
            recoverable: true,
            suggestion: "Ensure capture groups ($1, $2, etc.) match groups in the pattern.".into(),
        },
        ErrorCodeSpec {
            code: "FILE_NOT_FOUND".into(),
            description: "One or more specified files or directories do not exist".into(),
            recoverable: true,
            suggestion: "Use 'atp scope' to verify which files exist in the target path.".into(),
        },
        ErrorCodeSpec {
            code: "PERMISSION_DENIED".into(),
            description: "Insufficient permissions to read or write a file".into(),
            recoverable: false,
            suggestion: "Check file permissions. Use 'atp scope' to identify accessible files."
                .into(),
        },
        ErrorCodeSpec {
            code: "INVALID_SCOPE".into(),
            description: "The scope configuration is invalid or matches no files".into(),
            recoverable: true,
            suggestion: "Use 'atp scope' to preview which files match your scope settings.".into(),
        },
        ErrorCodeSpec {
            code: "PIPELINE_ERROR".into(),
            description: "A pipeline stage failed during execution".into(),
            recoverable: true,
            suggestion: "Use 'atp pipeline --explain' to validate the pipeline before execution."
                .into(),
        },
        ErrorCodeSpec {
            code: "IO_ERROR".into(),
            description: "An I/O error occurred while reading or writing files".into(),
            recoverable: false,
            suggestion: "Check disk space, file locks, and network connectivity for remote paths."
                .into(),
        },
        ErrorCodeSpec {
            code: "ENCODING_ERROR".into(),
            description: "A file contains invalid UTF-8 or unexpected encoding".into(),
            recoverable: true,
            suggestion: "Use --include-binary to process binary files, or specify encoding.".into(),
        },
    ]
}

fn build_global_examples() -> Vec<UsageExample> {
    vec![
        UsageExample {
            description: "Agent discovery: get all capabilities".into(),
            command: "atp ontology --format json".into(),
            expected_output_summary: "Complete JSON ontology for agent bootstrapping".into(),
        },
        UsageExample {
            description: "Find all TODO comments in a Rust project".into(),
            command: "atp search 'TODO|FIXME|HACK' --include '*.rs' src/ --context 2".into(),
            expected_output_summary: "All TODO/FIXME/HACK comments with 2 lines of context".into(),
        },
        UsageExample {
            description: "Rename a type across all source files".into(),
            command: "atp transform -p 'OldType' -r 'NewType' -g --in-place src/".into(),
            expected_output_summary: "All occurrences renamed with change details".into(),
        },
        UsageExample {
            description: "Analyze log file for error frequencies".into(),
            command: "atp analyze -F '\\|' -p 'ERROR' --aggregate 'freq:3' app.log".into(),
            expected_output_summary: "Frequency distribution of error categories".into(),
        },
        UsageExample {
            description: "Pipeline: find large functions in Rust code".into(),
            command: "atp pipeline -e 'search:^\\s*fn | head:20' --include '*.rs' src/".into(),
            expected_output_summary: "First 20 function definitions found".into(),
        },
    ]
}
