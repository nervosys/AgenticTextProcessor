//! PII and secret detection & redaction.
//!
//! Regex + heuristic detectors for emails, SSNs, API keys, credit cards,
//! phone numbers and private keys. Configurable masking strategies:
//! full mask, partial mask, hash replacement, or full removal.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Category of sensitive data detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensitiveKind {
    Email,
    Ssn,
    CreditCard,
    PhoneNumber,
    ApiKey,
    PrivateKey,
    JwtToken,
    AwsAccessKey,
    Custom,
}

/// How to mask/replace detected sensitive data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskStrategy {
    /// Replace with `***REDACTED***`.
    #[default]
    FullMask,
    /// Show first/last N chars, mask the middle: `us****@example.com`.
    PartialMask,
    /// Replace with SHA-256 hash of the original value.
    Hash,
    /// Remove the sensitive value entirely (empty string).
    Remove,
}

/// A single detection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    /// What kind of data this detects.
    pub kind: SensitiveKind,
    /// Human-readable name.
    pub name: String,
    /// Regex pattern (must compile).
    pub regex: String,
    /// Masking strategy when this rule matches.
    pub strategy: MaskStrategy,
}

/// Configuration for the redactor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactConfig {
    /// Whether redaction is enabled.
    pub enabled: bool,
    /// Default masking strategy (used when a rule doesn't specify one).
    pub default_strategy: MaskStrategy,
    /// Custom detection rules (appended to built-ins).
    pub custom_rules: Vec<DetectionRule>,
    /// Which built-in kinds to enable. Empty = all.
    pub enabled_kinds: Vec<SensitiveKind>,
}

impl Default for RedactConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_strategy: MaskStrategy::FullMask,
            custom_rules: Vec::new(),
            enabled_kinds: Vec::new(), // empty = all
        }
    }
}

/// A detection hit in the text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    /// Kind of sensitive data.
    pub kind: SensitiveKind,
    /// Rule name that triggered.
    pub rule_name: String,
    /// The matched text.
    pub matched_text: String,
    /// Byte offset start.
    pub start: usize,
    /// Byte offset end.
    pub end: usize,
    /// Line number (1-based).
    pub line_number: usize,
}

/// Result of a redaction pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactResult {
    /// The redacted text.
    pub redacted_text: String,
    /// Total detections found.
    pub total_detections: usize,
    /// Detections by kind.
    pub detections_by_kind: BTreeMap<String, usize>,
    /// All individual detections.
    pub detections: Vec<Detection>,
}

// ---------------------------------------------------------------------------
// Built-in rules
// ---------------------------------------------------------------------------

fn builtin_rules() -> Vec<DetectionRule> {
    vec![
        DetectionRule {
            kind: SensitiveKind::Email,
            name: "email_address".into(),
            regex: r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}".into(),
            strategy: MaskStrategy::PartialMask,
        },
        DetectionRule {
            kind: SensitiveKind::Ssn,
            name: "us_ssn".into(),
            regex: r"\b\d{3}-\d{2}-\d{4}\b".into(),
            strategy: MaskStrategy::FullMask,
        },
        DetectionRule {
            kind: SensitiveKind::CreditCard,
            name: "credit_card".into(),
            regex: r"\b(?:4\d{3}|5[1-5]\d{2}|3[47]\d{2})[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b"
                .into(),
            strategy: MaskStrategy::PartialMask,
        },
        DetectionRule {
            kind: SensitiveKind::PhoneNumber,
            name: "us_phone".into(),
            regex: r"(?:\+1[\s-]?)?\(?\d{3}\)?[\s.-]\d{3}[\s.-]\d{4}".into(),
            strategy: MaskStrategy::PartialMask,
        },
        DetectionRule {
            kind: SensitiveKind::AwsAccessKey,
            name: "aws_access_key".into(),
            regex: r"AKIA[0-9A-Z]{16}".into(),
            strategy: MaskStrategy::FullMask,
        },
        DetectionRule {
            kind: SensitiveKind::ApiKey,
            name: "generic_api_key".into(),
            regex: r#"(?i)(?:api[_-]?key|apikey|token|secret|password)\s*[:=]\s*["']?([A-Za-z0-9_\-]{16,})["']?"#
                .into(),
            strategy: MaskStrategy::FullMask,
        },
        DetectionRule {
            kind: SensitiveKind::JwtToken,
            name: "jwt_token".into(),
            regex: r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+".into(),
            strategy: MaskStrategy::Hash,
        },
        DetectionRule {
            kind: SensitiveKind::PrivateKey,
            name: "private_key_header".into(),
            regex: r"-----BEGIN\s+(?:RSA\s+|EC\s+|DSA\s+)?PRIVATE KEY-----".into(),
            strategy: MaskStrategy::FullMask,
        },
    ]
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

