//! Tree-sitter code intelligence — AST-aware search, structural matching, and symbol analysis.
//!
//! Provides language-aware code navigation and search using tree-sitter parsers.
//! Falls back gracefully to regex-based matching when tree-sitter grammar is unavailable.
//!
//! # Supported Languages
//!
//! Language detection is by file extension. The module provides a framework
//! for tree-sitter integration without requiring tree-sitter grammar crates
//! at compile time (they can be loaded dynamically via plugins or enabled
//! as cargo features in the future).
//!
//! # Features
//!
//! - [`LanguageDetector`]: Detect programming language from file extension
//! - [`SymbolKind`] / [`Symbol`]: Extracted symbol information
//! - [`StructuralQuery`]: AST-pattern-based search specification
//! - [`CodeIntelligence`]: High-level API for code analysis

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ──────────────────────────────────────────────
// Tree-sitter abstraction layer
// ──────────────────────────────────────────────

/// Backend used for symbol extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntelBackend {
    /// Regex-based extraction (always available, approximate).
    Regex,
    /// Tree-sitter AST parsing (accurate, requires grammar binaries).
    TreeSitter,
}

/// Trait for pluggable symbol extraction backends.
///
/// Implement this to add tree-sitter or other AST-aware extractors.
/// The default implementation uses regex patterns.
pub trait SymbolExtractor: Send + Sync {
    /// Extract symbols from source code for a given language.
    fn extract(&self, source: &str, file_path: &str, language: Language) -> Vec<Symbol>;

    /// Name of this backend.
    fn backend(&self) -> IntelBackend;

    /// Languages supported by this extractor.
    fn supported_languages(&self) -> Vec<Language>;
}

// ──────────────────────────────────────────────
// Scope tree — hierarchical scope navigation
// ──────────────────────────────────────────────

/// A node in a scope tree — represents a named scope (function, class, module, block).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeNode {
    /// Name of this scope (e.g., function name, class name).
    pub name: String,
    /// Kind of scope.
    pub kind: SymbolKind,
    /// Start line (1-based).
    pub start_line: usize,
    /// End line (1-based).
    pub end_line: usize,
    /// Child scopes.
    pub children: Vec<ScopeNode>,
}

impl ScopeNode {
    /// Find the innermost scope containing the given line.
    pub fn scope_at_line(&self, line: usize) -> Option<&ScopeNode> {
        if line < self.start_line || line > self.end_line {
            return None;
        }
        // Check children first (more specific)
        for child in &self.children {
            if let Some(inner) = child.scope_at_line(line) {
                return Some(inner);
            }
        }
        Some(self)
    }

    /// Get the full scope path to a line (e.g., ["module", "class", "method"]).
    pub fn scope_path_at_line(&self, line: usize) -> Vec<String> {
        if line < self.start_line || line > self.end_line {
            return Vec::new();
        }
        let mut path = vec![self.name.clone()];
        for child in &self.children {
            let child_path = child.scope_path_at_line(line);
            if !child_path.is_empty() {
                path.extend(child_path);
                break;
            }
        }
        path
    }

    /// Flatten all scopes into a list with depth information.
    pub fn flatten(&self, depth: usize) -> Vec<(usize, &ScopeNode)> {
        let mut result = vec![(depth, self)];
        for child in &self.children {
            result.extend(child.flatten(depth + 1));
        }
        result
    }
}

/// Build a scope tree from extracted symbols.
pub fn build_scope_tree(symbols: &[Symbol], file_name: &str) -> ScopeNode {
    let mut root = ScopeNode {
        name: file_name.to_string(),
        kind: SymbolKind::Module,
        start_line: 1,
        end_line: symbols.iter().map(|s| s.end_line).max().unwrap_or(1),
        children: Vec::new(),
    };

    // Sort symbols by start line, then by scope size (larger first)
    let mut sorted: Vec<&Symbol> = symbols.iter().collect();
    sorted.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then_with(|| (b.end_line - b.start_line).cmp(&(a.end_line - a.start_line)))
    });

    for sym in sorted {
        let node = ScopeNode {
            name: sym.name.clone(),
            kind: sym.kind.clone(),
            start_line: sym.start_line,
            end_line: sym.end_line,
            children: Vec::new(),
        };
        insert_scope_node(&mut root, node);
    }

    root
}

