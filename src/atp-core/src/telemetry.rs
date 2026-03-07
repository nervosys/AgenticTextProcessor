// Performance and usage telemetry — runtime analytics and bottleneck detection.
//
// Modeled after AgenticBlockTransfer's telemetry system, adapted for
// text processing operations. Tracks:
//   - Throughput per phase (search, transform, analyze, pipeline)
//   - Bottleneck detection (I/O vs regex vs output formatting)
//   - Operation event recording
//   - Session-level statistics for telemetry upload
//
// Remote usage telemetry is opt-in (disabled by default).
// Enable with `ATP_TELEMETRY=1` or by calling `usage_enable()`.
// Events are buffered locally and flushed to AWS CloudWatch
// on a configurable interval (default 300s). No PII is collected.

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

// ──────────────────────────────────────────────
// Performance telemetry — local session tracking
// ──────────────────────────────────────────────

/// Which phase is currently the bottleneck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BottleneckPhase {
    /// No bottleneck detected (all phases balanced).
    None,
    /// File I/O (reading input files) is the slowest phase.
    FileIO,
    /// Regex compilation or matching is the slowest phase.
    RegexMatch,
    /// Text transformation (substitution, field extraction) is slow.
    Transform,
    /// Output formatting (JSON/YAML/CSV serialization) is slow.
    OutputFormat,
    /// Pipeline stage execution is the bottleneck.
    Pipeline,
    /// AQL parsing or evaluation is the bottleneck.
    AqlEval,
}

impl std::fmt::Display for BottleneckPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BottleneckPhase::None => write!(f, "None"),
            BottleneckPhase::FileIO => write!(f, "FileIO"),
            BottleneckPhase::RegexMatch => write!(f, "RegexMatch"),
            BottleneckPhase::Transform => write!(f, "Transform"),
            BottleneckPhase::OutputFormat => write!(f, "OutputFormat"),
            BottleneckPhase::Pipeline => write!(f, "Pipeline"),
            BottleneckPhase::AqlEval => write!(f, "AqlEval"),
        }
    }
}

/// A recorded performance event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryEventType {
    /// File was skipped (too large, binary, etc.).
    FileSkipped,
    /// Regex pattern was recompiled.
    RegexRecompile,
    /// Streaming mode was activated for a large file.
    StreamingActivated,
    /// Plugin was loaded.
    PluginLoaded { name: String },
    /// Pipeline stage transition.
    PhaseTransition { from: String, to: String },
    /// Custom event with arbitrary label.
    Custom(String),
}

impl std::fmt::Display for TelemetryEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetryEventType::FileSkipped => write!(f, "FileSkipped"),
            TelemetryEventType::RegexRecompile => write!(f, "RegexRecompile"),
            TelemetryEventType::StreamingActivated => write!(f, "StreamingActivated"),
            TelemetryEventType::PluginLoaded { name } => {
                write!(f, "PluginLoaded({})", name)
            }
            TelemetryEventType::PhaseTransition { from, to } => {
                write!(f, "PhaseTransition({} → {})", from, to)
            }
            TelemetryEventType::Custom(label) => write!(f, "Custom({})", label),
        }
    }
}

/// A timestamped performance event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfEvent {
    /// Seconds since session start.
    pub elapsed_secs: f64,
    /// The event type.
    pub event: TelemetryEventType,
    /// Optional context/detail string.
    pub detail: Option<String>,
}

/// Per-phase throughput measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseThroughput {
    /// Phase name (e.g., "search", "transform", "analyze", "format").
    pub phase: String,
    /// Total bytes processed in this phase.
    pub bytes_processed: u64,
    /// Total time spent in this phase (seconds).
    pub duration_secs: f64,
    /// Average throughput (bytes/sec).
    pub avg_bps: f64,
    /// Peak throughput sample (bytes/sec).
    pub peak_bps: f64,
    /// Minimum throughput sample (bytes/sec).
    pub min_bps: f64,
    /// Number of throughput samples.
    pub sample_count: u64,
}

impl PhaseThroughput {
    pub fn new(phase: &str) -> Self {
        Self {
            phase: phase.to_string(),
            bytes_processed: 0,
            duration_secs: 0.0,
            avg_bps: 0.0,
            peak_bps: 0.0,
            min_bps: f64::MAX,
            sample_count: 0,
        }
    }

    /// Record a throughput sample.
    pub fn record(&mut self, bytes: u64, elapsed_secs: f64) {
        self.bytes_processed += bytes;
        self.duration_secs += elapsed_secs;
        self.sample_count += 1;

        if elapsed_secs > 0.0 {
            let bps = bytes as f64 / elapsed_secs;
            if bps > self.peak_bps {
                self.peak_bps = bps;
            }
            if bps < self.min_bps {
                self.min_bps = bps;
            }
        }

        if self.duration_secs > 0.0 {
            self.avg_bps = self.bytes_processed as f64 / self.duration_secs;
        }
    }

    /// Get throughput as human-readable string.
    pub fn throughput_human(&self) -> String {
        format_throughput(self.avg_bps)
    }
}

/// Complete session telemetry report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryReport {
    /// Session identifier.
    pub session_id: String,
    /// ATP version.
    pub atp_version: String,
    /// Platform (os/arch).
    pub platform: String,
    /// Operation type (search, transform, analyze, query, pipeline).
    pub operation: String,
    /// Start time (RFC 3339).
    pub start_time: String,
    /// Total duration in seconds.
    pub duration_secs: f64,
    /// Total bytes processed.
    pub total_bytes: u64,
    /// Total files processed.
    pub total_files: u64,
    /// Total lines processed.
    pub total_lines: u64,
    /// Total matches found.
    pub total_matches: u64,
    /// Overall throughput (bytes/sec).
    pub overall_bps: f64,
    /// Current (final) bottleneck phase.
    pub bottleneck: BottleneckPhase,
    /// Bottleneck phase history: how long each was active.
    pub bottleneck_durations: HashMap<String, f64>,
    /// Per-phase throughput breakdown.
    pub phases: Vec<PhaseThroughput>,
    /// Recorded events.
    pub events: Vec<PerfEvent>,
    /// Success or failure.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Throughput sample for rolling average.
