// ---------------------------------------------------------------------------
// shell.rs — Shell script analysis
// ---------------------------------------------------------------------------
//
// Shebang detection, variable/command extraction, string quoting/escaping,
// argument splitting (POSIX + Windows), portability hints.
// ---------------------------------------------------------------------------

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Detected shell from shebang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Sh,
    Zsh,
    Fish,
    Dash,
    Csh,
    Tcsh,
    Ksh,
    PowerShell,
    Cmd,
    Python,
    Ruby,
    Perl,
    Node,
    Unknown(String),
}

/// A parsed shebang line.
#[derive(Debug, Clone)]
pub struct Shebang {
    /// Full shebang line (e.g. "#!/usr/bin/env bash").
    pub line: String,
    /// Interpreter path or name.
    pub interpreter: String,
    /// Detected shell kind.
    pub kind: ShellKind,
    /// Arguments after the interpreter.
    pub args: Vec<String>,
}

/// An extracted variable reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRef {
    /// Variable name.
    pub name: String,
    /// Whether it's a definition (VAR=...) or a reference ($VAR).
    pub is_definition: bool,
    /// Line number (1-based).
    pub line: usize,
}

/// A command invocation found in the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRef {
    /// Command name.
    pub name: String,
    /// Line number (1-based).
    pub line: usize,
}

/// Portability issue.
#[derive(Debug, Clone)]
pub struct PortabilityHint {
    /// Description of the issue.
    pub message: String,
    /// Line number (1-based), if applicable.
    pub line: Option<usize>,
    /// Severity: "warning" or "info".
    pub severity: String,
}

/// Full analysis result.
#[derive(Debug, Clone)]
pub struct ShellAnalysis {
    pub shebang: Option<Shebang>,
    pub variables: Vec<VarRef>,
    pub commands: Vec<CommandRef>,
    pub hints: Vec<PortabilityHint>,
}

// ---------------------------------------------------------------------------
// Shebang
// ---------------------------------------------------------------------------

/// Parse a shebang line.
pub fn parse_shebang(line: &str) -> Option<Shebang> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#!") {
        return None;
    }
    let rest = trimmed[2..].trim();
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    // Handle /usr/bin/env <interpreter>
    let (interpreter, args) = if parts[0].ends_with("/env") && parts.len() > 1 {
        (
            parts[1].to_string(),
            parts[2..].iter().map(|s| s.to_string()).collect(),
        )
    } else {
        (
            parts[0].to_string(),
            parts[1..].iter().map(|s| s.to_string()).collect(),
        )
    };

    let basename = interpreter.rsplit('/').next().unwrap_or(&interpreter);
    let kind = match basename {
        "bash" => ShellKind::Bash,
        "sh" => ShellKind::Sh,
        "zsh" => ShellKind::Zsh,
        "fish" => ShellKind::Fish,
        "dash" => ShellKind::Dash,
        "csh" => ShellKind::Csh,
        "tcsh" => ShellKind::Tcsh,
        "ksh" => ShellKind::Ksh,
        "pwsh" | "powershell" => ShellKind::PowerShell,
        "python" | "python3" => ShellKind::Python,
        "ruby" => ShellKind::Ruby,
        "perl" => ShellKind::Perl,
        "node" | "nodejs" => ShellKind::Node,
        other => ShellKind::Unknown(other.to_string()),
    };

    Some(Shebang {
        line: trimmed.to_string(),
        interpreter,
        kind,
        args,
    })
}

// ---------------------------------------------------------------------------
// Variable extraction
// ---------------------------------------------------------------------------

/// Extract variable definitions and references from shell script text.
pub fn extract_variables(text: &str) -> Vec<VarRef> {
    let mut vars = Vec::new();
    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        // Skip comments
        if trimmed.starts_with('#') {
            continue;
        }
        // Definitions: VAR=value or export VAR=value
        let check = if let Some(stripped) = trimmed.strip_prefix("export ") {
            stripped
        } else if let Some(stripped) = trimmed.strip_prefix("local ") {
            stripped
        } else if let Some(stripped) = trimmed.strip_prefix("readonly ") {
            stripped
        } else {
            trimmed
        };
        if let Some(eq_pos) = check.find('=') {
            let name = check[..eq_pos].trim();
            if !name.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                && name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
            {
                vars.push(VarRef {
                    name: name.to_string(),
                    is_definition: true,
                    line: line_num + 1,
                });
            }
        }
        // References: $VAR or ${VAR}
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' && i + 1 < bytes.len() {
                i += 1;
                if bytes[i] == b'{' {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'}' {
                        i += 1;
                    }
                    let name = &line[start..i];
                    // Strip parameter expansion operators
                    let clean = name
                        .split(|c: char| ":-+?#%/".contains(c))
                        .next()
                        .unwrap_or(name);
                    if !clean.is_empty() && clean.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        vars.push(VarRef {
                            name: clean.to_string(),
                            is_definition: false,
                            line: line_num + 1,
                        });
                    }
                    if i < bytes.len() {
                        i += 1; // skip }
                    }
                } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                    let start = i;
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    vars.push(VarRef {
                        name: line[start..i].to_string(),
                        is_definition: false,
                        line: line_num + 1,
                    });
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }
    vars
}

