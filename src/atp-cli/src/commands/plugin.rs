//! CLI command: `atp plugin` — manage ATP plugins.

use anyhow::Result;
use atp_core::plugin::{PluginRegistry, PluginSdk};
use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub action: PluginAction,
}

#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// List installed plugins
    #[command(alias = "ls")]
    List,

    /// Install a plugin from a local TOML file
    Install {
        /// Path to plugin TOML manifest
        path: String,
    },

    /// Remove an installed plugin
    #[command(alias = "rm")]
    Uninstall {
        /// Plugin name
        name: String,
    },

    /// Scaffold a new plugin template
    #[command(alias = "new")]
    Init {
        /// Plugin name
        name: String,

        /// Plugin kind: stage or format
        #[arg(long, short = 'k', default_value = "stage")]
        kind: String,

        /// Plugin description
        #[arg(long, short = 'd', default_value = "An ATP plugin")]
        description: String,
    },

    /// Validate a plugin manifest
    Check {
        /// Path to plugin TOML manifest
        path: String,
    },

    /// Show information about an installed plugin
    Info {
        /// Plugin name
        name: String,
    },
}

pub fn execute(args: PluginArgs) -> Result<()> {
    match args.action {
        PluginAction::List => {
            let mut registry = PluginRegistry::default_dir();
            let count = registry.discover()?;
            if count == 0 {
                println!("No plugins installed.");
                eprintln!("Install plugins with: atp plugin install <path.toml>");
                eprintln!("Create a new plugin with: atp plugin init <name>");
                return Ok(());
            }
            let plugins = registry.list_plugins();
            println!(
                "{:<25} {:<10} {:<10} DESCRIPTION",
                "NAME", "VERSION", "KIND"
            );
            println!("{}", "─".repeat(80));
            for p in &plugins {
                println!(
                    "{:<25} {:<10} {:<10} {}",
                    p.name,
                    p.version,
                    format!("{:?}", p.kind).to_lowercase(),
                    p.description
                );
            }
            eprintln!("\n{count} plugin(s) installed");
            Ok(())
        }

        PluginAction::Install { path } => {
            let mut registry = PluginRegistry::default_dir();
            registry.discover()?;
            let name = registry.install_local(std::path::Path::new(&path))?;
            println!("Installed plugin: {name}");
            Ok(())
        }

        PluginAction::Uninstall { name } => {
            let mut registry = PluginRegistry::default_dir();
            registry.discover()?;
            registry.uninstall(&name)?;
            println!("Uninstalled plugin: {name}");
            Ok(())
        }

        PluginAction::Init {
            name,
            kind,
            description,
        } => {
            let content = match kind.as_str() {
                "format" => PluginSdk::scaffold_format(&name, &name, &description),
                _ => PluginSdk::scaffold_stage(&name, &description),
            };
            let filename = format!("{name}.toml");
            std::fs::write(&filename, &content)?;
            println!("Created plugin template: {filename}");
            println!("\nNext steps:");
            println!("  1. Edit {filename} to customize your plugin");
            println!("  2. Validate with: atp plugin check {filename}");
            println!("  3. Install with:  atp plugin install {filename}");
            Ok(())
        }

        PluginAction::Check { path } => {
            let registry = PluginRegistry::default_dir();
            let manifest = registry.load_manifest(std::path::Path::new(&path))?;
            let errors = PluginSdk::validate_manifest(&manifest);
            if errors.is_empty() {
                println!("Plugin manifest is valid: {}", manifest.plugin.name);
                println!("  Version:     {}", manifest.plugin.version);
                println!("  Kind:        {:?}", manifest.plugin.kind);
                println!("  Description: {}", manifest.plugin.description);
            } else {
                println!("Plugin manifest has errors:");
                for e in &errors {
                    println!("  - {e}");
                }
                std::process::exit(1);
            }
            Ok(())
        }

        PluginAction::Info { name } => {
            let mut registry = PluginRegistry::default_dir();
            registry.discover()?;
            if let Some(manifest) = registry
                .find_stage(&name)
                .or_else(|| registry.manifests().iter().find(|m| m.plugin.name == name))
            {
                println!("Plugin: {}", manifest.plugin.name);
                println!("Version: {}", manifest.plugin.version);
                println!("Description: {}", manifest.plugin.description);
                println!("Kind: {:?}", manifest.plugin.kind);
                if let Some(author) = &manifest.plugin.author {
                    println!("Author: {author}");
                }
                if let Some(license) = &manifest.plugin.license {
                    println!("License: {license}");
                }
            } else {
                anyhow::bail!("Plugin not found: {name}");
            }
            Ok(())
        }
    }
}
