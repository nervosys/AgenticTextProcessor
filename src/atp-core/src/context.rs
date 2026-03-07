//! Smart context extraction for code-aware processing.
//!
//! Goes beyond simple line-based context to extract meaningful code regions:
//! - Function/method bodies containing a match
//! - Block-level context (if/else, loop, match arms)
//! - Import/use statement groups
//! - Custom scope detection via bracket/indentation tracking

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Type of context extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextMode {
    /// Fixed number of lines before/after
    Lines { before: usize, after: usize },
    /// Enclosing function/method
    Function,
    /// Enclosing block (braces/indentation)
    Block,
    /// Enclosing scope based on indentation
    Indent,
    /// Full file
    File,
}

/// A context window extracted around a match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindow {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub context_type: String,
    pub match_line: usize,
}

/// Extract a context window around a specific line in a file.
pub fn extract_context(
    path: &Path,
    match_line: usize, // 1-based
    mode: &ContextMode,
) -> anyhow::Result<ContextWindow> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let file_str = path.display().to_string();

    if match_line == 0 || match_line > lines.len() {
        anyhow::bail!(
            "Line {} is out of range (file has {} lines)",
            match_line,
            lines.len()
        );
    }

    let (start, end, ctx_type) = match mode {
        ContextMode::Lines { before, after } => {
            let start = match_line.saturating_sub(*before);
            let end = (match_line + after).min(lines.len());
            (start.max(1), end, "lines")
        }
        ContextMode::Function => {
            let (start, end) = find_enclosing_function(&lines, match_line - 1);
            (start + 1, end + 1, "function")
        }
        ContextMode::Block => {
            let (start, end) = find_enclosing_block(&lines, match_line - 1);
            (start + 1, end + 1, "block")
        }
        ContextMode::Indent => {
            let (start, end) = find_enclosing_indent(&lines, match_line - 1);
            (start + 1, end + 1, "indent")
        }
        ContextMode::File => (1, lines.len(), "file"),
    };

    let extracted: String = lines[(start - 1)..end]
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ContextWindow {
        file: file_str,
        start_line: start,
        end_line: end,
        content: extracted,
        context_type: ctx_type.to_string(),
        match_line,
    })
}

/// Find the enclosing function/method for a given line index (0-based).
///
/// Supports a wide range of languages and patterns:
/// - Rust: `fn`, `pub fn`, `async fn`, `pub(crate) fn`, `unsafe fn`, `extern fn`
/// - Python: `def`, `async def` (with decorator support)
/// - JavaScript/TypeScript: `function`, arrow functions (`=>`), class methods
/// - Go: `func`
/// - C/C++/Java/C#: return-type based (`void`, `int`, `bool`, `char`, `float`,
///   `double`, `long`, `unsigned`, `string`, `auto`, `var`), access modifiers
///   (`public`, `private`, `protected`, `static`, `abstract`, `virtual`, `override`)
/// - Ruby: `def`
/// - PHP: `function`
/// - Kotlin: `fun`
/// - Swift: `func`
///
/// Additionally handles:
/// - Decorators/attributes above function (`@decorator`, `#[attr]`, `#[derive(...)]`)
/// - `impl` blocks (Rust)
/// - `class` and `constructor` declarations
/// - Lambda/closure patterns
fn find_enclosing_function(lines: &[&str], target: usize) -> (usize, usize) {
    // Walk backwards to find function start
    let mut func_start = None;
    for i in (0..=target).rev() {
        let trimmed = lines[i].trim();

        if is_function_declaration(trimmed) {
            func_start = Some(i);
            break;
        }
    }

    let start = match func_start {
        Some(s) => {
            // Walk backwards from function declaration to include decorators/attributes
            let mut decl_start = s;
            for i in (0..s).rev() {
                let trimmed = lines[i].trim();
                if is_decorator_or_attribute(trimmed) {
                    decl_start = i;
                } else if trimmed.is_empty() {
                    // Allow blank lines between decorators
                    continue;
                } else {
                    break;
                }
            }
            decl_start
        }
        None => target,
    };

    // Find the end by matching braces or indentation
    let end = find_function_end(lines, func_start.unwrap_or(target));
    (start, end)
}

