//! Curated pattern library for common text-processing tasks.
//!
//! Ships named, composable pattern sets for log parsing (syslog, JSON, CLF),
//! security scanning (secrets, PII), code smells, and data extraction.
//! Each pattern set contains pre-compiled regexes with documentation.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single named pattern with its regex and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique pattern name within its set (e.g. "ipv4_address").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// The regex source string.
    pub regex: String,
    /// Named capture groups this pattern exposes.
    pub captures: Vec<String>,
    /// Severity / importance hint.
    pub severity: PatternSeverity,
    /// Tags for filtering (e.g. ["network", "address"]).
    pub tags: Vec<String>,
}

/// Severity hint associated with a pattern match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// A named collection of related patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSet {
    /// Set name (e.g. "log_parsing", "security").
    pub name: String,
    /// Human-readable description of the set.
    pub description: String,
    /// The patterns in this set.
    pub patterns: Vec<Pattern>,
}

/// Result of applying a pattern to a line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    /// Pattern name that matched.
    pub pattern_name: String,
    /// Set name the pattern belongs to.
    pub set_name: String,
    /// The full matched text.
    pub matched_text: String,
    /// Named captures: group_name → captured text.
    pub captures: BTreeMap<String, String>,
    /// Line number (1-based) if applicable.
    pub line_number: Option<usize>,
    /// Severity of the matched pattern.
    pub severity: PatternSeverity,
}

// ---------------------------------------------------------------------------
// Built-in pattern sets
// ---------------------------------------------------------------------------

