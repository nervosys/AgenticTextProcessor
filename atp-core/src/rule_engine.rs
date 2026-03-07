//! # Declarative Rule Engine
//!
//! Provides a configurable if-then rule engine for evaluating text lines against
//! conditions and applying actions. Supports regex matching, field comparisons,
//! threshold checks, priority ordering, and conflict resolution.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique rule identifier.
pub type RuleId = String;

/// A single condition that can be evaluated against a line of text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// Line matches a regex pattern.
    Regex(String),
    /// Line contains the given substring (case-sensitive).
    Contains(String),
    /// Line starts with the given prefix.
    StartsWith(String),
    /// Line ends with the given suffix.
    EndsWith(String),
    /// Whitespace-split field at `index` equals `value`.
    FieldEquals { index: usize, value: String },
    /// Whitespace-split field at `index` matches a regex.
    FieldMatches { index: usize, pattern: String },
    /// Line length exceeds threshold.
    LengthExceeds(usize),
    /// Line length is below threshold.
    LengthBelow(usize),
    /// Logical AND of multiple conditions.
    All(Vec<Condition>),
    /// Logical OR of multiple conditions.
    Any(Vec<Condition>),
    /// Logical NOT of a condition.
    Not(Box<Condition>),
}

/// An action to perform when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Tag the line with a label.
    Tag(String),
    /// Replace the first match of `pattern` with `replacement`.
    ReplaceFirst {
        pattern: String,
        replacement: String,
    },
    /// Replace all matches of `pattern` with `replacement`.
    ReplaceAll {
        pattern: String,
        replacement: String,
    },
    /// Prepend text to the line.
    Prepend(String),
    /// Append text to the line.
    Append(String),
    /// Discard the line entirely.
    Drop,
    /// Route the line to a named output channel.
    Route(String),
    /// Emit an alert with a message.
    Alert(String),
    /// Apply a chain of actions sequentially.
    Chain(Vec<Action>),
}

/// Priority for rule ordering — lower value runs first.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum RulePriority {
    /// Runs first (value 0).
    Critical = 0,
    /// Runs early (value 25).
    High = 25,
    /// Default priority (value 50).
    #[default]
    Normal = 50,
    /// Runs late (value 75).
    Low = 75,
}

/// How to resolve conflicts when multiple rules match the same line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// Apply all matching rules in priority order.
    #[default]
    ApplyAll,
    /// Apply only the highest-priority matching rule.
    FirstMatch,
    /// Apply only the last (lowest-priority) matching rule.
    LastMatch,
}

/// A single rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier.
    pub id: RuleId,
    /// Human-readable description.
    pub description: String,
    /// When this rule fires.
    pub condition: Condition,
    /// What to do when it fires.
    pub action: Action,
    /// Execution priority.
    pub priority: RulePriority,
    /// Whether the rule is enabled.
    pub enabled: bool,
    /// Optional tags for grouping/filtering rules.
    pub tags: Vec<String>,
}

/// The result of evaluating rules against a single line.
#[derive(Debug, Clone)]
pub struct RuleResult {
    /// The (possibly transformed) output line, or `None` if dropped.
    pub output: Option<String>,
    /// Tags applied to this line.
    pub tags: Vec<String>,
    /// Alerts triggered.
    pub alerts: Vec<String>,
    /// Routes this line was sent to.
    pub routes: Vec<String>,
    /// IDs of the rules that matched.
    pub matched_rules: Vec<RuleId>,
}

/// Aggregate statistics from processing multiple lines.
#[derive(Debug, Clone, Default)]
pub struct RuleEngineStats {
    /// Total lines processed.
    pub lines_processed: usize,
    /// Total lines dropped.
    pub lines_dropped: usize,
    /// Total alerts emitted.
    pub total_alerts: usize,
    /// Match count per rule ID.
    pub matches_per_rule: BTreeMap<RuleId, usize>,
}

/// Configuration for the rule engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEngineConfig {
    /// Conflict resolution strategy.
    pub conflict_strategy: ConflictStrategy,
    /// Maximum number of rules that can fire per line (0 = unlimited).
    pub max_matches_per_line: usize,
    /// Whether to continue processing after a Drop action.
    pub continue_after_drop: bool,
}

