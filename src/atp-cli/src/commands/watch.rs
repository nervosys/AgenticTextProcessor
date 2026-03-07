//! Watch command — monitor files for changes and re-run AQL queries.
//!
//! Uses the `notify` crate for cross-platform filesystem event watching.
//! When a file in the configured scope is modified, the AQL query is
//! automatically re-executed and results are displayed.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use atp_core::engine::aql::{self, AqlEngine};
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct WatchArgs {
    /// AQL query to execute on each file change
    pub query: String,

    /// Files or directories to watch
    pub paths: Vec<PathBuf>,

    /// Glob pattern for files to include
    #[arg(long)]
    pub include: Option<String>,

    /// Glob pattern for files to exclude
    #[arg(long)]
    pub exclude: Option<String>,

    /// Maximum directory depth
    #[arg(long, short = 'd')]
    pub max_depth: Option<usize>,

    /// Debounce interval in milliseconds (default: 500)
    #[arg(long, default_value = "500")]
    pub debounce: u64,

    /// Clear screen before each re-run
    #[arg(long)]
    pub clear: bool,
}

pub fn execute(args: WatchArgs, format: Format, raw: bool) -> Result<()> {
    let roots: Vec<PathBuf> = if args.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.paths.clone()
    };

    // Validate the query first
    let _ =
        aql::parse(&args.query).with_context(|| format!("Invalid AQL query: {}", args.query))?;

    println!(
        "{} {}",
        "Watching:".bold().cyan(),
        roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("{} {}", "Query:".bold().cyan(), args.query);
    println!("{}", "Press Ctrl-C to stop.".dimmed());
    println!();

    // Run query once immediately
    run_query(
        &args.query,
        &roots,
        args.include.as_deref(),
        args.exclude.as_deref(),
        args.max_depth,
        format,
        raw,
    )?;

    // Set up file watcher
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    for root in &roots {
        let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        watcher.watch(&canonical, RecursiveMode::Recursive)?;
    }

    let debounce_ms = Duration::from_millis(args.debounce);
    let mut last_run = Instant::now();

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                // Only react to modify/create events on files
                let is_relevant = matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_));
                if !is_relevant {
                    continue;
                }

                // Debounce: skip if we ran too recently
                let now = Instant::now();
                if now.duration_since(last_run) < debounce_ms {
                    continue;
                }
                last_run = now;

                // Report changed file
                let changed_files: Vec<String> = event
                    .paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect();

                if args.clear {
                    print!("\x1b[2J\x1b[H");
                }

                eprintln!(
                    "\n{} {} {}",
                    "──".dimmed(),
                    "File changed:".yellow().bold(),
                    changed_files.join(", ").dimmed()
                );

                match run_query(
                    &args.query,
                    &roots,
                    args.include.as_deref(),
                    args.exclude.as_deref(),
                    args.max_depth,
                    format,
                    raw,
                ) {
                    Ok(()) => {}
                    Err(e) => eprintln!("{} {e:#}", "Error:".red().bold()),
                }
            }
            Ok(Err(e)) => {
                eprintln!("{} {e}", "Watch error:".red().bold());
            }
            Err(e) => {
                eprintln!("{} {e}", "Channel error:".red().bold());
                break;
            }
        }
    }

    Ok(())
}

fn run_query(
    query: &str,
    roots: &[PathBuf],
    include: Option<&str>,
    exclude: Option<&str>,
    max_depth: Option<usize>,
    format: Format,
    raw: bool,
) -> Result<()> {
    let start = Instant::now();

    let pipeline = aql::parse(query)?;

    let scope = FileScope {
        roots: roots.to_vec(),
        include_globs: include.iter().map(|s| s.to_string()).collect(),
        exclude_globs: exclude.iter().map(|s| s.to_string()).collect(),
        max_depth,
        respect_gitignore: true,
        follow_symlinks: false,
    };
    let walker = Walker::new(scope)?;
    let file_paths = walker.collect_files()?;
    let scanned = file_paths.len();
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();

    let mut engine = AqlEngine::new();
    let results = engine.execute(&pipeline, &path_refs)?;

    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            for stage in &results.stages {
                eprintln!(
                    "  {} Stage {}: {} | {} → {} records ({}ms)",
                    "│".dimmed(),
                    stage.stage_index + 1,
                    stage.stage_type,
                    stage.records_in,
                    stage.records_out,
                    stage.duration_ms
                );
            }
            if let serde_json::Value::Array(arr) = &results.final_output {
                for item in arr {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        let line = item.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                        if file.is_empty() || line == 0 {
                            println!("{content}");
                        } else {
                            println!("{}{}", format!("{file}:{line}: ").dimmed(), content);
                        }
                    }
                }
            }
            eprintln!(
                "  {} {scanned} files scanned, {duration_ms}ms total",
                "└".dimmed()
            );
        }
        _ => {
            let provenance = Provenance::new("watch.query", vec![query.to_string()]);
            let metadata = ExecutionMetadata {
                files_scanned: scanned,
                files_matched: scanned,
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("watch.query", &results, metadata);
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