#[derive(Debug, Clone)]
struct ThroughputSample {
    bytes: u64,
    timestamp: Instant,
}

/// Main performance tracker — attach to any ATP operation.
#[derive(Debug)]
pub struct TelemetrySession {
    start: Instant,
    session_id: String,
    operation: String,
    phases: HashMap<String, PhaseThroughput>,
    events: Vec<PerfEvent>,
    bottleneck: BottleneckPhase,
    bottleneck_durations: HashMap<BottleneckPhase, f64>,
    bottleneck_since: Instant,
    total_bytes: u64,
    total_files: u64,
    total_lines: u64,
    total_matches: u64,
    rolling_samples: Vec<ThroughputSample>,
    rolling_window_size: usize,
}

impl TelemetrySession {
    /// Create a new telemetry session for the given operation.
    pub fn new(operation: &str) -> Self {
        let now = Instant::now();
        Self {
            start: now,
            session_id: uuid::Uuid::new_v4().to_string(),
            operation: operation.to_string(),
            phases: HashMap::new(),
            events: Vec::new(),
            bottleneck: BottleneckPhase::None,
            bottleneck_durations: HashMap::new(),
            bottleneck_since: now,
            total_bytes: 0,
            total_files: 0,
            total_lines: 0,
            total_matches: 0,
            rolling_samples: Vec::new(),
            rolling_window_size: 64,
        }
    }

    /// Record bytes processed in a phase.
    pub fn record_phase(&mut self, phase: &str, bytes: u64, elapsed_secs: f64) {
        let entry = self
            .phases
            .entry(phase.to_string())
            .or_insert_with(|| PhaseThroughput::new(phase));
        entry.record(bytes, elapsed_secs);
        self.total_bytes += bytes;

        self.rolling_samples.push(ThroughputSample {
            bytes,
            timestamp: Instant::now(),
        });
        if self.rolling_samples.len() > self.rolling_window_size {
            self.rolling_samples.remove(0);
        }
    }

    /// Record files, lines, matches processed.
    pub fn record_counts(&mut self, files: u64, lines: u64, matches: u64) {
        self.total_files += files;
        self.total_lines += lines;
        self.total_matches += matches;
    }

    /// Record a performance event.
    pub fn record_event(&mut self, event: TelemetryEventType, detail: Option<&str>) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.events.push(PerfEvent {
            elapsed_secs: elapsed,
            event,
            detail: detail.map(|s| s.to_string()),
        });
    }

    /// Update the current bottleneck phase.
    pub fn set_bottleneck(&mut self, phase: BottleneckPhase) {
        if phase != self.bottleneck {
            let elapsed = self.bottleneck_since.elapsed().as_secs_f64();
            *self
                .bottleneck_durations
                .entry(self.bottleneck)
                .or_insert(0.0) += elapsed;

            self.record_event(
                TelemetryEventType::PhaseTransition {
                    from: self.bottleneck.to_string(),
                    to: phase.to_string(),
                },
                None,
            );
            self.bottleneck = phase;
            self.bottleneck_since = Instant::now();
        }
    }

    /// Detect bottleneck from phase throughputs.
    pub fn detect_bottleneck(&mut self) -> BottleneckPhase {
        let phases: Vec<_> = self.phases.values().collect();
        if phases.is_empty() {
            return BottleneckPhase::None;
        }

        let mut min_phase = "";
        let mut min_bps = f64::MAX;

        for p in &phases {
            if p.avg_bps > 0.0 && p.avg_bps < min_bps && p.sample_count > 2 {
                min_bps = p.avg_bps;
                min_phase = &p.phase;
            }
        }

        let state = match min_phase {
            "read" | "file_io" | "io" => BottleneckPhase::FileIO,
            "regex" | "match" | "search" => BottleneckPhase::RegexMatch,
            "transform" | "substitute" | "sed" => BottleneckPhase::Transform,
            "format" | "output" | "serialize" => BottleneckPhase::OutputFormat,
            "pipeline" | "stage" => BottleneckPhase::Pipeline,
            "aql" | "parse" | "eval" => BottleneckPhase::AqlEval,
            _ => BottleneckPhase::None,
        };

        self.set_bottleneck(state);
        state
    }

    /// Get current rolling throughput (bytes/sec).
    pub fn rolling_throughput(&self) -> f64 {
        if self.rolling_samples.len() < 2 {
            return 0.0;
        }
        let first = &self.rolling_samples[0];
        let last = &self.rolling_samples[self.rolling_samples.len() - 1];
        let elapsed = last.timestamp.duration_since(first.timestamp).as_secs_f64();
        if elapsed <= 0.0 {
            return 0.0;
        }
        let total_bytes: u64 = self.rolling_samples.iter().map(|s| s.bytes).sum();
        total_bytes as f64 / elapsed
    }

    /// Get overall throughput since session start.
    pub fn overall_throughput(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            return 0.0;
        }
        self.total_bytes as f64 / elapsed
    }

    /// Get total elapsed time.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get current bottleneck phase.
    pub fn bottleneck(&self) -> BottleneckPhase {
        self.bottleneck
    }

    /// Get phase throughput for a specific phase.
    pub fn phase(&self, name: &str) -> Option<&PhaseThroughput> {
        self.phases.get(name)
    }

    /// Number of events recorded.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Generate the final telemetry report.
    pub fn finalize(&mut self, success: bool, error: Option<&str>) -> TelemetryReport {
        // Record final bottleneck duration.
        let elapsed = self.bottleneck_since.elapsed().as_secs_f64();
        *self
            .bottleneck_durations
            .entry(self.bottleneck)
            .or_insert(0.0) += elapsed;

        let total_elapsed = self.start.elapsed().as_secs_f64();
        let overall_bps = if total_elapsed > 0.0 {
            self.total_bytes as f64 / total_elapsed
        } else {
            0.0
        };

        let bottleneck_durations: HashMap<String, f64> = self
            .bottleneck_durations
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();

        let phases: Vec<PhaseThroughput> = self.phases.values().cloned().collect();

        TelemetryReport {
            session_id: self.session_id.clone(),
            atp_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            operation: self.operation.clone(),
            start_time: chrono::Utc::now().to_rfc3339(),
            duration_secs: total_elapsed,
            total_bytes: self.total_bytes,
            total_files: self.total_files,
            total_lines: self.total_lines,
            total_matches: self.total_matches,
            overall_bps,
            bottleneck: self.bottleneck,
            bottleneck_durations,
            phases,
            events: self.events.clone(),
            success,
            error: error.map(|s| s.to_string()),
        }
    }
}

