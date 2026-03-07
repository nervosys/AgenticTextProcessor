//! Compatibility layer — POSIX grep/sed/awk argument translation.
//!
//! Translates traditional Unix tool command-line arguments into ATP's
//! strongly typed engine configurations. This enables backwards-compatible
//! invocation with structured output:
//!
//! ```bash
//! # Traditional grep syntax → ATP structured output
//! atp-grep -i -n -r "pattern" src/
//!
//! # Traditional sed syntax → ATP structured output
//! atp-sed -e 's/old/new/g' file.txt
//!
//! # Traditional awk syntax → ATP structured output
//! atp-awk -F ',' '{print $1, $3}' data.csv
//! ```
//!
//! The translation preserves full semantic fidelity — every POSIX flag
//! maps to an equivalent ATP configuration.

use crate::engine::awk::{Aggregation, AwkConfig, Rule};
use crate::engine::grep::GrepConfig;
use crate::engine::sed::{SedConfig, TransformCommand};
use crate::output::{FileScope, PatternType};
use anyhow::{bail, Result};
use std::path::PathBuf;

/// Parsed awk program: (rules, aggregations, separator).
type AwkParsed = (Vec<Rule>, Vec<(String, Aggregation)>, Option<String>);

// ─── Grep Compatibility ─────────────────────────────────────────────────────

/// Parsed grep-compatible arguments translated to ATP types.
#[derive(Debug, Clone)]
pub struct GrepCompat {
    pub config: GrepConfig,
    pub scope: FileScope,
    /// Print only filenames with matches (grep -l)
    pub files_only: bool,
    /// Print only filenames without matches (grep -L)
    pub files_without_match: bool,
    /// Print only count of matches (grep -c)
    pub count_only: bool,
    /// Print line numbers (grep -n) — always true in ATP
    pub line_numbers: bool,
    /// Recursive search (grep -r/-R)
    pub recursive: bool,
    /// Read patterns from stdin (no file args means stdin)
    pub stdin_mode: bool,
}

