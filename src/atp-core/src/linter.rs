//! # Configurable Text / Code Linter
//!
//! Rule-based linting with severity levels, autofix suggestions,
//! file-type targeting, and built-in rules for common text issues.

use regex::Regex;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Lint severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single lint diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Rule ID that produced this diagnostic.
    pub rule_id: String,
    /// Severity level.
    pub severity: Severity,
    /// 1-based line number.
    pub line: usize,
    /// 0-based column (byte offset within line).
    pub column: usize,
    /// Human-readable message.
    pub message: String,
    /// Suggested fix (if any).
    pub fix: Option<Fix>,
}

/// An autofix suggestion.
#[derive(Debug, Clone)]
pub struct Fix {
    /// Replacement text for the matched region.
    pub replacement: String,
    /// Start byte offset within the line.
    pub start: usize,
    /// End byte offset within the line (exclusive).
    pub end: usize,
}

/// Glob-like file-type filter.
#[derive(Debug, Clone)]
pub enum FileFilter {
    /// Matches any file.
    Any,
    /// Matches files whose name ends with the given suffix (e.g., ".rs").
    Extension(String),
    /// Matches files whose name contains the given substring.
    NameContains(String),
}

impl FileFilter {
    pub fn matches(&self, filename: &str) -> bool {
        match self {
            FileFilter::Any => true,
            FileFilter::Extension(ext) => filename.ends_with(ext.as_str()),
            FileFilter::NameContains(sub) => filename.contains(sub.as_str()),
        }
    }
}

/// A lint rule definition.
#[derive(Debug, Clone)]
pub struct LintRule {
    /// Unique rule identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Default severity.
    pub severity: Severity,
    /// File filter — which files this rule applies to.
    pub file_filter: FileFilter,
    /// Whether the rule is enabled.
    pub enabled: bool,
    /// The checker function (boxed for object safety).
    checker: RuleChecker,
}

/// Internal checker variant — keeps `LintRule` `Clone`-able.
#[derive(Debug, Clone)]
enum RuleChecker {
    TrailingWhitespace,
    LongLine(usize),
    InconsistentEol,
    TodoMarker,
    TabIndent,
    ConsecutiveBlankLines(usize),
    Regex { pattern: String, message: String },
}

impl RuleChecker {
    fn check(&self, lines: &[&str]) -> Vec<Diagnostic> {
        match self {
            RuleChecker::TrailingWhitespace => check_trailing_whitespace(lines),
            RuleChecker::LongLine(max) => check_long_lines(lines, *max),
            RuleChecker::InconsistentEol => check_inconsistent_eol(lines),
            RuleChecker::TodoMarker => check_todo_markers(lines),
            RuleChecker::TabIndent => check_tab_indent(lines),
            RuleChecker::ConsecutiveBlankLines(max) => check_consecutive_blanks(lines, *max),
            RuleChecker::Regex { pattern, message } => check_regex_pattern(lines, pattern, message),
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in checkers
// ---------------------------------------------------------------------------

fn check_trailing_whitespace(lines: &[&str]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end();
        if trimmed.len() < line.len() {
            diags.push(Diagnostic {
                rule_id: "trailing-whitespace".into(),
                severity: Severity::Warning,
                line: i + 1,
                column: trimmed.len(),
                message: "Trailing whitespace".into(),
                fix: Some(Fix {
                    replacement: trimmed.to_string(),
                    start: 0,
                    end: line.len(),
                }),
            });
        }
    }
    diags
}

fn check_long_lines(lines: &[&str], max: usize) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.len() > max {
            diags.push(Diagnostic {
                rule_id: "long-line".into(),
                severity: Severity::Warning,
                line: i + 1,
                column: max,
                message: format!("Line exceeds {} characters ({} chars)", max, line.len()),
                fix: None,
            });
        }
    }
    diags
}