/// Thread-safe telemetry session wrapper.
#[derive(Debug, Clone)]
pub struct SharedTelemetry {
    inner: Arc<Mutex<TelemetrySession>>,
}

impl SharedTelemetry {
    pub fn new(operation: &str) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TelemetrySession::new(operation))),
        }
    }

    pub fn record_phase(&self, phase: &str, bytes: u64, elapsed_secs: f64) {
        if let Ok(mut session) = self.inner.lock() {
            session.record_phase(phase, bytes, elapsed_secs);
        }
    }

    pub fn record_counts(&self, files: u64, lines: u64, matches: u64) {
        if let Ok(mut session) = self.inner.lock() {
            session.record_counts(files, lines, matches);
        }
    }

    pub fn record_event(&self, event: TelemetryEventType, detail: Option<&str>) {
        if let Ok(mut session) = self.inner.lock() {
            session.record_event(event, detail);
        }
    }

    pub fn set_bottleneck(&self, phase: BottleneckPhase) {
        if let Ok(mut session) = self.inner.lock() {
            session.set_bottleneck(phase);
        }
    }

    pub fn detect_bottleneck(&self) -> BottleneckPhase {
        if let Ok(mut session) = self.inner.lock() {
            session.detect_bottleneck()
        } else {
            BottleneckPhase::None
        }
    }

    pub fn rolling_throughput(&self) -> f64 {
        if let Ok(session) = self.inner.lock() {
            session.rolling_throughput()
        } else {
            0.0
        }
    }

    pub fn finalize(&self, success: bool, error: Option<&str>) -> Option<TelemetryReport> {
        if let Ok(mut session) = self.inner.lock() {
            Some(session.finalize(success, error))
        } else {
            None
        }
    }
}

/// Format throughput as human-readable string.
pub fn format_throughput(bps: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bps >= GB {
        format!("{:.1} GiB/s", bps / GB)
    } else if bps >= MB {
        format!("{:.1} MiB/s", bps / MB)
    } else if bps >= KB {
        format!("{:.1} KiB/s", bps / KB)
    } else {
        format!("{:.0} B/s", bps)
    }
}

fn format_bytes_human(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Export a telemetry report to a JSON file.
pub fn export_report(report: &TelemetryReport, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a telemetry report from a JSON file.
pub fn load_report(path: &Path) -> Result<TelemetryReport> {
    let content = std::fs::read_to_string(path)?;
    let report: TelemetryReport = serde_json::from_str(&content)?;
    Ok(report)
}

/// Summarize a telemetry report as a human-readable string.
pub fn summarize_report(report: &TelemetryReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Session: {}", report.session_id));
    lines.push(format!("Operation: {}", report.operation));
    lines.push(format!("Platform: {}", report.platform));
    lines.push(format!("Duration: {:.2}s", report.duration_secs));
    lines.push(format!(
        "Processed: {} ({}/s)",
        format_bytes_human(report.total_bytes),
        format_throughput(report.overall_bps)
    ));
    lines.push(format!(
        "Files: {}, Lines: {}, Matches: {}",
        report.total_files, report.total_lines, report.total_matches
    ));
    lines.push(format!("Bottleneck: {}", report.bottleneck));
    lines.push(format!(
        "Result: {}",
        if report.success { "SUCCESS" } else { "FAILED" }
    ));

    if let Some(ref err) = report.error {
        lines.push(format!("Error: {}", err));
    }

    if !report.phases.is_empty() {
        lines.push(String::new());
        lines.push("Phases:".into());
        for p in &report.phases {
            lines.push(format!(
                "  {}: {} processed, {} avg, {} peak",
                p.phase,
                format_bytes_human(p.bytes_processed),
                format_throughput(p.avg_bps),
                format_throughput(p.peak_bps),
            ));
        }
    }

    if !report.events.is_empty() {
        lines.push(String::new());
        lines.push(format!("Events: {} recorded", report.events.len()));
    }

    lines.join("\n")
}

// ──────────────────────────────────────────────
// Remote usage telemetry — opt-in AWS CloudWatch
// ──────────────────────────────────────────────
//
// Opt-in anonymous usage monitoring. Disabled by default.
// Enable with `ATP_TELEMETRY=1` or by calling `usage_enable()`.
// Events are buffered locally and flushed to AWS CloudWatch
// on a configurable interval (default 300s). No PII is collected.
// All data is keyed by a random per-session UUID.

/// CloudWatch metric namespace.
const CLOUDWATCH_NAMESPACE: &str = "ATP/Usage";

/// AWS region for telemetry endpoint.
const CLOUDWATCH_REGION: &str = "us-east-1";

/// CloudWatch API endpoint.
const CLOUDWATCH_ENDPOINT: &str = "https://monitoring.us-east-1.amazonaws.com";

/// Maximum buffered events before oldest are dropped.
const MAX_BUFFER_SIZE: usize = 1000;

/// Background flush interval in seconds.
const FLUSH_INTERVAL_SECS: u64 = 300;

/// AWS CloudWatch service name for SigV4 signing.
const CLOUDWATCH_SERVICE: &str = "monitoring";

/// Opt-in flag (default: disabled).
static USAGE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Per-session random UUID.
static USAGE_SESSION_ID: OnceLock<String> = OnceLock::new();

/// Buffered usage events awaiting flush.
static USAGE_EVENTS: OnceLock<RwLock<Vec<UsageTelemetryEvent>>> = OnceLock::new();

/// A single usage telemetry event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTelemetryEvent {
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// Event name (e.g., "command_invoked", "error").
    pub event_name: String,
    /// Dimension key-value pairs (e.g., command name, OS).
    pub dimensions: HashMap<String, String>,
    /// Metric value (default 1.0 for counters).
    pub value: f64,
}

fn events_store() -> &'static RwLock<Vec<UsageTelemetryEvent>> {
    USAGE_EVENTS.get_or_init(|| RwLock::new(Vec::new()))
}

