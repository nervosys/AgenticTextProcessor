//! Scope command — list files matching scope configuration.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct ScopeArgs {
    /// Root paths to scan (default: current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

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

    /// Follow symbolic links
    #[arg(long)]
    pub follow_symlinks: bool,
}

pub fn execute(args: ScopeArgs, format: Format, raw: bool) -> Result<()> {
    let scope = FileScope {
        roots: args.paths,
        include_globs: args.include.iter().cloned().collect(),
        exclude_globs: args.exclude.iter().cloned().collect(),
        max_depth: args.max_depth,
        respect_gitignore: !args.no_gitignore,
        follow_symlinks: args.follow_symlinks,
    };

    let walker = Walker::new(scope)?;
    let scope_results = walker.list_scope()?;

    match format {
        Format::Human => {
            for file in &scope_results.files {
                println!("{file}");
            }
            eprintln!(
                "\n{} files, {} bytes total",
                scope_results.total_files, scope_results.total_size_bytes
            );
        }
        _ => {
            let provenance = Provenance::new("scope", vec![]);
            let metadata = ExecutionMetadata {
                files_scanned: scope_results.total_files,
                files_matched: scope_results.total_files,
                duration_ms: 0,
                provenance,
            };
            let envelope = AtpEnvelope::new("scope", &scope_results, metadata);
            if raw {
                println!("{}", serde_json::to_string_pretty(&scope_results)?);
            } else {
                let output = OutputFormatter::format(&envelope, format)?;
                println!("{output}");
            }
        }
    }

    Ok(())
}
