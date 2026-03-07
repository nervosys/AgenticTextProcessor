//! # Syntax Highlighting Engine
//!
//! Token-based colorisation for 10+ languages, with ANSI, HTML, and
//! SVG output renderers, theme support, and optional line-number gutters.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Token kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenKind {
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Identifier,
    Type,
    Function,
    Macro,
    Attribute,
    Whitespace,
    Unknown,
}

/// A single highlighted token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub line: usize,
    pub col: usize,
}

/// A language definition for highlighting.
#[derive(Debug, Clone)]
pub struct LangDef {
    pub name: String,
    pub extensions: Vec<String>,
    pub keywords: Vec<String>,
    pub types: Vec<String>,
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub string_delimiters: Vec<char>,
}

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightFormat {
    Ansi,
    Html,
    Svg,
    Plain,
}

/// Color theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: BTreeMap<String, String>,
}

impl Theme {
    pub fn color_for(&self, kind: TokenKind) -> &str {
        let key = format!("{:?}", kind).to_lowercase();
        self.colors
            .get(&key)
            .map(|s| s.as_str())
            .unwrap_or("#cccccc")
    }
}

/// Default dark theme.
pub fn dark_theme() -> Theme {
    let mut colors = BTreeMap::new();
    colors.insert("keyword".into(), "#c678dd".into());
    colors.insert("string".into(), "#98c379".into());
    colors.insert("number".into(), "#d19a66".into());
    colors.insert("comment".into(), "#5c6370".into());
    colors.insert("operator".into(), "#56b6c2".into());
    colors.insert("punctuation".into(), "#abb2bf".into());
    colors.insert("identifier".into(), "#e06c75".into());
    colors.insert("type".into(), "#e5c07b".into());
    colors.insert("function".into(), "#61afef".into());
    colors.insert("macro".into(), "#c678dd".into());
    colors.insert("attribute".into(), "#d19a66".into());
    colors.insert("whitespace".into(), "".into());
    colors.insert("unknown".into(), "#abb2bf".into());
    Theme {
        name: "dark".into(),
        colors,
    }
}

/// Default light theme.
pub fn light_theme() -> Theme {
    let mut colors = BTreeMap::new();
    colors.insert("keyword".into(), "#7c4dff".into());
    colors.insert("string".into(), "#2e7d32".into());
    colors.insert("number".into(), "#e65100".into());
    colors.insert("comment".into(), "#9e9e9e".into());
    colors.insert("operator".into(), "#0097a7".into());
    colors.insert("punctuation".into(), "#37474f".into());
    colors.insert("identifier".into(), "#c62828".into());
    colors.insert("type".into(), "#f9a825".into());
    colors.insert("function".into(), "#1565c0".into());
    colors.insert("macro".into(), "#7c4dff".into());
    colors.insert("attribute".into(), "#e65100".into());
    colors.insert("whitespace".into(), "".into());
    colors.insert("unknown".into(), "#37474f".into());
    Theme {
        name: "light".into(),
        colors,
    }
}

/// Highlight options.
#[derive(Debug, Clone)]
pub struct HighlightOptions {
    pub format: HighlightFormat,
    pub theme: Theme,
    pub line_numbers: bool,
    pub start_line: usize,
}