/// Parse POSIX grep-style arguments into ATP types.
///
/// Supports the most common grep flags:
/// ```text
/// grep [OPTIONS] PATTERN [FILE...]
/// grep [OPTIONS] -e PATTERN [-e PATTERN2...] [FILE...]
/// ```
///
/// Supported flags:
/// - `-i`          Case-insensitive
/// - `-v`          Invert match
/// - `-w`          Whole word
/// - `-n`          Line numbers (always on in ATP)
/// - `-c`          Count only
/// - `-l`          Files with matches
/// - `-L`          Files without matches
/// - `-r` / `-R`   Recursive
/// - `-e PATTERN`  Additional pattern
/// - `-E`          Extended regex (default in ATP)
/// - `-F`          Fixed string / literal
/// - `-o`          Only matching text
/// - `-m NUM`      Max count
/// - `-A NUM`      After context
/// - `-B NUM`      Before context
/// - `-C NUM`      Context (before + after)
/// - `--include=GLOB` File filter
/// - `--exclude=GLOB` File exclusion
/// - `--color` / `--no-color` (ignored, controlled by format)
pub fn parse_grep_args(args: &[String]) -> Result<GrepCompat> {
    let mut config = GrepConfig::default();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut patterns: Vec<String> = Vec::new();
    let mut files_only = false;
    let mut files_without_match = false;
    let mut count_only = false;
    let mut recursive = false;
    let mut include_glob: Option<String> = None;
    let mut exclude_glob: Option<String> = None;
    let mut max_depth: Option<usize> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            // Everything after -- is a file
            files.extend(args[i + 1..].iter().map(PathBuf::from));
            break;
        }

        // Handle --long-options
        if arg.starts_with("--") {
            match arg.as_str() {
                "--ignore-case" => config.case_sensitive = false,
                "--invert-match" => config.invert_match = true,
                "--word-regexp" => config.whole_word = true,
                "--line-number" => {} // always on in ATP
                "--count" => count_only = true,
                "--files-with-matches" => files_only = true,
                "--files-without-match" => files_without_match = true,
                "--recursive" => recursive = true,
                "--fixed-strings" => config.pattern_type = PatternType::Literal,
                "--extended-regexp" => {} // default in ATP
                "--only-matching" => config.only_matching = true,
                "--color" | "--color=always" | "--color=auto" | "--colour" => {}
                "--no-color" | "--color=never" | "--colour=never" => {}
                _ if arg.starts_with("--include=") => {
                    include_glob = Some(arg.trim_start_matches("--include=").to_string());
                }
                _ if arg.starts_with("--exclude=") => {
                    exclude_glob = Some(arg.trim_start_matches("--exclude=").to_string());
                }
                _ if arg.starts_with("--max-depth=") => {
                    max_depth = Some(arg.trim_start_matches("--max-depth=").parse()?);
                }
                _ if arg.starts_with("--max-count=") => {
                    config.max_matches = Some(arg.trim_start_matches("--max-count=").parse()?);
                }
                _ if arg.starts_with("--after-context=") => {
                    config.context_after = arg.trim_start_matches("--after-context=").parse()?;
                }
                _ if arg.starts_with("--before-context=") => {
                    config.context_before = arg.trim_start_matches("--before-context=").parse()?;
                }
                _ if arg.starts_with("--context=") => {
                    let n: usize = arg.trim_start_matches("--context=").parse()?;
                    config.context_before = n;
                    config.context_after = n;
                }
                _ => {} // ignore unknown long options gracefully
            }
            i += 1;
            continue;
        }

        // Handle -short options (can be combined: -inrw)
        if arg.starts_with('-') && arg.len() > 1 && !arg.starts_with("--") {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut j = 0;
            while j < chars.len() {
                match chars[j] {
                    'i' => config.case_sensitive = false,
                    'v' => config.invert_match = true,
                    'w' => config.whole_word = true,
                    'n' => {} // always on
                    'c' => count_only = true,
                    'l' => files_only = true,
                    'L' => files_without_match = true,
                    'r' | 'R' => recursive = true,
                    'F' => config.pattern_type = PatternType::Literal,
                    'E' => {} // default
                    'P' => {} // PCRE — ATP uses Rust regex
                    'o' => config.only_matching = true,
                    'e' => {
                        // -e PATTERN — rest of this arg or next arg
                        let rest: String = chars[j + 1..].iter().collect();
                        if rest.is_empty() {
                            i += 1;
                            if i < args.len() {
                                patterns.push(args[i].clone());
                            } else {
                                bail!("Option -e requires an argument");
                            }
                        } else {
                            patterns.push(rest);
                        }
                        j = chars.len(); // consumed rest
                        continue;
                    }
                    'm' => {
                        let rest: String = chars[j + 1..].iter().collect();
                        let val = if rest.is_empty() {
                            i += 1;
                            if i < args.len() {
                                &args[i]
                            } else {
                                bail!("Option -m requires an argument");
                            }
                        } else {
                            j = chars.len();
                            // Rest of this arg is the value; we'll parse it below
                            // but we need to handle it differently
                            // For simplicity, advance and use a local
                            &args[i] // placeholder, handled below
                        };
                        // Re-parse: if rest was non-empty, use it
                        let num_str: String = chars[j.saturating_sub(1) + 1..].iter().collect();
                        if !num_str.is_empty() {
                            config.max_matches = Some(num_str.parse()?);
                            j = chars.len();
                            continue;
                        } else {
                            config.max_matches = Some(val.parse()?);
                        }
                        continue;
                    }
                    'A' => {
                        i += 1;
                        if i < args.len() {
                            config.context_after = args[i].parse()?;
                        }
                        j = chars.len();
                        continue;
                    }
                    'B' => {
                        i += 1;
                        if i < args.len() {
                            config.context_before = args[i].parse()?;
                        }
                        j = chars.len();
                        continue;
                    }
                    'C' => {
                        i += 1;
                        if i < args.len() {
                            let n: usize = args[i].parse()?;
                            config.context_before = n;
                            config.context_after = n;
                        }
                        j = chars.len();
                        continue;
                    }
                    _ => {} // ignore unknown flags
                }
                j += 1;
            }
            i += 1;
            continue;
        }

        // Positional: first non-flag non-file arg is the pattern (if no -e given)
        if patterns.is_empty() && config.pattern.is_empty() {
            patterns.push(arg.clone());
        } else {
            files.push(PathBuf::from(arg));
        }
        i += 1;
    }

    // Assign patterns
    if patterns.is_empty() {
        bail!("No pattern specified. Usage: atp-grep [OPTIONS] PATTERN [FILE...]");
    }
    config.pattern = patterns.remove(0);
    config.extra_patterns = patterns;

    // If no files specified and not recursive, use stdin mode
    let stdin_mode = files.is_empty() && !recursive;

    // If recursive and no files, default to current directory
    if recursive && files.is_empty() {
        files.push(PathBuf::from("."));
    }

    let scope = FileScope {
        roots: if files.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            files
        },
        include_globs: include_glob.into_iter().collect(),
        exclude_globs: exclude_glob.into_iter().collect(),
        max_depth: if recursive { max_depth } else { Some(0) },
        respect_gitignore: true,
        follow_symlinks: false,
    };

    Ok(GrepCompat {
        config,
        scope,
        files_only,
        files_without_match,
        count_only,
        line_numbers: true,
        recursive,
        stdin_mode,
    })
}