// ---------------------------------------------------------------------------
// Command extraction
// ---------------------------------------------------------------------------

/// Extract command names from shell script text (simple heuristic).
pub fn extract_commands(text: &str) -> Vec<CommandRef> {
    let mut cmds = Vec::new();
    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Split on pipes and semicolons
        for segment in trimmed.split(['|', ';', '&']) {
            let seg = segment.trim();
            if seg.is_empty() {
                continue;
            }
            // Skip control flow keywords
            let first_word = seg.split_whitespace().next().unwrap_or("");
            let keyword = matches!(
                first_word,
                "if" | "then"
                    | "else"
                    | "elif"
                    | "fi"
                    | "for"
                    | "while"
                    | "do"
                    | "done"
                    | "case"
                    | "esac"
                    | "in"
                    | "function"
                    | "{"
                    | "}"
                    | "("
                    | ")"
                    | "export"
                    | "local"
                    | "readonly"
                    | "return"
                    | "exit"
            );
            if keyword {
                continue;
            }
            // Skip variable assignments
            if first_word.contains('=') && !first_word.starts_with('-') {
                continue;
            }
            if !first_word.is_empty()
                && !first_word.starts_with('$')
                && !first_word.starts_with('\'')
                && !first_word.starts_with('"')
            {
                cmds.push(CommandRef {
                    name: first_word.to_string(),
                    line: line_num + 1,
                });
            }
        }
    }
    cmds
}

// ---------------------------------------------------------------------------
// Quoting / escaping
// ---------------------------------------------------------------------------