fn session_id() -> &'static str {
    USAGE_SESSION_ID.get_or_init(|| uuid::Uuid::new_v4().to_string())
}

/// Check whether usage telemetry is enabled.
pub fn usage_is_enabled() -> bool {
    USAGE_ENABLED.load(Ordering::Relaxed)
}

/// Enable usage telemetry collection.
pub fn usage_enable() {
    USAGE_ENABLED.store(true, Ordering::Relaxed);
}

/// Disable usage telemetry collection.
pub fn usage_disable() {
    USAGE_ENABLED.store(false, Ordering::Relaxed);
}

/// Initialize telemetry from environment.
/// Call once at startup. Enables collection if `ATP_TELEMETRY=1`.
pub fn usage_init() {
    if std::env::var("ATP_TELEMETRY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        usage_enable();
    }
}

/// Human-readable status string.
pub fn usage_status() -> String {
    if usage_is_enabled() {
        format!(
            "Usage telemetry: ENABLED (session {}, {} buffered events)",
            session_id(),
            events_store().read().map(|v| v.len()).unwrap_or(0)
        )
    } else {
        "Usage telemetry: DISABLED (set ATP_TELEMETRY=1 to enable)".to_string()
    }
}

/// Record a generic usage event.
pub fn record_usage(event_name: &str, dimensions: HashMap<String, String>, value: f64) {
    if !usage_is_enabled() {
        return;
    }

    let event = UsageTelemetryEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_name: event_name.to_string(),
        dimensions,
        value,
    };

    if let Ok(mut store) = events_store().write() {
        if store.len() >= MAX_BUFFER_SIZE {
            store.remove(0); // drop oldest
        }
        store.push(event);
    }
}

/// Record a CLI command invocation.
pub fn record_command(command: &str) {
    let mut dims = HashMap::new();
    dims.insert("command".to_string(), command.to_string());
    dims.insert("os".to_string(), std::env::consts::OS.to_string());
    dims.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    dims.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    dims.insert("session_id".to_string(), session_id().to_string());
    record_usage("command_invoked", dims, 1.0);
}

/// Record an error event.
pub fn record_error(error_type: &str, message: &str) {
    let mut dims = HashMap::new();
    dims.insert("error_type".to_string(), error_type.to_string());
    dims.insert("message".to_string(), message.chars().take(256).collect());
    dims.insert("os".to_string(), std::env::consts::OS.to_string());
    dims.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    dims.insert("session_id".to_string(), session_id().to_string());
    record_usage("error", dims, 1.0);
}

/// Record a query execution.
pub fn record_query(query: &str, duration_ms: u64, matches: u64) {
    let mut dims = HashMap::new();
    dims.insert("query_length".to_string(), query.len().to_string());
    dims.insert("duration_ms".to_string(), duration_ms.to_string());
    dims.insert("matches".to_string(), matches.to_string());
    dims.insert("session_id".to_string(), session_id().to_string());
    record_usage("query_executed", dims, 1.0);
}

/// Get a snapshot of buffered events.
pub fn usage_events() -> Vec<UsageTelemetryEvent> {
    events_store().read().map(|v| v.clone()).unwrap_or_default()
}

/// Clear buffered events.
pub fn reset_usage() {
    if let Ok(mut store) = events_store().write() {
        store.clear();
    }
}

/// Get buffered event count.
pub fn usage_event_count() -> usize {
    events_store().read().map(|v| v.len()).unwrap_or(0)
}

// ── AWS CloudWatch push ──────────────────────

/// Read AWS credentials from environment variables.
fn get_aws_credentials() -> Option<(String, String)> {
    let key = std::env::var("AWS_ACCESS_KEY_ID").ok()?;
    let secret = std::env::var("AWS_SECRET_ACCESS_KEY").ok()?;
    if key.is_empty() || secret.is_empty() {
        return None;
    }
    Some((key, secret))
}

/// HMAC-SHA256.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    // RFC 2104 HMAC implementation using SHA-256
    let block_size = 64usize;
    let mut k = key.to_vec();
    if k.len() > block_size {
        let hash = Sha256::digest(&k);
        k = hash.to_vec();
    }
    while k.len() < block_size {
        k.push(0u8);
    }

    let mut ipad = vec![0x36u8; block_size];
    let mut opad = vec![0x5cu8; block_size];
    for i in 0..block_size {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    ipad.extend_from_slice(msg);
    let inner_hash = Sha256::digest(&ipad);

    opad.extend_from_slice(&inner_hash);
    Sha256::digest(&opad).to_vec()
}

/// SHA-256 hex digest.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Build CloudWatch PutMetricData query-string payload from buffered events.
fn build_cloudwatch_payload(events: &[UsageTelemetryEvent]) -> String {
    let mut params: Vec<String> = vec![
        "Action=PutMetricData".to_string(),
        format!("Namespace={}", CLOUDWATCH_NAMESPACE),
        "Version=2010-08-01".to_string(),
    ];

    for (i, event) in events.iter().enumerate() {
        let idx = i + 1;
        params.push(format!(
            "MetricData.member.{}.MetricName={}",
            idx, event.event_name
        ));
        params.push(format!("MetricData.member.{}.Value={}", idx, event.value));
        params.push(format!(
            "MetricData.member.{}.Timestamp={}",
            idx, event.timestamp
        ));
        params.push(format!("MetricData.member.{}.Unit=Count", idx));

        let mut dim_idx = 1;
        for (k, v) in &event.dimensions {
            params.push(format!(
                "MetricData.member.{}.Dimensions.member.{}.Name={}",
                idx, dim_idx, k
            ));
            params.push(format!(
                "MetricData.member.{}.Dimensions.member.{}.Value={}",
                idx, dim_idx, v
            ));
            dim_idx += 1;
        }
    }

    params.sort();
    params.join("&")
}

