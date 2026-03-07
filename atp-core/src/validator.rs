//! # Multi-Format Validator
//!
//! Validate email addresses, URLs, semantic versions, UUIDs, IP addresses,
//! a JSON Schema subset, and custom rule DSL.  Supports batch validation
//! with diagnostic output.

use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// Severity of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A single diagnostic produced by validation.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub path: Option<String>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.as_deref().unwrap_or("");
        write!(
            f,
            "[{}] {}{}",
            self.severity,
            if path.is_empty() {
                String::new()
            } else {
                format!("{path}: ")
            },
            self.message
        )
    }
}

/// Result of validating a single value.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            diagnostics: Vec::new(),
        }
    }

    pub fn fail(msg: impl Into<String>) -> Self {
        Self {
            valid: false,
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                message: msg.into(),
                path: None,
            }],
        }
    }

    pub fn with_path(mut self, path: &str) -> Self {
        for d in &mut self.diagnostics {
            d.path = Some(path.into());
        }
        self
    }
}

// ---------------------------------------------------------------------------
// Built-in validators
// ---------------------------------------------------------------------------

/// Validate an email address (simplified RFC 5321).
pub fn validate_email(input: &str) -> ValidationResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ValidationResult::fail("empty email");
    }
    let parts: Vec<&str> = trimmed.splitn(2, '@').collect();
    if parts.len() != 2 {
        return ValidationResult::fail("missing @ sign");
    }
    let (local, domain) = (parts[0], parts[1]);
    if local.is_empty() {
        return ValidationResult::fail("empty local part");
    }
    if local.len() > 64 {
        return ValidationResult::fail("local part exceeds 64 characters");
    }
    if domain.is_empty() {
        return ValidationResult::fail("empty domain");
    }
    if !domain.contains('.') {
        return ValidationResult::fail("domain must contain at least one dot");
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return ValidationResult::fail("domain cannot start or end with a dot");
    }
    for ch in local.chars() {
        if !ch.is_alphanumeric() && !"._+-".contains(ch) {
            return ValidationResult::fail(format!("invalid character '{}' in local part", ch));
        }
    }
    for ch in domain.chars() {
        if !ch.is_alphanumeric() && !".-".contains(ch) {
            return ValidationResult::fail(format!("invalid character '{}' in domain", ch));
        }
    }
    ValidationResult::ok()
}

/// Validate a URL (http/https).
pub fn validate_url(input: &str) -> ValidationResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ValidationResult::fail("empty URL");
    }
    let after_scheme = if let Some(rest) = trimmed.strip_prefix("https://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest
    } else {
        return ValidationResult::fail("URL must start with http:// or https://");
    };
    if after_scheme.is_empty() {
        return ValidationResult::fail("URL has no host");
    }
    let host = after_scheme.split('/').next().unwrap_or("");
    let host_no_port = host.split(':').next().unwrap_or("");
    if host_no_port.is_empty() {
        return ValidationResult::fail("URL has no host");
    }
    if !host_no_port.contains('.') && host_no_port != "localhost" {
        return ValidationResult::fail("host must contain a dot or be localhost");
    }
    ValidationResult::ok()
}

/// Validate a semantic version string (major.minor.patch with optional pre-release).
pub fn validate_semver(input: &str) -> ValidationResult {
    let trimmed = input.trim().trim_start_matches('v');
    if trimmed.is_empty() {
        return ValidationResult::fail("empty version");
    }
    let base = trimmed.split('-').next().unwrap_or(trimmed);
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() != 3 {
        return ValidationResult::fail("semver requires exactly major.minor.patch");
    }
    for (i, label) in ["major", "minor", "patch"].iter().enumerate() {
        if parts[i].parse::<u64>().is_err() {
            return ValidationResult::fail(format!("{} is not a valid number", label));
        }
    }
    ValidationResult::ok()
}