fn insert_scope_node(parent: &mut ScopeNode, node: ScopeNode) {
    // Try to insert into an existing child that fully contains this node
    for child in &mut parent.children {
        if node.start_line >= child.start_line && node.end_line <= child.end_line {
            insert_scope_node(child, node);
            return;
        }
    }
    parent.children.push(node);
}

// ──────────────────────────────────────────────
// Call graph — function reference tracking
// ──────────────────────────────────────────────

/// An edge in the call graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdge {
    /// Calling function.
    pub caller: String,
    /// Called function.
    pub callee: String,
    /// Line where the call occurs.
    pub line: usize,
    /// File containing the call.
    pub file: String,
}

/// A call graph built from source analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallGraph {
    /// All call edges.
    pub edges: Vec<CallEdge>,
    /// Set of all known function names.
    pub functions: Vec<String>,
}

impl CallGraph {
    /// Get all functions called by a given function.
    pub fn callees_of(&self, function: &str) -> Vec<&CallEdge> {
        self.edges.iter().filter(|e| e.caller == function).collect()
    }

    /// Get all functions that call a given function.
    pub fn callers_of(&self, function: &str) -> Vec<&CallEdge> {
        self.edges.iter().filter(|e| e.callee == function).collect()
    }

    /// Get functions with no callers (entry points / roots).
    pub fn roots(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|f| !self.edges.iter().any(|e| &e.callee == *f))
            .map(|s| s.as_str())
            .collect()
    }

    /// Get functions with no callees (leaf functions).
    pub fn leaves(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|f| !self.edges.iter().any(|e| &e.caller == *f))
            .map(|s| s.as_str())
            .collect()
    }
}

/// Build a simple call graph from symbols by scanning function bodies for references.
pub fn build_call_graph(source: &str, symbols: &[Symbol], file_path: &str) -> CallGraph {
    let lines: Vec<&str> = source.lines().collect();
    let fn_names: Vec<String> = symbols
        .iter()
        .filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
        .map(|s| s.name.clone())
        .collect();

    let mut edges = Vec::new();

    for sym in symbols
        .iter()
        .filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
    {
        // Scan the body of this function for calls to other known functions
        let body_start = sym.start_line; // 1-based
        let body_end = sym.end_line.min(lines.len());

        for (line_idx, line) in lines.iter().enumerate().take(body_end).skip(body_start) {
            for callee in &fn_names {
                if callee == &sym.name {
                    continue; // skip self-references on the definition line
                }
                // Simple heuristic: look for `callee(` pattern
                let call_pattern = format!("{}(", callee);
                if line.contains(&call_pattern) {
                    edges.push(CallEdge {
                        caller: sym.name.clone(),
                        callee: callee.clone(),
                        line: line_idx + 1,
                        file: file_path.to_string(),
                    });
                }
            }
        }
    }

    CallGraph {
        edges,
        functions: fn_names,
    }
}

/// Supported programming languages for code intelligence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    Ruby,
    Shell,
    Toml,
    Yaml,
    Json,
    Markdown,
    Unknown,
}

impl Language {
    /// Detect language from a file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "py" | "pyi" | "pyw" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" | "mts" => Language::TypeScript,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Language::Cpp,
            "rb" | "rake" => Language::Ruby,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            "toml" => Language::Toml,
            "yml" | "yaml" => Language::Yaml,
            "json" => Language::Json,
            "md" | "mdx" | "markdown" => Language::Markdown,
            _ => Language::Unknown,
        }
    }

    /// Detect language from a file path.
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|e| e.to_str())
            .map(Self::from_extension)
            .unwrap_or(Language::Unknown)
    }

    /// Get the display name of the language.
    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Ruby => "Ruby",
            Language::Shell => "Shell",
            Language::Toml => "TOML",
            Language::Yaml => "YAML",
            Language::Json => "JSON",
            Language::Markdown => "Markdown",
            Language::Unknown => "Unknown",
        }
    }

    /// Get the single-line comment prefix for the language.
    pub fn comment_prefix(&self) -> Option<&'static str> {
        match self {
            Language::Rust | Language::Go | Language::Java | Language::C | Language::Cpp => {
                Some("//")
            }
            Language::JavaScript | Language::TypeScript => Some("//"),
            Language::Python | Language::Ruby | Language::Shell => Some("#"),
            Language::Toml | Language::Yaml => Some("#"),
            _ => None,
        }
    }
}

