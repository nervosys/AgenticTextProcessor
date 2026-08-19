//! ATP CLI — Agentic Text Processor command-line interface.
//!
//! An agentic-first successor to grep, sed, and awk with:
//! - Machine-readable ontology for AI agent discovery
//! - Strongly typed JSON/YAML/CSV output
//! - Deterministic, reproducible results
//! - Pipeline composition
//! - Smart context extraction

mod commands;

use clap::{Parser, Subcommand};

/// Agentic Text Processor — agentic-first successor to grep, sed, and awk.
///
/// ATP provides strongly typed, deterministic text processing with a complete
/// machine-readable ontology for AI agent discoverability and operation.
///
/// QUICK START:
///   atp search 'pattern' .              Search for a pattern
///   atp transform -e 's/old/new/g' .    Transform text
///   atp analyze -F ',' data.csv         Analyze structured text
///   atp pipeline -e 'search:X | head:5' Pipeline operations
///   atp ontology                        Get machine-readable ontology
///
/// OUTPUT FORMATS:
///   All commands support --format (json|json-pretty|jsonl|yaml|csv|human)
///   Default: json (for agent consumption), human (when TTY detected)
#[derive(Parser)]
#[command(name = "atp", version, about, long_about)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format: json, json-pretty, jsonl, yaml, csv, human
    #[arg(long, short = 'F', global = true, default_value = "auto")]
    format: String,

    /// Suppress envelope metadata (output only the data payload)
    #[arg(long, global = true, default_value_t = false)]
    raw: bool,

    /// Disable colored output
    #[arg(long, global = true, default_value_t = false)]
    no_color: bool,

    /// Enable verbose/debug output
    #[arg(long, global = true, default_value_t = false)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for patterns in files (grep-like)
    #[command(alias = "s", alias = "grep", alias = "find")]
    Search(commands::search::SearchArgs),

    /// Transform text with pattern-based substitution (sed-like)
    #[command(alias = "t", alias = "sed", alias = "replace")]
    Transform(commands::transform::TransformArgs),

    /// Analyze text with field-based processing (awk-like)
    #[command(alias = "a", alias = "awk", alias = "fields")]
    Analyze(commands::analyze::AnalyzeArgs),

    /// Execute multi-stage processing pipelines
    #[command(alias = "pipe", alias = "chain")]
    Pipeline(commands::pipeline::PipelineArgs),

    /// Execute AQL (ATP Query Language) queries — unified syntax for agents and humans
    ///
    /// AQL replaces grep/sed/awk regex syntax with a readable, unambiguous,
    /// keyword-based language optimized for both AI agents and humans.
    ///
    /// Examples:
    ///   atp query 'find "error" ignore_case'
    ///   atp query 'replace "old" with "new" all'
    ///   atp query 'find "TODO" | sort | unique | count'
    #[command(alias = "q", alias = "aql", alias = "run")]
    Query(commands::query::QueryArgs),

    /// Output the complete ATP ontology for agent discovery
    #[command(alias = "onto", alias = "capabilities", alias = "schema")]
    Ontology(commands::ontology::OntologyArgs),

    /// Explain what a command will do before execution
    #[command(alias = "x", alias = "preview")]
    Explain(commands::explain::ExplainArgs),

    /// List files matching scope configuration
    #[command(alias = "ls", alias = "files")]
    Scope(commands::scope::ScopeArgs),

    /// Validate a pattern, expression, or pipeline without executing
    #[command(alias = "check")]
    Validate(commands::validate::ValidateArgs),

    /// Show contextual information around a match
    #[command(alias = "ctx")]
    Context(commands::context::ContextArgs),

    /// Compliance reporting — NIST FIPS, CMMC 2.0, DoD regulatory posture
    ///
    /// Generate compliance reports, list CMMC 2.0 controls, and verify
    /// file integrity using FIPS 180-4 SHA-256.
    ///
    /// Examples:
    ///   atp compliance                  Full compliance report
    ///   atp compliance --controls       List all CMMC 2.0 controls
    ///   atp compliance -i file.txt      SHA-256 integrity hash
    #[command(alias = "cmmc", alias = "fips")]
    Compliance(commands::compliance::ComplianceArgs),

    /// Interactive AQL REPL — query shell with history
    ///
    /// Launch an interactive AQL session with readline editing, history,
    /// and configurable scope. Each line is executed as an AQL query.
    ///
    /// Examples:
    ///   atp repl                        Start REPL in current directory
    ///   atp repl src/                   Start REPL scoped to src/
    ///   atp repl --include '*.rs'       Start REPL for Rust files only
    #[command(alias = "shell", alias = "interactive")]
    Repl(commands::repl::ReplArgs),

    /// Watch files for changes and re-run a query
    ///
    /// Monitor files for changes and automatically re-execute an AQL query
    /// or search operation when files are modified.
    ///
    /// Examples:
    ///   atp watch 'find "TODO"' src/    Watch src/ for changes and re-run query
    ///   atp watch 'find "error"' .      Monitor current dir for the pattern
    #[command(alias = "monitor", alias = "w")]
    Watch(commands::watch::WatchArgs),

    /// Generate shell completions
    ///
    /// Print shell completion scripts to stdout.
    ///
    /// Examples:
    ///   atp completions bash > /etc/bash_completion.d/atp
    ///   atp completions zsh > ~/.zfunc/_atp
    ///   atp completions fish > ~/.config/fish/completions/atp.fish
    ///   atp completions powershell >> $PROFILE
    #[command(alias = "complete")]
    Completions {
        /// Shell name: bash, zsh, fish, powershell, elvish
        shell: String,
    },

    /// Generate man pages
    ///
    /// Print man page to stdout, or write all man pages to a directory.
    ///
    /// Examples:
    ///   atp manpage                     Print main man page to stdout
    ///   atp manpage --dir ./man         Write all man pages to ./man/
    #[command(alias = "man")]
    Manpage {
        /// Output directory for man pages (writes all subcommand pages)
        #[arg(long)]
        dir: Option<String>,
    },

    /// View and manage ATP configuration
    ///
    /// Load defaults from ~/.atp/config.toml (global) and .atprc (per-project).
    ///
    /// Examples:
    ///   atp config                      Show effective config
    ///   atp config --init               Create default ~/.atp/config.toml
    ///   atp config --path               Print config file path
    #[command(alias = "cfg")]
    Config(commands::config::ConfigArgs),

    /// Run as a Model Context Protocol (MCP) server
    ///
    /// Exposes ATP tools (search, transform, query, analyze) over
    /// JSON-RPC 2.0 stdin/stdout for AI agent integration.
    ///
    /// Examples:
    ///   atp mcp                         Start MCP server on stdio
    #[command(alias = "serve")]
    Mcp(commands::mcp::McpArgs),

    /// AI / LLM integration — natural language to AQL, explain, suggest
    ///
    /// Use a local or remote LLM to translate natural language to AQL,
    /// explain AQL queries, suggest queries, or do semantic search.
    ///
    /// Examples:
    ///   atp ai --nl "find all TODO comments"         NL-to-AQL
    ///   atp ai --explain 'find "TODO" | count'        Explain a query
    ///   atp ai --suggest src/                          Suggest queries
    #[command(alias = "llm")]
    Ai(commands::ai::AiArgs),

    /// Remote execution — run ATP commands on remote hosts via SSH
    ///
    /// Execute ATP commands on one or more remote hosts in parallel or
    /// sequentially, collecting results.
    ///
    /// Examples:
    ///   atp remote --hosts user@server -- search 'TODO' src/
    ///   atp remote --host-file hosts.txt --parallel -- query 'find "error"'
    #[command(alias = "ssh")]
    Remote(commands::remote::RemoteArgs),

    /// Code intelligence — extract and search symbols in source code
    ///
    /// Parse source files to extract function, class, struct, and other
    /// symbol definitions. Filter by kind, name, visibility, and language.
    ///
    /// Examples:
    ///   atp symbols src/ -r                           List all symbols
    ///   atp symbols -k function -n 'test_.*' src/     Find test functions
    ///   atp symbols -l rust --names-only lib.rs        Rust symbol names
    #[command(alias = "sym", alias = "code")]
    Symbols(commands::symbols::SymbolsArgs),

    /// Manage ATP plugins — install, remove, scaffold, validate
    ///
    /// Plugins extend ATP with custom pipeline stages and output formats.
    /// Plugins are TOML manifests stored in ~/.atp/plugins/.
    ///
    /// Examples:
    ///   atp plugin list                         List installed plugins
    ///   atp plugin init my-plugin                Scaffold a new plugin
    ///   atp plugin install my-plugin.toml        Install from file
    ///   atp plugin check my-plugin.toml          Validate manifest
    #[command(alias = "plug")]
    Plugin(commands::plugin::PluginArgs),

    /// Build, search, and manage the file index
    ///
    /// Trigram-accelerated full-text search index with incremental updates.
    ///
    /// Examples:
    ///   atp index build                           Build index for current dir
    ///   atp index search -p "TODO"                Search index for pattern
    ///   atp index status                           Show index stats
    ///   atp index files -g "*.rs"                  List files matching glob
    #[command(alias = "idx")]
    Index(commands::index::IndexArgs),

    /// Step-through AQL debugger — inspect pipeline stages
    ///
    /// Debug AQL queries by stepping through each pipeline stage,
    /// inspecting input/output at breakpoints, and watching expressions.
    ///
    /// Examples:
    ///   atp debug 'find "error" | sort | count' src/ -m step
    ///   atp debug 'find "TODO" | freq' src/ -b 1 -m run
    ///   atp debug 'find "error"' src/ -m info
    #[command(alias = "dbg")]
    Debug(commands::debug::DebugArgs),

    /// Execute literate AQL notebooks (Markdown + AQL)
    ///
    /// Parse and execute Markdown documents with embedded AQL code blocks.
    /// Supports cell-level execution and rendered output insertion.
    ///
    /// Examples:
    ///   atp notebook report.md                     Run all AQL cells
    ///   atp notebook report.md -a info              Show notebook structure
    ///   atp notebook report.md -a render -o out.md   Render with outputs
    ///   atp notebook report.md -c 0                 Execute cell 0 only
    #[command(alias = "nb", alias = "literate")]
    Notebook(commands::notebook::NotebookArgs),

    /// Distributed scatter/gather pipeline execution
    ///
    /// Partition work across remote nodes, execute in parallel,
    /// and merge results using configurable strategies.
    ///
    /// Examples:
    ///   atp distributed 'grep "ERROR" | count' logs/ --hosts n1,n2,n3
    ///   atp distributed 'grep "ERROR" | freq' . --hosts n1,n2 -m merge-freq
    ///   atp distributed 'find "TODO"' src/ --hosts n1,n2 --dry-run
    #[command(alias = "dist", alias = "scatter")]
    Distributed(commands::distributed::DistributedArgs),
}