/// Validate a UUID (v4 format, 8-4-4-4-12 hex).
pub fn validate_uuid(input: &str) -> ValidationResult {
    let trimmed = input.trim();
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() != 5 {
        return ValidationResult::fail("UUID must have 5 dash-separated groups");
    }
    let expected_lens = [8, 4, 4, 4, 12];
    for (i, &expected) in expected_lens.iter().enumerate() {
        if parts[i].len() != expected {
            return ValidationResult::fail(format!(
                "group {} should be {} hex chars, got {}",
                i + 1,
                expected,
                parts[i].len()
            ));
        }
        if !parts[i].chars().all(|c| c.is_ascii_hexdigit()) {
            return ValidationResult::fail(format!("group {} contains non-hex characters", i + 1));
        }
    }
    ValidationResult::ok()
}

/// Validate an IPv4 address.
pub fn validate_ipv4(input: &str) -> ValidationResult {
    let trimmed = input.trim();
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 4 {
        return ValidationResult::fail("IPv4 must have exactly 4 octets");
    }
    for (i, part) in parts.iter().enumerate() {
        match part.parse::<u16>() {
            Ok(v) if v <= 255 => {}
            Ok(v) => {
                return ValidationResult::fail(format!(
                    "octet {} value {} out of range 0-255",
                    i + 1,
                    v
                ));
            }
            Err(_) => {
                return ValidationResult::fail(format!("octet {} is not a number", i + 1));
            }
        }
    }
    ValidationResult::ok()
}

/// Validate an IPv6 address (simplified: 8 colon-separated hex groups or
/// common abbreviations with "::").
pub fn validate_ipv6(input: &str) -> ValidationResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ValidationResult::fail("empty IPv6");
    }
    // Handle :: abbreviation
    if trimmed.contains("::") {
        let halves: Vec<&str> = trimmed.splitn(2, "::").collect();
        let left: Vec<&str> = if halves[0].is_empty() {
            Vec::new()
        } else {
            halves[0].split(':').collect()
        };
        let right: Vec<&str> = if halves.len() > 1 && !halves[1].is_empty() {
            halves[1].split(':').collect()
        } else {
            Vec::new()
        };
        let total = left.len() + right.len();
        if total > 7 {
            return ValidationResult::fail("too many groups with :: abbreviation");
        }
        for g in left.iter().chain(right.iter()) {
            if g.len() > 4 || !g.chars().all(|c| c.is_ascii_hexdigit()) {
                return ValidationResult::fail(format!("invalid hex group '{}'", g));
            }
        }
        return ValidationResult::ok();
    }

    let groups: Vec<&str> = trimmed.split(':').collect();
    if groups.len() != 8 {
        return ValidationResult::fail("IPv6 must have 8 colon-separated groups (or use ::)");
    }
    for g in &groups {
        if g.len() > 4 || g.is_empty() || !g.chars().all(|c| c.is_ascii_hexdigit()) {
            return ValidationResult::fail(format!("invalid hex group '{}'", g));
        }
    }
    ValidationResult::ok()
}

// ---------------------------------------------------------------------------
// JSON Schema subset
// ---------------------------------------------------------------------------

/// Minimal JSON Schema type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaType {
    String,
    Number,
    Bool,
    Array,
    Object,
    Null,
}

/// A simple JSON Schema node.
#[derive(Debug, Clone)]
pub struct SchemaNode {
    pub ty: Option<SchemaType>,
    pub required: Vec<String>,
    pub properties: BTreeMap<String, SchemaNode>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub pattern: Option<String>,
    pub enum_values: Vec<String>,
}

impl SchemaNode {
    pub fn new(ty: SchemaType) -> Self {
        Self {
            ty: Some(ty),
            required: Vec::new(),
            properties: BTreeMap::new(),
            min_length: None,
            max_length: None,
            minimum: None,
            maximum: None,
            pattern: None,
            enum_values: Vec::new(),
        }
    }

    pub fn any() -> Self {
        Self {
            ty: None,
            required: Vec::new(),
            properties: BTreeMap::new(),
            min_length: None,
            max_length: None,
            minimum: None,
            maximum: None,
            pattern: None,
            enum_values: Vec::new(),
        }
    }
}