fn check_inconsistent_eol(lines: &[&str]) -> Vec<Diagnostic> {
    let mut has_cr = false;
    let mut has_no_cr = false;

    for line in lines {
        if line.ends_with('\r') {
            has_cr = true;
        } else {
            has_no_cr = true;
        }
    }

    if has_cr && has_no_cr {
        vec![Diagnostic {
            rule_id: "inconsistent-eol".into(),
            severity: Severity::Warning,
            line: 1,
            column: 0,
            message: "File has inconsistent line endings (mixed CR and LF)".into(),
            fix: None,
        }]
    } else {
        Vec::new()
    }
}

fn check_todo_markers(lines: &[&str]) -> Vec<Diagnostic> {
    let re = Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b").unwrap();
    let mut diags = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for mat in re.find_iter(line) {
            diags.push(Diagnostic {
                rule_id: "todo-marker".into(),
                severity: Severity::Info,
                line: i + 1,
                column: mat.start(),
                message: format!("Found {} marker", mat.as_str()),
                fix: None,
            });
        }
    }
    diags
}

fn check_tab_indent(lines: &[&str]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with('\t') {
            diags.push(Diagnostic {
                rule_id: "tab-indent".into(),
                severity: Severity::Hint,
                line: i + 1,
                column: 0,
                message: "Line uses tab indentation".into(),
                fix: Some(Fix {
                    replacement: line.replace('\t', "    "),
                    start: 0,
                    end: line.len(),
                }),
            });
        }
    }
    diags
}

fn check_consecutive_blanks(lines: &[&str], max: usize) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut blank_count = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count > max {
                diags.push(Diagnostic {
                    rule_id: "consecutive-blanks".into(),
                    severity: Severity::Hint,
                    line: i + 1,
                    column: 0,
                    message: format!("More than {max} consecutive blank lines"),
                    fix: None,
                });
            }
        } else {
            blank_count = 0;
        }
    }
    diags
}

fn check_regex_pattern(lines: &[&str], pattern: &str, msg: &str) -> Vec<Diagnostic> {
    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut diags = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for mat in re.find_iter(line) {
            diags.push(Diagnostic {
                rule_id: "regex-match".into(),
                severity: Severity::Warning,
                line: i + 1,
                column: mat.start(),
                message: msg.to_string(),
                fix: None,
            });
        }
    }
    diags
}

// ---------------------------------------------------------------------------
// Linter
// ---------------------------------------------------------------------------

/// The linter engine holding a collection of lint rules.
#[derive(Debug, Clone)]
pub struct Linter {
    rules: Vec<LintRule>,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    /// Create a linter with no rules.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a linter pre-loaded with all built-in rules.
    pub fn with_defaults() -> Self {
        let mut linter = Self::new();
        linter.add_rule(Self::trailing_whitespace_rule());
        linter.add_rule(Self::long_line_rule(120));
        linter.add_rule(Self::todo_marker_rule());
        linter.add_rule(Self::tab_indent_rule());
        linter.add_rule(Self::consecutive_blanks_rule(2));
        linter
    }

    // -- Built-in rule factories --

    pub fn trailing_whitespace_rule() -> LintRule {
        LintRule {
            id: "trailing-whitespace".into(),
            name: "Trailing whitespace".into(),
            severity: Severity::Warning,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::TrailingWhitespace,
        }
    }

    pub fn long_line_rule(max: usize) -> LintRule {
        LintRule {
            id: "long-line".into(),
            name: format!("Line length > {max}"),
            severity: Severity::Warning,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::LongLine(max),
        }
    }

    pub fn inconsistent_eol_rule() -> LintRule {
        LintRule {
            id: "inconsistent-eol".into(),
            name: "Inconsistent line endings".into(),
            severity: Severity::Warning,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::InconsistentEol,
        }
    }

    pub fn todo_marker_rule() -> LintRule {
        LintRule {
            id: "todo-marker".into(),
            name: "TODO / FIXME markers".into(),
            severity: Severity::Info,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::TodoMarker,
        }
    }

