//! Shell completion generator for ATP CLI.

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

/// Generate shell completions and print to stdout.
///
/// Supported shells: bash, zsh, fish, powershell, elvish
pub fn execute(shell_name: &str) -> Result<()> {
    let shell: Shell = shell_name.parse().map_err(|_| {
        anyhow::anyhow!(
            "Unknown shell: {shell_name}. Supported: bash, zsh, fish, powershell, elvish"
        )
    })?;

    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, "atp", &mut io::stdout());
    Ok(())
}
