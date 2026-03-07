//! Search command — grep-like pattern searching with strongly typed output.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::time::Instant;

use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct SearchArgs {
    /// The search pattern (regex by default)
    pub pattern: String,

    /// Files or directories to search (default: current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Treat pattern as a literal string, not regex (same as grep -F)
    #[arg(long, short = 'L')]
    pub literal: bool,

    /// Case-insensitive matching
    #[arg(long, short = 'i')]
    pub case_insensitive: bool,

    /// Match whole words only
    #[arg(long, short = 'w')]
    pub whole_word: bool,

    /// Invert match (show non-matching lines)
    #[arg(long, short = 'v')]
    pub invert: bool,

    /// Number of context lines before and after each match
    #[arg(long, short = 'C', default_value = "0")]
    pub context: usize,

    /// Lines of context before each match
    #[arg(long, short = 'B', default_value = "0")]
    pub before_context: usize,

    /// Lines of context after each match
    #[arg(long, short = 'A', default_value = "0")]
    pub after_context: usize,

    /// Maximum total matches to return
    #[arg(long, short = 'm')]
    pub max_matches: Option<usize>,

    /// Maximum matches per file
    #[arg(long)]
    pub max_per_file: Option<usize>,

    /// Multiline matching
    #[arg(long)]
    pub multiline: bool,

    /// Glob pattern for files to include (e.g. '*.rs')
    #[arg(long)]
    pub include: Option<String>,

    /// Glob pattern for files to exclude
    #[arg(long)]
    pub exclude: Option<String>,

    /// Maximum directory depth
    #[arg(long, short = 'd')]
    pub max_depth: Option<usize>,

    /// Do not respect .gitignore
    #[arg(long)]
    pub no_gitignore: bool,

    /// Show only file names with matches (no match details)
    #[arg(long)]
    pub files_only: bool,

    /// Show only the count of matches per file
    #[arg(long, short = 'c')]
    pub count: bool,

    /// Show only the matched text, not the full line (like grep -o)
    #[arg(long, short = 'o')]
    pub only_matching: bool,

    /// Additional patterns to match (like grep -e); any pattern matches
    #[arg(long = "and-pattern", short = 'e')]
    pub extra_patterns: Vec<String>,

    /// Print only filenames of files with NO matches (like grep -L)
    #[arg(long)]
    pub files_without_match: bool,
}

pub fn execute(args: SearchArgs, format: Format, raw: bool, no_color: bool) -> Result<()> {
    let start = Instant::now();

    // Build scope
    let scope = FileScope {
        roots: args.paths.clone(),
        include_globs: args.include.iter().cloned().collect(),
        exclude_globs: args.exclude.iter().cloned().collect(),
        max_depth: args.max_depth,
        respect_gitignore: !args.no_gitignore,
        follow_symlinks: false,
    };

    // Collect files
    let walker = Walker::new(scope)?;
    let file_paths = walker.collect_files()?;

    // Build grep config
    let ctx_before = if args.context > 0 {
        args.context
    } else {
        args.before_context
    };
    let ctx_after = if args.context > 0 {
        args.context
    } else {
        args.after_context
    };

    let config = GrepConfig {
        pattern: args.pattern.clone(),
        pattern_type: if args.literal {
            PatternType::Literal
        } else {
            PatternType::Regex
        },
        case_sensitive: !args.case_insensitive,
        whole_word: args.whole_word,
        invert_match: args.invert,
        context_before: ctx_before,
        context_after: ctx_after,
        max_matches: args.max_matches,
        max_matches_per_file: args.max_per_file,
        multiline: args.multiline,
        include_binary: false,
        extra_patterns: args.extra_patterns.clone(),
        only_matching: args.only_matching,
    };

    let engine = GrepEngine::new(config)?;
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();
    let results = engine.search_files(&path_refs)?;

    let duration_ms = start.elapsed().as_millis() as u64;

    // Output
    match format {
        Format::Human => {
            if args.count {
                // Count mode
                let mut per_file: std::collections::BTreeMap<&str, usize> =
                    std::collections::BTreeMap::new();
                for m in &results.matches {
                    *per_file.entry(&m.file).or_insert(0) += 1;
                }
                for (file, count) in &per_file {
                    println!("{file}:{count}");
                }
            } else if args.files_only {
                let mut seen = std::collections::HashSet::new();
                for m in &results.matches {
                    if seen.insert(&m.file) {
                        println!("{}", m.file);
                    }
                }
            } else if args.files_without_match {
                // Show files that had NO matches
                let matched_files: std::collections::HashSet<_> =
                    results.matches.iter().map(|m| &m.file).collect();
                for path in &file_paths {
                    let p = path.display().to_string();
                    if !matched_files.contains(&p) {
                        println!("{p}");
                    }
                }
            } else if args.only_matching {
                // Print only the matched text (like grep -o)
                for m in &results.matches {
                    println!("{}", m.matched_text);
                }
            } else {
                let output = format_search_human(&results, !no_color);
                print!("{output}");
            }
        }
        _ => {
            let provenance = Provenance::new("search", vec![args.pattern.clone()]);
            let metadata = ExecutionMetadata {
                files_scanned: file_paths.len(),
                files_matched: results.files_with_matches,
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("search", &results, metadata);
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