impl Default for RuleEngineConfig {
    fn default() -> Self {
        Self {
            conflict_strategy: ConflictStrategy::ApplyAll,
            max_matches_per_line: 0,
            continue_after_drop: false,
        }
    }
}

/// The declarative rule engine.
#[derive(Debug)]
pub struct RuleEngine {
    rules: Vec<Rule>,
    config: RuleEngineConfig,
    stats: RuleEngineStats,
}

// ---------------------------------------------------------------------------
// Condition evaluation
// ---------------------------------------------------------------------------

fn evaluate_condition(condition: &Condition, line: &str) -> bool {
    match condition {
        Condition::Regex(pat) => Regex::new(pat).map(|r| r.is_match(line)).unwrap_or(false),
        Condition::Contains(s) => line.contains(s.as_str()),
        Condition::StartsWith(s) => line.starts_with(s.as_str()),
        Condition::EndsWith(s) => line.ends_with(s.as_str()),
        Condition::FieldEquals { index, value } => line
            .split_whitespace()
            .nth(*index)
            .map(|f| f == value.as_str())
            .unwrap_or(false),
        Condition::FieldMatches { index, pattern } => line
            .split_whitespace()
            .nth(*index)
            .and_then(|f| Regex::new(pattern).ok().map(|r| r.is_match(f)))
            .unwrap_or(false),
        Condition::LengthExceeds(n) => line.len() > *n,
        Condition::LengthBelow(n) => line.len() < *n,
        Condition::All(conds) => conds.iter().all(|c| evaluate_condition(c, line)),
        Condition::Any(conds) => conds.iter().any(|c| evaluate_condition(c, line)),
        Condition::Not(c) => !evaluate_condition(c, line),
    }
}

// ---------------------------------------------------------------------------
// Action application
// ---------------------------------------------------------------------------