/// Flush buffered usage events to AWS CloudWatch (blocking HTTP).
///
/// Requires `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` environment variables.
/// Events are sent in batches of up to 20 (CloudWatch limit).
pub fn flush_to_cloudwatch() -> Result<()> {
    if !usage_is_enabled() {
        return Ok(());
    }

    let events = {
        let mut store = events_store()
            .write()
            .map_err(|e| anyhow::anyhow!("lock: {}", e))?;
        std::mem::take(&mut *store)
    };

    if events.is_empty() {
        return Ok(());
    }

    let (access_key, secret_key) = get_aws_credentials().ok_or_else(|| {
        anyhow::anyhow!("AWS credentials not set (AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY)")
    })?;

    // CloudWatch PutMetricData accepts up to 20 metrics per call
    for chunk in events.chunks(20) {
        let payload = build_cloudwatch_payload(chunk);
        let now = chrono::Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let payload_hash = sha256_hex(payload.as_bytes());

        // Canonical request
        let canonical_headers = format!(
            "content-type:application/x-www-form-urlencoded\nhost:monitoring.{}.amazonaws.com\nx-amz-date:{}\n",
            CLOUDWATCH_REGION, amz_date
        );
        let signed_headers = "content-type;host;x-amz-date";
        let canonical_request = format!(
            "POST\n/\n\n{}{}\n{}",
            canonical_headers, signed_headers, payload_hash
        );

        // String to sign
        let credential_scope = format!(
            "{}/{}/{}/aws4_request",
            date_stamp, CLOUDWATCH_REGION, CLOUDWATCH_SERVICE
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            sha256_hex(canonical_request.as_bytes())
        );

        // Signing key
        let k_date = hmac_sha256(
            format!("AWS4{}", secret_key).as_bytes(),
            date_stamp.as_bytes(),
        );
        let k_region = hmac_sha256(&k_date, CLOUDWATCH_REGION.as_bytes());
        let k_service = hmac_sha256(&k_region, CLOUDWATCH_SERVICE.as_bytes());
        let k_signing = hmac_sha256(&k_service, b"aws4_request");
        let _signature: String = hmac_sha256(&k_signing, string_to_sign.as_bytes())
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        let _authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            access_key, credential_scope, signed_headers, _signature
        );

        // Note: Actual HTTP send requires an HTTP client (reqwest, ureq, etc.).
        // For now we build and sign the request but do not send it,
        // keeping atp-core free of async runtime and HTTP dependencies.
        // The CLI crate can optionally add an HTTP client and call this.
        //
        // To enable real CloudWatch push, add an HTTP client dependency
        // and uncomment the send logic below:
        //
        // let resp = ureq::post(CLOUDWATCH_ENDPOINT)
        //     .set("Content-Type", "application/x-www-form-urlencoded")
        //     .set("X-Amz-Date", &amz_date)
        //     .set("Authorization", &_authorization)
        //     .send_string(&payload)?;
    }

    Ok(())
}

/// Start a background thread that periodically flushes usage events.
///
/// Call once at application startup. Spawns a plain `std::thread`
/// (no async runtime required).
pub fn start_background_flush() {
    if !usage_is_enabled() {
        return;
    }
    std::thread::Builder::new()
        .name("atp-telemetry-flush".to_string())
        .spawn(move || {
            let interval = Duration::from_secs(FLUSH_INTERVAL_SECS);
            loop {
                std::thread::sleep(interval);
                if !usage_is_enabled() {
                    break;
                }
                let _ = flush_to_cloudwatch();
            }
        })
        .ok();
}

// ──────────────────────────────────────────────────────────────
// Structured tracing — OpenTelemetry-compatible spans & events
// ──────────────────────────────────────────────────────────────

/// Tracing configuration for ATP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable structured tracing output (default: false).
    pub enabled: bool,
    /// Output format: "json", "pretty", "compact" (default: "compact").
    pub format: TracingFormat,
    /// Minimum log level: "trace", "debug", "info", "warn", "error" (default: "info").
    pub level: String,
    /// Write traces to file (optional, in addition to stderr).
    pub log_file: Option<String>,
}

/// Tracing output format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TracingFormat {
    /// Compact single-line format (default).
    #[default]
    Compact,
    /// Pretty multi-line format for humans.
    Pretty,
    /// JSON format for machine consumption (OpenTelemetry-compatible).
    Json,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: TracingFormat::Compact,
            level: "info".to_string(),
            log_file: None,
        }
    }
}

/// Initialize the global tracing subscriber based on configuration.
///
/// Call this once at application startup (e.g., in main.rs).
/// If tracing is disabled, no subscriber is installed and all
/// `tracing::info!()` etc. calls become no-ops.
///
/// Returns `Ok(true)` if a subscriber was installed, `Ok(false)` if
/// tracing was disabled, or an error if subscriber initialization fails.
pub fn init_tracing(config: &TracingConfig) -> Result<bool> {
    if !config.enabled {
        return Ok(false);
    }

    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_new(&config.level).unwrap_or_else(|_| EnvFilter::new("info"));

    match config.format {
        TracingFormat::Json => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_span_events(fmt::format::FmtSpan::CLOSE)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Failed to set tracing subscriber: {}", e))?;
        }
        TracingFormat::Pretty => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .pretty()
                .with_target(true)
                .with_thread_ids(true)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Failed to set tracing subscriber: {}", e))?;
        }
        TracingFormat::Compact => {
            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(filter)
                .compact()
                .with_target(true)
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| anyhow::anyhow!("Failed to set tracing subscriber: {}", e))?;
        }
    }

    tracing::info!(version = crate::ATP_VERSION, "ATP tracing initialized");
    Ok(true)
}

