//! Debug Adapter Protocol (DAP) for AQL step-through execution.
//!
//! Enables interactive debugging of AQL pipelines with breakpoints on
//! pipeline stages, inspection of intermediate results, and step-by-step
//! evaluation.
//!
//! # Architecture
//!
//! - [`Breakpoint`]: A breakpoint on a specific AQL pipeline stage
//! - [`DebugState`]: Current state of the debug session (paused, running, etc.)
//! - [`StageSnapshot`]: Captured state at a breakpoint (input, output, variables)
//! - [`AqlDebugger`]: The main debugger engine
//! - [`DapMessage`] / [`DapRequest`] / [`DapResponse`] / [`DapEvent`]: DAP wire protocol types
//!
//! # Wire Protocol (v2)
//!
//! The DAP wire protocol uses JSON messages with `Content-Length` headers
//! (identical to LSP). Messages are one of:
//! - **Request** — client → adapter (e.g. `initialize`, `setBreakpoints`, `next`)
//! - **Response** — adapter → client (success/error reply)
//! - **Event** — adapter → client (e.g. `stopped`, `output`, `terminated`)

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A breakpoint on a specific AQL pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    /// Stage index (0-based).
    pub stage: usize,
    /// Optional condition (AQL expression that must be true to break).
    pub condition: Option<String>,
    /// Whether this breakpoint is enabled.
    pub enabled: bool,
    /// Hit count (how many times this breakpoint has been hit).
    pub hit_count: u64,
}

impl Breakpoint {
    /// Create a new unconditional breakpoint at a stage.
    pub fn at_stage(stage: usize) -> Self {
        Self {
            stage,
            condition: None,
            enabled: true,
            hit_count: 0,
        }
    }

    /// Create a conditional breakpoint.
    pub fn conditional(stage: usize, condition: &str) -> Self {
        Self {
            stage,
            condition: Some(condition.to_string()),
            enabled: true,
            hit_count: 0,
        }
    }
}

/// State of the debugger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugState {
    /// Not started.
    Idle,
    /// Running (no breakpoint hit).
    Running,
    /// Paused at a breakpoint.
    Paused,
    /// Execution completed.
    Completed,
    /// Error occurred.
    Error,
}

/// Captured state at a pipeline stage (breakpoint snapshot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageSnapshot {
    /// Stage index (0-based).
    pub stage_index: usize,
    /// Stage description (AQL text).
    pub stage_text: String,
    /// Number of input lines/items to this stage.
    pub input_count: usize,
    /// Number of output lines/items from this stage.
    pub output_count: usize,
    /// Sample of input lines (first N).
    pub input_sample: Vec<String>,
    /// Sample of output lines (first N).
    pub output_sample: Vec<String>,
    /// Duration of this stage in milliseconds.
    pub duration_ms: u64,
    /// Variables in scope at this point.
    pub variables: HashMap<String, String>,
}

/// A watch expression — evaluated at each breakpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchExpression {
    /// The expression text.
    pub expression: String,
    /// Current value (updated at each break).
    pub value: Option<String>,
}

/// AQL debug session — tracks state through a pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSession {
    /// The AQL query being debugged.
    pub query: String,
    /// Current state.
    pub state: DebugState,
    /// Current stage index.
    pub current_stage: usize,
    /// Total number of stages.
    pub total_stages: usize,
    /// Breakpoints.
    pub breakpoints: Vec<Breakpoint>,
    /// Snapshots captured at each completed stage.
    pub snapshots: Vec<StageSnapshot>,
    /// Watch expressions.
    pub watches: Vec<WatchExpression>,
    /// Stage descriptions parsed from the query.
    pub stage_descriptions: Vec<String>,
}

impl DebugSession {
    /// Get the snapshot at the current stage (if paused).
    pub fn current_snapshot(&self) -> Option<&StageSnapshot> {
        if self.state == DebugState::Paused && self.current_stage > 0 {
            self.snapshots.get(self.current_stage - 1)
        } else {
            None
        }
    }

    /// Get all snapshots up to the current point.
    pub fn history(&self) -> &[StageSnapshot] {
        &self.snapshots
    }

    /// Check if a breakpoint is set at a given stage.
    pub fn has_breakpoint(&self, stage: usize) -> bool {
        self.breakpoints
            .iter()
            .any(|b| b.stage == stage && b.enabled)
    }
}

