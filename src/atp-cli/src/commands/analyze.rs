//! Analyze command — awk-like field processing and aggregation.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::time::Instant;

use atp_core::engine::awk::{Aggregation, AwkConfig, AwkEngine, Rule};
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Files or directories to analyze (default: current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Field separator (regex). Default: whitespace
    #[arg(long, default_value = r"\s+")]
    pub separator: String,

    /// Fields to select (1-based, comma-separated, e.g. '1,3,5')
    #[arg(long, short = 'f')]
    pub fields: Option<String>,

    /// Pattern to filter lines before processing
    #[arg(long, short = 'p')]
    pub pattern: Option<String>,

    /// Aggregations to compute (e.g. 'count', 'sum:2', 'avg:3', 'freq:1')
    #[arg(long, short = 'a')]
    pub aggregate: Vec<String>,

    /// Input has a header line
    #[arg(long)]
    pub header: bool,

    /// Skip first N lines
    #[arg(long, default_value = "0")]
    pub skip: usize,

    /// Maximum records to process
    #[arg(long, short = 'm')]
    pub max_records: Option<usize>,

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

pub fn execute(args: AnalyzeArgs, format: Format, raw: bool) -> Result<()> {
    let start = Instant::now();

    // Parse field selection
    let select_fields: Vec<usize> = args
        .fields
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    // Parse aggregations
    let aggregations: Vec<(String, Aggregation)> = args
        .aggregate
        .iter()
        .filter_map(|spec| parse_aggregation(spec))
        .collect();

    // Build rule
    let rule = Rule {
        pattern: args.pattern.clone(),
        select_fields,
        computed_fields: Vec::new(),
        condition: None,
    };

    let config = AwkConfig {
        field_separator: args.separator.clone(),
        output_separator: "\t".to_string(),
        rules: vec![rule],
        aggregations,
        has_header: args.header,
        skip_lines: args.skip,
        max_records: args.max_records,
    };

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

    let engine = AwkEngine::new(config)?;
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();
    let results = engine.process_files(&path_refs)?;

    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            // Print records as tab-separated lines
            for record in &results.output_records {
                println!("{}", record.fields.join("\t"));
            }
            // Print aggregations
            if !results.aggregations.is_empty() {
                println!("\n--- Aggregations ---");
                for (name, value) in &results.aggregations {
                    println!("{name}: {}", serde_json::to_string(value)?);
                }
            }
        }
        _ => {
            let provenance = Provenance::new("analyze", vec![args.separator.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned: file_paths.len(),
                files_matched: file_paths.len(),
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("analyze", &results, metadata);
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

fn parse_aggregation(spec: &str) -> Option<(String, Aggregation)> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    let name = parts[0].to_lowercase();
    let field: Option<usize> = parts.get(1).and_then(|s| s.parse().ok());

    match name.as_str() {
        "count" => Some(("count".to_string(), Aggregation::Count)),
        "sum" => field.map(|f| (format!("sum_{f}"), Aggregation::Sum(f))),
        "avg" | "average" => field.map(|f| (format!("avg_{f}"), Aggregation::Average(f))),
        "min" => field.map(|f| (format!("min_{f}"), Aggregation::Min(f))),
        "max" => field.map(|f| (format!("max_{f}"), Aggregation::Max(f))),
        "distinct" => field.map(|f| (format!("distinct_{f}"), Aggregation::Distinct(f))),
        "freq" | "frequency" => field.map(|f| (format!("freq_{f}"), Aggregation::Frequency(f))),
        _ => None,
    }
}
