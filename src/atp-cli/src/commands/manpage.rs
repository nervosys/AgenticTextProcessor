//! Man page generator for ATP CLI.

use anyhow::Result;
use clap::CommandFactory;
use clap_mangen::Man;

/// Generate man pages and write to stdout or the specified directory.
///
/// Usage:
///   atp manpage              Print main man page to stdout
///   atp manpage --dir ./man  Write all man pages to directory
pub fn execute(output_dir: Option<&str>) -> Result<()> {
    let cmd = crate::Cli::command();

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;

        // Main command
        let man = Man::new(cmd.clone());
        let mut f = std::fs::File::create(format!("{dir}/atp.1"))?;
        man.render(&mut f)?;

        // Subcommands
        for sub in cmd.get_subcommands() {
            let name = sub.get_name();
            let man = Man::new(sub.clone());
            let mut f = std::fs::File::create(format!("{dir}/atp-{name}.1"))?;
            man.render(&mut f)?;
        }

        eprintln!("Man pages written to {dir}/");
    } else {
        let man = Man::new(cmd);
        man.render(&mut std::io::stdout())?;
    }

    Ok(())
}
