//! # Multi-Pass Rewrite Rules Engine
//!
//! Named rule sets with match pattern + replacement template, scope constraints,
//! dry-run preview, and rewrite history tracking.

use regex::Regex;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Scope constraint — which part of the text a rule applies to.
#[derive(Debug, Clone, Default)]
pub enum Scope {
    /// Applies to every line.
    #[default]
    All,
    /// Applies only to lines in the given 1-based range (inclusive).
    LineRange(usize, usize),
    /// Applies only to lines matching the given regex.
    MatchingLines(String),
    /// Applies only to lines NOT matching the given regex.
    ExcludeLines(String),
}

/// A single rewrite rule.
#[derive(Debug, Clone)]
pub struct RewriteRule {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Regex pattern to match.
    pub pattern: String,
    /// Replacement template (supports `$1`, `$2`, etc.).
    pub replacement: String,
    /// Scope constraint.
    pub scope: Scope,
    /// Whether this rule is enabled.
    pub enabled: bool,
    /// Maximum number of times to apply (0 = unlimited).
    pub max_applications: usize,
}

impl RewriteRule {
    /// Create a new rewrite rule.
    pub fn new(id: &str, pattern: &str, replacement: &str) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(),
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            scope: Scope::All,
            enabled: true,
            max_applications: 0,
        }
    }

    /// Set name.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set scope.
    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }

    /// Set max applications.
    pub fn with_max(mut self, max: usize) -> Self {
        self.max_applications = max;
        self
    }
}

/// A named set of rewrite rules applied in order.
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// Name of this rule set.
    pub name: String,
    /// Description.
    pub description: String,
    /// Ordered rules.
    pub rules: Vec<RewriteRule>,
    /// Number of passes to run (1 = single pass, >1 = multi-pass until stable or max).
    pub max_passes: usize,
}

impl RuleSet {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            rules: Vec::new(),
            max_passes: 1,
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    pub fn with_max_passes(mut self, n: usize) -> Self {
        self.max_passes = n;
        self
    }

    pub fn add_rule(&mut self, rule: RewriteRule) {
        self.rules.push(rule);
    }
}

/// A single rewrite application record.
#[derive(Debug, Clone)]
pub struct RewriteRecord {
    /// Rule ID that was applied.
    pub rule_id: String,
    /// 1-based line number.
    pub line: usize,
    /// Original text of the line.
    pub before: String,
    /// New text of the line.
    pub after: String,
    /// Pass number (1-based).
    pub pass: usize,
}

/// Result of a rewrite operation.
#[derive(Debug, Clone)]
pub struct RewriteResult {
    /// The rewritten text.
    pub text: String,
    /// History of all rewrites applied.
    pub history: Vec<RewriteRecord>,
    /// Number of passes executed.
    pub passes: usize,
    /// Total number of individual rewrites applied.
    pub total_rewrites: usize,
}

/// Rewrite engine statistics.
#[derive(Debug, Clone, Default)]
pub struct RewriteStats {
    pub rules_applied: BTreeMap<String, usize>,
    pub total_rewrites: usize,
    pub total_passes: usize,
    pub lines_affected: usize,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The rewrite engine.
#[derive(Debug, Clone, Default)]
pub struct RewriteEngine {
    rule_sets: BTreeMap<String, RuleSet>,
}

impl RewriteEngine {
    pub fn new() -> Self {
        Self { rule_sets: BTreeMap::new() }
    }

    /// Register a rule set.
    pub fn add_rule_set(&mut self, rule_set: RuleSet) {
        self.rule_sets.insert(rule_set.name.clone(), rule_set);
    }

    /// Remove a rule set.
    pub fn remove_rule_set(&mut self, name: &str) -> Option<RuleSet> {
        self.rule_sets.remove(name)
    }

    /// List rule set names.
    pub fn rule_set_names(&self) -> Vec<&str> {
        self.rule_sets.keys().map(|s| s.as_str()).collect()
    }

    /// Get a rule set by name.
    pub fn get_rule_set(&self, name: &str) -> Option<&RuleSet> {
        self.rule_sets.get(name)
    }

    /// Dry-run: preview what would change without modifying the text.
    pub fn dry_run(&self, text: &str, rule_set_name: &str) -> RewriteResult {
        self.apply_internal(text, rule_set_name, true)
    }

    /// Apply a rule set to the text.
    pub fn apply(&self, text: &str, rule_set_name: &str) -> RewriteResult {
        self.apply_internal(text, rule_set_name, false)
    }