/// Kind of symbol extracted from code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Constant,
    Variable,
    Import,
    Type,
}

/// A symbol extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Language the symbol is written in.
    pub language: Language,
    /// File path.
    pub file: String,
    /// Start line (1-based).
    pub start_line: usize,
    /// End line (1-based).
    pub end_line: usize,
    /// The defining line of the symbol.
    pub signature: String,
    /// Visibility/scope (e.g., "pub", "private", "export").
    pub visibility: Option<String>,
    /// Documentation comment if present.
    pub doc: Option<String>,
}

/// Structural query — describe code patterns to search for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralQuery {
    /// Symbol kind filter.
    pub kind: Option<SymbolKind>,
    /// Name pattern (regex).
    pub name_pattern: Option<String>,
    /// Language filter.
    pub language: Option<Language>,
    /// Visibility filter.
    pub visibility: Option<String>,
    /// Search within documentation.
    pub doc_contains: Option<String>,
}

impl StructuralQuery {
    /// Create a query that matches functions by name pattern.
    pub fn functions(pattern: &str) -> Self {
        Self {
            kind: Some(SymbolKind::Function),
            name_pattern: Some(pattern.to_string()),
            language: None,
            visibility: None,
            doc_contains: None,
        }
    }

    /// Create a query that matches all symbols of a given kind.
    pub fn by_kind(kind: SymbolKind) -> Self {
        Self {
            kind: Some(kind),
            name_pattern: None,
            language: None,
            visibility: None,
            doc_contains: None,
        }
    }
}

/// Code intelligence engine — regex-based symbol extraction with language awareness.
///
/// Uses language-specific regex patterns for symbol detection. This provides
/// a practical code intelligence layer that works without external tree-sitter
/// grammar binaries, while producing structured [`Symbol`] output.
pub struct CodeIntelligence {
    patterns: HashMap<Language, Vec<SymbolPattern>>,
}

#[derive(Clone)]
struct SymbolPattern {
    kind: SymbolKind,
    regex: Regex,
    visibility_group: Option<usize>,
    name_group: usize,
}

impl CodeIntelligence {
    /// Create a new CodeIntelligence engine with built-in patterns for all supported languages.
    pub fn new() -> Self {
        let mut patterns: HashMap<Language, Vec<SymbolPattern>> = HashMap::new();

        // Rust patterns
        patterns.insert(
            Language::Rust,
            vec![
                SymbolPattern {
                    kind: SymbolKind::Function,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?(?:async\s+)?fn\s+(\w+)")
                        .unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Struct,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?struct\s+(\w+)").unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Enum,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?enum\s+(\w+)").unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Trait,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?trait\s+(\w+)").unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Module,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?mod\s+(\w+)").unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Constant,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?const\s+(\w+)").unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Type,
                    regex: Regex::new(r"(?m)^\s*(pub(?:\(crate\))?\s+)?type\s+(\w+)").unwrap(),
                    visibility_group: Some(1),
                    name_group: 2,
                },
                SymbolPattern {
                    kind: SymbolKind::Import,
                    regex: Regex::new(r"(?m)^\s*use\s+(.+);").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
            ],
        );