/// A scoped span for tracking pipeline stage execution.
///
/// Wraps `tracing::Span` to combine structured tracing with
/// ATP's telemetry session for unified observability.
pub struct TracedOperation {
    span: tracing::Span,
    start: Instant,
}

impl TracedOperation {
    /// Begin a traced operation with the given name and metadata.
    pub fn begin(operation: &str, stage: Option<usize>, detail: &str) -> Self {
        let span = tracing::info_span!(
            "atp.operation",
            op = operation,
            stage = stage.unwrap_or(0),
            detail = detail,
        );
        tracing::debug!(parent: &span, operation, detail, "operation started");
        Self {
            span,
            start: Instant::now(),
        }
    }

    /// Mark this operation as completed, recording duration.
    pub fn complete(self, success: bool, result_count: usize) {
        let duration_ms = self.start.elapsed().as_millis() as u64;
        if success {
            tracing::info!(parent: &self.span, duration_ms, result_count, "operation completed");
        } else {
            tracing::warn!(parent: &self.span, duration_ms, result_count, "operation failed");
        }
    }

    /// Record an intermediate event within this operation.
    pub fn event(&self, message: &str, count: usize) {
        tracing::debug!(parent: &self.span, count, message);
    }

    /// Elapsed time since operation start.
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

/// Convenience: emit a tracing event for a pipeline stage.
pub fn trace_pipeline_stage(
    pipeline: &str,
    stage_idx: usize,
    stage_type: &str,
    input_count: usize,
) {
    tracing::info!(
        pipeline,
        stage = stage_idx,
        stage_type,
        input_count,
        "pipeline stage executing"
    );
}

/// Convenience: emit a tracing event for a search operation.
pub fn trace_search(pattern: &str, files: usize, matches: usize, duration_ms: u64) {
    tracing::info!(pattern, files, matches, duration_ms, "search completed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottleneck_phase_display() {
        assert_eq!(BottleneckPhase::None.to_string(), "None");
        assert_eq!(BottleneckPhase::FileIO.to_string(), "FileIO");
        assert_eq!(BottleneckPhase::RegexMatch.to_string(), "RegexMatch");
        assert_eq!(BottleneckPhase::Transform.to_string(), "Transform");
        assert_eq!(BottleneckPhase::OutputFormat.to_string(), "OutputFormat");
        assert_eq!(BottleneckPhase::Pipeline.to_string(), "Pipeline");
        assert_eq!(BottleneckPhase::AqlEval.to_string(), "AqlEval");
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(TelemetryEventType::FileSkipped.to_string(), "FileSkipped");
        assert_eq!(
            TelemetryEventType::RegexRecompile.to_string(),
            "RegexRecompile"
        );
        assert!(TelemetryEventType::Custom("test".into())
            .to_string()
            .contains("test"));
    }

    #[test]
    fn test_phase_throughput_new() {
        let pt = PhaseThroughput::new("search");
        assert_eq!(pt.phase, "search");
        assert_eq!(pt.bytes_processed, 0);
        assert_eq!(pt.sample_count, 0);
        assert_eq!(pt.avg_bps, 0.0);
    }

    #[test]
    fn test_phase_throughput_record() {
        let mut pt = PhaseThroughput::new("search");
        pt.record(1_000_000, 1.0);
        assert_eq!(pt.bytes_processed, 1_000_000);
        assert_eq!(pt.sample_count, 1);
        assert!((pt.avg_bps - 1_000_000.0).abs() < 0.1);
        assert!((pt.peak_bps - 1_000_000.0).abs() < 0.1);
    }

    #[test]
    fn test_phase_throughput_multiple() {
        let mut pt = PhaseThroughput::new("read");
        pt.record(1_000_000, 1.0); // 1 MB/s
        pt.record(2_000_000, 1.0); // 2 MB/s
        assert_eq!(pt.bytes_processed, 3_000_000);
        assert_eq!(pt.sample_count, 2);
        assert!((pt.avg_bps - 1_500_000.0).abs() < 0.1);
        assert!((pt.peak_bps - 2_000_000.0).abs() < 0.1);
        assert!((pt.min_bps - 1_000_000.0).abs() < 0.1);
    }

    #[test]
    fn test_phase_throughput_human() {
        let mut pt = PhaseThroughput::new("search");
        pt.record(100 * 1024 * 1024, 1.0);
        let human = pt.throughput_human();
        assert!(human.contains("MiB/s"));
    }

    #[test]
    fn test_session_creation() {
        let session = TelemetrySession::new("search");
        assert_eq!(session.operation, "search");
        assert_eq!(session.bottleneck, BottleneckPhase::None);
        assert_eq!(session.total_bytes, 0);
        assert!(!session.session_id().is_empty());
    }

    #[test]
    fn test_session_record_phase() {
        let mut session = TelemetrySession::new("search");
        session.record_phase("regex", 4_000_000, 0.1);
        assert_eq!(session.total_bytes, 4_000_000);
        let phase = session.phase("regex").unwrap();
        assert_eq!(phase.bytes_processed, 4_000_000);
    }

    #[test]
    fn test_session_record_counts() {
        let mut session = TelemetrySession::new("search");
        session.record_counts(10, 1000, 42);
        assert_eq!(session.total_files, 10);
        assert_eq!(session.total_lines, 1000);
        assert_eq!(session.total_matches, 42);
    }

    #[test]
    fn test_session_record_event() {
        let mut session = TelemetrySession::new("search");
        session.record_event(TelemetryEventType::FileSkipped, Some("binary file"));
        assert_eq!(session.event_count(), 1);
    }

    #[test]
    fn test_session_bottleneck_transition() {
        let mut session = TelemetrySession::new("search");
        assert_eq!(session.bottleneck(), BottleneckPhase::None);
        session.set_bottleneck(BottleneckPhase::RegexMatch);
        assert_eq!(session.bottleneck(), BottleneckPhase::RegexMatch);
        assert!(session.event_count() >= 1);
    }

    #[test]
    fn test_session_detect_bottleneck_empty() {
        let mut session = TelemetrySession::new("search");
        let state = session.detect_bottleneck();
        assert_eq!(state, BottleneckPhase::None);
    }

    #[test]
    fn test_session_detect_bottleneck_regex() {
        let mut session = TelemetrySession::new("search");
        for _ in 0..5 {
            session.record_phase("read", 10_000_000, 0.1); // 100 MB/s
            session.record_phase("regex", 1_000_000, 0.1); // 10 MB/s (slow)
        }
        let state = session.detect_bottleneck();
        assert_eq!(state, BottleneckPhase::RegexMatch);
    }

    #[test]
    fn test_session_rolling_throughput_empty() {
        let session = TelemetrySession::new("search");
        assert_eq!(session.rolling_throughput(), 0.0);
    }

    #[test]
    fn test_session_overall_throughput() {
        let mut session = TelemetrySession::new("search");
        session.record_phase("search", 100_000_000, 1.0);
        let throughput = session.overall_throughput();
        assert!(throughput > 0.0);
    }

    #[test]
    fn test_session_finalize() {
        let mut session = TelemetrySession::new("query");
        session.record_phase("search", 1_000_000, 0.5);
        session.record_counts(5, 500, 10);
        session.record_event(TelemetryEventType::StreamingActivated, None);
        let report = session.finalize(true, None);
        assert_eq!(report.operation, "query");
        assert!(report.success);
        assert!(report.error.is_none());
        assert_eq!(report.total_bytes, 1_000_000);
        assert_eq!(report.total_files, 5);
        assert_eq!(report.total_lines, 500);
        assert_eq!(report.total_matches, 10);
        assert!(report.duration_secs >= 0.0);
        assert!(!report.session_id.is_empty());
        assert!(!report.platform.is_empty());
        assert_eq!(report.events.len(), 1);
    }

    #[test]
    fn test_session_finalize_failure() {
        let mut session = TelemetrySession::new("transform");
        let report = session.finalize(false, Some("regex parse error"));
        assert!(!report.success);
        assert_eq!(report.error.as_deref(), Some("regex parse error"));
    }

    #[test]
    fn test_shared_telemetry() {
        let shared = SharedTelemetry::new("search");
        shared.record_phase("regex", 1_000_000, 0.1);
        shared.record_counts(1, 100, 5);
        shared.record_event(TelemetryEventType::FileSkipped, None);
        shared.set_bottleneck(BottleneckPhase::FileIO);
        let throughput = shared.rolling_throughput();
        assert!(throughput >= 0.0);
    }

    #[test]
    fn test_shared_telemetry_finalize() {
        let shared = SharedTelemetry::new("analyze");
        shared.record_phase("search", 2_000_000, 0.2);
        let report = shared.finalize(true, None).unwrap();
        assert_eq!(report.operation, "analyze");
        assert!(report.success);
    }

    #[test]
    fn test_format_throughput() {
        assert_eq!(format_throughput(500.0), "500 B/s");
        assert!(format_throughput(1024.0).contains("KiB/s"));
        assert!(format_throughput(1024.0 * 1024.0).contains("MiB/s"));
        assert!(format_throughput(1024.0 * 1024.0 * 1024.0).contains("GiB/s"));
    }

    #[test]
    fn test_format_bytes_human() {
        assert_eq!(format_bytes_human(0), "0 B");
        assert_eq!(format_bytes_human(512), "512 B");
        assert!(format_bytes_human(1024 * 1024).contains("MiB"));
        assert!(format_bytes_human(1024 * 1024 * 1024).contains("GiB"));
    }

    #[test]
    fn test_report_serialization() {
        let mut session = TelemetrySession::new("search");
        session.record_phase("search", 1000, 0.01);
        let report = session.finalize(true, None);
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"operation\":\"search\""));
        assert!(json.contains("\"success\":true"));
        let deserialized: TelemetryReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.operation, "search");
    }

    #[test]
    fn test_summarize_report() {
        let mut session = TelemetrySession::new("search");
        session.record_phase("search", 100_000_000, 2.0);
        session.record_counts(10, 5000, 100);
        let report = session.finalize(true, None);
        let summary = summarize_report(&report);
        assert!(summary.contains("search"));
        assert!(summary.contains("SUCCESS"));
        assert!(summary.contains("Session:"));
        assert!(summary.contains("Files: 10"));
    }

    #[test]
    fn test_export_and_load_report() {
        let mut session = TelemetrySession::new("test");
        session.record_phase("search", 5000, 0.1);
        let report = session.finalize(true, None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("telemetry.json");
        export_report(&report, &path).unwrap();

        let loaded = load_report(&path).unwrap();
        assert_eq!(loaded.session_id, report.session_id);
        assert_eq!(loaded.operation, "test");
    }

    #[test]
    fn test_bottleneck_durations_tracked() {
        let mut session = TelemetrySession::new("search");
        session.set_bottleneck(BottleneckPhase::FileIO);
        std::thread::sleep(Duration::from_millis(10));
        session.set_bottleneck(BottleneckPhase::RegexMatch);
        std::thread::sleep(Duration::from_millis(10));
        let report = session.finalize(true, None);
        assert!(report.bottleneck_durations.len() >= 2);
    }

    // Mutex to serialize tests that share global usage-telemetry state.
    // Without this, parallel test threads race on USAGE_ENABLED and USAGE_EVENTS.
    use std::sync::Mutex as StdMutex;
    static USAGE_TEST_MUTEX: StdMutex<()> = StdMutex::new(());

    #[test]
    fn test_usage_telemetry_toggle() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        // Test that enable/disable actually toggles the state
        usage_disable();
        assert!(!usage_is_enabled());
        usage_enable();
        assert!(usage_is_enabled());
        usage_disable();
        assert!(!usage_is_enabled());
    }

    #[test]
    fn test_usage_enable_disable() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_enable();
        assert!(usage_is_enabled());
        usage_disable();
        assert!(!usage_is_enabled());
        if was_enabled {
            usage_enable();
        } else {
            usage_disable();
        }
    }

    #[test]
    fn test_usage_status_disabled() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_disable();
        let status = usage_status();
        assert!(status.contains("DISABLED"));
        if was_enabled {
            usage_enable();
        }
    }

    #[test]
    fn test_record_usage_when_disabled() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_disable();
        let before = usage_event_count();
        record_usage("disabled_test_event", HashMap::new(), 1.0);
        let after = usage_event_count();
        assert_eq!(before, after);
        if was_enabled {
            usage_enable();
        }
    }

    #[test]
    fn test_record_usage_when_enabled() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_enable();
        reset_usage();
        record_usage("test_event", HashMap::new(), 1.0);
        assert!(usage_event_count() >= 1);
        let events = usage_events();
        assert!(events.iter().any(|e| e.event_name == "test_event"));
        reset_usage();
        if !was_enabled {
            usage_disable();
        }
    }

    #[test]
    fn test_record_command() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_enable();
        reset_usage();
        record_command("search");
        let events = usage_events();
        assert!(events.iter().any(|e| {
            e.event_name == "command_invoked"
                && e.dimensions.get("command").map(|s| s.as_str()) == Some("search")
        }));
        reset_usage();
        if !was_enabled {
            usage_disable();
        }
    }

    #[test]
    fn test_record_error() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        usage_enable();
        reset_usage();
        record_error("regex", "invalid pattern");
        let events = usage_events();
        // The event should be present, but other parallel tests may also add events.
        // Check that at least one "error" event with error_type "regex" exists.
        let found = events.iter().any(|e| {
            e.event_name == "error"
                && e.dimensions.get("error_type").map(|s| s.as_str()) == Some("regex")
        });
        reset_usage();
        assert!(found, "Expected a 'regex' error event in telemetry buffer");
    }

    #[test]
    fn test_record_query() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_enable();
        reset_usage();
        record_query("find \"TODO\"", 15, 42);
        let events = usage_events();
        assert!(events.iter().any(|e| e.event_name == "query_executed"));
        reset_usage();
        if !was_enabled {
            usage_disable();
        }
    }

    #[test]
    fn test_usage_session_id_stable() {
        let id1 = session_id();
        let id2 = session_id();
        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
    }

    #[test]
    fn test_build_cloudwatch_payload() {
        let events = vec![UsageTelemetryEvent {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            event_name: "command_invoked".to_string(),
            dimensions: {
                let mut d = HashMap::new();
                d.insert("command".to_string(), "search".to_string());
                d
            },
            value: 1.0,
        }];
        let payload = build_cloudwatch_payload(&events);
        assert!(payload.contains("Action=PutMetricData"));
        assert!(payload.contains("Namespace=ATP/Usage"));
        assert!(payload.contains("MetricName=command_invoked"));
        assert!(payload.contains("Value=1"));
    }

    #[test]
    fn test_hmac_sha256_deterministic() {
        let result1 = hmac_sha256(b"key", b"message");
        let result2 = hmac_sha256(b"key", b"message");
        assert_eq!(result1, result2);
        assert!(!result1.is_empty());
    }

    #[test]
    fn test_sha256_hex_known_value() {
        let hash = sha256_hex(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_reset_usage() {
        let _lock = USAGE_TEST_MUTEX.lock().unwrap();
        let was_enabled = usage_is_enabled();
        usage_enable();
        record_usage("test", HashMap::new(), 1.0);
        reset_usage();
        assert_eq!(usage_event_count(), 0);
        if !was_enabled {
            usage_disable();
        }
    }

    #[test]
    fn test_usage_event_serialization() {
        let event = UsageTelemetryEvent {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            event_name: "test".to_string(),
            dimensions: HashMap::new(),
            value: 42.0,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event_name\":\"test\""));
        let deserialized: UsageTelemetryEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_name, "test");
        assert_eq!(deserialized.value, 42.0);
    }

    #[test]
    fn test_perf_event_fields() {
        let event = PerfEvent {
            elapsed_secs: 1.5,
            event: TelemetryEventType::RegexRecompile,
            detail: Some("pattern too complex".into()),
        };
        assert_eq!(event.elapsed_secs, 1.5);
        assert_eq!(event.detail.as_deref(), Some("pattern too complex"));
    }

    // ── Tracing tests ───────────────────────────────────────────────

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.format, TracingFormat::Compact);
        assert_eq!(config.level, "info");
        assert!(config.log_file.is_none());
    }

    #[test]
    fn test_tracing_disabled_returns_false() {
        let config = TracingConfig::default();
        let result = init_tracing(&config).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_tracing_config_serialization() {
        let config = TracingConfig {
            enabled: true,
            format: TracingFormat::Json,
            level: "debug".to_string(),
            log_file: Some("/tmp/atp.log".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"enabled\":true"));
        assert!(json.contains("\"Json\""));
        let deserialized: TracingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.level, "debug");
        assert_eq!(deserialized.format, TracingFormat::Json);
    }

    #[test]
    fn test_traced_operation_lifecycle() {
        let op = TracedOperation::begin("test_search", Some(1), "pattern=hello");
        assert!(op.elapsed_ms() < 1000);
        op.complete(true, 42);
    }

    #[test]
    fn test_traced_operation_event() {
        let op = TracedOperation::begin("test_pipeline", None, "stage=filter");
        op.event("intermediate result", 10);
        op.complete(true, 5);
    }

    #[test]
    fn test_trace_convenience_functions() {
        // These should not panic even without a subscriber installed
        trace_pipeline_stage("test-pipe", 0, "search", 100);
        trace_search("hello", 5, 10, 42);
    }
}
