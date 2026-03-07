//! Integration tests for ATP core engines.
//!
//! These tests exercise complete end-to-end workflows across multiple
//! engine modules, verifying that the pieces work together correctly.

use std::io::Cursor;
use std::path::Path;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// End-to-end grep → pipeline integration
// ---------------------------------------------------------------------------

#[test]
fn test_grep_then_aql_pipeline() {
    // Write a temp file, grep it, then run AQL on the same content
    let mut f = NamedTempFile::new().unwrap();
    use std::io::Write;
    write!(
        f,
        "TODO: first task\nDone: completed\nTODO: second task\nTODO: third task\n"
    )
    .unwrap();

    // Grep for TODO
    let config = atp_core::engine::grep::GrepConfig {
        pattern: "TODO".to_string(),
        ..Default::default()
    };
    let engine = atp_core::engine::grep::GrepEngine::new(config).unwrap();
    let matches = engine.search_file(f.path()).unwrap();
    assert_eq!(matches.len(), 3);

    // Now verify AQL produces equivalent results
    let pipeline = atp_core::engine::aql::parse(r#"find "TODO""#).unwrap();
    let mut aql = atp_core::engine::aql::AqlEngine::new();
    let paths: Vec<&Path> = vec![f.path()];
    let results = aql.execute(&pipeline, &paths).unwrap();
    let final_output = results.final_output.as_array().unwrap();
    assert_eq!(final_output.len(), 3);
}

#[test]
fn test_sed_then_grep_roundtrip() {
    // Transform text with sed, then verify with grep
    let input = "hello world\nfoo bar\nhello again\n";
    let cursor = Cursor::new(input.as_bytes());
    let mut output = Vec::new();

    let config = atp_core::engine::sed::SedConfig {
        commands: vec![atp_core::engine::sed::TransformCommand::Substitute {
            pattern: "hello".to_string(),
            replacement: "REPLACED".to_string(),
            global: true,
            case_insensitive: false,
        }],
        dry_run: true,
        ..Default::default()
    };
    let engine = atp_core::engine::sed::SedEngine::new(config);
    engine
        .transform_reader(cursor, &mut output, "test")
        .unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert!(output_str.contains("REPLACED world"));
    assert!(output_str.contains("REPLACED again"));
    assert!(!output_str.contains("hello"));

    // Grep the output to verify
    let grep_config = atp_core::engine::grep::GrepConfig {
        pattern: "REPLACED".to_string(),
        ..Default::default()
    };
    let grep = atp_core::engine::grep::GrepEngine::new(grep_config).unwrap();
    let cursor2 = Cursor::new(output_str.as_bytes());
    let matches = grep.search_reader(cursor2, "output").unwrap();
    assert_eq!(matches.len(), 2);
}

#[test]
fn test_awk_with_csv_then_aql_filter() {
    let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCharlie,35,NYC\n";

    // awk: process CSV with comma separator
    let config = atp_core::engine::awk::AwkConfig {
        field_separator: ",".to_string(),
        has_header: true,
        rules: vec![atp_core::engine::awk::Rule {
            pattern: None,
            condition: None,
            select_fields: vec![1, 3], // name and city
            computed_fields: Vec::new(),
        }],
        ..Default::default()
    };
    let engine = atp_core::engine::awk::AwkEngine::new(config).unwrap();
    let cursor = Cursor::new(csv.as_bytes());
    let records = engine.process_reader(cursor, "test.csv", 0).unwrap();
    // awk with has_header processes all lines including header as records
    assert!(
        records.len() >= 3,
        "expected at least 3 records, got {}",
        records.len()
    );

    // AQL: filter for NYC
    let pipeline = atp_core::engine::aql::parse(r#"find "NYC""#).unwrap();
    let data = atp_core::engine::pipeline::PipelineData {
        lines: csv
            .lines()
            .enumerate()
            .map(|(i, line)| atp_core::engine::pipeline::PipelineLine {
                source_file: "test.csv".to_string(),
                source_line: i + 1,
                content: line.to_string(),
                fields: Vec::new(),
            })
            .collect(),
        source_files: Vec::new(),
    };
    let mut aql = atp_core::engine::aql::AqlEngine::new();
    let results = aql.execute_on_data(&pipeline, data).unwrap();
    let output = results.final_output.as_array().unwrap();
    assert_eq!(output.len(), 2); // Alice+NYC and Charlie+NYC
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_grep_empty_content() {
    let config = atp_core::engine::grep::GrepConfig {
        pattern: "anything".to_string(),
        ..Default::default()
    };
    let engine = atp_core::engine::grep::GrepEngine::new(config).unwrap();
    let cursor = Cursor::new(b"" as &[u8]);
    let matches = engine.search_reader(cursor, "empty").unwrap();
    assert!(matches.is_empty());
}

#[test]
fn test_grep_single_char_pattern() {
    let config = atp_core::engine::grep::GrepConfig {
        pattern: "x".to_string(),
        ..Default::default()
    };
    let engine = atp_core::engine::grep::GrepEngine::new(config).unwrap();
    let cursor = Cursor::new(b"axb\ncd\nxyz" as &[u8]);
    let matches = engine.search_reader(cursor, "test").unwrap();
    assert_eq!(matches.len(), 2);
}

#[test]
fn test_sed_no_match() {
    let config = atp_core::engine::sed::SedConfig {
        commands: vec![atp_core::engine::sed::TransformCommand::Substitute {
            pattern: "NONEXISTENT".to_string(),
            replacement: "x".to_string(),
            global: true,
            case_insensitive: false,
        }],
        dry_run: true,
        ..Default::default()
    };
    let engine = atp_core::engine::sed::SedEngine::new(config);
    let cursor = Cursor::new(b"hello world\n" as &[u8]);
    let mut output = Vec::new();
    let changes = engine
        .transform_reader(cursor, &mut output, "test")
        .unwrap();
    let output_str = String::from_utf8(output).unwrap();
    assert_eq!(output_str, "hello world\n");
    assert!(changes.changes.is_empty());
}

#[test]
fn test_sed_empty_input() {
    let config = atp_core::engine::sed::SedConfig {
        commands: vec![atp_core::engine::sed::TransformCommand::Substitute {
            pattern: "a".to_string(),
            replacement: "b".to_string(),
            global: true,
            case_insensitive: false,
        }],
        dry_run: true,
        ..Default::default()
    };
    let engine = atp_core::engine::sed::SedEngine::new(config);
    let cursor = Cursor::new(b"" as &[u8]);
    let mut output = Vec::new();
    let changes = engine
        .transform_reader(cursor, &mut output, "test")
        .unwrap();
    assert!(output.is_empty());
    assert!(changes.changes.is_empty());
}

#[test]
fn test_aql_parse_all_stage_types() {
    // Verify all stage types can parse
    let queries = vec![
        r#"find "x""#,
        r#"replace "a" with "b""#,
        r#"delete lines matching "x""#,
        r#"sort"#,
        r#"sort by field 1"#,
        r#"unique"#,
        r#"take 5"#,
        r#"skip 3"#,
        r#"count"#,
        r#"find "x" | sort | unique | count"#,
    ];
    for q in queries {
        assert!(
            atp_core::engine::aql::parse(q).is_ok(),
            "Failed to parse: {q}"
        );
    }
}

#[test]
fn test_aql_explain_all_stages() {
    let pipeline =
        atp_core::engine::aql::parse(r#"find "x" ignore_case | sort | unique | take 5 | count"#)
            .unwrap();
    let explanation = pipeline.explain();
    assert_eq!(explanation.len(), 5);
    assert!(explanation[0].contains("Search"), "got: {}", explanation[0]);
    assert!(explanation[1].contains("Sort"), "got: {}", explanation[1]);
    assert!(
        explanation[2].contains("Deduplicate"),
        "got: {}",
        explanation[2]
    );
}

// ---------------------------------------------------------------------------
// Semantic search integration
// ---------------------------------------------------------------------------

#[test]
fn test_semantic_index_and_query() {
    let mut index = atp_core::semantic::TfIdfIndex::new();

    // Add documents from multiple "files"
    index.add_document("rust.txt", 1, "Rust is a systems programming language");
    index.add_document("rust.txt", 2, "It focuses on safety and performance");
    index.add_document("python.txt", 1, "Python is a scripting language");
    index.add_document("python.txt", 2, "It is great for machine learning");

    index.finalize();

    let results = index.query("systems programming safety", 4);
    assert!(!results.is_empty());
    // Rust documents should rank higher
    assert_eq!(results[0].file, "rust.txt");
}

#[test]
fn test_semantic_empty_query() {
    let mut index = atp_core::semantic::TfIdfIndex::new();
    index.add_document("a.txt", 1, "some content here");
    index.finalize();

    let results = index.query("", 10);
    assert!(results.is_empty());
}

#[test]
fn test_semantic_no_documents() {
    let mut index = atp_core::semantic::TfIdfIndex::new();
    index.finalize();
    let results = index.query("anything", 10);
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// Telemetry integration
// ---------------------------------------------------------------------------

#[test]
fn test_telemetry_session_full_lifecycle() {
    use atp_core::telemetry::*;

    let mut session = TelemetrySession::new("test");

    // Simulate a processing workflow — need 3+ samples per phase for detect_bottleneck
    for _ in 0..3 {
        session.record_phase("file_io", 100, 0.1); // 1000 B/s
        session.record_phase("regex", 200, 0.5); // 400 B/s (slowest)
        session.record_phase("transform", 50, 0.05); // 1000 B/s
        session.record_phase("format", 30, 0.02); // 1500 B/s
    }

    session.record_counts(10, 1000, 42);

    let bottleneck = session.detect_bottleneck();
    assert_eq!(bottleneck, BottleneckPhase::RegexMatch); // lowest throughput

    let report = session.finalize(true, None);
    assert_eq!(report.total_files, 10);
    assert_eq!(report.total_lines, 1000);
    assert_eq!(report.total_matches, 42);
    assert!(!report.phases.is_empty());
}

#[test]
fn test_shared_telemetry_thread_safety() {
    use atp_core::telemetry::*;
    use std::thread;

    let shared = SharedTelemetry::new("test");
    let shared_clone = shared.clone();

    let handle = thread::spawn(move || {
        shared_clone.record_phase("pipeline", 100, 0.1);
    });

    shared.record_phase("file_io", 50, 0.05);
    handle.join().unwrap();

    let report = shared.finalize(true, None).unwrap();
    assert!(!report.phases.is_empty());
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

#[test]
fn test_output_format_parsing() {
    use std::str::FromStr;
    assert!(atp_core::Format::from_str("json").is_ok());
    assert!(atp_core::Format::from_str("yaml").is_ok());
    assert!(atp_core::Format::from_str("csv").is_ok());
    assert!(atp_core::Format::from_str("human").is_ok());
    assert!(atp_core::Format::from_str("json-pretty").is_ok());
    assert!(atp_core::Format::from_str("jsonl").is_ok());
    assert!(atp_core::Format::from_str("invalid").is_err());
}

#[test]
fn test_ontology_completeness() {
    let ont = atp_core::ontology::build_ontology();
    assert_eq!(ont.tool, "atp");
    assert!(!ont.capabilities.is_empty());
    assert!(!ont.commands.is_empty());
    assert!(!ont.types.is_empty());
    assert!(!ont.output_formats.is_empty());
}

// ---------------------------------------------------------------------------
// Compliance
// ---------------------------------------------------------------------------

#[test]
fn test_compliance_sha256_deterministic() {
    let hash1 = atp_core::compliance::sha256_hash(b"hello world");
    let hash2 = atp_core::compliance::sha256_hash(b"hello world");
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // hex-encoded SHA-256
}

#[test]
fn test_compliance_report_generation() {
    let report = atp_core::compliance::generate_compliance_report();
    assert!(!report.frameworks.is_empty());
    assert!(!report.controls.is_empty());
    assert!(!report.cryptographic_modules.is_empty());
}

// ---------------------------------------------------------------------------
// Multi-file processing
// ---------------------------------------------------------------------------

#[test]
fn test_grep_multi_file() {
    let mut f1 = NamedTempFile::new().unwrap();
    let mut f2 = NamedTempFile::new().unwrap();
    use std::io::Write;
    write!(f1, "line one\nTODO here\nline three\n").unwrap();
    write!(f2, "another file\nno matches\n").unwrap();

    let config = atp_core::engine::grep::GrepConfig {
        pattern: "TODO".to_string(),
        ..Default::default()
    };
    let engine = atp_core::engine::grep::GrepEngine::new(config).unwrap();
    let paths = vec![f1.path(), f2.path()];
    let results = engine.search_files(&paths).unwrap();
    assert_eq!(results.total_matches, 1);
    assert_eq!(results.files_with_matches, 1);
}

#[test]
fn test_sed_multiple_commands() {
    let input = "hello world\nfoo bar\n";
    let cursor = Cursor::new(input.as_bytes());
    let mut output = Vec::new();

    let config = atp_core::engine::sed::SedConfig {
        commands: vec![
            atp_core::engine::sed::TransformCommand::Substitute {
                pattern: "hello".to_string(),
                replacement: "hi".to_string(),
                global: true,
                case_insensitive: false,
            },
            atp_core::engine::sed::TransformCommand::Substitute {
                pattern: "foo".to_string(),
                replacement: "baz".to_string(),
                global: true,
                case_insensitive: false,
            },
        ],
        dry_run: true,
        ..Default::default()
    };
    let engine = atp_core::engine::sed::SedEngine::new(config);
    engine
        .transform_reader(cursor, &mut output, "test")
        .unwrap();

    let out = String::from_utf8(output).unwrap();
    assert!(out.contains("hi world"));
    assert!(out.contains("baz bar"));
}

// ---------------------------------------------------------------------------
// Plugin system
// ---------------------------------------------------------------------------

#[test]
fn test_plugin_registry_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = atp_core::plugin::PluginRegistry::new(dir.path().to_path_buf());
    assert!(registry.list_plugins().is_empty());

    // Write a plugin manifest TOML to the temp directory
    let manifest_toml = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
kind = "format"

[format]
name = "test"
template = "{0}\t{1}"
"#;
    std::fs::write(dir.path().join("test-plugin.toml"), manifest_toml).unwrap();

    let count = registry.discover().unwrap();
    assert_eq!(count, 1);
    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "test-plugin");
}

// ---------------------------------------------------------------------------
// v2: End-to-end / CLI-level integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_streaming_pipeline() {
    use atp_core::engine::pipeline::{PipelineDefinition, PipelineStage};
    use atp_core::StreamingConfig;
    use atp_core::StreamingPipeline;

    let mut f = NamedTempFile::new().unwrap();
    use std::io::Write;
    for i in 0..50 {
        writeln!(f, "line {i}").unwrap();
    }

    let def = PipelineDefinition {
        name: None,
        description: None,
        stages: vec![
            PipelineStage::Head { count: 10 },
            PipelineStage::Sort {
                reverse: false,
                field: None,
                numeric: false,
            },
        ],
    };
    let sp = StreamingPipeline::new(def, StreamingConfig::default());

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(sp.execute_streaming(&[f.path()])).unwrap();
    // head 10 → 10 lines, sort → 10 lines
    assert!(result.stages.len() >= 2);
}

#[test]
fn test_e2e_aho_corasick_multi_pattern() {
    use atp_core::engine::grep::GrepConfig;
    use atp_core::engine::grep::GrepEngine;
    use atp_core::output::PatternType;

    let mut f = NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(f, "alpha\nbeta\ngamma\ndelta\nepsilon").unwrap();

    let config = GrepConfig {
        pattern: "alpha".to_string(),
        pattern_type: PatternType::Literal,
        extra_patterns: vec!["gamma".to_string(), "epsilon".to_string()],
        ..Default::default()
    };
    let engine = GrepEngine::new(config).unwrap();
    let matches = engine.search_file(f.path()).unwrap();
    assert_eq!(matches.len(), 3);
}

#[test]
fn test_e2e_tracing_init() {
    use atp_core::init_tracing;
    use atp_core::TracingConfig;

    let config = TracingConfig {
        enabled: false,
        ..Default::default()
    };
    // Should be a no-op when disabled
    assert!(!init_tracing(&config).unwrap());
}

#[test]
fn test_e2e_wasm_engine_roundtrip() {
    use atp_core::wasm_runtime::{
        build_test_wasm_module, BuiltinWasmEngine, WasmEngine, WasmLimits,
    };

    let wasm = build_test_wasm_module(&["transform", "validate"]);
    let engine = BuiltinWasmEngine::new();
    let exports = engine.validate(&wasm).unwrap();
    assert_eq!(exports, vec!["transform", "validate"]);

    let result = engine
        .execute(&wasm, "transform", "input data", &WasmLimits::default())
        .unwrap();
    assert!(result.success);
    assert!(result.output.contains("transform"));
}

#[test]
fn test_e2e_project_config_merge() {
    use atp_core::config::{AtpConfig, PipelineDefaults, ScopeDefaults};

    let global = AtpConfig {
        format: Some("human".into()),
        search: atp_core::config::SearchConfig {
            case_insensitive: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let project = AtpConfig {
        format: Some("json".into()),
        pipeline: PipelineDefaults {
            channel_capacity: Some(4096),
            batch_size: Some(128),
            saved: [("lint".into(), "find \"TODO\" | count".into())]
                .into_iter()
                .collect(),
        },
        scope: ScopeDefaults {
            roots: vec!["src".into()],
            extensions: vec!["rs".to_string()],
            follow_symlinks: false,
        },
        ..Default::default()
    };
    let merged = global.merge(project);
    assert_eq!(merged.format.as_deref(), Some("json"));
    assert!(merged.search.case_insensitive);
    assert_eq!(merged.pipeline.channel_capacity, Some(4096));
    assert_eq!(merged.pipeline.saved.len(), 1);
    assert_eq!(merged.scope.extensions, vec!["rs"]);
}

#[test]
fn test_e2e_dap_wire_roundtrip() {
    use atp_core::dap::{DapEvent, DapMessage, DapRequest, DapResponse};

    // Request roundtrip
    let req = DapRequest::new(
        1,
        "initialize",
        Some(serde_json::json!({"clientID": "test"})),
    );
    let wire = DapMessage::Request(req).encode().unwrap();
    let (msg, _) = DapMessage::decode(&wire).unwrap();
    assert!(matches!(msg, DapMessage::Request(_)));

    // Response roundtrip
    let resp = DapResponse::success(2, 1, "initialize", None);
    let wire = DapMessage::Response(resp).encode().unwrap();
    let (msg, _) = DapMessage::decode(&wire).unwrap();
    assert!(matches!(msg, DapMessage::Response(_)));

    // Event roundtrip
    let evt = DapEvent::stopped(3, "breakpoint", 1);
    let wire = DapMessage::Event(evt).encode().unwrap();
    let (msg, _) = DapMessage::decode(&wire).unwrap();
    assert!(matches!(msg, DapMessage::Event(_)));
}

#[test]
fn test_e2e_aql_pipeline_explain() {
    let pipeline =
        atp_core::engine::aql::parse("find \"error\" ignore_case | sort | unique | count").unwrap();
    assert_eq!(pipeline.stages.len(), 4);
}

#[test]
fn test_e2e_code_intel_symbols() {
    use atp_core::code_intel::{CodeIntelligence, StructuralQuery};

    let ci = CodeIntelligence::new();
    let source = r#"
fn hello() {}
fn world() {}
struct Foo;
"#;
    let query = StructuralQuery {
        kind: None,
        name_pattern: None,
        language: None,
        visibility: None,
        doc_contains: None,
    };
    let symbols = ci.query_symbols(source, "test.rs", &query);
    assert!(symbols.len() >= 2);
}

#[test]
fn test_e2e_debugger_full_session() {
    use atp_core::dap::{AqlDebugger, DebugState};

    let mut debugger = AqlDebugger::new();
    debugger.add_default_breakpoint(1);
    let mut session = debugger.start("find \"x\" | sort | count", &[]).unwrap();

    let input = vec!["x line 1".into(), "y line 2".into(), "x line 3".into()];
    let state = debugger.run(&mut session, &input).unwrap();
    assert_eq!(state, DebugState::Paused);
    assert_eq!(session.current_stage, 1);

    // Continue to completion
    let output = session.snapshots.last().unwrap().output_sample.clone();
    let state = debugger.run(&mut session, &output).unwrap();
    assert_eq!(state, DebugState::Completed);
    assert_eq!(session.snapshots.len(), 3);
}
