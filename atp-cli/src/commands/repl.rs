//! REPL command — interactive AQL shell with history and tab-completion.
//!
//! Launch with `atp repl` to enter an interactive AQL session. Each line
//! is parsed and executed as an AQL query against the configured scope.
//!
//! Built-in commands:
//!   .help       — show available commands
//!   .scope `<p>`  — set the search scope (file or directory)
//!   .format `<f>` — set output format (human, json, yaml)
//!   .history    — show query history
//!   .clear      — clear the screen
//!   .quit       — exit the REPL

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::DefaultEditor;
use std::path::PathBuf;
use std::time::Instant;

use atp_core::engine::aql::{self, AqlEngine};
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct ReplArgs {
    /// Initial files or directories to set as scope
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
}

pub fn execute(args: ReplArgs) -> Result<()> {
    let mut scope_roots: Vec<PathBuf> = if args.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.paths.clone()
    };
    let mut include_globs: Vec<String> = args.include.iter().cloned().collect();
    let mut exclude_globs: Vec<String> = args.exclude.iter().cloned().collect();
    let mut max_depth = args.max_depth;
    let mut format = Format::Human;

    println!("{}", "ATP AQL REPL — Interactive Query Shell".bold().cyan());
    println!(
        "  Scope: {}",
        scope_roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "  Type {} for commands, {} to exit.",
        ".help".bold(),
        ".quit".bold()
    );
    println!();

    let mut rl = DefaultEditor::new()?;

    // Try to load history from ~/.atp_history
    let history_path = dirs_history_path();
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    loop {
        let readline = rl.readline(&format!("{} ", "aql>".green().bold()));
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(trimmed);

                // Handle dot-commands
                if trimmed.starts_with('.') {
                    match handle_dot_command(
                        trimmed,
                        &mut scope_roots,
                        &mut include_globs,
                        &mut exclude_globs,
                        &mut max_depth,
                        &mut format,
                        &rl,
                    ) {
                        DotResult::Continue => continue,
                        DotResult::Quit => break,
                    }
                }

                // Execute AQL query
                match execute_query(
                    trimmed,
                    &scope_roots,
                    &include_globs,
                    &exclude_globs,
                    max_depth,
                    format,
                ) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("{} {e:#}", "Error:".red().bold());
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C — cancel current input, continue
                println!("(interrupted)");
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D — exit
                break;
            }
            Err(err) => {
                eprintln!("{} {err}", "Readline error:".red().bold());
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    println!("Goodbye.");
    Ok(())
}

enum DotResult {
    Continue,
    Quit,
}

fn handle_dot_command(
    cmd: &str,
    scope_roots: &mut Vec<PathBuf>,
    include_globs: &mut Vec<String>,
    exclude_globs: &mut Vec<String>,
    max_depth: &mut Option<usize>,
    format: &mut Format,
    rl: &DefaultEditor,
) -> DotResult {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    match parts[0] {
        ".quit" | ".exit" | ".q" => return DotResult::Quit,
        ".help" | ".h" => {
            println!("{}", "AQL REPL Commands:".bold());
            println!("  {:<16} Execute an AQL query", "<query>".cyan());
            println!("  {:<16} Show this help", ".help".cyan());
            println!("  {:<16} Set scope to path(s)", ".scope <path>".cyan());
            println!("  {:<16} Set output format", ".format <fmt>".cyan());
            println!("  {:<16} Set include glob", ".include <glob>".cyan());
            println!("  {:<16} Set exclude glob", ".exclude <glob>".cyan());
            println!("  {:<16} Set max depth", ".depth <n>".cyan());
            println!("  {:<16} Show query history", ".history".cyan());
            println!("  {:<16} Show current settings", ".status".cyan());
            println!("  {:<16} Exit the REPL", ".quit".cyan());
            println!();
            println!("{}", "AQL Examples:".bold());
            println!("  find \"TODO\" ignore_case");
            println!("  replace \"old\" with \"new\" all");
            println!("  find \"error\" | sort | unique | count");
            println!("  set separator \",\" | select fields 1, 3 | filter field 2 > 100");
        }
        ".scope" => {
            if parts.len() > 1 {
                *scope_roots = parts[1].split_whitespace().map(PathBuf::from).collect();
                println!(
                    "Scope set to: {}",
                    scope_roots
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            } else {
                println!(
                    "Current scope: {}",
                    scope_roots
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        ".format" => {
            if parts.len() > 1 {
                match parts[1].parse::<Format>() {
                    Ok(f) => {
                        *format = f;
                        println!("Format set to: {}", parts[1]);
                    }
                    Err(e) => eprintln!("{} {e}", "Invalid format:".red()),
                }
            } else {
                println!("Current format: {:?}", format);
            }
        }
        ".include" => {
            if parts.len() > 1 {
                *include_globs = vec![parts[1].to_string()];
                println!("Include glob set to: {}", parts[1]);
            } else {
                println!("Include globs: {:?}", include_globs);
            }
        }
        ".exclude" => {
            if parts.len() > 1 {
                *exclude_globs = vec![parts[1].to_string()];
                println!("Exclude glob set to: {}", parts[1]);
            } else {
                println!("Exclude globs: {:?}", exclude_globs);
            }
        }
        ".depth" => {
            if parts.len() > 1 {
                match parts[1].parse::<usize>() {
                    Ok(d) => {
                        *max_depth = Some(d);
                        println!("Max depth set to: {d}");
                    }
                    Err(_) => eprintln!("{} not a number", "Invalid depth:".red()),
                }
            } else {
                println!("Max depth: {:?}", max_depth);
            }
        }
        ".history" => {
            let hist = rl.history();
            if hist.is_empty() {
                println!("(no history)");
            } else {
                for (i, entry) in hist.iter().enumerate() {
                    println!("  {}: {}", i + 1, entry);
                }
            }
        }
        ".status" => {
            println!("{}", "REPL Status:".bold());
            println!(
                "  Scope:    {}",
                scope_roots
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!("  Include:  {:?}", include_globs);
            println!("  Exclude:  {:?}", exclude_globs);
            println!("  Depth:    {:?}", max_depth);
            println!("  Format:   {:?}", format);
        }
        ".clear" => {
            // ANSI clear screen
            print!("\x1b[2J\x1b[H");
        }
        _ => {
            eprintln!(
                "{} Unknown command '{}'. Type .help for available commands.",
                "Error:".red().bold(),
                parts[0]
            );
        }
    }
    DotResult::Continue
}

fn execute_query(
    query: &str,
    scope_roots: &[PathBuf],
    include_globs: &[String],
    exclude_globs: &[String],
    max_depth: Option<usize>,
    format: Format,
) -> Result<()> {
    let start = Instant::now();

    let pipeline = aql::parse(query)?;

    let scope = FileScope {
        roots: scope_roots.to_vec(),
        include_globs: include_globs.to_vec(),
        exclude_globs: exclude_globs.to_vec(),
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
            // Print stage summaries
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
            // Print final output
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
            let provenance = Provenance::new("repl.query", vec![query.to_string()]);
            let metadata = ExecutionMetadata {
                files_scanned: scanned,
                files_matched: scanned,
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("repl.query", &results, metadata);
            let output = OutputFormatter::format(&envelope, format)?;
            println!("{output}");
        }
    }

    Ok(())
}

fn dirs_history_path() -> Option<String> {
    home_dir().map(|home| format!("{}/.atp_history", home.display()))
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