/// Check if a line is a function/method declaration.
fn is_function_declaration(trimmed: &str) -> bool {
    // ── Rust ─────────────────────────────────────────────────────────
    if trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("async fn ")
        || trimmed.starts_with("pub async fn ")
        || trimmed.starts_with("pub(crate) async fn ")
        || trimmed.starts_with("unsafe fn ")
        || trimmed.starts_with("pub unsafe fn ")
        || trimmed.starts_with("extern fn ")
        || trimmed.starts_with("const fn ")
        || trimmed.starts_with("pub const fn ")
        || trimmed.starts_with("impl ")
    {
        return true;
    }

    // ── Python ───────────────────────────────────────────────────────
    if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
        return true;
    }

    // ── JavaScript/TypeScript ────────────────────────────────────────
    if trimmed.starts_with("function ")
        || trimmed.starts_with("function(")
        || trimmed.starts_with("async function ")
        || trimmed.starts_with("export function ")
        || trimmed.starts_with("export async function ")
        || trimmed.starts_with("export default function")
    {
        return true;
    }

    // Arrow functions: `const foo = (args) =>` or `const foo = args =>`
    if (trimmed.starts_with("const ") || trimmed.starts_with("let ") || trimmed.starts_with("var "))
        && trimmed.contains("=>")
    {
        return true;
    }

    // Class methods in JS/TS: `methodName(`, `async methodName(`, `get prop()`, `set prop()`
    // But not if it looks like a function call (no leading keyword or access modifier)
    if (trimmed.starts_with("get ") || trimmed.starts_with("set ")) && trimmed.contains('(') {
        return true;
    }

    // ── Go ───────────────────────────────────────────────────────────
    if trimmed.starts_with("func ") {
        return true;
    }

    // ── Kotlin ───────────────────────────────────────────────────────
    if trimmed.starts_with("fun ") || trimmed.contains(" fun ") {
        return true;
    }

    // ── Class/Constructor ────────────────────────────────────────────
    if trimmed.starts_with("class ")
        || trimmed.starts_with("pub struct ")
        || trimmed.starts_with("struct ")
        || trimmed.starts_with("enum ")
        || trimmed.starts_with("pub enum ")
        || trimmed.starts_with("interface ")
        || trimmed.starts_with("trait ")
        || trimmed.starts_with("pub trait ")
        || trimmed.starts_with("constructor(")
        || trimmed.starts_with("constructor (")
    {
        return true;
    }

    // ── C/C++/Java/C# with return types / access modifiers ──────────
    let access_or_qualifier = [
        "public ",
        "private ",
        "protected ",
        "static ",
        "abstract ",
        "virtual ",
        "override ",
        "final ",
        "synchronized ",
        "internal ",
        "extern ",
    ];
    let return_types = [
        "void ",
        "int ",
        "bool ",
        "char ",
        "float ",
        "double ",
        "long ",
        "unsigned ",
        "string ",
        "auto ",
        "var ",
        "String ",
        "boolean ",
        "byte ",
        "short ",
    ];

    for prefix in &access_or_qualifier {
        if trimmed.starts_with(prefix) && trimmed.contains('(') {
            return true;
        }
    }

    for prefix in &return_types {
        if trimmed.starts_with(prefix) && trimmed.contains('(') {
            return true;
        }
    }

    // Type-qualified returns: `MyType functionName(` or `std::vector<int> func(`
    // Heuristic: word followed by space followed by word followed by `(`
    if trimmed.contains('(') && !trimmed.starts_with("//") && !trimmed.starts_with('#') {
        let before_paren = trimmed.split('(').next().unwrap_or("");
        let parts: Vec<&str> = before_paren.split_whitespace().collect();
        // Pattern: [qualifiers...] ReturnType FuncName(
        if parts.len() >= 2 {
            let last = parts[parts.len() - 1];
            // The function name should be an identifier (starts with letter/underscore)
            if last.starts_with(|c: char| c.is_alphabetic() || c == '_')
                && last.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                // The previous part should look like a type (starts with uppercase or is a keyword)
                let prev = parts[parts.len() - 2];
                let stripped = prev.trim_start_matches('*').trim_start_matches('&');
                if stripped.starts_with(|c: char| c.is_uppercase())
                    || return_types.iter().any(|rt| rt.trim() == stripped)
                    || access_or_qualifier.iter().any(|aq| aq.trim() == stripped)
                    || stripped.contains("::")
                    || stripped.contains('<')
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if a line is a decorator or attribute (preceding a function declaration).
fn is_decorator_or_attribute(trimmed: &str) -> bool {
    // Python/Java/TypeScript decorators: @decorator, @decorator(args)
    if trimmed.starts_with('@') {
        return true;
    }
    // Rust attributes: #[...], #![...]
    if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
        return true;
    }
    // C# attributes: [Attribute], [Attribute(args)]
    if trimmed.starts_with('[')
        && trimmed.ends_with(']')
        && trimmed.len() > 2
        && trimmed[1..2].starts_with(|c: char| c.is_uppercase())
    {
        return true;
    }
    // C/C++ macros that precede functions: __attribute__, __declspec
    if trimmed.starts_with("__attribute__") || trimmed.starts_with("__declspec") {
        return true;
    }
    // Documentation comments that are part of the function: ///, //!
    if trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return true;
    }
    false
}