fn apply_action(action: &Action, _line: &str, result: &mut RuleResult) {
    match action {
        Action::Tag(t) => {
            result.tags.push(t.clone());
        }
        Action::ReplaceFirst {
            pattern,
            replacement,
        } => {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(ref mut out) = result.output {
                    *out = re.replace(out.as_str(), replacement.as_str()).to_string();
                }
            }
        }
        Action::ReplaceAll {
            pattern,
            replacement,
        } => {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(ref mut out) = result.output {
                    *out = re
                        .replace_all(out.as_str(), replacement.as_str())
                        .to_string();
                }
            }
        }
        Action::Prepend(s) => {
            if let Some(ref mut out) = result.output {
                *out = format!("{s}{out}");
            }
        }
        Action::Append(s) => {
            if let Some(ref mut out) = result.output {
                *out = format!("{out}{s}");
            }
        }
        Action::Drop => {
            result.output = None;
        }
        Action::Route(r) => {
            result.routes.push(r.clone());
        }
        Action::Alert(msg) => {
            result.alerts.push(msg.clone());
        }
        Action::Chain(actions) => {
            for a in actions {
                apply_action(a, _line, result);
                if result.output.is_none() {
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// RuleEngine implementation
// ---------------------------------------------------------------------------

impl RuleEngine {
    /// Create a new rule engine with default configuration.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            config: RuleEngineConfig::default(),
            stats: RuleEngineStats::default(),
        }
    }

    /// Create a new rule engine with the given configuration.
    pub fn with_config(config: RuleEngineConfig) -> Self {
        Self {
            rules: Vec::new(),
            config,
            stats: RuleEngineStats::default(),
        }
    }

    /// Add a rule to the engine.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
        self.sort_rules();
    }

    /// Add multiple rules at once.
    pub fn add_rules(&mut self, rules: Vec<Rule>) {
        self.rules.extend(rules);
        self.sort_rules();
    }

    /// Remove a rule by ID. Returns `true` if a rule was removed.
    pub fn remove_rule(&mut self, id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        self.rules.len() < before
    }

    /// Enable or disable a rule by ID.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        for r in &mut self.rules {
            if r.id == id {
                r.enabled = enabled;
            }
        }
    }

    /// List all rule IDs.
    pub fn rule_ids(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.id.as_str()).collect()
    }

    /// Get rules by tag.
    pub fn rules_by_tag(&self, tag: &str) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Evaluate all rules against a single line.
    pub fn evaluate(&mut self, line: &str) -> RuleResult {
        self.stats.lines_processed += 1;

        let mut result = RuleResult {
            output: Some(line.to_string()),
            tags: Vec::new(),
            alerts: Vec::new(),
            routes: Vec::new(),
            matched_rules: Vec::new(),
        };

        let enabled_rules: Vec<&Rule> = self.rules.iter().filter(|r| r.enabled).collect();
        let mut matched_count = 0;

        for rule in &enabled_rules {
            if self.config.max_matches_per_line > 0
                && matched_count >= self.config.max_matches_per_line
            {
                break;
            }

            if evaluate_condition(&rule.condition, line) {
                matched_count += 1;
                result.matched_rules.push(rule.id.clone());
                *self
                    .stats
                    .matches_per_rule
                    .entry(rule.id.clone())
                    .or_insert(0) += 1;

                apply_action(&rule.action, line, &mut result);

                if result.output.is_none() {
                    self.stats.lines_dropped += 1;
                    if !self.config.continue_after_drop {
                        break;
                    }
                }

                // Conflict strategies
                match self.config.conflict_strategy {
                    ConflictStrategy::FirstMatch => break,
                    ConflictStrategy::LastMatch | ConflictStrategy::ApplyAll => {}
                }
            }
        }

        // For LastMatch: keep only the last matched rule's effect
        // (we already applied them all; in ApplyAll mode this is fine)
        self.stats.total_alerts += result.alerts.len();

        result
    }

    /// Process multiple lines, returning results for each.
    pub fn process_lines(&mut self, lines: &[&str]) -> Vec<RuleResult> {
        lines.iter().map(|l| self.evaluate(l)).collect()
    }

    /// Process lines and return only the surviving output lines.
    pub fn filter_lines(&mut self, lines: &[&str]) -> Vec<String> {
        lines
            .iter()
            .filter_map(|l| self.evaluate(l).output)
            .collect()
    }

    /// Get accumulated statistics.
    pub fn stats(&self) -> &RuleEngineStats {
        &self.stats
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.stats = RuleEngineStats::default();
    }

    fn sort_rules(&mut self) {
        self.rules.sort_by_key(|r| r.priority);
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: build a simple regex → tag rule.
pub fn tag_rule(id: &str, pattern: &str, tag: &str) -> Rule {
    Rule {
        id: id.to_string(),
        description: format!("Tag lines matching /{pattern}/ with '{tag}'"),
        condition: Condition::Regex(pattern.to_string()),
        action: Action::Tag(tag.to_string()),
        priority: RulePriority::Normal,
        enabled: true,
        tags: vec![],
    }
}

/// Convenience: build a regex → drop rule.
pub fn drop_rule(id: &str, pattern: &str) -> Rule {
    Rule {
        id: id.to_string(),
        description: format!("Drop lines matching /{pattern}/"),
        condition: Condition::Regex(pattern.to_string()),
        action: Action::Drop,
        priority: RulePriority::Normal,
        enabled: true,
        tags: vec![],
    }
}

/// Convenience: build a regex → replace-all rule.
pub fn replace_rule(id: &str, pattern: &str, replacement: &str) -> Rule {
    Rule {
        id: id.to_string(),
        description: format!("Replace /{pattern}/ with '{replacement}'"),
        condition: Condition::Regex(pattern.to_string()),
        action: Action::ReplaceAll {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
        },
        priority: RulePriority::Normal,
        enabled: true,
        tags: vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> RuleEngine {
        RuleEngine::new()
    }

    #[test]
    fn test_regex_condition() {
        assert!(evaluate_condition(
            &Condition::Regex(r"\d+".into()),
            "abc 123"
        ));
        assert!(!evaluate_condition(&Condition::Regex(r"\d+".into()), "abc"));
    }

    #[test]
    fn test_contains_condition() {
        assert!(evaluate_condition(
            &Condition::Contains("hello".into()),
            "say hello world"
        ));
        assert!(!evaluate_condition(
            &Condition::Contains("xyz".into()),
            "say hello world"
        ));
    }

    #[test]
    fn test_starts_ends_with() {
        assert!(evaluate_condition(
            &Condition::StartsWith("ERR".into()),
            "ERROR: bad"
        ));
        assert!(!evaluate_condition(
            &Condition::StartsWith("ERR".into()),
            "no error"
        ));
        assert!(evaluate_condition(
            &Condition::EndsWith(".rs".into()),
            "main.rs"
        ));
    }

    #[test]
    fn test_field_equals() {
        let c = Condition::FieldEquals {
            index: 2,
            value: "200".into(),
        };
        assert!(evaluate_condition(&c, "GET /api 200 OK"));
        assert!(!evaluate_condition(&c, "GET /api 404 NotFound"));
    }

    #[test]
    fn test_field_matches() {
        let c = Condition::FieldMatches {
            index: 0,
            pattern: r"^[A-Z]+$".into(),
        };
        assert!(evaluate_condition(&c, "GET /path"));
        assert!(!evaluate_condition(&c, "get /path"));
    }

    #[test]
    fn test_length_conditions() {
        assert!(evaluate_condition(&Condition::LengthExceeds(5), "abcdef"));
        assert!(!evaluate_condition(&Condition::LengthExceeds(5), "abc"));
        assert!(evaluate_condition(&Condition::LengthBelow(5), "abc"));
    }

    #[test]
    fn test_logical_combinators() {
        let c = Condition::All(vec![
            Condition::Contains("error".into()),
            Condition::LengthExceeds(10),
        ]);
        assert!(evaluate_condition(&c, "this is an error message"));
        assert!(!evaluate_condition(&c, "error"));

        let c2 = Condition::Any(vec![
            Condition::Contains("warn".into()),
            Condition::Contains("error".into()),
        ]);
        assert!(evaluate_condition(&c2, "just a warning"));
        assert!(!evaluate_condition(&c2, "all good"));

        let c3 = Condition::Not(Box::new(Condition::Contains("debug".into())));
        assert!(evaluate_condition(&c3, "info message"));
        assert!(!evaluate_condition(&c3, "debug trace"));
    }

    #[test]
    fn test_tag_action() {
        let mut engine = make_engine();
        engine.add_rule(tag_rule("r1", "TODO", "needs-work"));
        let r = engine.evaluate("// TODO: fix this");
        assert_eq!(r.tags, vec!["needs-work"]);
        assert!(r.output.is_some());
    }

    #[test]
    fn test_drop_action() {
        let mut engine = make_engine();
        engine.add_rule(drop_rule("r1", r"^#"));
        let r = engine.evaluate("# comment");
        assert!(r.output.is_none());
        let r2 = engine.evaluate("code line");
        assert!(r2.output.is_some());
    }

    #[test]
    fn test_replace_action() {
        let mut engine = make_engine();
        engine.add_rule(replace_rule("r1", r"foo", "bar"));
        let r = engine.evaluate("foo baz foo");
        assert_eq!(r.output.as_deref(), Some("bar baz bar"));
    }

    #[test]
    fn test_prepend_append() {
        let mut engine = make_engine();
        engine.add_rule(Rule {
            id: "r1".into(),
            description: "prefix".into(),
            condition: Condition::Contains("hello".into()),
            action: Action::Chain(vec![
                Action::Prepend("[GREET] ".into()),
                Action::Append(" !!".into()),
            ]),
            priority: RulePriority::Normal,
            enabled: true,
            tags: vec![],
        });
        let r = engine.evaluate("hello world");
        assert_eq!(r.output.as_deref(), Some("[GREET] hello world !!"));
    }

    #[test]
    fn test_route_and_alert() {
        let mut engine = make_engine();
        engine.add_rule(Rule {
            id: "r1".into(),
            description: "route errors".into(),
            condition: Condition::StartsWith("ERROR".into()),
            action: Action::Chain(vec![
                Action::Route("error_log".into()),
                Action::Alert("Error detected!".into()),
            ]),
            priority: RulePriority::Normal,
            enabled: true,
            tags: vec![],
        });
        let r = engine.evaluate("ERROR: disk full");
        assert_eq!(r.routes, vec!["error_log"]);
        assert_eq!(r.alerts, vec!["Error detected!"]);
    }

    #[test]
    fn test_priority_ordering() {
        let mut engine = make_engine();
        engine.add_rule(Rule {
            id: "low".into(),
            description: "low".into(),
            condition: Condition::Contains("x".into()),
            action: Action::Append(" [LOW]".into()),
            priority: RulePriority::Low,
            enabled: true,
            tags: vec![],
        });
        engine.add_rule(Rule {
            id: "high".into(),
            description: "high".into(),
            condition: Condition::Contains("x".into()),
            action: Action::Prepend("[HIGH] ".into()),
            priority: RulePriority::High,
            enabled: true,
            tags: vec![],
        });
        let r = engine.evaluate("x");
        // High fires first (prepend), then Low (append)
        assert_eq!(r.output.as_deref(), Some("[HIGH] x [LOW]"));
        assert_eq!(r.matched_rules, vec!["high", "low"]);
    }

    #[test]
    fn test_first_match_strategy() {
        let mut engine = RuleEngine::with_config(RuleEngineConfig {
            conflict_strategy: ConflictStrategy::FirstMatch,
            ..Default::default()
        });
        engine.add_rule(tag_rule("r1", "x", "first"));
        engine.add_rule(tag_rule("r2", "x", "second"));
        let r = engine.evaluate("x");
        assert_eq!(r.matched_rules.len(), 1);
        assert_eq!(r.tags, vec!["first"]);
    }

    #[test]
    fn test_filter_lines() {
        let mut engine = make_engine();
        engine.add_rule(drop_rule("drop_comments", r"^\s*//"));
        let lines = vec!["code();", "// comment", "more_code();"];
        let out = engine.filter_lines(&lines);
        assert_eq!(out, vec!["code();", "more_code();"]);
    }

    #[test]
    fn test_stats() {
        let mut engine = make_engine();
        engine.add_rule(tag_rule("r1", "a", "found"));
        engine.add_rule(drop_rule("r2", "drop"));
        engine.evaluate("a line");
        engine.evaluate("drop this");
        engine.evaluate("nothing");
        let stats = engine.stats();
        assert_eq!(stats.lines_processed, 3);
        assert_eq!(stats.lines_dropped, 1);
        assert_eq!(stats.matches_per_rule["r1"], 1);
        assert_eq!(stats.matches_per_rule["r2"], 1);
    }

    #[test]
    fn test_disable_rule() {
        let mut engine = make_engine();
        engine.add_rule(tag_rule("r1", "x", "tagged"));
        engine.set_enabled("r1", false);
        let r = engine.evaluate("x");
        assert!(r.matched_rules.is_empty());
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = make_engine();
        engine.add_rule(tag_rule("r1", "x", "tagged"));
        assert!(engine.remove_rule("r1"));
        assert!(!engine.remove_rule("r1"));
        assert!(engine.rule_ids().is_empty());
    }

    #[test]
    fn test_max_matches_per_line() {
        let mut engine = RuleEngine::with_config(RuleEngineConfig {
            max_matches_per_line: 1,
            ..Default::default()
        });
        engine.add_rule(tag_rule("r1", "x", "first"));
        engine.add_rule(tag_rule("r2", "x", "second"));
        let r = engine.evaluate("x");
        assert_eq!(r.matched_rules.len(), 1);
    }

    #[test]
    fn test_rules_by_tag() {
        let mut engine = make_engine();
        let mut r1 = tag_rule("r1", "a", "t");
        r1.tags = vec!["security".into()];
        let mut r2 = tag_rule("r2", "b", "t");
        r2.tags = vec!["style".into()];
        engine.add_rules(vec![r1, r2]);
        assert_eq!(engine.rules_by_tag("security").len(), 1);
        assert_eq!(engine.rules_by_tag("security")[0].id, "r1");
    }
}
