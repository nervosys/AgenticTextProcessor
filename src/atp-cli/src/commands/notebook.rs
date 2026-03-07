//! CLI command: `atp notebook` — execute literate AQL notebooks.

use anyhow::Result;
use atp_core::notebook::Notebook;
use atp_core::output::Format;
use clap::Args;
use std::path::Path;

#[derive(Args, Debug)]
pub struct NotebookArgs {
    /// Notebook file (.md with AQL blocks)
    pub file: String,

    /// Subcommand: run, render, info (default: run)
    #[arg(long, short = 'a', default_value = "run")]
    pub action: String,

    /// Output file (for render action)
    #[arg(long, short = 'o')]
    pub output: Option<String>,

    /// Only execute cell at this index (0-based)
    #[arg(long, short = 'c')]
    pub cell: Option<usize>,
}

pub fn execute(args: NotebookArgs, format: Format, _raw: bool) -> Result<()> {
    let path = Path::new(&args.file);
    let mut nb = Notebook::load(path)?;

    match args.action.as_str() {
        "info" => match format {
            Format::Human => {
                println!("Notebook: {}", args.file);
                if let Some(ref title) = nb.title {
                    println!("Title: {}", title);
                }
                println!(
                    "Cells: {} total ({} AQL, {} markdown)",
                    nb.cells.len(),
                    nb.aql_cell_count(),
                    nb.markdown_cell_count()
                );
                for (i, cell) in nb.cells.iter().enumerate() {
                    let kind = match cell.kind {
                        atp_core::notebook::CellKind::Markdown => "md",
                        atp_core::notebook::CellKind::Aql => "aql",
                        atp_core::notebook::CellKind::Output => "out",
                    };
                    let preview: String = cell.source.chars().take(60).collect();
                    println!("  [{}] {} | {}", i, kind, preview.replace('\n', "↵"));
                }
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(&nb)?);
            }
        },
        "run" => {
            if let Some(cell_idx) = args.cell {
                nb.execute_cell(cell_idx)?;
                let cell = nb.cell(cell_idx).unwrap();
                match format {
                    Format::Human => {
                        if let Some(ref out) = cell.output {
                            println!("{}", out.text);
                        }
                    }
                    _ => {
                        println!("{}", serde_json::to_string_pretty(&cell.output)?);
                    }
                }
            } else {
                let summary = nb.execute_all()?;
                match format {
                    Format::Human => {
                        println!(
                            "Executed {} cells: {} succeeded, {} failed ({}ms)",
                            summary.total_cells,
                            summary.succeeded,
                            summary.failed,
                            summary.total_duration_ms
                        );
                        for err in &summary.errors {
                            eprintln!("  Error: {}", err);
                        }
                    }
                    _ => {
                        println!("{}", serde_json::to_string_pretty(&summary)?);
                    }
                }
            }

            // If output path given, save the rendered notebook
            if let Some(ref out_path) = args.output {
                nb.save(Path::new(out_path))?;
                if matches!(format, Format::Human) {
                    println!("Rendered to: {}", out_path);
                }
            }
        }
        "render" => {
            let rendered = nb.render();
            if let Some(ref out_path) = args.output {
                std::fs::write(out_path, &rendered)?;
                if matches!(format, Format::Human) {
                    println!("Rendered to: {}", out_path);
                }
            } else {
                print!("{}", rendered);
            }
        }
        other => {
            anyhow::bail!(
                "Unknown notebook action '{}'. Use: run, render, info",
                other
            );
        }
    }

    Ok(())
}