/// Single-quote a string for POSIX shell (handles embedded single quotes).
pub fn single_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Double-quote a string for POSIX shell, escaping special characters.
pub fn double_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' | '\\' | '$' | '`' | '!' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Escape a string for Windows cmd.exe.
pub fn cmd_escape(s: &str) -> String {
    let needs_quote = s.contains(' ')
        || s.contains('&')
        || s.contains('|')
        || s.contains('<')
        || s.contains('>')
        || s.contains('^');
    if needs_quote {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Argument splitting
// ---------------------------------------------------------------------------

/// Split a command string into arguments (POSIX-style).
///
/// Handles single quotes, double quotes, and backslash escaping.
pub fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        match chars[i] {
            ' ' | '\t' => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            '\'' => {
                i += 1;
                while i < len && chars[i] != '\'' {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            '"' => {
                i += 1;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 1;
                        current.push(chars[i]);
                    } else {
                        current.push(chars[i]);
                    }
                    i += 1;
                }
            }
            '\\' => {
                if i + 1 < len {
                    i += 1;
                    current.push(chars[i]);
                }
            }
            c => current.push(c),
        }
        i += 1;
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

// ---------------------------------------------------------------------------
// Portability hints
// ---------------------------------------------------------------------------

/// Check script text for common portability issues.
pub fn check_portability(text: &str) -> Vec<PortabilityHint> {
    let mut hints = Vec::new();
    let bashisms = [
        ("[[", "Use [ ] instead of [[ ]] for POSIX compatibility"),
        ("${!", "Indirect expansion ${!var} is a bash extension"),
        ("&>>", "&>> redirection is bash-specific"),
        ("<<<", "Here-string <<< is a bash extension"),
        (
            "declare ",
            "'declare' is a bash builtin; use 'typeset' or omit for POSIX",
        ),
        ("select ", "'select' is a bash/ksh extension"),
        ("pushd", "'pushd' is not POSIX; use cd with a variable"),
        ("popd", "'popd' is not POSIX; use cd with a variable"),
        ("source ", "'source' is bash; use '.' for POSIX"),
        ("shopt ", "'shopt' is a bash builtin"),
        ("$RANDOM", "$RANDOM is not POSIX"),
        ("$BASHPID", "$BASHPID is bash-specific"),
        ("BASH_", "BASH_* variables are bash-specific"),
    ];

    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        for (pattern, message) in &bashisms {
            if trimmed.contains(pattern) {
                hints.push(PortabilityHint {
                    message: message.to_string(),
                    line: Some(line_num + 1),
                    severity: "warning".to_string(),
                });
            }
        }
    }

    // Check shebang
    if let Some(first_line) = text.lines().next() {
        if let Some(shebang) = parse_shebang(first_line) {
            if shebang.kind == ShellKind::Bash {
                hints.push(PortabilityHint {
                    message: "Script targets bash specifically; consider #!/bin/sh for portability"
                        .to_string(),
                    line: Some(1),
                    severity: "info".to_string(),
                });
            }
        } else if !text.is_empty() {
            hints.push(PortabilityHint {
                message: "No shebang line found; consider adding #!/bin/sh".to_string(),
                line: None,
                severity: "info".to_string(),
            });
        }
    }
    hints
}

// ---------------------------------------------------------------------------
// Full analysis
// ---------------------------------------------------------------------------

/// Perform full shell script analysis.
pub fn analyze(text: &str) -> ShellAnalysis {
    let shebang = text.lines().next().and_then(parse_shebang);
    let variables = extract_variables(text);
    let commands = extract_commands(text);
    let hints = check_portability(text);
    ShellAnalysis {
        shebang,
        variables,
        commands,
        hints,
    }
}

/// Return unique variable names from a script.
pub fn variable_names(text: &str) -> Vec<String> {
    let vars = extract_variables(text);
    let unique: HashSet<String> = vars.into_iter().map(|v| v.name).collect();
    let mut names: Vec<String> = unique.into_iter().collect();
    names.sort();
    names
}

/// Return unique command names from a script.
pub fn command_names(text: &str) -> Vec<String> {
    let cmds = extract_commands(text);
    let unique: HashSet<String> = cmds.into_iter().map(|c| c.name).collect();
    let mut names: Vec<String> = unique.into_iter().collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SCRIPT: &str = r#"#!/usr/bin/env bash
# Sample script
NAME="world"
export GREETING="Hello"
echo "$GREETING, $NAME!"
ls -la /tmp
grep -r "pattern" .
if [[ -f "$NAME" ]]; then
    cat "$NAME"
fi
"#;

    #[test]
    fn test_parse_shebang_bash() {
        let s = parse_shebang("#!/usr/bin/env bash").unwrap();
        assert_eq!(s.kind, ShellKind::Bash);
        assert_eq!(s.interpreter, "bash");
    }

    #[test]
    fn test_parse_shebang_sh() {
        let s = parse_shebang("#!/bin/sh").unwrap();
        assert_eq!(s.kind, ShellKind::Sh);
    }

    #[test]
    fn test_parse_shebang_none() {
        assert!(parse_shebang("echo hello").is_none());
    }

    #[test]
    fn test_extract_variables() {
        let vars = extract_variables(SAMPLE_SCRIPT);
        let defs: Vec<_> = vars.iter().filter(|v| v.is_definition).collect();
        assert!(defs.iter().any(|v| v.name == "NAME"));
        assert!(defs.iter().any(|v| v.name == "GREETING"));
    }

    #[test]
    fn test_extract_variable_refs() {
        let vars = extract_variables(SAMPLE_SCRIPT);
        let refs: Vec<_> = vars.iter().filter(|v| !v.is_definition).collect();
        assert!(refs.iter().any(|v| v.name == "GREETING"));
        assert!(refs.iter().any(|v| v.name == "NAME"));
    }

    #[test]
    fn test_extract_commands() {
        let cmds = extract_commands(SAMPLE_SCRIPT);
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"ls"));
        assert!(names.contains(&"grep"));
    }

    #[test]
    fn test_single_quote() {
        assert_eq!(single_quote("hello"), "'hello'");
        assert_eq!(single_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_double_quote() {
        assert_eq!(double_quote("hello $world"), "\"hello \\$world\"");
    }

    #[test]
    fn test_cmd_escape() {
        assert_eq!(cmd_escape("hello world"), "\"hello world\"");
        assert_eq!(cmd_escape("simple"), "simple");
    }

    #[test]
    fn test_split_args() {
        let args = split_args("echo 'hello world' foo");
        assert_eq!(args, vec!["echo", "hello world", "foo"]);
    }

    #[test]
    fn test_split_args_double_quote() {
        let args = split_args(r#"echo "hello world" bar"#);
        assert_eq!(args, vec!["echo", "hello world", "bar"]);
    }

    #[test]
    fn test_split_args_escape() {
        let args = split_args(r"echo hello\ world");
        assert_eq!(args, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_portability_bashism() {
        let hints = check_portability("if [[ -f file ]]; then echo ok; fi");
        assert!(hints.iter().any(|h| h.message.contains("[[")));
    }

    #[test]
    fn test_portability_source() {
        let hints = check_portability("source ~/.bashrc");
        assert!(hints.iter().any(|h| h.message.contains("source")));
    }

    #[test]
    fn test_analyze_full() {
        let a = analyze(SAMPLE_SCRIPT);
        assert!(a.shebang.is_some());
        assert!(!a.variables.is_empty());
        assert!(!a.commands.is_empty());
    }

    #[test]
    fn test_variable_names() {
        let names = variable_names(SAMPLE_SCRIPT);
        assert!(names.contains(&"NAME".to_string()));
    }

    #[test]
    fn test_command_names() {
        let names = command_names(SAMPLE_SCRIPT);
        assert!(names.contains(&"echo".to_string()));
    }
}