/// AQL debugger — manages debug sessions for AQL pipeline execution.
pub struct AqlDebugger {
    /// Default breakpoints to apply to new sessions.
    default_breakpoints: Vec<usize>,
    /// Maximum sample size for input/output snapshots.
    sample_size: usize,
}

impl Default for AqlDebugger {
    fn default() -> Self {
        Self::new()
    }
}

impl AqlDebugger {
    /// Create a new debugger.
    pub fn new() -> Self {
        Self {
            default_breakpoints: Vec::new(),
            sample_size: 20,
        }
    }

    /// Set a default breakpoint stage for all new sessions.
    pub fn add_default_breakpoint(&mut self, stage: usize) {
        self.default_breakpoints.push(stage);
    }

    /// Set the sample size for snapshots.
    pub fn set_sample_size(&mut self, size: usize) {
        self.sample_size = size;
    }

    /// Start a debug session for an AQL query.
    ///
    /// Parses the query into stages and sets up the session.
    /// Does not execute anything until `step()` or `run()` is called.
    pub fn start(&self, query: &str, files: &[&Path]) -> Result<DebugSession> {
        let pipeline = crate::engine::aql::parse(query)?;
        let stage_descriptions: Vec<String> =
            pipeline.stages.iter().map(|s| format!("{:?}", s)).collect();
        let total_stages = stage_descriptions.len();

        if total_stages == 0 {
            bail!("Empty AQL pipeline — nothing to debug");
        }

        let breakpoints: Vec<Breakpoint> = self
            .default_breakpoints
            .iter()
            .filter(|&&s| s < total_stages)
            .map(|&s| Breakpoint::at_stage(s))
            .collect();

        let _ = files; // files used during actual execution

        Ok(DebugSession {
            query: query.to_string(),
            state: DebugState::Idle,
            current_stage: 0,
            total_stages,
            breakpoints,
            snapshots: Vec::new(),
            watches: Vec::new(),
            stage_descriptions,
        })
    }

    /// Execute one stage and capture a snapshot.
    ///
    /// This runs a single pipeline stage and records the intermediate
    /// results, then pauses.
    pub fn step(
        &self,
        session: &mut DebugSession,
        input_lines: &[String],
    ) -> Result<StageSnapshot> {
        if session.current_stage >= session.total_stages {
            session.state = DebugState::Completed;
            bail!("Pipeline execution already completed");
        }

        session.state = DebugState::Running;

        let stage_text = session.stage_descriptions[session.current_stage].clone();
        let start = std::time::Instant::now();

        // The output is a simplified simulation — in a real implementation this
        // would actually run the AQL stage through the engine. For now, we pass
        // through and record.
        let output_lines: Vec<String> = input_lines.to_vec();

        let duration_ms = start.elapsed().as_millis() as u64;

        let snapshot = StageSnapshot {
            stage_index: session.current_stage,
            stage_text,
            input_count: input_lines.len(),
            output_count: output_lines.len(),
            input_sample: input_lines.iter().take(self.sample_size).cloned().collect(),
            output_sample: output_lines
                .iter()
                .take(self.sample_size)
                .cloned()
                .collect(),
            duration_ms,
            variables: HashMap::new(),
        };

        session.snapshots.push(snapshot.clone());
        session.current_stage += 1;

        // Check if we hit a breakpoint
        if session.has_breakpoint(session.current_stage) {
            session.state = DebugState::Paused;
            // Increment hit count
            for bp in &mut session.breakpoints {
                if bp.stage == session.current_stage && bp.enabled {
                    bp.hit_count += 1;
                }
            }
        } else if session.current_stage >= session.total_stages {
            session.state = DebugState::Completed;
        } else {
            session.state = DebugState::Paused;
        }

        Ok(snapshot)
    }

    /// Run until the next breakpoint or completion.
    pub fn run(&self, session: &mut DebugSession, input_lines: &[String]) -> Result<DebugState> {
        let mut current_input = input_lines.to_vec();

        while session.current_stage < session.total_stages {
            let snapshot = self.step(session, &current_input)?;
            current_input = snapshot.output_sample;

            if session.state == DebugState::Paused && session.has_breakpoint(session.current_stage)
            {
                return Ok(DebugState::Paused);
            }
        }

        session.state = DebugState::Completed;
        Ok(DebugState::Completed)
    }

