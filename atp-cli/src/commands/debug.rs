//! CLI command: `atp debug` — step-through AQL debugger.

use anyhow::Result;
use atp_core::dap::AqlDebugger;
use atp_core::output::Format;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct DebugArgs {
    /// AQL query to debug
    pub query: String,

    /// Files / directories to operate on
    pub paths: Vec<String>,

    /// Breakpoint at stage index (0-based, can repeat)
    #[arg(long, short = 'b')]
    pub breakpoint: Vec<usize>,

    /// Run mode: step, run, info
    #[arg(long, short = 'm', default_value = "run")]
    pub mode: String,
}

pub fn execute(args: DebugArgs, format: Format, _raw: bool) -> Result<()> {
    let files: Vec<PathBuf> = args.paths.iter().map(PathBuf::from).collect();
    let file_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();

    let mut debugger = AqlDebugger::new();
    for &bp in &args.breakpoint {
        debugger.add_default_breakpoint(bp);
    }

    let mut session = debugger.start(&args.query, &file_refs)?;

    match args.mode.as_str() {
        "info" => {
            // Just show the parsed pipeline info
            match format {
                Format::Human => {
                    println!("Query: {}", session.query);
                    println!("Stages: {}", session.total_stages);
                    for (i, desc) in session.stage_descriptions.iter().enumerate() {
                        println!("  [{}] {}", i, desc);
                    }
                    if !session.breakpoints.is_empty() {
                        println!("Breakpoints:");
                        for bp in &session.breakpoints {
                            println!("  stage {} (enabled: {})", bp.stage, bp.enabled);
                        }
                    }
                }
                _ => {
                    println!("{}", serde_json::to_string_pretty(&session)?);
                }
            }
        }
        "step" => {
            // Step through each stage
            let mut input_lines: Vec<String> = Vec::new();
            loop {
                let snap = match debugger.step(&mut session, &input_lines) {
                    Ok(s) => s,
                    Err(_) => break,
                };
                input_lines = snap.output_sample.clone();
                let _ = snap;

                match format {
                    Format::Human => {
                        if let Some(snap) = session.current_snapshot() {
                            println!(
                                "Stage {}/{}: {}",
                                snap.stage_index + 1,
                                session.total_stages,
                                snap.stage_text
                            );
                            println!(
                                "  Input: {} lines → Output: {} lines ({}ms)",
                                snap.input_count, snap.output_count, snap.duration_ms
                            );
                            if !snap.output_sample.is_empty() {
                                println!("  Sample:");
                                for s in &snap.output_sample {
                                    println!("    {}", s);
                                }
                            }
                        }
                    }
                    _ => {
                        if let Some(snap) = session.current_snapshot() {
                            println!("{}", serde_json::to_string_pretty(snap)?);
                        }
                    }
                }

                if session.current_stage >= session.total_stages {
                    break;
                }
            }

            if format == Format::Human {
                println!("\n{}", AqlDebugger::session_summary(&session));
            }
        }
        "run" => {
            // Run to completion or first breakpoint
            let input_lines: Vec<String> = Vec::new();
            let _output = debugger.run(&mut session, &input_lines)?;

            match format {
                Format::Human => {
                    println!("{}", AqlDebugger::session_summary(&session));
                }
                _ => {
                    println!("{}", serde_json::to_string_pretty(&session)?);
                }
            }
        }
        other => {
            anyhow::bail!("Unknown debug mode '{}'. Use: step, run, info", other);
        }
    }

    Ok(())
}