/// Validate a JSON value against a schema node.
pub fn validate_json_schema(value: &serde_json::Value, schema: &SchemaNode) -> ValidationResult {
    let mut diagnostics = Vec::new();
    validate_schema_inner(value, schema, "", &mut diagnostics);
    let valid = !diagnostics.iter().any(|d| d.severity == Severity::Error);
    ValidationResult { valid, diagnostics }
}

fn validate_schema_inner(
    value: &serde_json::Value,
    schema: &SchemaNode,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    // Type check
    if let Some(ty) = &schema.ty {
        let ok = match ty {
            SchemaType::String => value.is_string(),
            SchemaType::Number => value.is_number(),
            SchemaType::Bool => value.is_boolean(),
            SchemaType::Array => value.is_array(),
            SchemaType::Object => value.is_object(),
            SchemaType::Null => value.is_null(),
        };
        if !ok {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!("expected {:?}, got {:?}", ty, json_type_name(value)),
                path: Some(path.to_string()),
            });
            return;
        }
    }

    // String constraints
    if let Some(s) = value.as_str() {
        if let Some(min) = schema.min_length {
            if s.len() < min {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("string length {} < minLength {}", s.len(), min),
                    path: Some(path.to_string()),
                });
            }
        }
        if let Some(max) = schema.max_length {
            if s.len() > max {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("string length {} > maxLength {}", s.len(), max),
                    path: Some(path.to_string()),
                });
            }
        }
        if let Some(pat) = &schema.pattern {
            if let Ok(re) = regex::Regex::new(pat) {
                if !re.is_match(s) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("string does not match pattern '{}'", pat),
                        path: Some(path.to_string()),
                    });
                }
            }
        }
        if !schema.enum_values.is_empty() && !schema.enum_values.contains(&s.to_string()) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                message: format!("value '{}' not in enum {:?}", s, schema.enum_values),
                path: Some(path.to_string()),
            });
        }
    }

    // Number constraints
    if let Some(n) = value.as_f64() {
        if let Some(min) = schema.minimum {
            if n < min {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("value {} < minimum {}", n, min),
                    path: Some(path.to_string()),
                });
            }
        }
        if let Some(max) = schema.maximum {
            if n > max {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("value {} > maximum {}", n, max),
                    path: Some(path.to_string()),
                });
            }
        }
    }

    // Object constraints
    if let Some(obj) = value.as_object() {
        for req in &schema.required {
            if !obj.contains_key(req) {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("missing required property '{}'", req),
                    path: Some(path.to_string()),
                });
            }
        }
        for (key, sub_schema) in &schema.properties {
            if let Some(sub_val) = obj.get(key) {
                let sub_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                validate_schema_inner(sub_val, sub_schema, &sub_path, diags);
            }
        }
    }
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

// ---------------------------------------------------------------------------
// Custom rule DSL
// ---------------------------------------------------------------------------

/// A custom validation rule.
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub condition: RuleCondition,
    pub severity: Severity,
    pub message: String,
}

/// Condition for a custom rule.
#[derive(Debug, Clone)]
pub enum RuleCondition {
    /// Value must match a regex.
    Matches(String),
    /// String length must be in range.
    LengthBetween(usize, usize),
    /// Value must be one of these.
    OneOf(Vec<String>),
    /// Must not be empty/whitespace-only.
    NotBlank,
    /// Must start with a prefix.
    StartsWith(String),
    /// Must end with a suffix.
    EndsWith(String),
    /// All sub-conditions must pass.
    All(Vec<RuleCondition>),
    /// At least one must pass.
    Any(Vec<RuleCondition>),
}

/// Evaluate a rule against a string value.
pub fn evaluate_rule(value: &str, rule: &Rule) -> ValidationResult {
    if check_condition(value, &rule.condition) {
        ValidationResult::ok()
    } else {
        ValidationResult {
            valid: false,
            diagnostics: vec![Diagnostic {
                severity: rule.severity,
                message: format!("[{}] {}", rule.name, rule.message),
                path: None,
            }],
        }
    }
}