    /// Add a breakpoint to a session.
    pub fn set_breakpoint(session: &mut DebugSession, stage: usize) {
        if stage < session.total_stages && !session.breakpoints.iter().any(|b| b.stage == stage) {
            session.breakpoints.push(Breakpoint::at_stage(stage));
        }
    }

    /// Remove a breakpoint from a session.
    pub fn clear_breakpoint(session: &mut DebugSession, stage: usize) {
        session.breakpoints.retain(|b| b.stage != stage);
    }

    /// Add a watch expression.
    pub fn add_watch(session: &mut DebugSession, expression: &str) {
        session.watches.push(WatchExpression {
            expression: expression.to_string(),
            value: None,
        });
    }

    /// Get a summary of the debug session for display.
    pub fn session_summary(session: &DebugSession) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("Query: {}\n", session.query));
        summary.push_str(&format!("State: {:?}\n", session.state));
        summary.push_str(&format!(
            "Progress: {}/{} stages\n",
            session.current_stage, session.total_stages
        ));
        summary.push_str(&format!(
            "Breakpoints: {}\n",
            session.breakpoints.iter().filter(|b| b.enabled).count()
        ));
        summary.push_str(&format!("Snapshots: {}\n", session.snapshots.len()));

        if let Some(snap) = session.current_snapshot() {
            summary.push_str(&format!(
                "\nCurrent stage: {} ({})\n",
                snap.stage_index, snap.stage_text
            ));
            summary.push_str(&format!(
                "  Input: {} items → Output: {} items ({} ms)\n",
                snap.input_count, snap.output_count, snap.duration_ms
            ));
        }

        summary
    }
}

// ---------------------------------------------------------------------------
// DAP Wire Protocol (v2)
// ---------------------------------------------------------------------------

/// A DAP wire protocol message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DapMessage {
    #[serde(rename = "request")]
    Request(DapRequest),
    #[serde(rename = "response")]
    Response(DapResponse),
    #[serde(rename = "event")]
    Event(DapEvent),
}

/// DAP request (client → adapter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapRequest {
    /// Sequence number — monotonically increasing.
    pub seq: u64,
    /// The command to execute (e.g. "initialize", "setBreakpoints", "next").
    pub command: String,
    /// Optional arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// DAP response (adapter → client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapResponse {
    /// Sequence number of this response.
    pub seq: u64,
    /// Sequence number of the request this is a response to.
    pub request_seq: u64,
    /// Whether the request was successful.
    pub success: bool,
    /// The command that was executed.
    pub command: String,
    /// Optional result body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// Error message if `success` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// DAP event (adapter → client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapEvent {
    /// Sequence number.
    pub seq: u64,
    /// Event type (e.g. "stopped", "output", "terminated").
    pub event: String,
    /// Optional event body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

impl DapMessage {
    /// Encode the message as a DAP wire-format string with `Content-Length` header.
    pub fn encode(&self) -> Result<String> {
        let json = serde_json::to_string(self)?;
        Ok(format!("Content-Length: {}\r\n\r\n{}", json.len(), json))
    }

    /// Decode a DAP wire-format message from a `Content-Length`-prefixed string.
    ///
    /// Returns `(message, bytes_consumed)`.
    pub fn decode(input: &str) -> Result<(Self, usize)> {
        let header_end = input
            .find("\r\n\r\n")
            .ok_or_else(|| anyhow::anyhow!("Missing header delimiter"))?;
        let header = &input[..header_end];
        let content_length: usize = header
            .strip_prefix("Content-Length: ")
            .ok_or_else(|| anyhow::anyhow!("Missing Content-Length header"))?
            .trim()
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid Content-Length: {e}"))?;

        let body_start = header_end + 4; // skip \r\n\r\n
        let body_end = body_start + content_length;
        if input.len() < body_end {
            bail!("Incomplete message body");
        }

        let body = &input[body_start..body_end];
        let msg: DapMessage = serde_json::from_str(body)?;
        Ok((msg, body_end))
    }
}

impl DapRequest {
    /// Create a new request.
    pub fn new(seq: u64, command: &str, arguments: Option<serde_json::Value>) -> Self {
        Self {
            seq,
            command: command.into(),
            arguments,
        }
    }
}

impl DapResponse {
    /// Create a success response.
    pub fn success(
        seq: u64,
        request_seq: u64,
        command: &str,
        body: Option<serde_json::Value>,
    ) -> Self {
        Self {
            seq,
            request_seq,
            success: true,
            command: command.into(),
            body,
            message: None,
        }
    }