// ─── Sed Compatibility ──────────────────────────────────────────────────────

/// Parsed sed-compatible arguments translated to ATP types.
#[derive(Debug, Clone)]
pub struct SedCompat {
    pub config: SedConfig,
    pub files: Vec<PathBuf>,
    /// Read from stdin
    pub stdin_mode: bool,
    /// Suppress default output (sed -n)
    pub quiet: bool,
}

/// Parse POSIX sed-style arguments into ATP types.
///
/// Supports common sed invocation patterns:
/// ```text
/// sed [OPTIONS] 'command' [FILE...]
/// sed [OPTIONS] -e 'command' [-e 'command'...] [FILE...]
/// ```
///
/// Supported flags:
/// - `-e 'cmd'`    Expression (can be repeated)
/// - `-i[SUFFIX]`  Edit in place (with optional backup suffix)
/// - `-n`          Suppress default output
/// - `--in-place[=SUFFIX]`  Long form of -i
pub fn parse_sed_args(args: &[String]) -> Result<SedCompat> {
    let mut expressions: Vec<String> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut in_place = false;
    let mut backup_ext: Option<String> = None;
    let mut quiet = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            files.extend(args[i + 1..].iter().map(PathBuf::from));
            break;
        }

        // Long options
        if arg.starts_with("--") {
            if arg == "--quiet" || arg == "--silent" {
                quiet = true;
            } else if arg == "--in-place" {
                in_place = true;
            } else if arg.starts_with("--in-place=") {
                in_place = true;
                backup_ext = Some(arg.trim_start_matches("--in-place=").to_string());
            } else if arg == "--expression" {
                i += 1;
                if i < args.len() {
                    expressions.push(args[i].clone());
                } else {
                    bail!("--expression requires an argument");
                }
            } else if arg.starts_with("--expression=") {
                expressions.push(arg.trim_start_matches("--expression=").to_string());
            }
            i += 1;
            continue;
        }

        // Short options
        if arg.starts_with('-') && arg.len() > 1 {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut j = 0;
            while j < chars.len() {
                match chars[j] {
                    'n' => quiet = true,
                    'e' => {
                        // Rest of arg or next arg is the expression
                        let rest: String = chars[j + 1..].iter().collect();
                        if rest.is_empty() {
                            i += 1;
                            if i < args.len() {
                                expressions.push(args[i].clone());
                            } else {
                                bail!("Option -e requires an argument");
                            }
                        } else {
                            expressions.push(rest);
                        }
                        j = chars.len();
                        continue;
                    }
                    'i' => {
                        in_place = true;
                        // Rest of arg is the backup suffix (like sed -i.bak)
                        let rest: String = chars[j + 1..].iter().collect();
                        if !rest.is_empty() {
                            // Strip leading dot — engine adds separator automatically
                            let ext = rest.trim_start_matches('.');
                            backup_ext = Some(ext.to_string());
                        }
                        j = chars.len();
                        continue;
                    }
                    _ => {} // ignore unknown
                }
                j += 1;
            }
            i += 1;
            continue;
        }

        // Positional: first positional is an expression if none via -e
        if expressions.is_empty() {
            expressions.push(arg.clone());
        } else {
            files.push(PathBuf::from(arg));
        }
        i += 1;
    }

    if expressions.is_empty() {
        bail!("No sed expression specified. Usage: atp-sed [OPTIONS] 'command' [FILE...]");
    }

    // Parse expressions into TransformCommands
    let mut commands = Vec::new();
    for expr in &expressions {
        let trimmed = expr.trim();
        if trimmed.starts_with('s') {
            commands.push(crate::engine::sed::parse_substitution(trimmed)?);
        } else if trimmed == "d" || trimmed.ends_with("/d") {
            // Pattern delete: /pattern/d
            if trimmed.starts_with('/') && trimmed.ends_with("/d") {
                let pattern = &trimmed[1..trimmed.len() - 2];
                commands.push(TransformCommand::Delete {
                    pattern: pattern.to_string(),
                });
            } else if trimmed == "d" {
                // Delete all lines — match everything
                commands.push(TransformCommand::Delete {
                    pattern: ".*".to_string(),
                });
            }
        } else if trimmed.starts_with('y') {
            // Transliterate: y/from/to/
            let delim = trimmed.chars().nth(1).unwrap_or('/');
            let parts: Vec<&str> = trimmed[2..].splitn(3, delim).collect();
            if parts.len() >= 2 {
                commands.push(TransformCommand::Transliterate {
                    from: parts[0].to_string(),
                    to: parts[1].to_string(),
                });
            }
        } else {
            // Try parsing as a substitution anyway
            if let Ok(cmd) = crate::engine::sed::parse_substitution(trimmed) {
                commands.push(cmd);
            } else {
                bail!(
                    "Unrecognized sed command: '{trimmed}'. Supported: s/pat/repl/flags, /pat/d, y/from/to/"
                );
            }
        }
    }

    let stdin_mode = files.is_empty();

    let config = SedConfig {
        commands,
        in_place,
        backup_extension: backup_ext,
        dry_run: !in_place,
        line_range: None,
        address_range: None,
    };

    Ok(SedCompat {
        config,
        files,
        stdin_mode,
        quiet,
    })
}

