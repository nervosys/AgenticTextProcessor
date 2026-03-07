//! CLI command: `atp remote` — distributed execution over SSH.

use anyhow::Result;
use atp_core::remote::{RemoteExecutor, RemoteTarget};
use clap::Args;

#[derive(Args, Debug)]
pub struct RemoteArgs {
    /// Comma-separated list of hosts (user@host:port)
    #[arg(long)]
    pub hosts: Option<String>,

    /// File containing host list (one per line)
    #[arg(long)]
    pub host_file: Option<String>,

    /// SSH identity file (private key)
    #[arg(long, short = 'i')]
    pub identity: Option<String>,

    /// Remote working directory
    #[arg(long)]
    pub remote_dir: Option<String>,

    /// Run on all hosts in parallel
    #[arg(long)]
    pub parallel: bool,

    /// ATP command to run on remote hosts (everything after --)
    #[arg(last = true)]
    pub command: Vec<String>,
}

pub fn execute(args: RemoteArgs) -> Result<()> {
    if args.command.is_empty() {
        anyhow::bail!("No command specified. Pass the command after `--`, e.g.:\n  atp remote --hosts user@host -- search -p 'pattern' file.txt");
    }

    let mut targets: Vec<RemoteTarget> = Vec::new();

    if let Some(host_file) = &args.host_file {
        targets.extend(RemoteTarget::parse_file(host_file)?);
    }

    if let Some(hosts_str) = &args.hosts {
        targets.extend(RemoteTarget::parse_list(hosts_str));
    }

    if targets.is_empty() {
        anyhow::bail!("No hosts specified. Use --hosts or --host-file.");
    }

    // Apply identity file and remote dir overrides
    for t in &mut targets {
        if let Some(id) = &args.identity {
            t.identity_file = Some(id.clone());
        }
        if let Some(dir) = &args.remote_dir {
            t.path = Some(dir.clone());
        }
    }

    let atp_args: Vec<&str> = args.command.iter().map(|s| s.as_str()).collect();
    let executor = RemoteExecutor::new();

    let results = if args.parallel {
        executor.execute_parallel(&targets, &atp_args)
    } else {
        executor.execute_on_all(&targets, &atp_args)
    };

    // Display results
    let mut any_failure = false;
    for r in &results {
        let status = if r.success { "OK" } else { "FAIL" };
        println!("── {} [{}] ({} ms) ──", r.host, status, r.duration_ms);
        if !r.stdout.is_empty() {
            print!("{}", r.stdout);
        }
        if !r.stderr.is_empty() {
            eprint!("{}", r.stderr);
        }
        if !r.success {
            any_failure = true;
        }
    }

    let ok_count = results.iter().filter(|r| r.success).count();
    eprintln!("\n{}/{} hosts succeeded", ok_count, results.len());

    if any_failure {
        std::process::exit(1);
    }

    Ok(())
}