fn main() {
    let cli = Cli::parse();

    // Determine output format: auto = json if piped, human if TTY
    // Exception: query command defaults to human even when piped,
    // so shell pipes work naturally: atp query '...' | atp query '...'
    // Use --format json explicitly for agent/JSON output.
    let format_str = if cli.format == "auto" {
        if atty_is_stdout() {
            "human"
        } else {
            match &cli.command {
                Commands::Query(_) => "human",
                _ => "json",
            }
        }
    } else {
        &cli.format
    };

    let format: atp_core::Format = match format_str.parse() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let result = match cli.command {
        Commands::Search(args) => commands::search::execute(args, format, cli.raw, cli.no_color),
        Commands::Transform(args) => commands::transform::execute(args, format, cli.raw),
        Commands::Analyze(args) => commands::analyze::execute(args, format, cli.raw),
        Commands::Pipeline(args) => commands::pipeline::execute(args, format, cli.raw),
        Commands::Query(args) => commands::query::execute(args, format, cli.raw),
        Commands::Ontology(args) => commands::ontology::execute(args, format),
        Commands::Explain(args) => commands::explain::execute(args, format),
        Commands::Scope(args) => commands::scope::execute(args, format, cli.raw),
        Commands::Validate(args) => commands::validate::execute(args, format),
        Commands::Context(args) => commands::context::execute(args, format, cli.raw),
        Commands::Compliance(args) => commands::compliance::execute(args, format, cli.raw),
        Commands::Repl(args) => commands::repl::execute(args),
        Commands::Watch(args) => commands::watch::execute(args, format, cli.raw),
        Commands::Completions { shell } => commands::completions::execute(&shell),
        Commands::Manpage { dir } => commands::manpage::execute(dir.as_deref()),
        Commands::Config(args) => commands::config::execute(&args),
        Commands::Mcp(args) => commands::mcp::execute(&args),
        Commands::Ai(args) => commands::ai::execute(args),
        Commands::Remote(args) => commands::remote::execute(args),
        Commands::Symbols(args) => commands::symbols::execute(args, format),
        Commands::Plugin(args) => commands::plugin::execute(args),
        Commands::Index(args) => commands::index::execute(args, format, cli.raw),
        Commands::Debug(args) => commands::debug::execute(args, format, cli.raw),
        Commands::Notebook(args) => commands::notebook::execute(args, format, cli.raw),
        Commands::Distributed(args) => commands::distributed::execute(args, format, cli.raw),
    };

    if let Err(e) = result {
        let error_output = atp_core::output::AtpError {
            code: atp_core::output::ErrorCode::InvalidArgument,
            message: format!("{e:#}"),
            context: None,
            suggestion: Some("Run 'atp ontology' for complete command documentation.".into()),
        };
        match format {
            atp_core::Format::Human => eprintln!("Error: {e:#}"),
            _ => {
                if let Ok(json) = serde_json::to_string_pretty(&error_output) {
                    eprintln!("{json}");
                } else {
                    eprintln!("Error: {e:#}");
                }
            }
        }
        std::process::exit(1);
    }
}