        // Python patterns
        patterns.insert(
            Language::Python,
            vec![
                SymbolPattern {
                    kind: SymbolKind::Function,
                    regex: Regex::new(r"(?m)^\s*(?:async\s+)?def\s+(\w+)").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
                SymbolPattern {
                    kind: SymbolKind::Class,
                    regex: Regex::new(r"(?m)^\s*class\s+(\w+)").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
                SymbolPattern {
                    kind: SymbolKind::Import,
                    regex: Regex::new(r"(?m)^\s*(?:from\s+\S+\s+)?import\s+(.+)").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
            ],
        );

        // JavaScript/TypeScript patterns (built once, cloned for both languages)
        let js_ts_patterns = vec![
            SymbolPattern {
                kind: SymbolKind::Function,
                regex: Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)").unwrap(),
                visibility_group: None,
                name_group: 1,
            },
            SymbolPattern {
                kind: SymbolKind::Class,
                regex: Regex::new(r"(?m)^\s*(?:export\s+)?class\s+(\w+)").unwrap(),
                visibility_group: None,
                name_group: 1,
            },
            SymbolPattern {
                kind: SymbolKind::Constant,
                regex: Regex::new(r"(?m)^\s*(?:export\s+)?const\s+(\w+)").unwrap(),
                visibility_group: None,
                name_group: 1,
            },
            SymbolPattern {
                kind: SymbolKind::Interface,
                regex: Regex::new(r"(?m)^\s*(?:export\s+)?interface\s+(\w+)").unwrap(),
                visibility_group: None,
                name_group: 1,
            },
            SymbolPattern {
                kind: SymbolKind::Import,
                regex: Regex::new(r"(?m)^\s*import\s+(.+)").unwrap(),
                visibility_group: None,
                name_group: 1,
            },
        ];
        patterns.insert(Language::JavaScript, js_ts_patterns.clone());
        patterns.insert(Language::TypeScript, js_ts_patterns);

        // Go patterns
        patterns.insert(
            Language::Go,
            vec![
                SymbolPattern {
                    kind: SymbolKind::Function,
                    regex: Regex::new(r"(?m)^func\s+(?:\([^)]+\)\s+)?(\w+)").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
                SymbolPattern {
                    kind: SymbolKind::Struct,
                    regex: Regex::new(r"(?m)^type\s+(\w+)\s+struct").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
                SymbolPattern {
                    kind: SymbolKind::Interface,
                    regex: Regex::new(r"(?m)^type\s+(\w+)\s+interface").unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
            ],
        );

        // Java patterns
        patterns.insert(
            Language::Java,
            vec![
                SymbolPattern {
                    kind: SymbolKind::Class,
                    regex: Regex::new(
                        r"(?m)^\s*(?:public|private|protected)?\s*(?:abstract\s+)?class\s+(\w+)",
                    )
                    .unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
                SymbolPattern {
                    kind: SymbolKind::Method,
                    regex: Regex::new(
                        r"(?m)^\s*(?:public|private|protected)?\s*(?:static\s+)?(?:\w+\s+)(\w+)\s*\(",
                    )
                    .unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
                SymbolPattern {
                    kind: SymbolKind::Interface,
                    regex: Regex::new(
                        r"(?m)^\s*(?:public\s+)?interface\s+(\w+)",
                    )
                    .unwrap(),
                    visibility_group: None,
                    name_group: 1,
                },
            ],
        );

        Self { patterns }
    }

    /// Extract all symbols from source code.
    pub fn extract_symbols(&self, source: &str, file_path: &str) -> Vec<Symbol> {
        let language = Language::from_path(Path::new(file_path));
        let lines: Vec<&str> = source.lines().collect();

        let Some(lang_patterns) = self.patterns.get(&language) else {
            return Vec::new();
        };

        let mut symbols = Vec::new();
        let comment_prefix = language.comment_prefix();

        for pattern in lang_patterns {
            for cap in pattern.regex.captures_iter(source) {
                let full_match = cap.get(0).unwrap();
                let name = cap.get(pattern.name_group).unwrap().as_str().to_string();

                // Calculate line number
                let start_byte = full_match.start();
                let start_line = source[..start_byte].lines().count() + 1;

                // Estimate end line (look for closing brace or next blank line)
                let end_line = find_block_end(&lines, start_line);

                // Extract visibility
                let visibility = pattern
                    .visibility_group
                    .and_then(|g| cap.get(g))
                    .map(|m| m.as_str().trim().to_string());

                // Extract doc comment
                let doc = extract_doc_comment(&lines, start_line, comment_prefix);

                // Get signature (the matching line)
                let signature = if start_line <= lines.len() {
                    lines[start_line - 1].trim().to_string()
                } else {
                    full_match.as_str().trim().to_string()
                };

                symbols.push(Symbol {
                    name,
                    kind: pattern.kind.clone(),
                    language,
                    file: file_path.to_string(),
                    start_line,
                    end_line,
                    signature,
                    visibility,
                    doc,
                });
            }
        }

        // Sort by line number for stable output
        symbols.sort_by_key(|s| s.start_line);
        symbols
    }

    /// Search for symbols matching a structural query.
    pub fn query_symbols(
        &self,
        source: &str,
        file_path: &str,
        query: &StructuralQuery,
    ) -> Vec<Symbol> {
        let mut symbols = self.extract_symbols(source, file_path);

        // Language filter
        if let Some(lang) = &query.language {
            symbols.retain(|s| s.language == *lang);
        }

        // Kind filter
        if let Some(kind) = &query.kind {
            symbols.retain(|s| s.kind == *kind);
        }

        // Name pattern filter
        if let Some(pattern) = &query.name_pattern {
            if let Ok(re) = Regex::new(pattern) {
                symbols.retain(|s| re.is_match(&s.name));
            }
        }

        // Visibility filter
        if let Some(vis) = &query.visibility {
            symbols.retain(|s| {
                s.visibility
                    .as_ref()
                    .is_some_and(|v| v.contains(vis.as_str()))
            });
        }

        // Doc filter
        if let Some(doc_text) = &query.doc_contains {
            symbols.retain(|s| {
                s.doc
                    .as_ref()
                    .is_some_and(|d| d.contains(doc_text.as_str()))
            });
        }

        symbols
    }

    /// Extract all symbols from a file.
    pub fn analyze_file(&self, path: &Path) -> Result<Vec<Symbol>> {
        let source = std::fs::read_to_string(path)?;
        let file_path = path.to_string_lossy().to_string();
        Ok(self.extract_symbols(&source, &file_path))
    }

    /// Extract all symbols from multiple files.
    pub fn analyze_files(&self, paths: &[&Path]) -> Result<Vec<Symbol>> {
        let mut all_symbols = Vec::new();
        for path in paths {
            match self.analyze_file(path) {
                Ok(symbols) => all_symbols.extend(symbols),
                Err(_) => continue, // skip unreadable files
            }
        }
        Ok(all_symbols)
    }
}

impl Default for CodeIntelligence {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolExtractor for CodeIntelligence {
    fn extract(&self, source: &str, file_path: &str, _language: Language) -> Vec<Symbol> {
        self.extract_symbols(source, file_path)
    }

    fn backend(&self) -> IntelBackend {
        IntelBackend::Regex
    }

    fn supported_languages(&self) -> Vec<Language> {
        self.patterns.keys().copied().collect()
    }
}

impl CodeIntelligence {
    /// Build a scope tree from a file.
    pub fn scope_tree(&self, source: &str, file_path: &str) -> ScopeNode {
        let symbols = self.extract_symbols(source, file_path);
        build_scope_tree(&symbols, file_path)
    }

    /// Build a call graph from a file.
    pub fn call_graph(&self, source: &str, file_path: &str) -> CallGraph {
        let symbols = self.extract_symbols(source, file_path);
        build_call_graph(source, &symbols, file_path)
    }
}

/// Find the approximate end of a code block starting at the given line.
fn find_block_end(lines: &[&str], start_line: usize) -> usize {
    if start_line == 0 || start_line > lines.len() {
        return start_line;
    }

    let mut depth = 0i32;
    let mut found_open = false;

    for (i, line) in lines.iter().enumerate().skip(start_line - 1) {
        for ch in line.chars() {
            if ch == '{' || ch == '(' {
                depth += 1;
                found_open = true;
            } else if ch == '}' || ch == ')' {
                depth -= 1;
            }
        }
        if found_open && depth <= 0 {
            return i + 1; // 1-based
        }
    }

    // For languages without braces (Python), use indentation
    if !found_open && start_line <= lines.len() {
        let base_indent = lines[start_line - 1]
            .len()
            .saturating_sub(lines[start_line - 1].trim_start().len());

        for (i, line) in lines.iter().enumerate().skip(start_line) {
            if line.trim().is_empty() {
                continue;
            }
            let indent = line.len().saturating_sub(line.trim_start().len());
            if indent <= base_indent {
                return i; // 1-based (i is 0-based index of line *after* block)
            }
        }
    }

    lines.len() // extends to end of file
}

/// Extract documentation comments preceding a symbol.
fn extract_doc_comment(
    lines: &[&str],
    start_line: usize,
    comment_prefix: Option<&str>,
) -> Option<String> {
    let prefix = comment_prefix?;

    let mut doc_lines = Vec::new();
    let start_idx = if start_line > 1 {
        start_line - 2
    } else {
        return None;
    };

    // Walk backwards from the line before the symbol
    for i in (0..=start_idx).rev() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with(prefix) {
            let comment_text = trimmed
                .strip_prefix(prefix)
                .unwrap_or("")
                .trim_start_matches(['/', '!', ' ']);
            doc_lines.push(comment_text.to_string());
        } else if trimmed.starts_with("///") || trimmed.starts_with("#") {
            let comment_text = trimmed.trim_start_matches(['/', '#', '!', ' ']);
            doc_lines.push(comment_text.to_string());
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        doc_lines.reverse();
        Some(doc_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("ts"), Language::TypeScript);
        assert_eq!(Language::from_extension("go"), Language::Go);
        assert_eq!(Language::from_extension("xyz"), Language::Unknown);
    }

    #[test]
    fn test_language_from_path() {
        assert_eq!(Language::from_path(Path::new("foo.rs")), Language::Rust);
        assert_eq!(Language::from_path(Path::new("bar.py")), Language::Python);
        assert_eq!(Language::from_path(Path::new("baz")), Language::Unknown);
    }

    #[test]
    fn test_language_name() {
        assert_eq!(Language::Rust.name(), "Rust");
        assert_eq!(Language::Python.name(), "Python");
        assert_eq!(Language::Cpp.name(), "C++");
    }

    #[test]
    fn test_language_comment_prefix() {
        assert_eq!(Language::Rust.comment_prefix(), Some("//"));
        assert_eq!(Language::Python.comment_prefix(), Some("#"));
        assert_eq!(Language::Json.comment_prefix(), None);
    }

    #[test]
    fn test_extract_rust_symbols() {
        let ci = CodeIntelligence::new();
        let source = r#"
/// A greeting function.
pub fn hello(name: &str) -> String {
    format!("Hello, {name}!")
}

struct Point {
    x: f64,
    y: f64,
}

pub enum Color {
    Red,
    Green,
    Blue,
}

pub trait Drawable {
    fn draw(&self);
}
"#;
        let symbols = ci.extract_symbols(source, "test.rs");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"Point"));
        assert!(names.contains(&"Color"));
        assert!(names.contains(&"Drawable"));

        let hello = symbols.iter().find(|s| s.name == "hello").unwrap();
        assert_eq!(hello.kind, SymbolKind::Function);
        assert!(hello.doc.is_some());
        assert!(hello.doc.as_ref().unwrap().contains("greeting"));
    }

    #[test]
    fn test_extract_python_symbols() {
        let ci = CodeIntelligence::new();
        let source = r#"
class MyClass:
    def method(self):
        pass

def standalone():
    return 42

import os
from pathlib import Path
"#;
        let symbols = ci.extract_symbols(source, "test.py");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyClass"));
        assert!(names.contains(&"method"));
        assert!(names.contains(&"standalone"));
    }

    #[test]
    fn test_extract_js_symbols() {
        let ci = CodeIntelligence::new();
        let source = r#"
export function greet(name) {
    console.log(name);
}

class Widget {
    constructor() {}
}

export const API_KEY = "abc";
"#;
        let symbols = ci.extract_symbols(source, "test.js");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"));
        assert!(names.contains(&"Widget"));
        assert!(names.contains(&"API_KEY"));
    }

    #[test]
    fn test_structural_query_functions() {
        let ci = CodeIntelligence::new();
        let source = r#"
pub fn search() {}
pub fn transform() {}
fn helper() {}
struct Config {}
"#;
        let query = StructuralQuery::functions("search|transform");
        let results = ci.query_symbols(source, "test.rs", &query);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|s| s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_structural_query_by_kind() {
        let ci = CodeIntelligence::new();
        let source = r#"
pub struct Alpha {}
pub struct Beta {}
pub fn gamma() {}
pub enum Delta {}
"#;
        let query = StructuralQuery::by_kind(SymbolKind::Struct);
        let results = ci.query_symbols(source, "test.rs", &query);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_structural_query_with_visibility() {
        let ci = CodeIntelligence::new();
        let source = r#"
pub fn public_fn() {}
fn private_fn() {}
pub fn another_pub() {}
"#;
        let query = StructuralQuery {
            kind: Some(SymbolKind::Function),
            name_pattern: None,
            language: None,
            visibility: Some("pub".into()),
            doc_contains: None,
        };
        let results = ci.query_symbols(source, "test.rs", &query);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_go_symbols() {
        let ci = CodeIntelligence::new();
        let source = r#"
func main() {
    fmt.Println("hello")
}

type Config struct {
    Name string
}

func (c *Config) Validate() error {
    return nil
}
"#;
        let symbols = ci.extract_symbols(source, "main.go");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"Config"));
        assert!(names.contains(&"Validate"));
    }

    #[test]
    fn test_scope_tree_basic() {
        let ci = CodeIntelligence::new();
        let source = r#"
pub fn outer() {
    let x = 1;
}

pub fn inner() {
    let y = 2;
}
"#;
        let tree = ci.scope_tree(source, "test.rs");
        assert_eq!(tree.name, "test.rs");
        assert!(tree.children.len() >= 2);
    }

    #[test]
    fn test_scope_at_line() {
        let ci = CodeIntelligence::new();
        let source = r#"
pub fn alpha() {
    let x = 1;
}

pub fn beta() {
    let y = 2;
}
"#;
        let tree = ci.scope_tree(source, "test.rs");
        // Line 3 (let x = 1;) should be inside alpha
        let scope = tree.scope_at_line(3);
        assert!(scope.is_some());
        assert_eq!(scope.unwrap().name, "alpha");
    }

    #[test]
    fn test_scope_path() {
        let ci = CodeIntelligence::new();
        let source = r#"
pub fn outer() {
    let x = 1;
}
"#;
        let tree = ci.scope_tree(source, "test.rs");
        let path = tree.scope_path_at_line(3);
        assert!(path.contains(&"test.rs".to_string()));
        assert!(path.contains(&"outer".to_string()));
    }

    #[test]
    fn test_call_graph_basic() {
        let ci = CodeIntelligence::new();
        let source = r#"
fn helper() {
    println!("help");
}

fn main() {
    helper();
    helper();
}
"#;
        let graph = ci.call_graph(source, "test.rs");
        assert!(!graph.functions.is_empty());
        let callees = graph.callees_of("main");
        assert!(!callees.is_empty());
        assert!(callees.iter().any(|e| e.callee == "helper"));
    }

    #[test]
    fn test_call_graph_roots_and_leaves() {
        let ci = CodeIntelligence::new();
        let source = r#"
fn leaf_fn() {
    println!("leaf");
}

fn middle() {
    leaf_fn();
}

fn entry() {
    middle();
}
"#;
        let graph = ci.call_graph(source, "test.rs");
        let roots = graph.roots();
        let leaves = graph.leaves();
        assert!(roots.contains(&"entry"));
        assert!(leaves.contains(&"leaf_fn"));
    }

    #[test]
    fn test_symbol_extractor_trait() {
        let ci = CodeIntelligence::new();
        assert_eq!(ci.backend(), IntelBackend::Regex);
        let langs = ci.supported_languages();
        assert!(langs.contains(&Language::Rust));
        assert!(langs.contains(&Language::Python));
    }
}
