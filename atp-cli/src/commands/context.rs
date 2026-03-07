//! Context command — smart context extraction around matches.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use atp_core::context::{extract_context, ContextMode};
use atp_core::output::*;

#[derive(Args)]
pub struct ContextArgs {
    /// File to extract context from
    pub file: PathBuf,

    /// Line number to center context around (1-based)
    #[arg(long, short = 'L')]
    pub line: usize,

    /// Context mode: lines, function, block, indent, file
    #[arg(long, short = 'm', default_value = "function")]
    pub mode: String,

    /// Number of context lines (for 'lines' mode)
    #[arg(long, short = 'n', default_value = "5")]
    pub count: usize,
}

pub fn execute(args: ContextArgs, format: Format, raw: bool) -> Result<()> {
    let mode = match args.mode.as_str() {
        "lines" | "l" => ContextMode::Lines {
            before: args.count,
            after: args.count,
        },
        "function" | "fn" | "func" => ContextMode::Function,
        "block" | "b" => ContextMode::Block,
        "indent" | "i" => ContextMode::Indent,
        "file" | "f" => ContextMode::File,
        other => anyhow::bail!(
            "Unknown context mode: '{other}'. Use: lines, function, block, indent, file"
        ),
    };

    let window = extract_context(&args.file, args.line, &mode)?;

    match format {
        Format::Human => {
            eprintln!(
                "[{} context] {}:{}-{} (match at line {})",
                window.context_type,
                window.file,
                window.start_line,
                window.end_line,
                window.match_line
            );
            for (i, line) in window.content.lines().enumerate() {
                let line_num = window.start_line + i;
                let marker = if line_num == window.match_line {
                    ">>>"
                } else {
                    "   "
                };
                println!("{marker} {:>4} | {}", line_num, line);
            }
        }
        _ => {
            let provenance = Provenance::new("context", vec![args.file.display().to_string()]);
            let metadata = ExecutionMetadata {
                files_scanned: 1,
                files_matched: 1,
                duration_ms: 0,
                provenance,
            };
            let envelope = AtpEnvelope::new("context", &window, metadata);
            if raw {
                println!("{}", serde_json::to_string_pretty(&window)?);
            } else {
                let output = OutputFormatter::format(&envelope, format)?;
                println!("{output}");
            }
        }
    }

    Ok(())
}
