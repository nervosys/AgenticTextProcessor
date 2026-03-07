//! Query command — execute AQL (ATP Query Language) pipelines.
//!
//! The unified, human-and-agent-friendly syntax for text processing.
//! Replaces the fragmented regex-based syntaxes of grep/sed/awk with
//! a single composable language.

use anyhow::Result;
use clap::Args;
use std::io::{self, BufRead, IsTerminal};
use std::path::PathBuf;
use std::time::Instant;

use atp_core::engine::aql::{self, AqlEngine};
use atp_core::engine::pipeline::{PipelineData, PipelineLine};
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct QueryArgs {
    /// AQL query expression
    ///
    /// Examples:
    ///   find "error" ignore_case
    ///   replace "old" with "new" all
    ///   find "TODO" | sort | unique | count
    ///   set separator "," | select fields 1, 3 | filter field 2 > 100
    pub query: String,

    /// Files or directories to process.
    ///
    /// When omitted and stdin is piped, reads from stdin (enables shell pipes):
    ///   cat file.txt | atp query 'find "error"'
    ///   git log | atp query 'find "fix" | count'
    ///   atp query 'find "TODO"' src/ | atp query 'sort | unique'
    ///
    /// Use "-" to explicitly read from stdin even when paths are given.
    pub paths: Vec<PathBuf>,

    /// Explain what the query will do without executing it
    #[arg(long)]
    pub explain: bool,

    /// Validate the query syntax without executing
    #[arg(long)]
    pub validate: bool,

    /// Glob pattern for files to include
    #[arg(long)]
    pub include: Option<String>,

    /// Glob pattern for files to exclude
    #[arg(long)]
    pub exclude: Option<String>,

    /// Maximum directory depth
    #[arg(long, short = 'd')]
    pub max_depth: Option<usize>,
}

pub fn execute(args: QueryArgs, format: Format, raw: bool) -> Result<()> {
    let start = Instant::now();

    // Parse the AQL query
    let pipeline = aql::parse(&args.query)?;

    // Validate-only mode
    if args.validate {
        let explanation = pipeline.explain();
        match format {
            Format::Human => {
                println!("Query is valid. {} stage(s):", pipeline.stages.len());
                for step in &explanation {
                    println!("  {step}");
                }
            }
            _ => {
                let result = serde_json::json!({
                    "valid": true,
                    "stages": pipeline.stages.len(),
                    "explanation": explanation,
                });
                let provenance = Provenance::new("query.validate", vec![args.query.clone()]);
                let metadata = ExecutionMetadata {
                    files_scanned: 0,
                    files_matched: 0,
                    duration_ms: 0,
                    provenance,
                };
                let envelope = AtpEnvelope::new("query.validate", &result, metadata);
                let output = OutputFormatter::format(&envelope, format)?;
                println!("{output}");
            }
        }
        return Ok(());
    }

    // Explain-only mode
    if args.explain {
        let explanation = pipeline.explain();
        match format {
            Format::Human => {
                println!("AQL Pipeline Explanation:");
                for step in &explanation {
                    println!("  {step}");
                }
            }
            _ => {
                let explain_result = ExplainResult {
                    command: args.query.clone(),
                    description: "AQL pipeline operation".to_string(),
                    will_modify_files: false,
                    estimated_files_affected: 0,
                    steps: explanation
                        .iter()
                        .enumerate()
                        .map(|(i, desc)| ExplainStep {
                            order: i + 1,
                            action: pipeline.stages[i].stage_name().to_string(),
                            description: desc.clone(),
                        })
                        .collect(),
                    warnings: Vec::new(),
                };
                let provenance = Provenance::new("query.explain", vec![args.query.clone()]);
                let metadata = ExecutionMetadata {
                    files_scanned: 0,
                    files_matched: 0,
                    duration_ms: 0,
                    provenance,
                };
                let envelope = AtpEnvelope::new("query.explain", &explain_result, metadata);
                let output = OutputFormatter::format(&envelope, format)?;
                println!("{output}");
            }
        }
        return Ok(());
    }

    // Extract InFiles modifier from first stage for scope filtering
    let aql_include = pipeline.stages.first().and_then(|stage| match stage {
        aql::AqlStage::Find { modifiers, .. } | aql::AqlStage::Replace { modifiers, .. } => {
            modifiers.iter().find_map(|m| {
                if let aql::AqlModifier::InFiles(glob) = m {
                    Some(glob.clone())
                } else {
                    None
                }
            })
        }
        _ => None,
    });

    // Determine input source: stdin vs files
    let stdin_is_piped = !io::stdin().is_terminal();
    let use_stdin = stdin_is_piped && (args.paths.is_empty() || args.paths == [PathBuf::from("-")]);

    let (results, files_scanned) = if use_stdin {
        // ── Read from stdin ──
        let stdin = io::stdin();
        let data = PipelineData {
            lines: stdin
                .lock()
                .lines()
                .enumerate()
                .filter_map(|(idx, line)| {
                    line.ok().map(|content| PipelineLine {
                        source_file: String::new(),
                        source_line: idx + 1,
                        content,
                        fields: Vec::new(),
                    })
                })
                .collect(),
            source_files: Vec::new(),
        };
        let mut engine = AqlEngine::new();
        let results = engine.execute_on_data(&pipeline, data)?;
        (results, 0_usize)
    } else {
        // ── Read from files ──
        let roots = if args.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            args.paths.clone()
        };

        let mut include_globs: Vec<String> = args.include.iter().cloned().collect();
        if let Some(aql_glob) = aql_include {
            include_globs.push(aql_glob);
        }

        let scope = FileScope {
            roots,
            include_globs,
            exclude_globs: args.exclude.iter().cloned().collect(),
            max_depth: args.max_depth,
            respect_gitignore: true,
            follow_symlinks: false,
        };
        let walker = Walker::new(scope)?;
        let file_paths = walker.collect_files()?;
        let scanned = file_paths.len();
        let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();

        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &path_refs)?;
        (results, scanned)
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            // Print stage summaries to stderr
            for stage in &results.stages {
                eprintln!(
                    "  Stage {}: {} | {} → {} records ({}ms)",
                    stage.stage_index + 1,
                    stage.stage_type,
                    stage.records_in,
                    stage.records_out,
                    stage.duration_ms
                );
            }
            // Print final output
            if let serde_json::Value::Array(arr) = &results.final_output {
                for item in arr {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        let line = item.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                        if file.is_empty() || line == 0 {
                            println!("{content}");
                        } else {
                            println!("{file}:{line}: {content}");
                        }
                    }
                }
            }
            eprintln!("  Total: {}ms", results.total_duration_ms);
        }
        _ => {
            let provenance = Provenance::new("query", vec![args.query.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned,
                files_matched: files_scanned,
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("query", &results, metadata);
            if raw {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                let output = OutputFormatter::format(&envelope, format)?;
                println!("{output}");
            }
        }
    }

    Ok(())
}