fn apply_mask(text: &str, strategy: MaskStrategy) -> String {
    match strategy {
        MaskStrategy::FullMask => "***REDACTED***".to_string(),
        MaskStrategy::PartialMask => {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len();
            if len <= 4 {
                "*".repeat(len)
            } else {
                let show = (len / 4).clamp(1, 4);
                let prefix: String = chars[..show].iter().collect();
                let suffix: String = chars[len - show..].iter().collect();
                let masked = "*".repeat(len - 2 * show);
                format!("{prefix}{masked}{suffix}")
            }
        }
        MaskStrategy::Hash => {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            let hash = format!("{:x}", hasher.finalize());
            format!("[SHA256:{}]", &hash[..16])
        }
        MaskStrategy::Remove => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Redactor
// ---------------------------------------------------------------------------

/// The main redaction engine.
#[derive(Debug, Clone)]
pub struct Redactor {
    config: RedactConfig,
    rules: Vec<DetectionRule>,
}

impl Redactor {
    /// Create a redactor with the given config.
    pub fn new(config: RedactConfig) -> Self {
        let mut rules = if config.enabled_kinds.is_empty() {
            builtin_rules()
        } else {
            builtin_rules()
                .into_iter()
                .filter(|r| config.enabled_kinds.contains(&r.kind))
                .collect()
        };
        rules.extend(config.custom_rules.clone());
        Self { config, rules }
    }

    /// Create a redactor with all defaults.
    pub fn default_redactor() -> Self {
        Self::new(RedactConfig::default())
    }

    /// Scan text for sensitive data without modifying it.
    pub fn scan(&self, text: &str) -> Vec<Detection> {
        if !self.config.enabled {
            return Vec::new();
        }
        let mut detections = Vec::new();
        for rule in &self.rules {
            let Ok(re) = Regex::new(&rule.regex) else {
                continue;
            };
            let mut line_start = 0;
            for (line_idx, line) in text.lines().enumerate() {
                for m in re.find_iter(line) {
                    detections.push(Detection {
                        kind: rule.kind,
                        rule_name: rule.name.clone(),
                        matched_text: m.as_str().to_string(),
                        start: line_start + m.start(),
                        end: line_start + m.end(),
                        line_number: line_idx + 1,
                    });
                }
                line_start += line.len() + 1; // +1 for \n
            }
        }
        // Deduplicate overlapping detections (keep the one with lower start, or longer)
        detections.sort_by_key(|d| (d.start, std::cmp::Reverse(d.end)));
        detections
    }

    /// Redact all sensitive data in the text.
    pub fn redact(&self, text: &str) -> RedactResult {
        if !self.config.enabled {
            return RedactResult {
                redacted_text: text.to_string(),
                total_detections: 0,
                detections_by_kind: BTreeMap::new(),
                detections: Vec::new(),
            };
        }

        let detections = self.scan(text);

        // Build non-overlapping redaction spans, prioritizing earlier/longer matches
        let mut spans: Vec<(usize, usize, MaskStrategy, &str)> = Vec::new();
        for d in &detections {
            let strategy = self
                .rules
                .iter()
                .find(|r| r.name == d.rule_name)
                .map(|r| r.strategy)
                .unwrap_or(self.config.default_strategy);
            // Check for overlap with existing spans
            let overlaps = spans.iter().any(|&(s, e, _, _)| d.start < e && d.end > s);
            if !overlaps {
                spans.push((d.start, d.end, strategy, &d.matched_text));
            }
        }
        spans.sort_by_key(|(start, _, _, _)| *start);

        // Apply replacements from end to start to preserve offsets
        let mut result = text.to_string();
        for &(start, end, strategy, matched) in spans.iter().rev() {
            if end <= result.len() {
                let replacement = apply_mask(matched, strategy);
                result.replace_range(start..end, &replacement);
            }
        }

        let mut detections_by_kind = BTreeMap::new();
        for d in &detections {
            *detections_by_kind
                .entry(format!("{:?}", d.kind))
                .or_insert(0) += 1;
        }

        RedactResult {
            redacted_text: result,
            total_detections: detections.len(),
            detections_by_kind,
            detections,
        }
    }

    /// Get the list of active rules.
    pub fn active_rules(&self) -> &[DetectionRule] {
        &self.rules
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::default_redactor()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_redaction() {
        let r = Redactor::default();
        let result = r.redact("Contact user@example.com for info");
        assert!(!result.redacted_text.contains("user@example.com"));
        assert_eq!(result.total_detections, 1);
        assert!(result.detections_by_kind.contains_key("Email"));
    }

    #[test]
    fn test_ssn_redaction() {
        let r = Redactor::default();
        let result = r.redact("SSN: 123-45-6789");
        assert!(!result.redacted_text.contains("123-45-6789"));
        assert!(result.redacted_text.contains("***REDACTED***"));
    }

    #[test]
    fn test_credit_card_redaction() {
        let r = Redactor::default();
        let result = r.redact("Card: 4111-1111-1111-1111 on file");
        assert!(!result.redacted_text.contains("4111-1111-1111-1111"));
        assert_eq!(result.total_detections, 1);
    }

    #[test]
    fn test_aws_key_redaction() {
        let r = Redactor::default();
        let result = r.redact("key=AKIAIOSFODNN7EXAMPLE");
        assert!(!result.redacted_text.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_partial_mask() {
        let masked = apply_mask("user@example.com", MaskStrategy::PartialMask);
        assert!(masked.starts_with('u'));
        assert!(masked.ends_with('m'));
        assert!(masked.contains('*'));
    }

    #[test]
    fn test_full_mask() {
        assert_eq!(
            apply_mask("secret", MaskStrategy::FullMask),
            "***REDACTED***"
        );
    }

    #[test]
    fn test_hash_mask() {
        let masked = apply_mask("secret", MaskStrategy::Hash);
        assert!(masked.starts_with("[SHA256:"));
        assert!(masked.ends_with(']'));
    }

    #[test]
    fn test_remove_mask() {
        assert_eq!(apply_mask("secret", MaskStrategy::Remove), "");
    }

    #[test]
    fn test_scan_only() {
        let r = Redactor::default();
        let detections = r.scan("email: a@b.com, SSN: 123-45-6789");
        assert!(detections.len() >= 2);
    }

    #[test]
    fn test_disabled() {
        let r = Redactor::new(RedactConfig {
            enabled: false,
            ..Default::default()
        });
        let result = r.redact("SSN: 123-45-6789");
        assert!(result.redacted_text.contains("123-45-6789"));
        assert_eq!(result.total_detections, 0);
    }

    #[test]
    fn test_custom_rule() {
        let config = RedactConfig {
            custom_rules: vec![DetectionRule {
                kind: SensitiveKind::Custom,
                name: "project_code".into(),
                regex: r"PROJ-\d{4,}".into(),
                strategy: MaskStrategy::FullMask,
            }],
            ..Default::default()
        };
        let r = Redactor::new(config);
        let result = r.redact("Reference: PROJ-12345");
        assert!(result.redacted_text.contains("***REDACTED***"));
    }

    #[test]
    fn test_multi_line() {
        let r = Redactor::default();
        let text = "Line 1: user@test.com\nLine 2: 123-45-6789\nLine 3: clean";
        let result = r.redact(text);
        assert!(!result.redacted_text.contains("user@test.com"));
        assert!(!result.redacted_text.contains("123-45-6789"));
        assert!(result.redacted_text.contains("clean"));
    }

    #[test]
    fn test_enabled_kinds_filter() {
        let config = RedactConfig {
            enabled_kinds: vec![SensitiveKind::Email],
            ..Default::default()
        };
        let r = Redactor::new(config);
        // Should only detect emails, not SSNs
        let detections = r.scan("user@test.com 123-45-6789");
        assert!(detections.iter().all(|d| d.kind == SensitiveKind::Email));
    }

    #[test]
    fn test_private_key_detection() {
        let r = Redactor::default();
        let text = "-----BEGIN RSA PRIVATE KEY-----\ndata\n-----END RSA PRIVATE KEY-----";
        let detections = r.scan(text);
        assert!(detections
            .iter()
            .any(|d| d.kind == SensitiveKind::PrivateKey));
    }
}