fn p(
    name: &str,
    desc: &str,
    regex: &str,
    captures: &[&str],
    severity: PatternSeverity,
    tags: &[&str],
) -> Pattern {
    Pattern {
        name: name.to_string(),
        description: desc.to_string(),
        regex: regex.to_string(),
        captures: captures.iter().map(|s| s.to_string()).collect(),
        severity,
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

/// Built-in: log parsing patterns.
pub fn log_parsing_patterns() -> PatternSet {
    PatternSet {
        name: "log_parsing".into(),
        description: "Common log format parsers (syslog, JSON, CLF, timestamp extraction)".into(),
        patterns: vec![
            p(
                "syslog_bsd",
                "BSD-style syslog: <priority>Mon DD HH:MM:SS hostname process[pid]: message",
                r"^<(?P<priority>\d+)>(?P<timestamp>\w{3}\s+\d+\s+\d{2}:\d{2}:\d{2})\s+(?P<hostname>\S+)\s+(?P<process>\S+?)(?:\[(?P<pid>\d+)\])?:\s+(?P<message>.+)$",
                &[
                    "priority",
                    "timestamp",
                    "hostname",
                    "process",
                    "pid",
                    "message",
                ],
                PatternSeverity::Info,
                &["log", "syslog"],
            ),
            p(
                "json_log_line",
                "JSON log line with level, message, and timestamp fields",
                r#"^\{.*"(?:level|lvl|severity)":\s*"(?P<level>[^"]+)".*"(?:msg|message)":\s*"(?P<message>[^"]+)".*\}$"#,
                &["level", "message"],
                PatternSeverity::Info,
                &["log", "json"],
            ),
            p(
                "common_log_format",
                "Apache/Nginx Common Log Format (CLF)",
                r#"^(?P<host>\S+)\s+\S+\s+(?P<user>\S+)\s+\[(?P<timestamp>[^\]]+)\]\s+"(?P<method>\w+)\s+(?P<path>\S+)\s+\S+"\s+(?P<status>\d{3})\s+(?P<bytes>\d+|-)"#,
                &[
                    "host",
                    "user",
                    "timestamp",
                    "method",
                    "path",
                    "status",
                    "bytes",
                ],
                PatternSeverity::Info,
                &["log", "http", "clf"],
            ),
            p(
                "iso8601_timestamp",
                "ISO 8601 timestamp extraction",
                r"(?P<timestamp>\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)",
                &["timestamp"],
                PatternSeverity::Info,
                &["timestamp", "date"],
            ),
            p(
                "log_level",
                "Common log level keywords",
                r"(?i)\b(?P<level>TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|CRITICAL|EMERG(?:ENCY)?)\b",
                &["level"],
                PatternSeverity::Info,
                &["log", "level"],
            ),
        ],
    }
}

/// Built-in: security / secret scanning patterns.
pub fn security_patterns() -> PatternSet {
    PatternSet {
        name: "security".into(),
        description: "Detect secrets, tokens, API keys, and sensitive data in text".into(),
        patterns: vec![
            p(
                "aws_access_key",
                "AWS access key ID",
                r"(?P<key>AKIA[0-9A-Z]{16})",
                &["key"],
                PatternSeverity::Critical,
                &["secret", "aws", "cloud"],
            ),
            p(
                "generic_api_key",
                "Generic API key pattern (key=... or api_key=...)",
                r#"(?i)(?:api[_-]?key|apikey|token|secret|password)\s*[:=]\s*["']?(?P<value>[A-Za-z0-9_\-]{16,})["']?"#,
                &["value"],
                PatternSeverity::Critical,
                &["secret", "api"],
            ),
            p(
                "private_key_header",
                "RSA/EC/DSA private key header",
                r"-----BEGIN\s+(?P<type>RSA |EC |DSA )?PRIVATE KEY-----",
                &["type"],
                PatternSeverity::Critical,
                &["secret", "crypto"],
            ),
            p(
                "jwt_token",
                "JSON Web Token (3-part base64url dot-separated)",
                r"(?P<token>eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)",
                &["token"],
                PatternSeverity::Warning,
                &["secret", "jwt"],
            ),
            p(
                "github_pat",
                "GitHub personal access token",
                r"(?P<token>ghp_[A-Za-z0-9]{36})",
                &["token"],
                PatternSeverity::Critical,
                &["secret", "github"],
            ),
        ],
    }
}

/// Built-in: PII detection patterns.
pub fn pii_patterns() -> PatternSet {
    PatternSet {
        name: "pii".into(),
        description: "Personally Identifiable Information detection".into(),
        patterns: vec![
            p(
                "email_address",
                "Email address",
                r"(?P<email>[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,})",
                &["email"],
                PatternSeverity::Warning,
                &["pii", "email"],
            ),
            p(
                "us_ssn",
                "US Social Security Number (XXX-XX-XXXX)",
                r"\b(?P<ssn>\d{3}-\d{2}-\d{4})\b",
                &["ssn"],
                PatternSeverity::Critical,
                &["pii", "ssn"],
            ),
            p(
                "credit_card",
                "Credit card number (Visa, MC, Amex patterns)",
                r"\b(?P<cc>(?:4\d{3}|5[1-5]\d{2}|3[47]\d{2})[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4})\b",
                &["cc"],
                PatternSeverity::Critical,
                &["pii", "financial"],
            ),
            p(
                "phone_us",
                "US phone number",
                r"(?P<phone>(?:\+1[\s-]?)?\(?\d{3}\)?[\s.-]?\d{3}[\s.-]?\d{4})",
                &["phone"],
                PatternSeverity::Warning,
                &["pii", "phone"],
            ),
            p(
                "ipv4_address",
                "IPv4 address",
                r"\b(?P<ip>(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?))\b",
                &["ip"],
                PatternSeverity::Info,
                &["network", "ip"],
            ),
        ],
    }
}

/// Built-in: code smell patterns.
pub fn code_smell_patterns() -> PatternSet {
    PatternSet {
        name: "code_smells".into(),
        description: "Detect common code quality issues".into(),
        patterns: vec![
            p(
                "todo_fixme",
                "TODO/FIXME/HACK/XXX comment markers",
                r"(?i)\b(?P<marker>TODO|FIXME|HACK|XXX|BUG)\b[:\s]*(?P<text>.*)",
                &["marker", "text"],
                PatternSeverity::Warning,
                &["code", "quality"],
            ),
            p(
                "magic_number",
                "Magic numbers in code (numeric literals > 1 digit, not 0/1/2/10/100/1000)",
                r"\b(?P<number>\d{3,})\b",
                &["number"],
                PatternSeverity::Info,
                &["code", "quality"],
            ),
            p(
                "long_line",
                "Lines exceeding 120 characters",
                r"^(?P<line>.{121,})$",
                &["line"],
                PatternSeverity::Info,
                &["code", "style"],
            ),
            p(
                "unsafe_block",
                "Rust unsafe block",
                r"\bunsafe\s*\{",
                &[],
                PatternSeverity::Warning,
                &["code", "rust", "safety"],
            ),
            p(
                "unwrap_call",
                "Rust .unwrap() call (may panic)",
                r"\.unwrap\(\)",
                &[],
                PatternSeverity::Warning,
                &["code", "rust", "quality"],
            ),
        ],
    }
}

// ---------------------------------------------------------------------------
// Pattern registry
// ---------------------------------------------------------------------------

/// Registry of all available pattern sets.
#[derive(Debug, Clone)]
pub struct PatternRegistry {
    sets: BTreeMap<String, PatternSet>,
}

impl PatternRegistry {
    /// Create a registry with all built-in pattern sets.
    pub fn with_builtins() -> Self {
        let mut sets = BTreeMap::new();
        for ps in [
            log_parsing_patterns(),
            security_patterns(),
            pii_patterns(),
            code_smell_patterns(),
        ] {
            sets.insert(ps.name.clone(), ps);
        }
        Self { sets }
    }

    /// Create an empty registry (no built-ins).
    pub fn empty() -> Self {
        Self {
            sets: BTreeMap::new(),
        }
    }

    /// Register a custom pattern set. Replaces any existing set with the same name.
    pub fn register(&mut self, set: PatternSet) {
        self.sets.insert(set.name.clone(), set);
    }

    /// List all set names.
    pub fn set_names(&self) -> Vec<&str> {
        self.sets.keys().map(|s| s.as_str()).collect()
    }

    /// Get a pattern set by name.
    pub fn get_set(&self, name: &str) -> Option<&PatternSet> {
        self.sets.get(name)
    }

    /// Get all patterns across all sets.
    pub fn all_patterns(&self) -> Vec<(&str, &Pattern)> {
        self.sets
            .iter()
            .flat_map(|(set_name, ps)| ps.patterns.iter().map(move |p| (set_name.as_str(), p)))
            .collect()
    }

    /// Search for patterns matching a tag.
    pub fn patterns_by_tag(&self, tag: &str) -> Vec<(&str, &Pattern)> {
        self.all_patterns()
            .into_iter()
            .filter(|(_, p)| p.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Apply a single pattern to text, returning all matches.
    pub fn apply_pattern(pattern: &Pattern, set_name: &str, text: &str) -> Vec<PatternMatch> {
        let Ok(re) = Regex::new(&pattern.regex) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        for (line_idx, line) in text.lines().enumerate() {
            for caps in re.captures_iter(line) {
                let mut captures = BTreeMap::new();
                for name in &pattern.captures {
                    if let Some(m) = caps.name(name) {
                        captures.insert(name.clone(), m.as_str().to_string());
                    }
                }
                results.push(PatternMatch {
                    pattern_name: pattern.name.clone(),
                    set_name: set_name.to_string(),
                    matched_text: caps
                        .get(0)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    captures,
                    line_number: Some(line_idx + 1),
                    severity: pattern.severity,
                });
            }
        }
        results
    }

    /// Scan text against all patterns in a specific set.
    pub fn scan_with_set(&self, set_name: &str, text: &str) -> Vec<PatternMatch> {
        let Some(ps) = self.sets.get(set_name) else {
            return Vec::new();
        };
        ps.patterns
            .iter()
            .flat_map(|p| Self::apply_pattern(p, set_name, text))
            .collect()
    }

    /// Scan text against all patterns in all sets.
    pub fn scan_all(&self, text: &str) -> Vec<PatternMatch> {
        self.sets
            .iter()
            .flat_map(|(name, ps)| {
                ps.patterns
                    .iter()
                    .flat_map(|p| Self::apply_pattern(p, name, text))
            })
            .collect()
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_sets() {
        let reg = PatternRegistry::with_builtins();
        let names = reg.set_names();
        assert!(names.contains(&"log_parsing"));
        assert!(names.contains(&"security"));
        assert!(names.contains(&"pii"));
        assert!(names.contains(&"code_smells"));
        assert_eq!(names.len(), 4);
    }

    #[test]
    fn test_syslog_parsing() {
        let reg = PatternRegistry::with_builtins();
        let text = "<134>Mar  5 14:23:01 myhost sshd[1234]: Accepted publickey for user";
        let matches = reg.scan_with_set("log_parsing", text);
        let syslog: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "syslog_bsd")
            .collect();
        assert_eq!(syslog.len(), 1);
        assert_eq!(syslog[0].captures.get("hostname").unwrap(), "myhost");
        assert_eq!(syslog[0].captures.get("pid").unwrap(), "1234");
    }

    #[test]
    fn test_clf_parsing() {
        let text =
            r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326"#;
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("log_parsing", text);
        let clf: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "common_log_format")
            .collect();
        assert_eq!(clf.len(), 1);
        assert_eq!(clf[0].captures.get("method").unwrap(), "GET");
        assert_eq!(clf[0].captures.get("status").unwrap(), "200");
    }

    #[test]
    fn test_aws_key_detection() {
        let text = "export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("security", text);
        let aws: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "aws_access_key")
            .collect();
        assert_eq!(aws.len(), 1);
        assert_eq!(aws[0].severity, PatternSeverity::Critical);
    }

    #[test]
    fn test_generic_api_key() {
        let text = r#"api_key = "sk_live_abcdef1234567890""#;
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("security", text);
        let keys: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "generic_api_key")
            .collect();
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn test_email_detection() {
        let text = "Contact: user@example.com for info";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("pii", text);
        let emails: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "email_address")
            .collect();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].captures.get("email").unwrap(), "user@example.com");
    }

    #[test]
    fn test_ssn_detection() {
        let text = "SSN: 123-45-6789, ref 987-65-4321";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("pii", text);
        let ssns: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "us_ssn")
            .collect();
        assert_eq!(ssns.len(), 2);
    }

    #[test]
    fn test_credit_card_detection() {
        let text = "Card: 4111-1111-1111-1111";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("pii", text);
        let cc: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "credit_card")
            .collect();
        assert_eq!(cc.len(), 1);
    }

    #[test]
    fn test_todo_detection() {
        let text = "// TODO: fix this later\n// FIXME: broken logic";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("code_smells", text);
        let todos: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "todo_fixme")
            .collect();
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].line_number, Some(1));
        assert_eq!(todos[1].line_number, Some(2));
    }

    #[test]
    fn test_scan_all() {
        let text = "user@test.com and AKIA1234567890123456";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_all(text);
        assert!(matches.len() >= 2); // email + aws key
    }

    #[test]
    fn test_custom_set() {
        let mut reg = PatternRegistry::empty();
        let custom = PatternSet {
            name: "custom".into(),
            description: "Custom test set".into(),
            patterns: vec![p(
                "greeting",
                "Greeting words",
                r"\b(?P<word>hello|hi|hey)\b",
                &["word"],
                PatternSeverity::Info,
                &["test"],
            )],
        };
        reg.register(custom);
        let matches = reg.scan_with_set("custom", "hello world, hey there");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_patterns_by_tag() {
        let reg = PatternRegistry::with_builtins();
        let secret_patterns = reg.patterns_by_tag("secret");
        assert!(secret_patterns.len() >= 4);
        for (_, p) in &secret_patterns {
            assert!(p.tags.contains(&"secret".to_string()));
        }
    }

    #[test]
    fn test_iso8601_timestamp() {
        let text = "Event at 2026-03-06T14:30:00Z was logged";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("log_parsing", text);
        let ts: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "iso8601_timestamp")
            .collect();
        assert_eq!(ts.len(), 1);
        assert_eq!(
            ts[0].captures.get("timestamp").unwrap(),
            "2026-03-06T14:30:00Z"
        );
    }

    #[test]
    fn test_log_level_extraction() {
        let text = "2026-03-06 ERROR: disk full\n2026-03-06 INFO: recovered";
        let reg = PatternRegistry::with_builtins();
        let matches = reg.scan_with_set("log_parsing", text);
        let levels: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "log_level")
            .collect();
        assert_eq!(levels.len(), 2);
    }
}
