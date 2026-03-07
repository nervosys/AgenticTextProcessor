//! Ontology command — machine-readable self-description for agent discovery.

use anyhow::Result;
use clap::Args;

use atp_core::ontology::build_ontology;
use atp_core::output::*;

#[derive(Args)]
pub struct OntologyArgs {
    /// Show ontology for a specific command only
    #[arg(long, short = 'c')]
    pub command: Option<String>,

    /// Show a specific section: capabilities, commands, types, errors, examples
    #[arg(long, short = 's')]
    pub section: Option<String>,
}

pub fn execute(args: OntologyArgs, format: Format) -> Result<()> {
    let ontology = build_ontology();

    // Filter by command or section if specified
    let output_value = if let Some(ref cmd_name) = args.command {
        let cmd = ontology
            .commands
            .iter()
            .find(|c| c.name == *cmd_name || c.aliases.contains(cmd_name))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Unknown command: {cmd_name}. Available: {}",
                    ontology
                        .commands
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        serde_json::to_value(cmd)?
    } else if let Some(ref section) = args.section {
        match section.as_str() {
            "capabilities" => serde_json::to_value(&ontology.capabilities)?,
            "commands" => serde_json::to_value(&ontology.commands)?,
            "types" => serde_json::to_value(&ontology.types)?,
            "errors" => serde_json::to_value(&ontology.error_codes)?,
            "examples" => serde_json::to_value(&ontology.examples)?,
            _ => {
                anyhow::bail!(
                    "Unknown section: {section}. Use: capabilities, commands, types, errors, examples"
                );
            }
        }
    } else {
        serde_json::to_value(&ontology)?
    };

    match format {
        Format::Human => {
            if args.command.is_some() || args.section.is_some() {
                println!("{}", serde_json::to_string_pretty(&output_value)?);
            } else {
                // Human-friendly summary
                println!("ATP — Agentic Text Processor v{}", ontology.version);
                println!("  {}\n", ontology.description);
                println!("Capabilities:");
                for cap in &ontology.capabilities {
                    println!("  • {} — {}", cap.name, cap.description);
                }
                println!("\nCommands:");
                for cmd in &ontology.commands {
                    let aliases = if cmd.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" (aliases: {})", cmd.aliases.join(", "))
                    };
                    println!("  atp {}{}", cmd.name, aliases);
                    println!("    {}", cmd.description);
                }
                println!("\nOutput Formats: {}", ontology.output_formats.join(", "));
                println!("\nFor machine-readable ontology: atp ontology --format json");
            }
        }
        Format::Yaml => {
            println!("{}", serde_yaml::to_string(&output_value)?);
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&output_value)?);
        }
    }

    Ok(())
}
