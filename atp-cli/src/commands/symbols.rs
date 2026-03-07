//! CLI command: `atp symbols` — code intelligence and structural search.

use anyhow::Result;
use atp_core::code_intel::{CodeIntelligence, Language, StructuralQuery, SymbolKind};
use atp_core::output::Format;
use clap::Args;

#[derive(Args, Debug)]
pub struct SymbolsArgs {
    /// File(s) or directory to analyze
    pub paths: Vec<String>,

    /// Filter by symbol kind: function, method, class, struct, enum, trait, module, constant, variable, import, type, interface
    #[arg(long, short = 'k')]
    pub kind: Option<String>,

    /// Filter by name pattern (regex)
    #[arg(long, short = 'n')]
    pub name: Option<String>,

    /// Filter by visibility (e.g., "pub")
    #[arg(long)]
    pub visibility: Option<String>,

    /// Filter by doc comment contents
    #[arg(long)]
    pub doc: Option<String>,

    /// Filter by language (e.g., "rust", "python", "typescript")
    #[arg(long, short = 'l')]
    pub language: Option<String>,

    /// Show only symbol names (one per line)
    #[arg(long)]
    pub names_only: bool,

    /// Recurse into directories
    #[arg(long, short = 'r')]
    pub recursive: bool,
}

fn parse_kind(s: &str) -> Option<SymbolKind> {
    match s.to_lowercase().as_str() {
        "function" | "fn" | "func" => Some(SymbolKind::Function),
        "method" => Some(SymbolKind::Method),
        "class" => Some(SymbolKind::Class),
        "struct" => Some(SymbolKind::Struct),
        "enum" => Some(SymbolKind::Enum),
        "interface" => Some(SymbolKind::Interface),
        "trait" => Some(SymbolKind::Trait),
        "module" | "mod" => Some(SymbolKind::Module),
        "constant" | "const" => Some(SymbolKind::Constant),
        "variable" | "var" => Some(SymbolKind::Variable),
        "import" | "use" => Some(SymbolKind::Import),
        "type" => Some(SymbolKind::Type),
        _ => None,
    }
}

fn parse_language(s: &str) -> Option<Language> {
    match s.to_lowercase().as_str() {
        "rust" | "rs" => Some(Language::Rust),
        "python" | "py" => Some(Language::Python),
        "javascript" | "js" => Some(Language::JavaScript),
        "typescript" | "ts" => Some(Language::TypeScript),
        "go" => Some(Language::Go),
        "java" => Some(Language::Java),
        "c" => Some(Language::C),
        "c++" | "cpp" => Some(Language::Cpp),
        "ruby" | "rb" => Some(Language::Ruby),
        "shell" | "sh" | "bash" => Some(Language::Shell),
        _ => None,
    }
}

pub fn execute(args: SymbolsArgs, format: Format) -> Result<()> {
    let ci = CodeIntelligence::new();

    // Collect files
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for path_str in &args.paths {
        let path = std::path::Path::new(path_str);
        if path.is_dir() {
            if args.recursive {
                for entry in walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        let lang = Language::from_path(entry.path());
                        if lang != Language::Unknown {
                            files.push(entry.path().to_path_buf());
                        }
                    }
                }
            } else {
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    if entry.file_type()?.is_file() {
                        let lang = Language::from_path(&entry.path());
                        if lang != Language::Unknown {
                            files.push(entry.path());
                        }
                    }
                }
            }
        } else if path.is_file() {
            files.push(path.to_path_buf());
        }
    }

    // Build query
    let query = StructuralQuery {
        kind: args.kind.as_deref().and_then(parse_kind),
        name_pattern: args.name.clone(),
        language: args.language.as_deref().and_then(parse_language),
        visibility: args.visibility.clone(),
        doc_contains: args.doc.clone(),
    };

    // Extract and filter symbols
    let mut all_symbols = Vec::new();
    for file in &files {
        if let Ok(source) = std::fs::read_to_string(file) {
            let file_str = file.to_string_lossy().to_string();
            let symbols = ci.query_symbols(&source, &file_str, &query);
            all_symbols.extend(symbols);
        }
    }

    // Output
    if args.names_only {
        for sym in &all_symbols {
            println!("{}", sym.name);
        }
    } else {
        match format {
            Format::Human => {
                for sym in &all_symbols {
                    println!(
                        "{:>10} {:20} {}:{}",
                        format!("{:?}", sym.kind).to_lowercase(),
                        sym.name,
                        sym.file,
                        sym.start_line,
                    );
                }
                eprintln!("\n{} symbols found", all_symbols.len());
            }
            _ => {
                let json = serde_json::to_string_pretty(&all_symbols)?;
                println!("{json}");
            }
        }
    }

    Ok(())
}