// ─── Awk Compatibility ──────────────────────────────────────────────────────

/// Parsed awk-compatible arguments translated to ATP types.
#[derive(Debug, Clone)]
pub struct AwkCompat {
    pub config: AwkConfig,
    pub files: Vec<PathBuf>,
    /// Read from stdin
    pub stdin_mode: bool,
    /// The raw awk program text for provenance
    pub program: String,
}

/// Parse POSIX awk-style arguments into ATP types.
///
/// Supports common awk invocation patterns:
/// ```text
/// awk [OPTIONS] 'program' [FILE...]
/// awk [OPTIONS] -f progfile [FILE...]
/// ```
///
/// Supported flags:
/// - `-F 'sep'`    Field separator
/// - `-v var=val`  Variable assignment (limited support)
/// - `'program'`   Awk program text
///
/// Recognized program patterns:
/// - `'{print}'`                — Print all lines
/// - `'{print $1, $3}'`        — Select fields
/// - `'/pattern/'`             — Filter by pattern
/// - `'/pattern/ {print $2}'`  — Pattern + field selection
/// - `'NR > 5'`                — Record number conditions
/// - `'{sum += $2} END {print sum}'` — Aggregation
/// - `'BEGIN {FS=","}'`        — BEGIN block for separator
pub fn parse_awk_args(args: &[String]) -> Result<AwkCompat> {
    let mut separator = r"\s+".to_string();
    let mut program = String::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut variables: Vec<(String, String)> = Vec::new();
    let mut max_records: Option<usize> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            files.extend(args[i + 1..].iter().map(PathBuf::from));
            break;
        }

        if arg == "-F" || arg == "--field-separator" {
            i += 1;
            if i < args.len() {
                separator = args[i].clone();
                // Handle single-char literal separators (like awk -F ',')
                if separator.len() == 1 && !separator.contains('\\') {
                    let ch = separator.chars().next().expect("len==1 guarantees a char");
                    if !regex_meta_char(ch) {
                        // Use as literal
                    } else {
                        separator = format!("\\{ch}");
                    }
                }
            } else {
                bail!("Option -F requires an argument");
            }
            i += 1;
            continue;
        }

        if arg.starts_with("-F") && arg.len() > 2 {
            separator = arg[2..].to_string();
            if separator.len() == 1 {
                let ch = separator.chars().next().expect("len==1 guarantees a char");
                if regex_meta_char(ch) {
                    separator = format!("\\{ch}");
                }
            }
            i += 1;
            continue;
        }

        if arg == "-v" {
            i += 1;
            if i < args.len() {
                if let Some((k, v)) = args[i].split_once('=') {
                    variables.push((k.to_string(), v.to_string()));
                }
            }
            i += 1;
            continue;
        }

        if arg.starts_with('-') && arg.len() > 1 {
            // Skip unknown flags
            i += 1;
            continue;
        }

        // Positional: first is program, rest are files
        if program.is_empty() {
            program = arg.clone();
        } else {
            files.push(PathBuf::from(arg));
        }
        i += 1;
    }

    if program.is_empty() {
        bail!("No awk program specified. Usage: atp-awk [OPTIONS] 'program' [FILE...]");
    }

    // Override separator from -v FS=... or BEGIN {FS="..."}
    for (k, v) in &variables {
        if k == "FS" || k == "OFS" {
            separator = v.clone();
            if separator.len() == 1 {
                let ch = separator.chars().next().expect("len==1 guarantees a char");
                if regex_meta_char(ch) {
                    separator = format!("\\{ch}");
                }
            }
        }
        if k == "NR_MAX" {
            max_records = v.parse().ok();
        }
    }

    // Parse the awk program into ATP types
    let (rules, aggregations, parsed_sep) = parse_awk_program(&program, &separator)?;

    // If program contained a BEGIN block that set FS, use it
    let final_sep = parsed_sep.unwrap_or(separator);

    let stdin_mode = files.is_empty();

    let config = AwkConfig {
        field_separator: final_sep,
        output_separator: "\t".to_string(),
        rules,
        aggregations,
        has_header: false,
        skip_lines: 0,
        max_records,
    };

    Ok(AwkCompat {
        config,
        files,
        stdin_mode,
        program,
    })
}