fn check_condition(value: &str, cond: &RuleCondition) -> bool {
    match cond {
        RuleCondition::Matches(pat) => regex::Regex::new(pat)
            .map(|re| re.is_match(value))
            .unwrap_or(false),
        RuleCondition::LengthBetween(min, max) => {
            let len = value.len();
            len >= *min && len <= *max
        }
        RuleCondition::OneOf(opts) => opts.iter().any(|o| o == value),
        RuleCondition::NotBlank => !value.trim().is_empty(),
        RuleCondition::StartsWith(prefix) => value.starts_with(prefix.as_str()),
        RuleCondition::EndsWith(suffix) => value.ends_with(suffix.as_str()),
        RuleCondition::All(subs) => subs.iter().all(|s| check_condition(value, s)),
        RuleCondition::Any(subs) => subs.iter().any(|s| check_condition(value, s)),
    }
}

// ---------------------------------------------------------------------------
// Batch validation
// ---------------------------------------------------------------------------

/// Batch validation entry.
#[derive(Debug, Clone)]
pub struct BatchEntry {
    pub label: String,
    pub value: String,
}

/// Kind of built-in validator to apply in batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorKind {
    Email,
    Url,
    Semver,
    Uuid,
    Ipv4,
    Ipv6,
}

/// Run a built-in validator across many values.
pub fn validate_batch(
    entries: &[BatchEntry],
    kind: ValidatorKind,
) -> Vec<(String, ValidationResult)> {
    entries
        .iter()
        .map(|e| {
            let result = match kind {
                ValidatorKind::Email => validate_email(&e.value),
                ValidatorKind::Url => validate_url(&e.value),
                ValidatorKind::Semver => validate_semver(&e.value),
                ValidatorKind::Uuid => validate_uuid(&e.value),
                ValidatorKind::Ipv4 => validate_ipv4(&e.value),
                ValidatorKind::Ipv6 => validate_ipv6(&e.value),
            };
            (e.label.clone(), result)
        })
        .collect()
}

