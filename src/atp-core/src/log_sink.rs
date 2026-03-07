//! Structured log ingestion and analysis.
//!
//! Parse multi-format logs (JSON, syslog, CLF, key=value), filter/aggregate
//! by severity/field, and generate summary statistics. Built for integration
//! with ATP pipelines for log-centric text processing.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported log formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    /// Newline-delimited JSON (`{"level":"INFO","msg":"..."}`).
    Json,
    /// BSD syslog.
    Syslog,
    /// Apache/Nginx Common Log Format.
    Clf,
    /// Key=value pairs.
    KeyValue,
    /// Auto-detect from line content.
    Auto,
}

/// Severity levels (unified across formats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Severity {
    /// Parse a severity string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" | "INFORMATION" => Some(Self::Info),
            "WARN" | "WARNING" => Some(Self::Warn),
            "ERROR" | "ERR" => Some(Self::Error),
            "FATAL" | "CRITICAL" | "CRIT" | "EMERGENCY" | "EMERG" | "PANIC" => Some(Self::Fatal),
            _ => None,
        }
    }
}

/// A parsed log record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    /// 1-based line number in the input.
    pub line_number: usize,
    /// Detected severity.
    pub severity: Option<Severity>,
    /// Timestamp string (if extracted).
    pub timestamp: Option<String>,
    /// Log message body.
    pub message: String,
    /// Additional structured fields.
    pub fields: BTreeMap<String, String>,
    /// Which format was used to parse this record.
    pub source_format: LogFormat,
}

/// Filter criteria for log records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFilter {
    /// Minimum severity (inclusive). `None` = all severities.
    pub min_severity: Option<Severity>,
    /// Only records whose message contains this substring.
    pub message_contains: Option<String>,
    /// Only records that have a specific field key.
    pub has_field: Option<String>,
    /// Only records where a specific field matches a value.
    pub field_equals: Option<(String, String)>,
    /// Maximum number of records to return (0 = unlimited).
    pub limit: usize,
}

/// Aggregate statistics over a set of log records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogStats {
    /// Total records parsed.
    pub total_records: usize,
    /// Records by severity.
    pub by_severity: BTreeMap<String, usize>,
    /// Records by source format.
    pub by_format: BTreeMap<String, usize>,
    /// Top unique messages (message → count).
    pub top_messages: Vec<(String, usize)>,
    /// Number of records that failed to parse.
    pub parse_errors: usize,
}

/// Alert rule: triggers when a condition is met.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Rule name.
    pub name: String,
    /// Minimum severity to trigger.
    pub min_severity: Severity,
    /// Optional message substring match.
    pub message_pattern: Option<String>,
    /// Description of what this alert means.
    pub description: String,
}

/// An alert triggered by a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub rule_name: String,
    pub record: LogRecord,
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse a single line as JSON log.
fn parse_json_line(line: &str, line_number: usize) -> Option<LogRecord> {
    let obj: serde_json::Value = serde_json::from_str(line).ok()?;
    let map = obj.as_object()?;

    let severity = map
        .iter()
        .find(|(k, _)| {
            let kl = k.to_lowercase();
            kl == "level" || kl == "lvl" || kl == "severity"
        })
        .and_then(|(_, v)| Severity::parse(v.as_str().unwrap_or("")));

    let message = map
        .iter()
        .find(|(k, _)| {
            let kl = k.to_lowercase();
            kl == "msg" || kl == "message"
        })
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
        .to_string();

    let timestamp = map
        .iter()
        .find(|(k, _)| {
            let kl = k.to_lowercase();
            kl == "ts" || kl == "timestamp" || kl == "time" || kl == "t"
        })
        .and_then(|(_, v)| v.as_str().map(|s| s.to_string()));

    let mut fields = BTreeMap::new();
    for (k, v) in map {
        let kl = k.to_lowercase();
        if !["level", "lvl", "severity", "msg", "message", "ts", "timestamp", "time", "t"]
            .contains(&kl.as_str())
        {
            fields.insert(k.clone(), v.to_string().trim_matches('"').to_string());
        }
    }

    Some(LogRecord {
        line_number,
        severity,
        timestamp,
        message,
        fields,
        source_format: LogFormat::Json,
    })
}