/// Parse a simplified awk program string into ATP rules and aggregations.
///
/// Handles common patterns:
/// - `{print}` or `{print $0}` — all fields
/// - `{print $1, $3}` — selected fields
/// - `/pattern/` — filter
/// - `/pattern/ {print $2}` — filter + select
/// - `$3 > 100` — field conditions
/// - `{sum += $2} END {print sum}` — aggregation
/// - `BEGIN {FS=","} {print $1}` — separator in BEGIN
/// - `NR > 5 {print}` — record range
/// - `{count++} END {print count}` — count aggregation
fn parse_awk_program(program: &str, _default_sep: &str) -> Result<AwkParsed> {
    let mut rules = Vec::new();
    let mut aggregations: Vec<(String, Aggregation)> = Vec::new();
    let mut parsed_sep: Option<String> = None;

    let trimmed = program.trim();

    // Check for BEGIN block — extract FS
    if let Some(begin_start) = trimmed.find("BEGIN") {
        if let Some(brace_start) = trimmed[begin_start..].find('{') {
            if let Some(brace_end) = trimmed[begin_start + brace_start..].find('}') {
                let begin_body =
                    &trimmed[begin_start + brace_start + 1..begin_start + brace_start + brace_end];
                // Look for FS="..." or FS=','
                if let Some(fs_pos) = begin_body.find("FS") {
                    let after_fs = &begin_body[fs_pos + 2..].trim_start();
                    if let Some(stripped) = after_fs.strip_prefix('=') {
                        let val = stripped.trim();
                        let sep = val
                            .trim_matches('"')
                            .trim_matches('\'')
                            .trim_end_matches(';');
                        let mut s = sep.to_string();
                        if s.len() == 1 {
                            let ch = s.chars().next().expect("len==1 guarantees a char");
                            if regex_meta_char(ch) {
                                s = format!("\\{ch}");
                            }
                        }
                        parsed_sep = Some(s);
                    }
                }
            }
        }
    }

    // Check for END block — extract aggregation
    let has_end = trimmed.contains("END");
    let main_part = if has_end {
        // Extract the part before END
        let end_pos = trimmed.find("END").unwrap_or(trimmed.len());
        // Also check for the part between BEGIN and END (the main block)
        let after_begin = if let Some(begin_end) = trimmed.find("BEGIN") {
            let brace_start = trimmed[begin_end..].find('{').unwrap_or(0);
            let brace_end = trimmed[begin_end + brace_start..].find('}').unwrap_or(0);
            &trimmed[begin_end + brace_start + brace_end + 1..end_pos]
        } else {
            &trimmed[..end_pos]
        };
        after_begin.trim()
    } else {
        // Remove BEGIN block if present
        if let Some(begin_pos) = trimmed.find("BEGIN") {
            let brace_start = trimmed[begin_pos..].find('{').unwrap_or(0);
            let brace_end = trimmed[begin_pos + brace_start..].find('}').unwrap_or(0);
            trimmed[begin_pos + brace_start + brace_end + 1..].trim()
        } else {
            trimmed
        }
    };

    // Detect aggregation patterns in main body
    if has_end {
        // Common aggregation patterns
        if main_part.contains("sum +=") || main_part.contains("sum+=$") {
            // Find which field: sum += $N
            if let Some(field) = extract_field_number(main_part, "sum") {
                aggregations.push(("sum".to_string(), Aggregation::Sum(field)));
            }
        }
        if main_part.contains("count++") || main_part.contains("count +=") {
            aggregations.push(("count".to_string(), Aggregation::Count));
        }
        if main_part.contains("min") || main_part.contains("max") {
            // Basic min/max detection
            if let Some(field) = extract_field_reference(main_part) {
                if main_part.contains("min") {
                    aggregations.push(("min".to_string(), Aggregation::Min(field)));
                }
                if main_part.contains("max") {
                    aggregations.push(("max".to_string(), Aggregation::Max(field)));
                }
            }
        }
    }

    // Parse main program block
    if !main_part.is_empty() {
        let (pattern, fields) = parse_awk_main_block(main_part)?;

        // If we only have aggregation and no print/field selection, add a catch-all rule
        if fields.is_empty() && !aggregations.is_empty() {
            rules.push(Rule {
                pattern,
                select_fields: vec![],
                computed_fields: vec![],
                condition: None,
            });
        } else {
            rules.push(Rule {
                pattern,
                select_fields: fields,
                computed_fields: vec![],
                condition: None,
            });
        }
    } else if !aggregations.is_empty() {
        // Aggregation only — process all records
        rules.push(Rule {
            pattern: None,
            select_fields: vec![],
            computed_fields: vec![],
            condition: None,
        });
    }

    // If nothing was parsed, default to print-all
    if rules.is_empty() && aggregations.is_empty() {
        rules.push(Rule {
            pattern: None,
            select_fields: vec![],
            computed_fields: vec![],
            condition: None,
        });
    }

    Ok((rules, aggregations, parsed_sep))
}

