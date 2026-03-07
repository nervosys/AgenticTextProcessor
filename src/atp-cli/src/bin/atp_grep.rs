//! atp-grep — POSIX grep-compatible interface with typed structured output.
//!
//! Drop-in replacement for grep that produces ATP's strongly typed
//! JSON/YAML/CSV output while accepting familiar flag syntax:
//!
//! ```bash
//! atp-grep -i -r "TODO" src/            # Search recursively, case-insensitive
//! atp-grep -n -w "fn main" *.rs         # Whole word, line numbers
//! atp-grep -c "error" log.txt           # Count matches
//! atp-grep -l "FIXME" **/*.py           # Files with matches
//! atp-grep -e "foo" -e "bar" src/       # Multiple patterns
//! atp-grep -F "hello.world" file.txt    # Fixed-string match
//! echo "text" | atp-grep "pattern"      # Stdin support
//! ```
//!
//! Output is typed JSON when piped, human-readable when in a terminal.
//! Override with `--format json|yaml|csv|human`.

use anyhow::Result;
use atp_core::output::{format_search_human, ExecutionMetadata};
use atp_core::{
    parse_grep_args, AtpEnvelope, Format, GrepEngine, OutputFormatter, Provenance, Walker,
};
use std::io::{self, BufRead, IsTerminal};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Extract --format flag before POSIX parsing
    let (format_override, filtered_args) = extract_format_flag(&args);

    let result = run(&filtered_args, format_override);

    if let Err(e) = result {
        eprintln!("atp-grep: {e:#}");
        std::process::exit(2); // grep convention: 2 = error
    }
}

fn run(args: &[String], format_override: Option<String>) -> Result<()> {
    let start = Instant::now();

    let compat = parse_grep_args(args)?;

    // Determine output format
    let format = resolve_format(format_override);

    if compat.stdin_mode {
        return run_stdin(&compat, format);
    }

    // File mode — use Walker + GrepEngine
    let walker = Walker::new(compat.scope.clone())?;
    let file_paths = walker.collect_files()?;

    let engine = GrepEngine::new(compat.config.clone())?;
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();
    let results = engine.search_files(&path_refs)?;

    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            if compat.count_only {
                let mut per_file: std::collections::BTreeMap<&str, usize> =
                    std::collections::BTreeMap::new();
                for m in &results.matches {
                    *per_file.entry(&m.file).or_insert(0) += 1;
                }
                for (file, count) in &per_file {
                    println!("{file}:{count}");
                }
            } else if compat.files_only {
                let mut seen = std::collections::HashSet::new();
                for m in &results.matches {
                    if seen.insert(&m.file) {
                        println!("{}", m.file);
                    }
                }
            } else if compat.files_without_match {
                let matched_files: std::collections::HashSet<_> =
                    results.matches.iter().map(|m| &m.file).collect();
                for path in &file_paths {
                    let p = path.display().to_string();
                    if !matched_files.contains(&p) {
                        println!("{p}");
                    }
                }
            } else {
                let output = format_search_human(&results, std::io::stdout().is_terminal());
                print!("{output}");
            }
        }
        _ => {
            let provenance = Provenance::new("grep-compat", vec![compat.config.pattern.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned: file_paths.len(),
                files_matched: results.files_with_matches,
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("search", &results, metadata);
            let output = OutputFormatter::format(&envelope, format)?;
            println!("{output}");
        }
    }

    // Exit code: 0 = matches found, 1 = no matches (grep convention)
    if results.total_matches == 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Handle stdin mode — read lines from stdin and apply grep.
fn run_stdin(compat: &atp_core::GrepCompat, format: Format) -> Result<()> {
    use atp_core::output::SearchResults;

    let stdin = io::stdin();
    let engine = GrepEngine::new(compat.config.clone())?;

    // Read all stdin into a string, then search it
    let mut input = String::new();
    for line in stdin.lock().lines() {
        input.push_str(&line?);
        input.push('\n');
    }

    // Write to a temp file for the engine (simplest integration)
    let tmp = std::env::temp_dir().join("atp-grep-stdin.tmp");
    std::fs::write(&tmp, &input)?;

    let results = engine.search_files(&[tmp.as_path()])?;

    // Clean up
    let _ = std::fs::remove_file(&tmp);

    // Remap file name from temp to "<stdin>"
    let remapped = SearchResults {
        pattern: results.pattern.clone(),
        pattern_type: results.pattern_type.clone(),
        case_sensitive: results.case_sensitive,
        matches: results
            .matches
            .into_iter()
            .map(|mut m| {
                m.file = "<stdin>".to_string();
                m
            })
            .collect(),
        total_matches: results.total_matches,
        files_with_matches: if results.total_matches > 0 { 1 } else { 0 },
    };

    match format {
        Format::Human => {
            if compat.count_only {
                println!("{}", remapped.total_matches);
            } else {
                let output = format_search_human(&remapped, std::io::stdout().is_terminal());
                print!("{output}");
            }
        }
        _ => {
            let provenance = Provenance::new("grep-compat", vec![compat.config.pattern.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned: 1,
                files_matched: remapped.files_with_matches,
                duration_ms: 0,
                provenance,
            };
            let envelope = AtpEnvelope::new("search", &remapped, metadata);
            let output = OutputFormatter::format(&envelope, format)?;
            println!("{output}");
        }
    }

    if remapped.total_matches == 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Extract `--format <value>` from args, returning the format and remaining args.
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

/// Resolve output format: explicit override > auto-detect (JSON if piped, human if TTY).
fn resolve_format(format_override: Option<String>) -> Format {
    if let Some(ref fmt) = format_override {
        fmt.parse().unwrap_or(Format::Json)
    } else if std::io::stdout().is_terminal() {
        Format::Human
    } else {
        Format::Json
    }
}