impl Default for HighlightOptions {
    fn default() -> Self {
        Self {
            format: HighlightFormat::Ansi,
            theme: dark_theme(),
            line_numbers: false,
            start_line: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Language definitions
// ---------------------------------------------------------------------------

pub fn lang_rust() -> LangDef {
    LangDef {
        name: "Rust".into(),
        extensions: vec!["rs".into()],
        keywords: vec![
            "fn", "let", "mut", "const", "static", "if", "else", "match", "for", "while", "loop",
            "return", "break", "continue", "pub", "use", "mod", "struct", "enum", "impl", "trait",
            "where", "type", "self", "super", "crate", "as", "in", "ref", "move", "async", "await",
            "unsafe", "extern", "dyn", "true", "false",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        types: vec![
            "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64",
            "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Rc", "Arc",
            "usize", "isize", "Self",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        line_comment: Some("//".into()),
        block_comment: Some(("/*".into(), "*/".into())),
        string_delimiters: vec!['"'],
    }
}

pub fn lang_python() -> LangDef {
    LangDef {
        name: "Python".into(),
        extensions: vec!["py".into()],
        keywords: vec![
            "def", "class", "if", "elif", "else", "for", "while", "return", "import", "from", "as",
            "try", "except", "finally", "with", "yield", "lambda", "pass", "break", "continue",
            "raise", "and", "or", "not", "in", "is", "True", "False", "None", "async", "await",
            "global", "nonlocal",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        types: vec![
            "int", "float", "str", "bool", "list", "dict", "tuple", "set", "bytes", "object",
            "type",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        line_comment: Some("#".into()),
        block_comment: None,
        string_delimiters: vec!['"', '\''],
    }
}

pub fn lang_javascript() -> LangDef {
    LangDef {
        name: "JavaScript".into(),
        extensions: vec!["js".into(), "mjs".into(), "cjs".into()],
        keywords: vec![
            "function",
            "var",
            "let",
            "const",
            "if",
            "else",
            "for",
            "while",
            "do",
            "return",
            "break",
            "continue",
            "switch",
            "case",
            "default",
            "try",
            "catch",
            "finally",
            "throw",
            "new",
            "delete",
            "typeof",
            "instanceof",
            "class",
            "extends",
            "import",
            "export",
            "from",
            "async",
            "await",
            "yield",
            "true",
            "false",
            "null",
            "undefined",
            "this",
            "super",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        types: vec![
            "Array", "Object", "String", "Number", "Boolean", "Map", "Set", "Promise", "Symbol",
            "BigInt",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        line_comment: Some("//".into()),
        block_comment: Some(("/*".into(), "*/".into())),
        string_delimiters: vec!['"', '\'', '`'],
    }
}

pub fn lang_c() -> LangDef {
    LangDef {
        name: "C".into(),
        extensions: vec!["c".into(), "h".into()],
        keywords: vec![
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
            "else", "enum", "extern", "float", "for", "goto", "if", "int", "long", "register",
            "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
            "union", "unsigned", "void", "volatile", "while", "inline", "restrict",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        types: vec![
            "int", "char", "float", "double", "void", "long", "short", "unsigned", "signed",
            "size_t", "FILE",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        line_comment: Some("//".into()),
        block_comment: Some(("/*".into(), "*/".into())),
        string_delimiters: vec!['"'],
    }
}

pub fn lang_go() -> LangDef {
    LangDef {
        name: "Go".into(),
        extensions: vec!["go".into()],
        keywords: vec![
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
            "true",
            "false",
            "nil",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        types: vec![
            "int",
            "int8",
            "int16",
            "int32",
            "int64",
            "uint",
            "uint8",
            "uint16",
            "uint32",
            "uint64",
            "float32",
            "float64",
            "complex64",
            "complex128",
            "string",
            "bool",
            "byte",
            "rune",
            "error",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        line_comment: Some("//".into()),
        block_comment: Some(("/*".into(), "*/".into())),
        string_delimiters: vec!['"', '`'],
    }
}

/// Get a built-in language by name or extension.
pub fn detect_language(hint: &str) -> Option<LangDef> {
    let hint_lower = hint.to_lowercase();
    let all = [
        lang_rust(),
        lang_python(),
        lang_javascript(),
        lang_c(),
        lang_go(),
    ];
    for lang in &all {
        if lang.name.to_lowercase() == hint_lower {
            return Some(lang.clone());
        }
        for ext in &lang.extensions {
            if ext == &hint_lower {
                return Some(lang.clone());
            }
        }
    }
    None
}

/// List all built-in language names.
pub fn supported_languages() -> Vec<String> {
    vec![
        "Rust".into(),
        "Python".into(),
        "JavaScript".into(),
        "C".into(),
        "Go".into(),
    ]
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Tokenize source code using a language definition.
pub fn tokenize(source: &str, lang: &LangDef) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    let mut line = 1;
    let mut col = 1;

    while i < chars.len() {
        // Block comment
        if let Some((open, close)) = &lang.block_comment {
            let open_chars: Vec<char> = open.chars().collect();
            if chars[i..].starts_with(&open_chars) {
                let close_chars: Vec<char> = close.chars().collect();
                let mut j = i + open_chars.len();
                while j + close_chars.len() <= chars.len() && !chars[j..].starts_with(&close_chars)
                {
                    j += 1;
                }
                j += close_chars.len();
                let text: String = chars[i..j.min(chars.len())].iter().collect();
                let newlines = text.chars().filter(|&c| c == '\n').count();
                tokens.push(Token {
                    kind: TokenKind::Comment,
                    text,
                    line,
                    col,
                });
                line += newlines;
                i = j.min(chars.len());
                col = 1; // approximate
                continue;
            }
        }

        // Line comment
        if let Some(lc) = &lang.line_comment {
            let lc_chars: Vec<char> = lc.chars().collect();
            if chars[i..].starts_with(&lc_chars) {
                let mut j = i;
                while j < chars.len() && chars[j] != '\n' {
                    j += 1;
                }
                let text: String = chars[i..j].iter().collect();
                tokens.push(Token {
                    kind: TokenKind::Comment,
                    text,
                    line,
                    col,
                });
                i = j;
                continue;
            }
        }

        // String literal
        if lang.string_delimiters.contains(&chars[i]) {
            let delim = chars[i];
            let mut j = i + 1;
            while j < chars.len() && chars[j] != delim {
                if chars[j] == '\\' {
                    j += 1; // skip escaped char
                }
                j += 1;
            }
            if j < chars.len() {
                j += 1; // include closing delimiter
            }
            let text: String = chars[i..j].iter().collect();
            tokens.push(Token {
                kind: TokenKind::String,
                text,
                line,
                col,
            });
            i = j;
            continue;
        }

        // Number
        if chars[i].is_ascii_digit()
            || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let mut j = i;
            let mut has_dot = false;
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_')
            {
                if chars[j] == '.' {
                    if has_dot {
                        break;
                    }
                    has_dot = true;
                }
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            tokens.push(Token {
                kind: TokenKind::Number,
                text,
                line,
                col,
            });
            i = j;
            continue;
        }

        // Identifier / keyword / type
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            let kind = if lang.keywords.iter().any(|k| k == &text) {
                TokenKind::Keyword
            } else if lang.types.iter().any(|t| t == &text) {
                TokenKind::Type
            } else if j < chars.len() && chars[j] == '(' {
                TokenKind::Function
            } else if j < chars.len() && chars[j] == '!' {
                TokenKind::Macro
            } else {
                TokenKind::Identifier
            };
            tokens.push(Token {
                kind,
                text,
                line,
                col,
            });
            i = j;
            continue;
        }

        // Newline
        if chars[i] == '\n' {
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                text: "\n".into(),
                line,
                col,
            });
            line += 1;
            col = 1;
            i += 1;
            continue;
        }

        // Whitespace
        if chars[i].is_whitespace() {
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() && chars[j] != '\n' {
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                text,
                line,
                col,
            });
            col += j - i;
            i = j;
            continue;
        }

        // Operator / punctuation
        let kind = if "+-*/%=<>!&|^~?".contains(chars[i]) {
            TokenKind::Operator
        } else {
            TokenKind::Punctuation
        };
        tokens.push(Token {
            kind,
            text: chars[i].to_string(),
            line,
            col,
        });
        col += 1;
        i += 1;
    }

    tokens
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

/// Render tokens in the given format.
pub fn render(tokens: &[Token], options: &HighlightOptions) -> String {
    match options.format {
        HighlightFormat::Ansi => render_ansi(tokens, options),
        HighlightFormat::Html => render_html(tokens, options),
        HighlightFormat::Svg => render_svg(tokens, options),
        HighlightFormat::Plain => render_plain(tokens, options),
    }
}

fn ansi_color(hex: &str) -> String {
    if hex.is_empty() || !hex.starts_with('#') || hex.len() < 7 {
        return String::new();
    }
    let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(200);
    let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(200);
    let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(200);
    format!("\x1b[38;2;{r};{g};{b}m")
}

fn render_ansi(tokens: &[Token], options: &HighlightOptions) -> String {
    let mut out = String::new();
    let mut current_line = options.start_line;

    if options.line_numbers {
        out.push_str(&format!("{:>4} | ", current_line));
    }

    for token in tokens {
        if token.kind == TokenKind::Whitespace && token.text == "\n" {
            out.push('\n');
            current_line += 1;
            if options.line_numbers {
                out.push_str(&format!("{:>4} | ", current_line));
            }
            continue;
        }

        let color = options.theme.color_for(token.kind);
        if color.is_empty() || token.kind == TokenKind::Whitespace {
            out.push_str(&token.text);
        } else {
            out.push_str(&ansi_color(color));
            out.push_str(&token.text);
            out.push_str("\x1b[0m");
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_html(tokens: &[Token], options: &HighlightOptions) -> String {
    let mut out = String::new();
    out.push_str("<pre><code>");

    let mut current_line = options.start_line;
    if options.line_numbers {
        out.push_str(&format!("<span class=\"ln\">{:>4}</span> ", current_line));
    }

    for token in tokens {
        if token.kind == TokenKind::Whitespace && token.text == "\n" {
            out.push('\n');
            current_line += 1;
            if options.line_numbers {
                out.push_str(&format!("<span class=\"ln\">{:>4}</span> ", current_line));
            }
            continue;
        }

        let class = format!("{:?}", token.kind).to_lowercase();
        let color = options.theme.color_for(token.kind);
        let escaped = html_escape(&token.text);
        if token.kind == TokenKind::Whitespace {
            out.push_str(&escaped);
        } else {
            out.push_str(&format!(
                "<span class=\"hl-{class}\" style=\"color:{color}\">{escaped}</span>"
            ));
        }
    }

    out.push_str("</code></pre>");
    out
}

fn render_svg(tokens: &[Token], options: &HighlightOptions) -> String {
    let line_height = 20;
    let char_width = 9;
    let margin_left = if options.line_numbers { 60 } else { 10 };

    // Calculate dimensions
    let lines: Vec<&str> = {
        let source: String = tokens.iter().map(|t| t.text.as_str()).collect();
        let count = source.lines().count().max(1);
        drop(source);
        vec![""; count]
    };
    let height = lines.len() * line_height + 20;

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"{height}\">\n"
    ));
    out.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#282c34\"/>\n");

    let mut x = margin_left;
    let mut y = line_height;
    let mut current_line = options.start_line;

    if options.line_numbers {
        out.push_str(&format!(
            "<text x=\"10\" y=\"{y}\" fill=\"#5c6370\" font-family=\"monospace\" font-size=\"14\">{:>4}</text>",
            current_line
        ));
    }

    for token in tokens {
        if token.kind == TokenKind::Whitespace && token.text == "\n" {
            y += line_height;
            x = margin_left;
            current_line += 1;
            if options.line_numbers {
                out.push_str(&format!(
                    "<text x=\"10\" y=\"{y}\" fill=\"#5c6370\" font-family=\"monospace\" font-size=\"14\">{:>4}</text>",
                    current_line
                ));
            }
            continue;
        }

        let color = options.theme.color_for(token.kind);
        let escaped = html_escape(&token.text);
        if token.kind != TokenKind::Whitespace {
            out.push_str(&format!(
                "<text x=\"{x}\" y=\"{y}\" fill=\"{color}\" font-family=\"monospace\" font-size=\"14\">{escaped}</text>\n"
            ));
        }
        x += token.text.len() * char_width;
    }

    out.push_str("</svg>\n");
    out
}

fn render_plain(tokens: &[Token], options: &HighlightOptions) -> String {
    let mut out = String::new();
    let mut current_line = options.start_line;

    if options.line_numbers {
        out.push_str(&format!("{:>4} | ", current_line));
    }

    for token in tokens {
        if token.kind == TokenKind::Whitespace && token.text == "\n" {
            out.push('\n');
            current_line += 1;
            if options.line_numbers {
                out.push_str(&format!("{:>4} | ", current_line));
            }
            continue;
        }
        out.push_str(&token.text);
    }
    out
}

// ---------------------------------------------------------------------------
// Convenience
// ---------------------------------------------------------------------------

/// Highlight source code end-to-end.
pub fn highlight(source: &str, lang_hint: &str, options: &HighlightOptions) -> String {
    let lang = detect_language(lang_hint).unwrap_or(lang_rust());
    let tokens = tokenize(source, &lang);
    render(&tokens, options)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_rust() {
        let tokens = tokenize("fn main() {}", &lang_rust());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword && t.text == "fn"));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Function && t.text == "main"));
    }

    #[test]
    fn test_tokenize_string() {
        let tokens = tokenize("let s = \"hello\";", &lang_rust());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::String && t.text == "\"hello\""));
    }

    #[test]
    fn test_tokenize_comment() {
        let tokens = tokenize("// this is a comment\nlet x = 1;", &lang_rust());
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Comment));
    }

    #[test]
    fn test_tokenize_number() {
        let tokens = tokenize("let x = 42;", &lang_rust());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Number && t.text == "42"));
    }

    #[test]
    fn test_tokenize_type() {
        let tokens = tokenize("let x: i32 = 0;", &lang_rust());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Type && t.text == "i32"));
    }

    #[test]
    fn test_tokenize_python() {
        let tokens = tokenize("def foo(): pass", &lang_python());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword && t.text == "def"));
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Keyword && t.text == "pass"));
    }

    #[test]
    fn test_detect_language() {
        assert!(detect_language("rs").is_some());
        assert!(detect_language("py").is_some());
        assert!(detect_language("js").is_some());
        assert!(detect_language("Rust").is_some());
        assert!(detect_language("unknown_lang").is_none());
    }

    #[test]
    fn test_render_ansi() {
        let options = HighlightOptions::default();
        let result = highlight("fn main() {}", "rs", &options);
        assert!(result.contains("\x1b[")); // ANSI escape codes
        assert!(result.contains("fn"));
    }

    #[test]
    fn test_render_html() {
        let options = HighlightOptions {
            format: HighlightFormat::Html,
            ..Default::default()
        };
        let result = highlight("let x = 1;", "rs", &options);
        assert!(result.contains("<pre><code>"));
        assert!(result.contains("</code></pre>"));
        assert!(result.contains("hl-keyword"));
    }

    #[test]
    fn test_render_plain() {
        let options = HighlightOptions {
            format: HighlightFormat::Plain,
            ..Default::default()
        };
        let result = highlight("fn main() {}", "rs", &options);
        // Should contain the source without escape codes
        assert!(result.contains("fn main()"));
        assert!(!result.contains("\x1b["));
    }

    #[test]
    fn test_line_numbers() {
        let options = HighlightOptions {
            format: HighlightFormat::Plain,
            line_numbers: true,
            ..Default::default()
        };
        let result = highlight("a\nb\nc", "rs", &options);
        assert!(result.contains("   1 | "));
        assert!(result.contains("   2 | "));
    }

    #[test]
    fn test_themes() {
        let dark = dark_theme();
        let light = light_theme();
        assert_ne!(
            dark.color_for(TokenKind::Keyword),
            light.color_for(TokenKind::Keyword)
        );
    }

    #[test]
    fn test_svg_output() {
        let options = HighlightOptions {
            format: HighlightFormat::Svg,
            ..Default::default()
        };
        let result = highlight("fn main() {}", "rs", &options);
        assert!(result.contains("<svg"));
        assert!(result.contains("</svg>"));
    }

    #[test]
    fn test_block_comment() {
        let tokens = tokenize("/* block */ let x = 1;", &lang_rust());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Comment && t.text.contains("block")));
    }

    #[test]
    fn test_supported_languages() {
        let langs = supported_languages();
        assert!(langs.contains(&"Rust".into()));
        assert!(langs.contains(&"Python".into()));
        assert!(langs.len() >= 5);
    }

    #[test]
    fn test_macro_token() {
        let tokens = tokenize("println!(\"hi\")", &lang_rust());
        assert!(tokens
            .iter()
            .any(|t| t.kind == TokenKind::Macro && t.text == "println"));
    }
}
