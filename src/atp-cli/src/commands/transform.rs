//! Transform command — sed-like text transformation with strongly typed output.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use std::time::Instant;

use atp_core::engine::sed::{parse_substitution, SedConfig, SedEngine, TransformCommand};
use atp_core::output::*;
use atp_core::traversal::Walker;

#[derive(Args)]
pub struct TransformArgs {
    /// Files or directories to transform (default: current directory)
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Sed-style expression (e.g. 's/pattern/replacement/flags'). Can be repeated.
    #[arg(long, short = 'e')]
    pub expression: Vec<String>,

    /// Pattern to match for substitution
    #[arg(long, short = 'p')]
    pub pattern: Option<String>,

    /// Replacement string (supports capture groups like $1, $2)
    #[arg(long, short = 'r')]
    pub replacement: Option<String>,

    /// Replace all occurrences per line
    #[arg(long, short = 'g')]
    pub global: bool,

    /// Case-insensitive matching
    #[arg(long, short = 'i')]
    pub case_insensitive: bool,

    /// Modify files in place
    #[arg(long)]
    pub in_place: bool,

    /// Backup extension for in-place editing (e.g. 'bak')
    #[arg(long)]
    pub backup: Option<String>,

    /// Preview changes without applying (default for safety)
    #[arg(long, short = 'n')]
    pub dry_run: bool,

    /// Delete lines matching pattern (instead of substituting)
    #[arg(long)]
    pub delete: Option<String>,

    /// Insert text before lines matching pattern
    #[arg(long)]
    pub insert_before: Option<String>,

    /// Insert text after lines matching pattern
    #[arg(long)]
    pub insert_after: Option<String>,

    /// Text to insert (used with --insert-before or --insert-after)
    #[arg(long)]
    pub insert_text: Option<String>,

    /// Line range to apply transformation (e.g. '10,20')
    #[arg(long)]
    pub line_range: Option<String>,

    /// Glob pattern for files to include
    #[arg(long)]
    pub include: Option<String>,

    /// Glob pattern for files to exclude
    #[arg(long)]
    pub exclude: Option<String>,

    /// Maximum directory depth
    #[arg(long, short = 'd')]
    pub max_depth: Option<usize>,

    /// Address-range: only transform lines between /start/ and /end/ patterns
    /// Format: '/start_pattern/,/end_pattern/'
    #[arg(long)]
    pub address_range: Option<String>,
}

pub fn execute(args: TransformArgs, format: Format, raw: bool) -> Result<()> {
    let start = Instant::now();

    // Build transform commands
    let mut commands = Vec::new();

    // Support multiple -e expressions (like sed -e cmd1 -e cmd2)
    for expr in &args.expression {
        commands.push(parse_substitution(expr)?);
    }

    if commands.is_empty() {
        if let (Some(pattern), Some(replacement)) = (&args.pattern, &args.replacement) {
            commands.push(TransformCommand::Substitute {
                pattern: pattern.clone(),
                replacement: replacement.clone(),
                global: args.global,
                case_insensitive: args.case_insensitive,
            });
        }
    }

    if let Some(ref delete_pattern) = args.delete {
        commands.push(TransformCommand::Delete {
            pattern: delete_pattern.clone(),
        });
    }

    if let (Some(ref before_pattern), Some(ref text)) = (&args.insert_before, &args.insert_text) {
        commands.push(TransformCommand::InsertBefore {
            pattern: before_pattern.clone(),
            text: text.clone(),
        });
    }

    if let (Some(ref after_pattern), Some(ref text)) = (&args.insert_after, &args.insert_text) {
        commands.push(TransformCommand::InsertAfter {
            pattern: after_pattern.clone(),
            text: text.clone(),
        });
    }

    if commands.is_empty() {
        anyhow::bail!(
            "No transform commands specified. Use -e 's/pattern/replacement/g', \
             or --pattern + --replacement, or --delete."
        );
    }

    // Parse line range
    let line_range = if let Some(ref range_str) = args.line_range {
        let parts: Vec<&str> = range_str.split(',').collect();
        if parts.len() == 2 {
            Some((parts[0].parse::<usize>()?, parts[1].parse::<usize>()?))
        } else {
            anyhow::bail!("Line range must be in format 'start,end' (e.g. '10,20')");
        }
    } else {
        None
    };

    // Parse address-range patterns (/start/,/end/)
    let address_range = if let Some(ref addr) = args.address_range {
        // Expect format: /start_pattern/,/end_pattern/
        let trimmed = addr.trim();
        let parts: Vec<&str> = trimmed.splitn(2, ',').collect();
        if parts.len() == 2 {
            let start = parts[0].trim().trim_matches('/').to_string();
            let end = parts[1].trim().trim_matches('/').to_string();
            Some((start, end))
        } else {
            anyhow::bail!("Address range must be in format '/start_pattern/,/end_pattern/'");
        }
    } else {
        None
    };

    // Default to dry-run if not explicitly doing in-place
    let is_dry_run = !args.in_place || args.dry_run;

    let config = SedConfig {
        commands,
        in_place: args.in_place,
        backup_extension: args.backup.clone(),
        dry_run: is_dry_run,
        line_range,
        address_range,
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

    let engine = SedEngine::new(config);
    let path_refs: Vec<&std::path::Path> = file_paths.iter().map(|p| p.as_path()).collect();
    let results = engine.transform_files(&path_refs)?;

    let duration_ms = start.elapsed().as_millis() as u64;

    match format {
        Format::Human => {
            let output = format_transform_human(&results, true);
            print!("{output}");
        }
        _ => {
            let provenance = Provenance::new(
                "transform",
                args.expression
                    .iter()
                    .chain(args.pattern.iter())
                    .cloned()
                    .collect(),
            );
            let metadata = ExecutionMetadata {
                files_scanned: file_paths.len(),
                files_matched: results.files_modified,
                duration_ms,
                provenance,
            };
            let envelope = AtpEnvelope::new("transform", &results, metadata);
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