    /// Apply all registered rule sets in order.
    pub fn apply_all(&self, text: &str) -> RewriteResult {
        let mut current = text.to_string();
        let mut all_history = Vec::new();
        let mut total_passes = 0;
        let mut total_rewrites = 0;

        for name in self.rule_sets.keys() {
            let result = self.apply_internal(&current, name, false);
            current = result.text;
            all_history.extend(result.history);
            total_passes += result.passes;
            total_rewrites += result.total_rewrites;
        }

        RewriteResult {
            text: current,
            history: all_history,
            passes: total_passes,
            total_rewrites,
        }
    }

    fn apply_internal(&self, text: &str, rule_set_name: &str, _dry_run: bool) -> RewriteResult {
        let rule_set = match self.rule_sets.get(rule_set_name) {
            Some(rs) => rs,
            None => return RewriteResult {
                text: text.to_string(),
                history: Vec::new(),
                passes: 0,
                total_rewrites: 0,
            },
        };

        let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let mut history = Vec::new();
        let mut pass = 0;

        loop {
            pass += 1;
            let mut changed = false;

            for rule in &rule_set.rules {
                if !rule.enabled {
                    continue;
                }

                let re = match Regex::new(&rule.pattern) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let mut applications = 0usize;

                for (i, line) in lines.iter_mut().enumerate() {
                    if rule.max_applications > 0 && applications >= rule.max_applications {
                        break;
                    }

                    if !in_scope(&rule.scope, i + 1, line) {
                        continue;
                    }

                    let new_line = re.replace_all(line, rule.replacement.as_str()).to_string();
                    if new_line != *line {
                        history.push(RewriteRecord {
                            rule_id: rule.id.clone(),
                            line: i + 1,
                            before: line.clone(),
                            after: new_line.clone(),
                            pass,
                        });
                        *line = new_line;
                        applications += 1;
                        changed = true;
                    }
                }
            }

            if !changed || pass >= rule_set.max_passes {
                break;
            }
        }

        let total_rewrites = history.len();
        RewriteResult {
            text: lines.join("\n"),
            history,
            passes: pass,
            total_rewrites,
        }
    }

    /// Compute statistics from a rewrite result.
    pub fn stats(result: &RewriteResult) -> RewriteStats {
        let mut rules_applied: BTreeMap<String, usize> = BTreeMap::new();
        let mut lines_affected = std::collections::BTreeSet::new();
        for rec in &result.history {
            *rules_applied.entry(rec.rule_id.clone()).or_insert(0) += 1;
            lines_affected.insert(rec.line);
        }
        RewriteStats {
            rules_applied,
            total_rewrites: result.total_rewrites,
            total_passes: result.passes,
            lines_affected: lines_affected.len(),
        }
    }
}

fn in_scope(scope: &Scope, line_num: usize, line: &str) -> bool {
    match scope {
        Scope::All => true,
        Scope::LineRange(start, end) => line_num >= *start && line_num <= *end,
        Scope::MatchingLines(pat) => {
            Regex::new(pat).map(|re| re.is_match(line)).unwrap_or(false)
        }
        Scope::ExcludeLines(pat) => {
            Regex::new(pat).map(|re| !re.is_match(line)).unwrap_or(true)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_replace() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("fix");
        rs.add_rule(RewriteRule::new("r1", "foo", "bar"));
        engine.add_rule_set(rs);
        let result = engine.apply("foo baz foo", "fix");
        assert_eq!(result.text, "bar baz bar");
        assert_eq!(result.total_rewrites, 1);
    }

    #[test]
    fn test_regex_capture() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("caps");
        rs.add_rule(RewriteRule::new("r1", r"(\w+)@(\w+)", "$2/$1"));
        engine.add_rule_set(rs);
        let result = engine.apply("user@host", "caps");
        assert_eq!(result.text, "host/user");
    }

    #[test]
    fn test_multi_line() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("ml");
        rs.add_rule(RewriteRule::new("r1", "old", "new"));
        engine.add_rule_set(rs);
        let result = engine.apply("old line\nanother old\nclean", "ml");
        assert_eq!(result.text, "new line\nanother new\nclean");
        assert_eq!(result.total_rewrites, 2);
    }