    pub fn tab_indent_rule() -> LintRule {
        LintRule {
            id: "tab-indent".into(),
            name: "Tab indentation".into(),
            severity: Severity::Hint,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::TabIndent,
        }
    }

    pub fn consecutive_blanks_rule(max: usize) -> LintRule {
        LintRule {
            id: "consecutive-blanks".into(),
            name: format!("Max {max} consecutive blank lines"),
            severity: Severity::Hint,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::ConsecutiveBlankLines(max),
        }
    }

    pub fn regex_rule(id: &str, name: &str, pattern: &str, message: &str) -> LintRule {
        LintRule {
            id: id.to_string(),
            name: name.to_string(),
            severity: Severity::Warning,
            file_filter: FileFilter::Any,
            enabled: true,
            checker: RuleChecker::Regex {
                pattern: pattern.to_string(),
                message: message.to_string(),
            },
        }
    }

    // -- Management --

    pub fn add_rule(&mut self, rule: LintRule) {
        self.rules.push(rule);
    }

    pub fn set_enabled(&mut self, rule_id: &str, enabled: bool) {
        for r in &mut self.rules {
            if r.id == rule_id {
                r.enabled = enabled;
            }
        }
    }

    pub fn set_severity(&mut self, rule_id: &str, severity: Severity) {
        for r in &mut self.rules {
            if r.id == rule_id {
                r.severity = severity;
            }
        }
    }