/// Find the end of a function starting at the given line.
///
/// For brace-delimited languages: finds matching `}`.
/// For indentation-based languages (Python, Ruby): follows indentation.
fn find_function_end(lines: &[&str], start: usize) -> usize {
    let trimmed = lines[start].trim();

    // Python-style: ends with `:`, use indentation
    if trimmed.ends_with(':')
        && (trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class "))
    {
        let base_indent = indent_level(lines[start]);
        let mut end = start;
        for (i, line) in lines.iter().enumerate().skip(start + 1) {
            if line.trim().is_empty() {
                end = i;
                continue;
            }
            if indent_level(line) <= base_indent {
                break;
            }
            end = i;
        }
        return end;
    }

    // Brace-delimited: find matching close brace
    find_matching_close(lines, start)
}

/// Find the enclosing block (brace-delimited) for a given line index.
fn find_enclosing_block(lines: &[&str], target: usize) -> (usize, usize) {
    // Walk backwards to find the nearest '{' that's not closed before target
    let mut depth = 0i32;
    let mut start = target;

    for i in (0..=target).rev() {
        for ch in lines[i].chars().rev() {
            match ch {
                '}' => depth += 1,
                '{' => {
                    depth -= 1;
                    if depth < 0 {
                        start = i;
                        // Now find the matching close
                        let end = find_matching_close(lines, start);
                        return (start, end);
                    }
                }
                _ => {}
            }
        }
    }

    // Fallback: return target line with some padding
    let end = (target + 5).min(lines.len() - 1);
    (start, end)
}

/// Find the enclosing indentation scope.
fn find_enclosing_indent(lines: &[&str], target: usize) -> (usize, usize) {
    let target_indent = indent_level(lines[target]);

    // Walk backwards to find the scope header (line with less indentation)
    let mut start = target;
    for i in (0..target).rev() {
        if !lines[i].trim().is_empty() && indent_level(lines[i]) < target_indent {
            start = i;
            break;
        }
    }

    // Walk forwards to find end of scope
    let mut end = target;
    for (i, line) in lines.iter().enumerate().skip(target + 1) {
        if !line.trim().is_empty() && indent_level(line) < target_indent {
            end = i - 1;
            break;
        }
        end = i;
    }

    (start, end)
}

/// Find the closing brace matching the first '{' found at or after line `start`.
fn find_matching_close(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    let mut found_open = false;

    for (i, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    found_open = true;
                }
                '}' => {
                    depth -= 1;
                    if found_open && depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
    }

    // Fallback: return end of file
    lines.len().saturating_sub(1)
}

/// Count leading whitespace characters.
fn indent_level(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        tmp
    }

    #[test]
    fn test_lines_context() {
        let tmp = write_tmp("line1\nline2\nline3\nline4\nline5\n");
        let ctx = extract_context(
            tmp.path(),
            3,
            &ContextMode::Lines {
                before: 1,
                after: 1,
            },
        )
        .unwrap();
        assert_eq!(ctx.start_line, 2);
        assert_eq!(ctx.end_line, 4);
        assert_eq!(ctx.match_line, 3);
        assert!(ctx.content.contains("line2"));
        assert!(ctx.content.contains("line3"));
        assert!(ctx.content.contains("line4"));
        assert_eq!(ctx.context_type, "lines");
    }

    #[test]
    fn test_file_context() {
        let tmp = write_tmp("a\nb\nc\n");
        let ctx = extract_context(tmp.path(), 1, &ContextMode::File).unwrap();
        assert_eq!(ctx.start_line, 1);
        assert_eq!(ctx.end_line, 3);
        assert_eq!(ctx.context_type, "file");
    }

    #[test]
    fn test_function_context() {
        let code = "fn main() {\n    let x = 1;\n    println!(\"{}\", x);\n}\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 2, &ContextMode::Function).unwrap();
        assert_eq!(ctx.context_type, "function");
        assert!(ctx.content.contains("fn main"));
        assert!(ctx.content.contains("}"));
    }

    #[test]
    fn test_block_context() {
        let code = "fn foo() {\n    if true {\n        println!(\"a\");\n    }\n}\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 3, &ContextMode::Block).unwrap();
        assert_eq!(ctx.context_type, "block");
    }

    #[test]
    fn test_indent_context() {
        let code = "def foo():\n    x = 1\n    y = 2\n    return x + y\n\ndef bar():\n    pass\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 2, &ContextMode::Indent).unwrap();
        assert_eq!(ctx.context_type, "indent");
        assert!(ctx.content.contains("x = 1"));
    }

    #[test]
    fn test_out_of_range() {
        let tmp = write_tmp("only one line\n");
        assert!(extract_context(tmp.path(), 0, &ContextMode::File).is_err());
        assert!(extract_context(tmp.path(), 99, &ContextMode::File).is_err());
    }

    #[test]
    fn test_indent_level() {
        assert_eq!(indent_level("hello"), 0);
        assert_eq!(indent_level("    hello"), 4);
        assert_eq!(indent_level("\thello"), 1);
    }

    // ── Improved function detection tests ────────────────────────────

    #[test]
    fn test_function_with_decorator() {
        let code = "@app.route('/api')\ndef handle_request():\n    return 'ok'\n\ndef other():\n    pass\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 3, &ContextMode::Function).unwrap();
        assert!(ctx.content.contains("@app.route"));
        assert!(ctx.content.contains("def handle_request"));
    }

    #[test]
    fn test_function_with_rust_attribute() {
        let code = "#[test]\nfn test_something() {\n    assert!(true);\n}\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 3, &ContextMode::Function).unwrap();
        assert!(ctx.content.contains("#[test]"));
        assert!(ctx.content.contains("fn test_something"));
    }

    #[test]
    fn test_arrow_function() {
        let code = "const noop = () => {};\nconst add = (a, b) => {\n    return a + b;\n};\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 3, &ContextMode::Function).unwrap();
        assert!(ctx.content.contains("const add"));
        assert!(ctx.content.contains("=>"));
    }

    #[test]
    fn test_java_method() {
        let code = "public class Foo {\n    public void doSomething() {\n        System.out.println(\"hi\");\n    }\n}\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 3, &ContextMode::Function).unwrap();
        assert!(ctx.content.contains("public void doSomething"));
    }

    #[test]
    fn test_impl_block() {
        let code = "impl Foo {\n    fn bar(&self) {\n        // body\n    }\n}\n";
        let tmp = write_tmp(code);
        let ctx = extract_context(tmp.path(), 3, &ContextMode::Function).unwrap();
        // Should find the inner fn bar, not impl
        assert!(ctx.content.contains("fn bar"));
    }

    #[test]
    fn test_is_function_declaration_basics() {
        assert!(is_function_declaration("fn main() {"));
        assert!(is_function_declaration("pub fn new() -> Self {"));
        assert!(is_function_declaration("async fn handle() {"));
        assert!(is_function_declaration("def __init__(self):"));
        assert!(is_function_declaration("function render() {"));
        assert!(is_function_declaration("func main() {"));
        assert!(is_function_declaration("void process() {"));
        assert!(is_function_declaration(
            "public static void main(String[] args) {"
        ));
        assert!(is_function_declaration("const handler = (req, res) => {"));
        assert!(is_function_declaration("class MyClass {"));
        assert!(is_function_declaration("constructor() {"));
        assert!(is_function_declaration("impl Display for Foo {"));
        assert!(!is_function_declaration("// just a comment"));
        assert!(!is_function_declaration("let x = 42;"));
    }

    #[test]
    fn test_is_decorator_or_attribute() {
        assert!(is_decorator_or_attribute("@property"));
        assert!(is_decorator_or_attribute("@app.route('/api')"));
        assert!(is_decorator_or_attribute("#[derive(Debug)]"));
        assert!(is_decorator_or_attribute("#[test]"));
        assert!(is_decorator_or_attribute("/// Documentation comment"));
        assert!(!is_decorator_or_attribute("let x = 1;"));
        assert!(!is_decorator_or_attribute("// regular comment"));
    }
}