/// Format batch results as a report string.
pub fn batch_report(results: &[(String, ValidationResult)]) -> String {
    let mut out = String::new();
    let total = results.len();
    let valid_count = results.iter().filter(|(_, r)| r.valid).count();
    out.push_str(&format!(
        "Validation Report: {}/{} passed\n",
        valid_count, total
    ));
    out.push_str(&"─".repeat(40));
    out.push('\n');
    for (label, result) in results {
        let status = if result.valid { "✓" } else { "✗" };
        out.push_str(&format!("{status} {label}"));
        if !result.diagnostics.is_empty() {
            for d in &result.diagnostics {
                out.push_str(&format!("\n    {}", d));
            }
        }
        out.push('\n');
    }
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Email --

    #[test]
    fn test_email_valid() {
        assert!(validate_email("user@example.com").valid);
        assert!(validate_email("a.b+c@sub.domain.org").valid);
    }

    #[test]
    fn test_email_invalid() {
        assert!(!validate_email("").valid);
        assert!(!validate_email("noatsign").valid);
        assert!(!validate_email("@domain.com").valid);
        assert!(!validate_email("user@").valid);
        assert!(!validate_email("user@domain").valid);
    }

    // -- URL --

    #[test]
    fn test_url_valid() {
        assert!(validate_url("https://example.com").valid);
        assert!(validate_url("http://localhost/path").valid);
        assert!(validate_url("https://sub.domain.org/page?q=1").valid);
    }

    #[test]
    fn test_url_invalid() {
        assert!(!validate_url("").valid);
        assert!(!validate_url("ftp://x.com").valid);
        assert!(!validate_url("https://").valid);
    }

    // -- Semver --

    #[test]
    fn test_semver_valid() {
        assert!(validate_semver("1.2.3").valid);
        assert!(validate_semver("v0.1.0").valid);
        assert!(validate_semver("1.0.0-beta").valid);
    }

    #[test]
    fn test_semver_invalid() {
        assert!(!validate_semver("1.2").valid);
        assert!(!validate_semver("abc").valid);
    }

    // -- UUID --

    #[test]
    fn test_uuid_valid() {
        assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000").valid);
    }

    #[test]
    fn test_uuid_invalid() {
        assert!(!validate_uuid("not-a-uuid").valid);
        assert!(!validate_uuid("550e8400-e29b-41d4-a716").valid);
    }

    // -- IPv4 --

    #[test]
    fn test_ipv4_valid() {
        assert!(validate_ipv4("192.168.1.1").valid);
        assert!(validate_ipv4("0.0.0.0").valid);
        assert!(validate_ipv4("255.255.255.255").valid);
    }

    #[test]
    fn test_ipv4_invalid() {
        assert!(!validate_ipv4("256.0.0.1").valid);
        assert!(!validate_ipv4("1.2.3").valid);
    }

    // -- IPv6 --

    #[test]
    fn test_ipv6_valid() {
        assert!(validate_ipv6("2001:0db8:85a3:0000:0000:8a2e:0370:7334").valid);
        assert!(validate_ipv6("::1").valid);
        assert!(validate_ipv6("fe80::1").valid);
    }

    #[test]
    fn test_ipv6_invalid() {
        assert!(!validate_ipv6("").valid);
        assert!(!validate_ipv6("12345::1").valid);
    }

    // -- JSON Schema --

    #[test]
    fn test_json_schema_basic() {
        let schema = SchemaNode {
            ty: Some(SchemaType::Object),
            required: vec!["name".into()],
            properties: {
                let mut p = BTreeMap::new();
                p.insert(
                    "name".into(),
                    SchemaNode {
                        ty: Some(SchemaType::String),
                        min_length: Some(1),
                        ..SchemaNode::any()
                    },
                );
                p.insert(
                    "age".into(),
                    SchemaNode {
                        ty: Some(SchemaType::Number),
                        minimum: Some(0.0),
                        maximum: Some(150.0),
                        ..SchemaNode::any()
                    },
                );
                p
            },
            ..SchemaNode::any()
        };

        let valid_json: serde_json::Value = serde_json::json!({"name": "Alice", "age": 30});
        assert!(validate_json_schema(&valid_json, &schema).valid);

        let missing_name: serde_json::Value = serde_json::json!({"age": 30});
        assert!(!validate_json_schema(&missing_name, &schema).valid);
    }

    // -- Custom rules --

    #[test]
    fn test_custom_rule() {
        let rule = Rule {
            name: "username".into(),
            condition: RuleCondition::All(vec![
                RuleCondition::NotBlank,
                RuleCondition::LengthBetween(3, 20),
                RuleCondition::Matches(r"^[a-zA-Z0-9_]+$".into()),
            ]),
            severity: Severity::Error,
            message: "invalid username".into(),
        };
        assert!(evaluate_rule("alice_42", &rule).valid);
        assert!(!evaluate_rule("ab", &rule).valid);
        assert!(!evaluate_rule("bad user!", &rule).valid);
    }

    // -- Batch --

    #[test]
    fn test_batch_validation() {
        let entries = vec![
            BatchEntry {
                label: "e1".into(),
                value: "user@example.com".into(),
            },
            BatchEntry {
                label: "e2".into(),
                value: "bad".into(),
            },
        ];
        let results = validate_batch(&entries, ValidatorKind::Email);
        assert_eq!(results.len(), 2);
        assert!(results[0].1.valid);
        assert!(!results[1].1.valid);
    }

    #[test]
    fn test_batch_report() {
        let results = vec![
            ("ok".into(), ValidationResult::ok()),
            ("bad".into(), ValidationResult::fail("nope")),
        ];
        let report = batch_report(&results);
        assert!(report.contains("1/2 passed"));
    }
}