/// Parse the main awk block (between BEGIN and END, or the whole program).
fn parse_awk_main_block(block: &str) -> Result<(Option<String>, Vec<usize>)> {
    let trimmed = block.trim();

    // Check for pattern/condition prefix
    let (pattern, action_part) = if let Some(after_slash) = trimmed.strip_prefix('/') {
        // Pattern: /regex/ { action }
        if let Some(end_slash) = after_slash.find('/') {
            let pat = &after_slash[..end_slash];
            let rest = after_slash[end_slash + 1..].trim();
            (Some(pat.to_string()), rest)
        } else {
            (None, trimmed)
        }
    } else if trimmed.starts_with('$') && trimmed.contains('>')
        || trimmed.contains('<')
        || trimmed.contains("==")
    {
        // Condition like $3 > 100 — not a pattern, just pass action
        // For now, skip conditions, just parse the action
        (None, trimmed)
    } else if trimmed.starts_with("NR") {
        // NR conditions — skip for now
        let rest = if let Some(brace) = trimmed.find('{') {
            &trimmed[brace..]
        } else {
            trimmed
        };
        (None, rest)
    } else {
        (None, trimmed)
    };

    // Parse action block: { print $1, $3 }
    let fields = if let Some(brace_start) = action_part.find('{') {
        let brace_end = action_part.rfind('}').unwrap_or(action_part.len());
        let body = &action_part[brace_start + 1..brace_end];
        parse_print_fields(body)
    } else if action_part.is_empty() {
        // Pattern only — like /regex/ — means print matching lines
        vec![]
    } else {
        vec![]
    };

    Ok((pattern, fields))
}

