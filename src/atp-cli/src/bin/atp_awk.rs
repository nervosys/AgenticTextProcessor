//! atp-awk — POSIX awk-compatible interface with typed structured output.
//!
//! Drop-in replacement for awk that produces ATP's strongly typed
//! JSON/YAML/CSV output while accepting familiar flag syntax:
//!
//! ```bash
//! atp-awk '{print $1, $3}' data.txt                 # Select fields
//! atp-awk -F ',' '{print $1, $2}' data.csv           # CSV fields
//! atp-awk '/error/ {print $0}' log.txt               # Pattern filter
//! atp-awk '{sum += $2} END {print sum}' numbers.txt   # Aggregation
//! atp-awk 'BEGIN {FS=","} {print $1}' data.csv        # BEGIN block
//! echo "a b c" | atp-awk '{print $2}'                 # Stdin support
//! ```
//!
//! Output is typed JSON when piped, human-readable when in a terminal.
//! Override with `--format json|yaml|csv|human`.

use anyhow::Result;
use atp_core::output::{ExecutionMetadata, FileScope};
use atp_core::{
    parse_awk_args, AtpEnvelope, AwkEngine, Format, OutputFormatter, Provenance, Walker,
};
use std::io::{self, BufRead, IsTerminal};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let (format_override, filtered_args) = extract_format_flag(&args);

    let result = run(&filtered_args, format_override);

    if let Err(e) = result {
        eprintln!("atp-awk: {e:#}");
        std::process::exit(2);
    }
}

fn run(args: &[String], format_override: Option<String>) -> Result<()> {
    let start = Instant::now();

    let compat = parse_awk_args(args)?;
    let format = resolve_format(format_override);

    if compat.stdin_mode {
        return run_stdin(&compat, format);
    }

    // File mode — use Walker + AwkEngine
    let scope = FileScope {
        roots: compat.files.clone(),
        include_globs: vec![],
        exclude_globs: vec![],
        max_depth: None,
        respect_gitignore: true,
        follow_symlinks: false,
    };
    let walker = Walker::new(scope)?;
    let file_paths = walker.collect_files()?;

    let engine = AwkEngine::new(compat.config.clone())?;
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();
    let results = engine.process_files(&path_refs)?;

    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            for record in &results.output_records {
                println!("{}", record.fields.join("\t"));
            }
            if !results.aggregations.is_empty() {
                println!("\n--- Aggregations ---");
                for (name, value) in &results.aggregations {
                    println!("{name}: {}", serde_json::to_string(value)?);
                }
            }
        }
        _ => {
            let provenance = Provenance::new("awk-compat", vec![compat.program.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned: file_paths.len(),
                files_matched: file_paths.len(),
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("analyze", &results, metadata);
            let output = OutputFormatter::format(&envelope, format)?;
            println!("{output}");
        }
    }

    Ok(())
}

/// Handle stdin mode — read from stdin, apply awk processing.
fn run_stdin(compat: &atp_core::AwkCompat, format: Format) -> Result<()> {
    let stdin = io::stdin();

    let mut input = String::new();
    for line in stdin.lock().lines() {
        input.push_str(&line?);
        input.push('\n');
    }

    // Write to temp file for engine
    let tmp = std::env::temp_dir().join("atp-awk-stdin.tmp");
    std::fs::write(&tmp, &input)?;

    let engine = AwkEngine::new(compat.config.clone())?;
    let results = engine.process_files(&[tmp.as_path()])?;

    let _ = std::fs::remove_file(&tmp);

    match format {
        Format::Human => {
            for record in &results.output_records {
                println!("{}", record.fields.join("\t"));
            }
            if !results.aggregations.is_empty() {
                println!("\n--- Aggregations ---");
                for (name, value) in &results.aggregations {
                    println!("{name}: {}", serde_json::to_string(value)?);
                }
            }
        }
        _ => {
            let provenance = Provenance::new("awk-compat", vec![compat.program.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned: 1,
                files_matched: 1,
                duration_ms: 0,
                provenance,
            };
            let envelope = AtpEnvelope::new("analyze", &results, metadata);
            let output = OutputFormatter::format(&envelope, format)?;
            println!("{output}");
        }
    }

    Ok(())
}

fn extract_format_flag(args: &[String]) -> (Option<String>, Vec<String>) {
    let mut format = None;
    let mut filtered = Vec::new();
    let mut skip_next = false;

    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--format" {
            if i + 1 < args.len() {
                format = Some(args[i + 1].clone());
                skip_next = true;
            }
        } else if arg.starts_with("--format=") {
            format = Some(arg.trim_start_matches("--format=").to_string());
        } else {
            filtered.push(arg.clone());
        }
    }

    (format, filtered)
}

fn resolve_format(format_override: Option<String>) -> Format {
    if let Some(ref fmt) = format_override {
        fmt.parse().unwrap_or(Format::Json)
    } else if std::io::stdout().is_terminal() {
        Format::Human
    } else {
        Format::Json
    }
}