/// Check if stdout is a terminal (for auto-format detection).
fn atty_is_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

#[cfg(test)]
mod ontology_completeness {
    use clap::CommandFactory;

    /// Every subcommand atp accepts has an ontology spec.
    ///
    /// Walks clap's own parsed `Command` rather than scanning text. This test
    /// exists because the ontology described eleven of seventeen subcommands
    /// while being the document an agent reads to decide what atp can do --
    /// `config`, which changes the defaults every later command runs under,
    /// and `mcp`, which re-exposes the whole tool surface over JSON-RPC, were
    /// both absent.
    ///
    /// The compatibility shims (atp-grep, atp-sed, atp-awk) are separate
    /// binaries rather than subcommands, so they are declared in the ontology
    /// without appearing in this list. That is the one asymmetry, and it is
    /// deliberate.
    #[test]
    fn every_subcommand_has_a_spec() {
        let ontology = atp_core::ontology::build_ontology();
        let declared: std::collections::HashSet<String> =
            ontology.commands.iter().map(|c| c.name.clone()).collect();

        let command = super::Cli::command();
        let missing: Vec<String> = command
            .get_subcommands()
            .map(|s| s.get_name().to_string())
            .filter(|name| !declared.contains(name))
            .collect();

        assert!(
            missing.is_empty(),
            "these subcommands exist but have no ontology spec: {missing:?}"
        );
    }

    /// The commands that rewrite files say so.
    ///
    /// `modifies_files` is the field an agent checks before letting atp near a
    /// working tree, so the claim is pinned rather than left to drift.
    #[test]
    fn rewriting_commands_declare_it() {
        let ontology = atp_core::ontology::build_ontology();
        for name in ["transform", "config"] {
            let spec = ontology
                .commands
                .iter()
                .find(|c| c.name == name)
                .unwrap_or_else(|| panic!("`{name}` is missing from the ontology"));
            assert!(
                spec.modifies_files,
                "`{name}` writes but does not declare modifies_files"
            );
        }
    }

    /// Reading never claims to write.
    ///
    /// The guard against the opposite failure: marking everything as
    /// modifying would satisfy the test above while making the field useless.
    #[test]
    fn reading_commands_do_not_claim_to_write() {
        let ontology = atp_core::ontology::build_ontology();
        for name in ["search", "analyze", "validate", "explain", "ontology"] {
            if let Some(spec) = ontology.commands.iter().find(|c| c.name == name) {
                assert!(
                    !spec.modifies_files,
                    "`{name}` only reads but declares modifies_files"
                );
            }
        }
    }
}
