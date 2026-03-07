//! CLI command: `atp index` — build and search the file index.

use anyhow::Result;
use atp_core::index::SearchIndex;
use atp_core::output::Format;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct IndexArgs {
    /// Subcommand: build, search, status, update
    pub action: String,

    /// Root directory to index (default: current directory)
    #[arg(long, short = 'd', default_value = ".")]
    pub directory: String,

    /// Search pattern (for 'search' action)
    #[arg(long, short = 'p')]
    pub pattern: Option<String>,

    /// Use regex for search
    #[arg(long, short = 'r')]
    pub regex: bool,

    /// Glob pattern for file matching
    #[arg(long, short = 'g')]
    pub glob: Option<String>,

    /// Index file path for save/load
    #[arg(long, default_value = ".atp-index.json")]
    pub index_file: String,
}

pub fn execute(args: IndexArgs, format: Format, _raw: bool) -> Result<()> {
    match args.action.as_str() {
        "build" => {
            let mut index = SearchIndex::new(PathBuf::from(&args.directory));
            index.build()?;
            let stats = index.stats();
            index.save(std::path::Path::new(&args.index_file))?;

            match format {
                Format::Human => {
                    println!(
                        "Index built: {} files, {} lines, {} bytes",
                        stats.file_count, stats.total_lines, stats.total_bytes
                    );
                    println!("Saved to: {}", args.index_file);
                }
                _ => {
                    println!("{}", serde_json::to_string_pretty(&stats)?);
                }
            }
        }
        "search" => {
            let pattern = args
                .pattern
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--pattern is required for search"))?;

            let index = if std::path::Path::new(&args.index_file).exists() {
                SearchIndex::load(std::path::Path::new(&args.index_file))?
            } else {
                let mut idx = SearchIndex::new(PathBuf::from(&args.directory));
                idx.build()?;
                idx
            };

            let hits = if args.regex {
                index.search_regex(pattern)?
            } else {
                index.search(pattern)
            };

            match format {
                Format::Human => {
                    for hit in &hits {
                        println!("{}:{}: {}", hit.path, hit.line, hit.content);
                    }
                    println!("\n{} matches", hits.len());
                }
                _ => {
                    println!("{}", serde_json::to_string_pretty(&hits)?);
                }
            }
        }
        "status" => {
            if std::path::Path::new(&args.index_file).exists() {
                let index = SearchIndex::load(std::path::Path::new(&args.index_file))?;
                let stats = index.stats();
                match format {
                    Format::Human => {
                        println!("Index: {}", args.index_file);
                        println!("  Files: {}", stats.file_count);
                        println!("  Lines: {}", stats.total_lines);
                        println!("  Bytes: {}", stats.total_bytes);
                        println!("  Trigrams: {}", stats.trigram_count);
                    }
                    _ => {
                        println!("{}", serde_json::to_string_pretty(&stats)?);
                    }
                }
            } else {
                match format {
                    Format::Human => println!("No index found at {}", args.index_file),
                    _ => println!("{{\"exists\": false}}"),
                }
            }
        }
        "update" => {
            if !std::path::Path::new(&args.index_file).exists() {
                anyhow::bail!(
                    "No index found at {}. Run 'atp index build' first.",
                    args.index_file
                );
            }
            let mut index = SearchIndex::load(std::path::Path::new(&args.index_file))?;
            // Rebuild to catch updates
            index.build()?;
            index.save(std::path::Path::new(&args.index_file))?;
            let stats = index.stats();
            match format {
                Format::Human => {
                    println!("Index updated: {} files", stats.file_count);
                }
                _ => {
                    println!("{}", serde_json::to_string_pretty(&stats)?);
                }
            }
        }
        "files" => {
            let glob = args
                .glob
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--glob is required for 'files' action"))?;

            let index = if std::path::Path::new(&args.index_file).exists() {
                SearchIndex::load(std::path::Path::new(&args.index_file))?
            } else {
                let mut idx = SearchIndex::new(PathBuf::from(&args.directory));
                idx.build()?;
                idx
            };

            let matches = index.files_matching(glob);
            match format {
                Format::Human => {
                    for p in &matches {
                        println!("{}", p);
                    }
                    println!("\n{} files", matches.len());
                }
                _ => {
                    println!("{}", serde_json::to_string_pretty(&matches)?);
                }
            }
        }
        other => {
            anyhow::bail!(
                "Unknown index action '{}'. Use: build, search, status, update, files",
                other
            );
        }
    }

    Ok(())
}