/// Parse a line as key=value format.
fn parse_kv_line(line: &str, line_number: usize) -> Option<LogRecord> {
    let re = Regex::new(r#"(?P<key>[A-Za-z_][A-Za-z0-9_]*)=(?:"(?P<qval>[^"]*)"|(?P<val>\S+))"#)
        .ok()?;
    let mut fields = BTreeMap::new();
    for caps in re.captures_iter(line) {
        let key = caps.name("key")?.as_str().to_string();
        let val = caps
            .name("qval")
            .or_else(|| caps.name("val"))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        fields.insert(key, val);
    }

    if fields.is_empty() {
        return None;
    }

    let severity = fields
        .get("level")
        .or_else(|| fields.get("severity"))
        .and_then(|v| Severity::parse(v));

    let message = fields
        .get("msg")
        .or_else(|| fields.get("message"))
        .cloned()
        .unwrap_or_default();

    let timestamp = fields.get("ts").or_else(|| fields.get("timestamp")).cloned();

    Some(LogRecord {
        line_number,
        severity,
        timestamp,
        message,
        fields,
        source_format: LogFormat::KeyValue,
    })
}

/// Parse a line using syslog BSD format.
fn parse_syslog_line(line: &str, line_number: usize) -> Option<LogRecord> {
    let re = Regex::new(
        r"^(?:<\d+>)?(?P<ts>\w{3}\s+\d+\s+\d{2}:\d{2}:\d{2})\s+(?P<host>\S+)\s+(?P<proc>\S+?)(?:\[(?P<pid>\d+)\])?:\s+(?P<msg>.+)$",
    )
    .ok()?;
    let caps = re.captures(line)?;
    let mut fields = BTreeMap::new();
    if let Some(host) = caps.name("host") {
        fields.insert("hostname".into(), host.as_str().into());
    }
    if let Some(proc) = caps.name("proc") {
        fields.insert("process".into(), proc.as_str().into());
    }
    if let Some(pid) = caps.name("pid") {
        fields.insert("pid".into(), pid.as_str().into());
    }

    let message = caps.name("msg")?.as_str().to_string();
    let timestamp = caps.name("ts").map(|m| m.as_str().to_string());

    // Syslog doesn't directly encode severity in BSD format text,
    // but we can try to detect from message keywords.
    let severity = detect_severity_from_text(&message);

    Some(LogRecord {
        line_number,
        severity,
        timestamp,
        message,
        fields,
        source_format: LogFormat::Syslog,
    })
}

/// Parse a line as Common Log Format (CLF).
fn parse_clf_line(line: &str, line_number: usize) -> Option<LogRecord> {
    let re = Regex::new(
        r#"^(?P<host>\S+)\s+\S+\s+(?P<user>\S+)\s+\[(?P<ts>[^\]]+)\]\s+"(?P<method>\w+)\s+(?P<path>\S+)\s+\S+"\s+(?P<status>\d{3})\s+(?P<bytes>\d+|-)"#,
    )
    .ok()?;
    let caps = re.captures(line)?;

    let mut fields = BTreeMap::new();
    fields.insert("host".into(), caps.name("host")?.as_str().into());
    fields.insert("user".into(), caps.name("user")?.as_str().into());
    fields.insert("method".into(), caps.name("method")?.as_str().into());
    fields.insert("path".into(), caps.name("path")?.as_str().into());
    fields.insert("status".into(), caps.name("status")?.as_str().into());
    fields.insert("bytes".into(), caps.name("bytes")?.as_str().into());

    let status: u16 = caps.name("status")?.as_str().parse().ok()?;
    let severity = match status {
        200..=399 => Some(Severity::Info),
        400..=499 => Some(Severity::Warn),
        500..=599 => Some(Severity::Error),
        _ => None,
    };

    let method = caps.name("method")?.as_str();
    let path = caps.name("path")?.as_str();
    let message = format!("{method} {path} → {status}");
    let timestamp = caps.name("ts").map(|m| m.as_str().to_string());

    Some(LogRecord {
        line_number,
        severity,
        timestamp,
        message,
        fields,
        source_format: LogFormat::Clf,
    })
}

/// Try to detect severity from free-text message.
fn detect_severity_from_text(text: &str) -> Option<Severity> {
    let upper = text.to_ascii_uppercase();
    // More specific first
    if upper.contains("FATAL")
        || upper.contains("CRITICAL")
        || upper.contains("PANIC")
        || upper.contains("EMERGENCY")
    {
        return Some(Severity::Fatal);
    }
    if upper.contains("ERROR") || upper.contains("FAILURE") || upper.contains("FAILED") {
        return Some(Severity::Error);
    }
    if upper.contains("WARN") {
        return Some(Severity::Warn);
    }
    if upper.contains("DEBUG") {
        return Some(Severity::Debug);
    }
    if upper.contains("TRACE") {
        return Some(Severity::Trace);
    }
    if upper.contains("INFO") {
        return Some(Severity::Info);
    }
    None
}

/// Auto-detect the format and parse.
fn parse_auto(line: &str, line_number: usize) -> Option<LogRecord> {
    // Try JSON first (starts with {)
    if line.trim_start().starts_with('{') {
        if let Some(r) = parse_json_line(line, line_number) {
            return Some(r);
        }
    }
    // Try CLF (starts with IP or hostname, has [...] bracket)
    if line.contains('[') && line.contains('"') {
        if let Some(r) = parse_clf_line(line, line_number) {
            return Some(r);
        }
    }
    // Try syslog
    if let Some(r) = parse_syslog_line(line, line_number) {
        return Some(r);
    }
    // Try key=value
    if line.contains('=') {
        if let Some(r) = parse_kv_line(line, line_number) {
            return Some(r);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// LogSink
// ---------------------------------------------------------------------------

/// Log ingestion engine: parse, filter, aggregate, alert.
#[derive(Debug, Clone)]
pub struct LogSink {
    /// Parsing format to use.
    pub format: LogFormat,
    /// Filter criteria.
    pub filter: LogFilter,
    /// Alert rules.
    pub alert_rules: Vec<AlertRule>,
}

impl LogSink {
    /// Create a new sink with auto-detection and no filter.
    pub fn new(format: LogFormat) -> Self {
        Self {
            format,
            filter: LogFilter::default(),
            alert_rules: Vec::new(),
        }
    }

    /// Add an alert rule.
    pub fn add_alert_rule(&mut self, rule: AlertRule) {
        self.alert_rules.push(rule);
    }

    /// Parse a single line.
    pub fn parse_line(&self, line: &str, line_number: usize) -> Option<LogRecord> {
        match self.format {
            LogFormat::Json => parse_json_line(line, line_number),
            LogFormat::Syslog => parse_syslog_line(line, line_number),
            LogFormat::Clf => parse_clf_line(line, line_number),
            LogFormat::KeyValue => parse_kv_line(line, line_number),
            LogFormat::Auto => parse_auto(line, line_number),
        }
    }

    /// Parse all lines from text. Non-parseable lines are skipped.
    pub fn parse_all(&self, text: &str) -> (Vec<LogRecord>, usize) {
        let mut records = Vec::new();
        let mut errors = 0;
        for (idx, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match self.parse_line(line, idx + 1) {
                Some(r) => records.push(r),
                None => errors += 1,
            }
        }
        (records, errors)
    }

    /// Apply filter to a set of records.
    pub fn apply_filter(&self, records: &[LogRecord]) -> Vec<LogRecord> {
        let mut out: Vec<LogRecord> = records
            .iter()
            .filter(|r| {
                if let Some(min) = &self.filter.min_severity {
                    if let Some(sev) = &r.severity {
                        if sev < min {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                if let Some(sub) = &self.filter.message_contains {
                    if !r.message.contains(sub.as_str()) {
                        return false;
                    }
                }
                if let Some(field) = &self.filter.has_field {
                    if !r.fields.contains_key(field) {
                        return false;
                    }
                }
                if let Some((k, v)) = &self.filter.field_equals {
                    if r.fields.get(k).map(|fv| fv.as_str()) != Some(v.as_str()) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if self.filter.limit > 0 {
            out.truncate(self.filter.limit);
        }
        out
    }

    /// Compute aggregate statistics.
    pub fn compute_stats(&self, records: &[LogRecord], parse_errors: usize) -> LogStats {
        let mut by_severity = BTreeMap::new();
        let mut by_format = BTreeMap::new();
        let mut msg_counts: BTreeMap<String, usize> = BTreeMap::new();

        for r in records {
            let sev_key = r
                .severity
                .map(|s| format!("{s:?}"))
                .unwrap_or_else(|| "Unknown".into());
            *by_severity.entry(sev_key).or_insert(0) += 1;

            let fmt_key = format!("{:?}", r.source_format);
            *by_format.entry(fmt_key).or_insert(0) += 1;

            *msg_counts.entry(r.message.clone()).or_insert(0) += 1;
        }

        let mut top_messages: Vec<(String, usize)> = msg_counts.into_iter().collect();
        top_messages.sort_by(|a, b| b.1.cmp(&a.1));
        top_messages.truncate(10);

        LogStats {
            total_records: records.len(),
            by_severity,
            by_format,
            top_messages,
            parse_errors,
        }
    }

    /// Check alert rules against records and return triggered alerts.
    pub fn check_alerts(&self, records: &[LogRecord]) -> Vec<Alert> {
        let mut alerts = Vec::new();
        for record in records {
            for rule in &self.alert_rules {
                let severity_ok = record
                    .severity
                    .map(|s| s >= rule.min_severity)
                    .unwrap_or(false);

                let pattern_ok = rule
                    .message_pattern
                    .as_ref()
                    .map(|p| record.message.contains(p.as_str()))
                    .unwrap_or(true);

                if severity_ok && pattern_ok {
                    alerts.push(Alert {
                        rule_name: rule.name.clone(),
                        record: record.clone(),
                    });
                }
            }
        }
        alerts
    }

    /// Full ingest pipeline: parse → filter → stats + alerts.
    pub fn ingest(&self, text: &str) -> LogIngestResult {
        let (all_records, parse_errors) = self.parse_all(text);
        let filtered = self.apply_filter(&all_records);
        let stats = self.compute_stats(&all_records, parse_errors);
        let alerts = self.check_alerts(&filtered);
        LogIngestResult {
            records: filtered,
            stats,
            alerts,
        }
    }
}

/// Result of a full ingest operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogIngestResult {
    pub records: Vec<LogRecord>,
    pub stats: LogStats,
    pub alerts: Vec<Alert>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json() {
        let line = r#"{"level":"ERROR","msg":"disk full","ts":"2026-03-06T10:00:00Z","host":"srv1"}"#;
        let r = parse_json_line(line, 1).unwrap();
        assert_eq!(r.severity, Some(Severity::Error));
        assert_eq!(r.message, "disk full");
        assert_eq!(r.timestamp.as_deref(), Some("2026-03-06T10:00:00Z"));
        assert_eq!(r.fields.get("host").unwrap(), "srv1");
    }

    #[test]
    fn test_parse_kv() {
        let line = r#"ts=2026-03-06 level=WARN msg="high latency" service=api"#;
        let r = parse_kv_line(line, 1).unwrap();
        assert_eq!(r.severity, Some(Severity::Warn));
        assert_eq!(r.message, "high latency");
        assert!(r.fields.contains_key("service"));
    }

    #[test]
    fn test_parse_syslog() {
        let line = "<134>Mar  5 14:23:01 myhost sshd[1234]: Connection closed";
        let r = parse_syslog_line(line, 1).unwrap();
        assert_eq!(r.fields.get("hostname").unwrap(), "myhost");
        assert_eq!(r.fields.get("pid").unwrap(), "1234");
        assert!(r.message.contains("Connection closed"));
    }

    #[test]
    fn test_parse_clf() {
        let line = r#"127.0.0.1 - user [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.1" 200 2326"#;
        let r = parse_clf_line(line, 1).unwrap();
        assert_eq!(r.severity, Some(Severity::Info));
        assert_eq!(r.fields.get("method").unwrap(), "GET");
        assert_eq!(r.fields.get("status").unwrap(), "200");
    }

    #[test]
    fn test_auto_detect_json() {
        let line = r#"{"level":"INFO","msg":"ready"}"#;
        let sink = LogSink::new(LogFormat::Auto);
        let r = sink.parse_line(line, 1).unwrap();
        assert_eq!(r.source_format, LogFormat::Json);
    }

    #[test]
    fn test_auto_detect_clf() {
        let line = r#"10.0.0.1 - admin [06/Mar/2026:09:00:00 +0000] "POST /api HTTP/1.1" 500 128"#;
        let sink = LogSink::new(LogFormat::Auto);
        let r = sink.parse_line(line, 1).unwrap();
        assert_eq!(r.source_format, LogFormat::Clf);
        assert_eq!(r.severity, Some(Severity::Error));
    }

    #[test]
    fn test_severity_filter() {
        let mut sink = LogSink::new(LogFormat::Json);
        sink.filter.min_severity = Some(Severity::Warn);
        let text = r#"{"level":"INFO","msg":"ok"}
{"level":"ERROR","msg":"fail"}
{"level":"WARN","msg":"slow"}"#;
        let (records, _) = sink.parse_all(text);
        let filtered = sink.apply_filter(&records);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.severity.unwrap() >= Severity::Warn));
    }

    #[test]
    fn test_message_filter() {
        let mut sink = LogSink::new(LogFormat::Json);
        sink.filter.message_contains = Some("disk".into());
        let text = r#"{"level":"ERROR","msg":"disk full"}
{"level":"ERROR","msg":"network timeout"}"#;
        let (records, _) = sink.parse_all(text);
        let filtered = sink.apply_filter(&records);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].message.contains("disk"));
    }

    #[test]
    fn test_stats() {
        let sink = LogSink::new(LogFormat::Json);
        let text = r#"{"level":"ERROR","msg":"fail"}
{"level":"ERROR","msg":"fail"}
{"level":"INFO","msg":"ok"}"#;
        let (records, errors) = sink.parse_all(text);
        let stats = sink.compute_stats(&records, errors);
        assert_eq!(stats.total_records, 3);
        assert_eq!(stats.by_severity.get("Error"), Some(&2));
        assert_eq!(stats.by_severity.get("Info"), Some(&1));
        assert_eq!(stats.top_messages[0], ("fail".to_string(), 2));
    }

    #[test]
    fn test_alerts() {
        let mut sink = LogSink::new(LogFormat::Json);
        sink.add_alert_rule(AlertRule {
            name: "disk_alert".into(),
            min_severity: Severity::Error,
            message_pattern: Some("disk".into()),
            description: "Disk issue detected".into(),
        });
        let text = r#"{"level":"ERROR","msg":"disk full"}
{"level":"ERROR","msg":"network error"}
{"level":"INFO","msg":"disk ok"}"#;
        let (records, _) = sink.parse_all(text);
        let alerts = sink.check_alerts(&records);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_name, "disk_alert");
    }

    #[test]
    fn test_full_ingest() {
        let mut sink = LogSink::new(LogFormat::Auto);
        sink.add_alert_rule(AlertRule {
            name: "error_alert".into(),
            min_severity: Severity::Error,
            message_pattern: None,
            description: "Any error".into(),
        });
        let text = r#"{"level":"ERROR","msg":"crash"}
{"level":"INFO","msg":"boot"}
not a log line"#;
        let result = sink.ingest(text);
        assert_eq!(result.stats.total_records, 2);
        assert_eq!(result.stats.parse_errors, 1);
        assert_eq!(result.alerts.len(), 1);
    }

    #[test]
    fn test_limit_filter() {
        let mut sink = LogSink::new(LogFormat::Json);
        sink.filter.limit = 1;
        let text = r#"{"level":"INFO","msg":"a"}
{"level":"INFO","msg":"b"}
{"level":"INFO","msg":"c"}"#;
        let (records, _) = sink.parse_all(text);
        let filtered = sink.apply_filter(&records);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_field_equals_filter() {
        let mut sink = LogSink::new(LogFormat::Json);
        sink.filter.field_equals = Some(("host".into(), "srv2".into()));
        let text = r#"{"level":"INFO","msg":"a","host":"srv1"}
{"level":"INFO","msg":"b","host":"srv2"}"#;
        let (records, _) = sink.parse_all(text);
        let filtered = sink.apply_filter(&records);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "b");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Trace < Severity::Debug);
        assert!(Severity::Debug < Severity::Info);
        assert!(Severity::Info < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
        assert!(Severity::Error < Severity::Fatal);
    }
}