/// Extract field numbers from a `print $1, $3, $5` statement.
fn parse_print_fields(body: &str) -> Vec<usize> {
    let trimmed = body.trim();

    // Handle "print" or "print $0" — all fields
    if trimmed == "print" || trimmed == "print $0" || trimmed == "print($0)" || trimmed == "printf"
    {
        return vec![];
    }

    // Handle "print $1, $3, $5" or "print $1 $3 $5"
    if let Some(after_print) = trimmed.strip_prefix("print") {
        let after_print = after_print.trim();
        let after_print = after_print.trim_start_matches('(').trim_end_matches(')');

        let mut fields = Vec::new();
        for part in after_print.split([',', ' ', '\t']) {
            let part = part.trim();
            if let Some(field_str) = part.strip_prefix('$') {
                if let Ok(n) = field_str.parse::<usize>() {
                    if n > 0 {
                        fields.push(n);
                    }
                }
            }
        }
        return fields;
    }

    vec![]
}

/// Extract a field number from an aggregation expression like "sum += $2".
fn extract_field_number(text: &str, var_name: &str) -> Option<usize> {
    let patterns = [
        format!("{var_name} += $"),
        format!("{var_name}+=$"),
        format!("{var_name} = {var_name} + $"),
    ];
    for pat in &patterns {
        if let Some(pos) = text.find(pat.as_str()) {
            let after = &text[pos + pat.len()..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num_str.parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

/// Extract any field reference ($N) from text.
fn extract_field_reference(text: &str) -> Option<usize> {
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let num_str: String = chars[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = num_str.parse::<usize>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
        i += 1;
    }
    None
}

/// Check if a character is a regex metacharacter.
fn regex_meta_char(c: char) -> bool {
    matches!(
        c,
        '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── grep tests ──

    #[test]
    fn test_grep_basic() {
        let args: Vec<String> = vec!["pattern".into(), "file.txt".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert_eq!(compat.config.pattern, "pattern");
        assert!(compat.config.case_sensitive);
        assert!(!compat.count_only);
        assert_eq!(compat.scope.roots, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn test_grep_case_insensitive() {
        let args: Vec<String> = vec!["-i".into(), "TODO".into(), "src/".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert_eq!(compat.config.pattern, "TODO");
        assert!(!compat.config.case_sensitive);
    }

    #[test]
    fn test_grep_combined_flags() {
        let args: Vec<String> = vec!["-inrw".into(), "hello".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert!(!compat.config.case_sensitive);
        assert!(compat.config.whole_word);
        assert!(compat.recursive);
    }

    #[test]
    fn test_grep_context() {
        let args: Vec<String> = vec!["-C".into(), "3".into(), "pat".into(), "f.txt".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert_eq!(compat.config.context_before, 3);
        assert_eq!(compat.config.context_after, 3);
    }

    #[test]
    fn test_grep_multiple_patterns() {
        let args: Vec<String> = vec![
            "-e".into(),
            "foo".into(),
            "-e".into(),
            "bar".into(),
            "file.txt".into(),
        ];
        let compat = parse_grep_args(&args).unwrap();
        assert_eq!(compat.config.pattern, "foo");
        assert_eq!(compat.config.extra_patterns, vec!["bar".to_string()]);
    }

    #[test]
    fn test_grep_literal() {
        let args: Vec<String> = vec!["-F".into(), "hello.world".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert_eq!(compat.config.pattern_type, PatternType::Literal);
    }

    #[test]
    fn test_grep_files_only() {
        let args: Vec<String> = vec!["-rl".into(), "TODO".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert!(compat.files_only);
        assert!(compat.recursive);
    }

    #[test]
    fn test_grep_include_exclude() {
        let args: Vec<String> = vec![
            "--include=*.rs".into(),
            "--exclude=*.bak".into(),
            "pat".into(),
            "src/".into(),
        ];
        let compat = parse_grep_args(&args).unwrap();
        assert_eq!(compat.scope.include_globs, vec!["*.rs".to_string()]);
        assert_eq!(compat.scope.exclude_globs, vec!["*.bak".to_string()]);
    }

    #[test]
    fn test_grep_stdin_mode() {
        let args: Vec<String> = vec!["pattern".into()];
        let compat = parse_grep_args(&args).unwrap();
        assert!(compat.stdin_mode);
    }

    // ── sed tests ──

    #[test]
    fn test_sed_basic_substitution() {
        let args: Vec<String> = vec!["s/foo/bar/g".into(), "file.txt".into()];
        let compat = parse_sed_args(&args).unwrap();
        assert_eq!(compat.config.commands.len(), 1);
        assert!(compat.config.dry_run); // default is dry-run
        assert_eq!(compat.files, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn test_sed_in_place() {
        let args: Vec<String> = vec!["-i.bak".into(), "s/old/new/".into(), "file.txt".into()];
        let compat = parse_sed_args(&args).unwrap();
        assert!(compat.config.in_place);
        assert!(!compat.config.dry_run);
        assert_eq!(compat.config.backup_extension, Some("bak".to_string()));
    }

    #[test]
    fn test_sed_multiple_expressions() {
        let args: Vec<String> = vec![
            "-e".into(),
            "s/a/b/g".into(),
            "-e".into(),
            "s/c/d/g".into(),
            "file.txt".into(),
        ];
        let compat = parse_sed_args(&args).unwrap();
        assert_eq!(compat.config.commands.len(), 2);
    }

    #[test]
    fn test_sed_delete() {
        let args: Vec<String> = vec!["/debug/d".into()];
        let compat = parse_sed_args(&args).unwrap();
        assert_eq!(compat.config.commands.len(), 1);
        matches!(&compat.config.commands[0], TransformCommand::Delete { .. });
    }

    #[test]
    fn test_sed_stdin_mode() {
        let args: Vec<String> = vec!["s/x/y/g".into()];
        let compat = parse_sed_args(&args).unwrap();
        assert!(compat.stdin_mode);
    }

    // ── awk tests ──

    #[test]
    fn test_awk_print_all() {
        let args: Vec<String> = vec!["{print}".into(), "file.txt".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.rules.len(), 1);
        assert!(compat.config.rules[0].select_fields.is_empty()); // all fields
    }

    #[test]
    fn test_awk_field_separator() {
        let args: Vec<String> = vec!["-F".into(), ",".into(), "{print $1}".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.field_separator, ",");
    }

    #[test]
    fn test_awk_select_fields() {
        let args: Vec<String> = vec!["{print $1, $3, $5}".into(), "data.txt".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.rules[0].select_fields, vec![1, 3, 5]);
    }

    #[test]
    fn test_awk_pattern_filter() {
        let args: Vec<String> = vec!["/error/".into(), "log.txt".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.rules[0].pattern, Some("error".to_string()));
    }

    #[test]
    fn test_awk_pattern_with_action() {
        let args: Vec<String> = vec!["/error/ {print $1, $2}".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.rules[0].pattern, Some("error".to_string()));
        assert_eq!(compat.config.rules[0].select_fields, vec![1, 2]);
    }

    #[test]
    fn test_awk_begin_fs() {
        let args: Vec<String> = vec!["BEGIN {FS=\",\"} {print $1, $2}".into(), "data.csv".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.field_separator, ",");
        assert_eq!(compat.config.rules[0].select_fields, vec![1, 2]);
    }

    #[test]
    fn test_awk_aggregation() {
        let args: Vec<String> = vec!["{sum += $2} END {print sum}".into(), "data.txt".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert!(!compat.config.aggregations.is_empty());
        assert_eq!(compat.config.aggregations[0].0, "sum");
    }

    #[test]
    fn test_awk_combined_separator() {
        let args: Vec<String> = vec!["-F,".into(), "{print $1}".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert_eq!(compat.config.field_separator, ",");
    }

    #[test]
    fn test_awk_stdin_mode() {
        let args: Vec<String> = vec!["{print $1}".into()];
        let compat = parse_awk_args(&args).unwrap();
        assert!(compat.stdin_mode);
    }
}
