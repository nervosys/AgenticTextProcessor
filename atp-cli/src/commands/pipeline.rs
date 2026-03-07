//! Pipeline command — composable multi-stage processing.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::time::Instant;

use atp_core::engine::pipeline::Pipeline;
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct PipelineArgs {
    /// Files or directories to process (default: current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Pipeline DSL expression (e.g. 'search:pattern | filter:x | head:10')
    #[arg(long, short = 'e')]
    pub expression: Option<String>,

    /// Path to a pipeline definition file (YAML/JSON)
    #[arg(long, short = 'f')]
    pub file: Option<PathBuf>,

    /// Explain what the pipeline will do without executing
    #[arg(long)]
    pub explain: bool,

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

pub fn execute(args: PipelineArgs, format: Format, raw: bool) -> Result<()> {
    let start = Instant::now();

    // Build pipeline from expression or file
    let pipeline = if let Some(ref expr) = args.expression {
        Pipeline::from_dsl(expr)?
    } else if let Some(ref path) = args.file {
        let content = std::fs::read_to_string(path)?;
        Pipeline::from_yaml(&content)?
    } else {
        anyhow::bail!("Provide a pipeline expression (-e) or definition file (-f)");
    };

    // Explain mode
    if args.explain {
        let explanation = pipeline.explain();
        match format {
            Format::Human => {
                println!("Pipeline Explanation:");
                for step in &explanation {
                    println!("  {step}");
                }
            }
            _ => {
                let explain_result = ExplainResult {
                    command: args.expression.unwrap_or_default(),
                    description: "Multi-stage pipeline operation".to_string(),
                    will_modify_files: false,
                    estimated_files_affected: 0,
                    steps: explanation
                        .iter()
                        .enumerate()
                        .map(|(i, desc)| ExplainStep {
                            order: i + 1,
                            action: "pipeline_stage".to_string(),
                            description: desc.clone(),
                        })
                        .collect(),
                    warnings: Vec::new(),
                };
                let provenance = Provenance::new("pipeline.explain", vec![]);
                let metadata = ExecutionMetadata {
                    files_scanned: 0,
                    files_matched: 0,
                    duration_ms: 0,
                    provenance,
                };
                let envelope = AtpEnvelope::new("pipeline.explain", &explain_result, metadata);
                let output = OutputFormatter::format(&envelope, format)?;
                println!("{output}");
            }
        }
        return Ok(());
    }

    // Collect files
    let scope = FileScope {
        roots: args.paths.clone(),
        include_globs: args.include.iter().cloned().collect(),
        exclude_globs: args.exclude.iter().cloned().collect(),
        max_depth: args.max_depth,
        respect_gitignore: true,
        follow_symlinks: false,
    };
    let walker = Walker::new(scope)?;
    let file_paths = walker.collect_files()?;
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();

    let results = pipeline.execute(&path_refs)?;
    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            // Print pipeline stage summaries
            for stage in &results.stages {
                eprintln!(
                    "Stage {}: {} | {} → {} records ({}ms)",
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
                        if !file.is_empty() {
                            println!("{file}:{line}: {content}");
                        } else {
                            println!("{content}");
                        }
                    }
                }
            }
            eprintln!("\nTotal: {}ms", results.total_duration_ms);
        }
        _ => {
            let provenance = Provenance::new("pipeline", args.expression.iter().cloned().collect());
            let metadata = ExecutionMetadata {
                files_scanned: file_paths.len(),
                files_matched: file_paths.len(),
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("pipeline", &results, metadata);
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