    #[test]
    fn test_scope_line_range() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("scoped");
        rs.add_rule(RewriteRule::new("r1", "x", "y").with_scope(Scope::LineRange(2, 3)));
        engine.add_rule_set(rs);
        let result = engine.apply("x\nx\nx\nx", "scoped");
        assert_eq!(result.text, "x\ny\ny\nx");
    }

    #[test]
    fn test_scope_matching_lines() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("match");
        rs.add_rule(
            RewriteRule::new("r1", "val", "VAL")
                .with_scope(Scope::MatchingLines("^#".into())),
        );
        engine.add_rule_set(rs);
        let result = engine.apply("# val here\nval there", "match");
        assert_eq!(result.text, "# VAL here\nval there");
    }

    #[test]
    fn test_scope_exclude_lines() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("excl");
        rs.add_rule(
            RewriteRule::new("r1", "a", "A")
                .with_scope(Scope::ExcludeLines("^//".into())),
        );
        engine.add_rule_set(rs);
        let result = engine.apply("a test\n// a comment\na again", "excl");
        assert_eq!(result.text, "A test\n// a comment\nA AgAin");
    }

    #[test]
    fn test_multi_pass() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("mp").with_max_passes(3);
        rs.add_rule(RewriteRule::new("r1", "aa", "a"));
        engine.add_rule_set(rs);
        let result = engine.apply("aaaa", "mp");
        // aaaa → aa → a (2 passes to stabilize; but max_passes=3 allows it)
        assert_eq!(result.text, "a");
        assert!(result.passes >= 2);
    }

    #[test]
    fn test_max_applications() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("maxapp");
        rs.add_rule(RewriteRule::new("r1", "x", "y").with_max(1));
        engine.add_rule_set(rs);
        let result = engine.apply("x\nx\nx", "maxapp");
        // Only first line should be replaced (max_applications = 1)
        assert_eq!(result.text, "y\nx\nx");
    }

    #[test]
    fn test_disabled_rule() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("dis");
        let mut rule = RewriteRule::new("r1", "a", "b");
        rule.enabled = false;
        rs.add_rule(rule);
        engine.add_rule_set(rs);
        let result = engine.apply("a test", "dis");
        assert_eq!(result.text, "a test");
    }

    #[test]
    fn test_dry_run() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("dr");
        rs.add_rule(RewriteRule::new("r1", "old", "new"));
        engine.add_rule_set(rs);
        let result = engine.dry_run("old text", "dr");
        // dry_run still returns the rewritten text + history for preview
        assert_eq!(result.text, "new text");
        assert_eq!(result.total_rewrites, 1);
    }

    #[test]
    fn test_history_tracking() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("hist");
        rs.add_rule(RewriteRule::new("r1", "A", "B"));
        engine.add_rule_set(rs);
        let result = engine.apply("A line\nA again", "hist");
        assert_eq!(result.history.len(), 2);
        assert_eq!(result.history[0].before, "A line");
        assert_eq!(result.history[0].after, "B line");
        assert_eq!(result.history[0].rule_id, "r1");
    }

    #[test]
    fn test_apply_all_rule_sets() {
        let mut engine = RewriteEngine::new();
        let mut rs1 = RuleSet::new("a");
        rs1.add_rule(RewriteRule::new("r1", "1", "2"));
        let mut rs2 = RuleSet::new("b");
        rs2.add_rule(RewriteRule::new("r2", "2", "3"));
        engine.add_rule_set(rs1);
        engine.add_rule_set(rs2);
        let result = engine.apply_all("1 and 1");
        assert_eq!(result.text, "3 and 3");
    }

    #[test]
    fn test_stats() {
        let mut engine = RewriteEngine::new();
        let mut rs = RuleSet::new("s");
        rs.add_rule(RewriteRule::new("r1", "x", "y"));
        engine.add_rule_set(rs);
        let result = engine.apply("x\nx\nz", "s");
        let stats = RewriteEngine::stats(&result);
        assert_eq!(stats.total_rewrites, 2);
        assert_eq!(stats.lines_affected, 2);
        assert_eq!(*stats.rules_applied.get("r1").unwrap(), 2);
    }

    #[test]
    fn test_nonexistent_rule_set() {
        let engine = RewriteEngine::new();
        let result = engine.apply("hello", "nonexistent");
        assert_eq!(result.text, "hello");
        assert_eq!(result.total_rewrites, 0);
    }

    #[test]
    fn test_rule_set_management() {
        let mut engine = RewriteEngine::new();
        engine.add_rule_set(RuleSet::new("alpha"));
        engine.add_rule_set(RuleSet::new("beta"));
        assert_eq!(engine.rule_set_names().len(), 2);
        engine.remove_rule_set("alpha");
        assert_eq!(engine.rule_set_names().len(), 1);
    }
}