    /// Create an error response.
    pub fn error(seq: u64, request_seq: u64, command: &str, message: &str) -> Self {
        Self {
            seq,
            request_seq,
            success: false,
            command: command.into(),
            body: None,
            message: Some(message.into()),
        }
    }
}

impl DapEvent {
    /// Create an event.
    pub fn new(seq: u64, event: &str, body: Option<serde_json::Value>) -> Self {
        Self {
            seq,
            event: event.into(),
            body,
        }
    }

    /// Create a "stopped" event (debugger hit breakpoint / step).
    pub fn stopped(seq: u64, reason: &str, stage: usize) -> Self {
        Self::new(
            seq,
            "stopped",
            Some(serde_json::json!({
                "reason": reason,
                "threadId": 1,
                "allThreadsStopped": true,
                "description": format!("Stopped at stage {stage}"),
            })),
        )
    }

    /// Create an "output" event (pipeline stage output).
    pub fn output(seq: u64, category: &str, output: &str) -> Self {
        Self::new(
            seq,
            "output",
            Some(serde_json::json!({
                "category": category,
                "output": output,
            })),
        )
    }

    /// Create a "terminated" event.
    pub fn terminated(seq: u64) -> Self {
        Self::new(seq, "terminated", None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_at_stage() {
        let bp = Breakpoint::at_stage(3);
        assert_eq!(bp.stage, 3);
        assert!(bp.enabled);
        assert!(bp.condition.is_none());
        assert_eq!(bp.hit_count, 0);
    }

    #[test]
    fn test_breakpoint_conditional() {
        let bp = Breakpoint::conditional(1, "count > 10");
        assert_eq!(bp.stage, 1);
        assert!(bp.condition.is_some());
        assert_eq!(bp.condition.unwrap(), "count > 10");
    }

    #[test]
    fn test_debug_state() {
        assert_ne!(DebugState::Idle, DebugState::Running);
        assert_ne!(DebugState::Paused, DebugState::Completed);
    }

    #[test]
    fn test_debugger_start_session() {
        let debugger = AqlDebugger::new();
        let session = debugger.start("find \"TODO\" | sort | count", &[]).unwrap();
        assert_eq!(session.state, DebugState::Idle);
        assert_eq!(session.total_stages, 3);
        assert_eq!(session.current_stage, 0);
        assert!(session.snapshots.is_empty());
    }

    #[test]
    fn test_debugger_step() {
        let debugger = AqlDebugger::new();
        let mut session = debugger.start("find \"TODO\" | sort | count", &[]).unwrap();
        let input = vec!["line one".to_string(), "TODO: fix".to_string()];
        let snapshot = debugger.step(&mut session, &input).unwrap();
        assert_eq!(snapshot.stage_index, 0);
        assert_eq!(snapshot.input_count, 2);
        assert_eq!(session.current_stage, 1);
        assert_eq!(session.snapshots.len(), 1);
    }

    #[test]
    fn test_debugger_breakpoint() {
        let debugger = AqlDebugger::new();
        let mut session = debugger.start("find \"TODO\" | sort | count", &[]).unwrap();
        AqlDebugger::set_breakpoint(&mut session, 2);
        assert!(session.has_breakpoint(2));

        AqlDebugger::clear_breakpoint(&mut session, 2);
        assert!(!session.has_breakpoint(2));
    }

    #[test]
    fn test_debugger_run_to_completion() {
        let debugger = AqlDebugger::new();
        let mut session = debugger.start("find \"TODO\" | sort | count", &[]).unwrap();
        let input = vec!["TODO: item".to_string()];
        let state = debugger.run(&mut session, &input).unwrap();
        assert_eq!(state, DebugState::Completed);
        assert_eq!(session.snapshots.len(), 3);
    }

    #[test]
    fn test_debugger_run_with_breakpoint() {
        let mut debugger = AqlDebugger::new();
        debugger.add_default_breakpoint(1);
        let mut session = debugger.start("find \"TODO\" | sort | count", &[]).unwrap();
        assert!(session.has_breakpoint(1));

        let input = vec!["TODO: test".to_string()];
        let state = debugger.run(&mut session, &input).unwrap();
        assert_eq!(state, DebugState::Paused);
        assert_eq!(session.current_stage, 1);
    }

    #[test]
    fn test_debugger_watch() {
        let debugger = AqlDebugger::new();
        let mut session = debugger.start("find \"TODO\" | count", &[]).unwrap();
        AqlDebugger::add_watch(&mut session, "line_count");
        assert_eq!(session.watches.len(), 1);
        assert_eq!(session.watches[0].expression, "line_count");
    }

    #[test]
    fn test_session_summary() {
        let debugger = AqlDebugger::new();
        let session = debugger.start("find \"TODO\" | count", &[]).unwrap();
        let summary = AqlDebugger::session_summary(&session);
        assert!(summary.contains("find \"TODO\""));
        assert!(summary.contains("Idle"));
        assert!(summary.contains("0/2 stages"));
    }

    #[test]
    fn test_stage_snapshot_fields() {
        let snap = StageSnapshot {
            stage_index: 0,
            stage_text: "find \"TODO\"".to_string(),
            input_count: 10,
            output_count: 3,
            input_sample: vec!["a".into(), "b".into()],
            output_sample: vec!["TODO: x".into()],
            duration_ms: 5,
            variables: HashMap::new(),
        };
        assert_eq!(snap.stage_index, 0);
        assert_eq!(snap.input_count, 10);
        assert_eq!(snap.output_count, 3);
    }

    #[test]
    fn test_debug_session_history() {
        let debugger = AqlDebugger::new();
        let mut session = debugger.start("find \"TODO\" | sort", &[]).unwrap();
        let input = vec!["line".to_string()];
        debugger.step(&mut session, &input).unwrap();
        debugger.step(&mut session, &input).unwrap();
        assert_eq!(session.history().len(), 2);
    }

    // ── DAP wire protocol tests ──────────────────────────────

    #[test]
    fn test_dap_request_encode_decode() {
        let req = DapRequest::new(
            1,
            "initialize",
            Some(serde_json::json!({"clientID": "atp"})),
        );
        let msg = DapMessage::Request(req);
        let wire = msg.encode().unwrap();
        assert!(wire.starts_with("Content-Length: "));
        let (decoded, _consumed) = DapMessage::decode(&wire).unwrap();
        match decoded {
            DapMessage::Request(r) => {
                assert_eq!(r.seq, 1);
                assert_eq!(r.command, "initialize");
            }
            _ => panic!("Expected Request"),
        }
    }

    #[test]
    fn test_dap_response_success() {
        let resp = DapResponse::success(
            2,
            1,
            "initialize",
            Some(serde_json::json!({"supportsSteppingGranularity": true})),
        );
        let msg = DapMessage::Response(resp);
        let wire = msg.encode().unwrap();
        let (decoded, _) = DapMessage::decode(&wire).unwrap();
        match decoded {
            DapMessage::Response(r) => {
                assert!(r.success);
                assert_eq!(r.request_seq, 1);
                assert_eq!(r.command, "initialize");
            }
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn test_dap_response_error() {
        let resp = DapResponse::error(3, 2, "setBreakpoints", "invalid stage");
        assert!(!resp.success);
        assert_eq!(resp.message.as_deref(), Some("invalid stage"));
    }

    #[test]
    fn test_dap_event_stopped() {
        let evt = DapEvent::stopped(4, "breakpoint", 2);
        let msg = DapMessage::Event(evt);
        let wire = msg.encode().unwrap();
        let (decoded, _) = DapMessage::decode(&wire).unwrap();
        match decoded {
            DapMessage::Event(e) => {
                assert_eq!(e.event, "stopped");
                let body = e.body.unwrap();
                assert_eq!(body["reason"], "breakpoint");
            }
            _ => panic!("Expected Event"),
        }
    }

    #[test]
    fn test_dap_event_output() {
        let evt = DapEvent::output(5, "stdout", "3 matches found\n");
        assert_eq!(evt.event, "output");
        let body = evt.body.as_ref().unwrap();
        assert_eq!(body["category"], "stdout");
    }

    #[test]
    fn test_dap_event_terminated() {
        let evt = DapEvent::terminated(6);
        assert_eq!(evt.event, "terminated");
        assert!(evt.body.is_none());
    }

    #[test]
    fn test_dap_decode_incomplete() {
        let result = DapMessage::decode("Content-Length: 999\r\n\r\n{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_dap_decode_missing_header() {
        let result = DapMessage::decode("no header here");
        assert!(result.is_err());
    }
}