    pub fn rule_ids(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.id.as_str()).collect()
    }

    // -- Linting --

    /// Lint text content, returning all diagnostics.
    pub fn lint(&self, content: &str, filename: Option<&str>) -> Vec<Diagnostic> {
        let lines: Vec<&str> = content.lines().collect();
        let mut all_diags = Vec::new();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            if let Some(fname) = filename {
                if !rule.file_filter.matches(fname) {
                    continue;
                }
            }

            let mut diags = rule.checker.check(&lines);
            // Override severity if rule has a different default
            for d in &mut diags {
                d.severity = rule.severity;
                d.rule_id.clone_from(&rule.id);
            }
            all_diags.extend(diags);
        }

        // Sort by line, then column
        all_diags.sort_by(|a, b| a.line.cmp(&b.line).then(a.column.cmp(&b.column)));
        all_diags
    }

    /// Apply all available autofixes and return the corrected text.
    pub fn autofix(&self, content: &str, filename: Option<&str>) -> String {
        let diags = self.lint(content, filename);
        let lines: Vec<&str> = content.lines().collect();
        let mut fixed: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        // Collect fixes keyed by (line - 1)
        let mut fixes_by_line: BTreeMap<usize, Vec<&Fix>> = BTreeMap::new();
        for d in &diags {
            if let Some(ref fix) = d.fix {
                fixes_by_line.entry(d.line - 1).or_default().push(fix);
            }
        }

        // Apply fixes (last fix per line wins for simplicity)
        for (line_idx, fixes) in &fixes_by_line {
            if let Some(last_fix) = fixes.last() {
                if *line_idx < fixed.len() {
                    fixed[*line_idx] = last_fix.replacement.clone();
                }
            }
        }

        fixed.join("\n")
    }

    /// Summarize diagnostics by severity.
    pub fn summary(diags: &[Diagnostic]) -> BTreeMap<Severity, usize> {
        let mut map = BTreeMap::new();
        for d in diags {
            *map.entry(d.severity).or_insert(0) += 1;
        }
        map
    }

    /// Format diagnostics as a human-readable report.
    pub fn format_report(diags: &[Diagnostic], filename: Option<&str>) -> String {
        let fname = filename.unwrap_or("<stdin>");
        let mut out = String::new();
        for d in diags {
            out.push_str(&format!(
                "{}:{}:{}: {} [{}] {}\n",
                fname, d.line, d.column, d.severity, d.rule_id, d.message,
            ));
        }
        let summary = Self::summary(diags);
        let total: usize = summary.values().sum();
        out.push_str(&format!(
            "\n{total} problem(s): {} error(s), {} warning(s), {} info, {} hint(s)\n",
            summary.get(&Severity::Error).unwrap_or(&0),
            summary.get(&Severity::Warning).unwrap_or(&0),
            summary.get(&Severity::Info).unwrap_or(&0),
            summary.get(&Severity::Hint).unwrap_or(&0),
        ));
        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trailing_whitespace() {
        let linter = Linter::with_defaults();
        let diags = linter.lint("hello   \nworld\n", None);
        assert!(diags.iter().any(|d| d.rule_id == "trailing-whitespace"));
    }

    #[test]
    fn test_no_trailing_whitespace() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::trailing_whitespace_rule());
        let diags = linter.lint("hello\nworld\n", None);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_long_line() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::long_line_rule(10));
        let diags = linter.lint("short\nthis is a very long line indeed\n", None);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 2);
    }

    #[test]
    fn test_todo_marker() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::todo_marker_rule());
        let diags = linter.lint("// TODO: fix this\n// FIXME: also this\n", None);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_tab_indent() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::tab_indent_rule());
        let diags = linter.lint("\thello\n    world\n", None);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].fix.is_some());
    }

    #[test]
    fn test_consecutive_blanks() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::consecutive_blanks_rule(1));
        let diags = linter.lint("a\n\n\n\nb\n", None);
        assert!(!diags.is_empty());
    }

    #[test]
    fn test_regex_rule() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::regex_rule(
            "no-console",
            "No console.log",
            r"console\.log",
            "Avoid console.log in production code",
        ));
        let diags = linter.lint("console.log('test');\nlet x = 1;\n", Some("app.js"));
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_file_filter() {
        let mut rule = Linter::trailing_whitespace_rule();
        rule.file_filter = FileFilter::Extension(".rs".into());
        let mut linter = Linter::new();
        linter.add_rule(rule);
        let diags_rs = linter.lint("hello   \n", Some("main.rs"));
        let diags_js = linter.lint("hello   \n", Some("main.js"));
        assert_eq!(diags_rs.len(), 1);
        assert!(diags_js.is_empty());
    }

    #[test]
    fn test_autofix_trailing_whitespace() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::trailing_whitespace_rule());
        let fixed = linter.autofix("hello   \nworld  \n", None);
        assert_eq!(fixed, "hello\nworld");
    }

    #[test]
    fn test_autofix_tabs() {
        let mut linter = Linter::new();
        linter.add_rule(Linter::tab_indent_rule());
        let fixed = linter.autofix("\thello\n", None);
        assert_eq!(fixed, "    hello");
    }

    #[test]
    fn test_set_enabled() {
        let mut linter = Linter::with_defaults();
        linter.set_enabled("trailing-whitespace", false);
        let diags = linter.lint("hello   \n", None);
        assert!(!diags.iter().any(|d| d.rule_id == "trailing-whitespace"));
    }

    #[test]
    fn test_set_severity() {
        let mut linter = Linter::with_defaults();
        linter.set_severity("trailing-whitespace", Severity::Error);
        let diags = linter.lint("hello   \n", None);
        let tw = diags.iter().find(|d| d.rule_id == "trailing-whitespace");
        assert_eq!(tw.unwrap().severity, Severity::Error);
    }

    #[test]
    fn test_format_report() {
        let linter = Linter::with_defaults();
        let diags = linter.lint("hello   \n// TODO: fix\n", Some("test.rs"));
        let report = Linter::format_report(&diags, Some("test.rs"));
        assert!(report.contains("test.rs"));
        assert!(report.contains("problem(s)"));
    }

    #[test]
    fn test_summary() {
        let linter = Linter::with_defaults();
        let diags = linter.lint("hello   \n// TODO fix\n\thello\n", None);
        let summary = Linter::summary(&diags);
        assert!(summary.values().sum::<usize>() > 0);
    }
}
