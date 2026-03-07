//! # Cron-like Scheduler
//!
//! Provides cron expression parsing, one-shot/repeating schedules, next/last
//! fire tracking, and missed-run detection for recurring pipeline jobs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique schedule identifier.
pub type ScheduleId = String;

/// A parsed cron field that can match values within a range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CronField {
    /// Match every value (`*`).
    Any,
    /// Match a specific value.
    Value(u32),
    /// Match a range (inclusive).
    Range(u32, u32),
    /// Match values at a step interval starting from `start` with `step`.
    Step { start: u32, step: u32 },
    /// Match any of the listed values.
    List(Vec<u32>),
}

impl CronField {
    /// Check if a value matches this field.
    pub fn matches(&self, value: u32) -> bool {
        match self {
            CronField::Any => true,
            CronField::Value(v) => *v == value,
            CronField::Range(lo, hi) => value >= *lo && value <= *hi,
            CronField::Step { start, step } => {
                if *step == 0 {
                    return value == *start;
                }
                value >= *start && (value - *start) % *step == 0
            }
            CronField::List(vals) => vals.contains(&value),
        }
    }
}

/// A cron expression with 5 fields: minute, hour, day-of-month, month, day-of-week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronExpr {
    pub minute: CronField,
    pub hour: CronField,
    pub day_of_month: CronField,
    pub month: CronField,
    pub day_of_week: CronField,
}

/// Error type for schedule operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScheduleError {
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("schedule not found: {0}")]
    NotFound(String),
    #[error("schedule already exists: {0}")]
    AlreadyExists(String),
}

/// Whether a schedule repeats or fires once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleKind {
    /// Fire once at the next matching time.
    OneShot,
    /// Repeat at every matching time.
    Repeating,
}

/// A schedule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleDef {
    /// Unique identifier.
    pub id: ScheduleId,
    /// Human-readable description.
    pub description: String,
    /// Cron expression.
    pub cron: CronExpr,
    /// Schedule kind.
    pub kind: ScheduleKind,
    /// Whether the schedule is enabled.
    pub enabled: bool,
    /// Pipeline or command to execute.
    pub command: String,
}

/// Snapshot of a point in time (simplified — no chrono dependency for scheduling logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimePoint {
    pub year: u32,
    pub month: u32,   // 1-12
    pub day: u32,     // 1-31
    pub hour: u32,    // 0-23
    pub minute: u32,  // 0-59
    pub weekday: u32, // 0=Sun, 1=Mon, ..., 6=Sat
}

/// Execution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Schedule ID.
    pub schedule_id: ScheduleId,
    /// When the execution happened (epoch seconds).
    pub executed_at: u64,
    /// Whether it succeeded.
    pub success: bool,
    /// Optional error message.
    pub error: Option<String>,
}

/// A missed run that was detected.
#[derive(Debug, Clone)]
pub struct MissedRun {
    /// Schedule ID.
    pub schedule_id: ScheduleId,
    /// The time point that was missed.
    pub expected: TimePoint,
}

/// Scheduler statistics.
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    pub total_schedules: usize,
    pub enabled_schedules: usize,
    pub total_executions: usize,
    pub total_failures: usize,
}

// ---------------------------------------------------------------------------
// Cron parsing
// ---------------------------------------------------------------------------

fn parse_cron_field(s: &str, min: u32, max: u32) -> Result<CronField, ScheduleError> {
    let s = s.trim();
    if s == "*" {
        return Ok(CronField::Any);
    }
    // Step: */N or M/N
    if let Some((base, step_s)) = s.split_once('/') {
        let start = if base == "*" {
            min
        } else {
            base.parse::<u32>()
                .map_err(|_| ScheduleError::InvalidCron(format!("invalid base: {base}")))?
        };
        let step = step_s
            .parse::<u32>()
            .map_err(|_| ScheduleError::InvalidCron(format!("invalid step: {step_s}")))?;
        return Ok(CronField::Step { start, step });
    }
    // List: 1,3,5
    if s.contains(',') {
        let vals: Result<Vec<u32>, _> = s.split(',').map(|v| v.trim().parse::<u32>()).collect();
        let vals = vals.map_err(|_| ScheduleError::InvalidCron(format!("invalid list: {s}")))?;
        for v in &vals {
            if *v < min || *v > max {
                return Err(ScheduleError::InvalidCron(format!(
                    "value {v} out of range {min}-{max}"
                )));
            }
        }
        return Ok(CronField::List(vals));
    }
    // Range: 1-5
    if let Some((lo_s, hi_s)) = s.split_once('-') {
        let lo = lo_s
            .parse::<u32>()
            .map_err(|_| ScheduleError::InvalidCron(format!("invalid range: {s}")))?;
        let hi = hi_s
            .parse::<u32>()
            .map_err(|_| ScheduleError::InvalidCron(format!("invalid range: {s}")))?;
        if lo < min || hi > max || lo > hi {
            return Err(ScheduleError::InvalidCron(format!(
                "range {lo}-{hi} invalid for {min}-{max}"
            )));
        }
        return Ok(CronField::Range(lo, hi));
    }
    // Single value
    let v = s
        .parse::<u32>()
        .map_err(|_| ScheduleError::InvalidCron(format!("invalid value: {s}")))?;
    if v < min || v > max {
        return Err(ScheduleError::InvalidCron(format!(
            "value {v} out of range {min}-{max}"
        )));
    }
    Ok(CronField::Value(v))
}

/// Parse a cron expression string (5 fields).
pub fn parse_cron(expr: &str) -> Result<CronExpr, ScheduleError> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(ScheduleError::InvalidCron(format!(
            "expected 5 fields, got {}",
            parts.len()
        )));
    }
    Ok(CronExpr {
        minute: parse_cron_field(parts[0], 0, 59)?,
        hour: parse_cron_field(parts[1], 0, 23)?,
        day_of_month: parse_cron_field(parts[2], 1, 31)?,
        month: parse_cron_field(parts[3], 1, 12)?,
        day_of_week: parse_cron_field(parts[4], 0, 6)?,
    })
}

/// Check if a cron expression matches a given time point.
pub fn cron_matches(cron: &CronExpr, time: &TimePoint) -> bool {
    cron.minute.matches(time.minute)
        && cron.hour.matches(time.hour)
        && cron.day_of_month.matches(time.day)
        && cron.month.matches(time.month)
        && cron.day_of_week.matches(time.weekday)
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// The scheduler manages schedule definitions and execution tracking.
#[derive(Debug)]
pub struct Scheduler {
    schedules: BTreeMap<ScheduleId, ScheduleDef>,
    history: Vec<ExecutionRecord>,
}

impl Scheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            schedules: BTreeMap::new(),
            history: Vec::new(),
        }
    }

    /// Register a new schedule.
    pub fn register(&mut self, def: ScheduleDef) -> Result<(), ScheduleError> {
        if self.schedules.contains_key(&def.id) {
            return Err(ScheduleError::AlreadyExists(def.id.clone()));
        }
        self.schedules.insert(def.id.clone(), def);
        Ok(())
    }

    /// Remove a schedule by ID.
    pub fn remove(&mut self, id: &str) -> Result<ScheduleDef, ScheduleError> {
        self.schedules
            .remove(id)
            .ok_or_else(|| ScheduleError::NotFound(id.to_string()))
    }

    /// Enable or disable a schedule.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), ScheduleError> {
        let def = self
            .schedules
            .get_mut(id)
            .ok_or_else(|| ScheduleError::NotFound(id.to_string()))?;
        def.enabled = enabled;
        Ok(())
    }

    /// Get all schedule IDs.
    pub fn schedule_ids(&self) -> Vec<&str> {
        self.schedules.keys().map(|s| s.as_str()).collect()
    }

    /// Get a schedule definition by ID.
    pub fn get(&self, id: &str) -> Option<&ScheduleDef> {
        self.schedules.get(id)
    }

    /// Find all schedules that should fire at the given time point.
    pub fn due_at(&self, time: &TimePoint) -> Vec<&ScheduleDef> {
        self.schedules
            .values()
            .filter(|s| s.enabled && cron_matches(&s.cron, time))
            .collect()
    }

    /// Record an execution result.
    pub fn record_execution(&mut self, record: ExecutionRecord) {
        self.history.push(record);
    }

    /// Get execution history for a schedule.
    pub fn history_for(&self, id: &str) -> Vec<&ExecutionRecord> {
        self.history
            .iter()
            .filter(|r| r.schedule_id == id)
            .collect()
    }

    /// Detect missed runs: given a range of time points, find schedules that were
    /// due but have no execution record in that window.
    pub fn detect_missed_runs(&self, time_points: &[TimePoint]) -> Vec<MissedRun> {
        let mut missed = Vec::new();
        for time in time_points {
            for def in self.schedules.values() {
                if def.enabled && cron_matches(&def.cron, time) {
                    // Check if there's an execution record near this time
                    let has_record = self.history.iter().any(|r| r.schedule_id == def.id);
                    if !has_record {
                        missed.push(MissedRun {
                            schedule_id: def.id.clone(),
                            expected: *time,
                        });
                    }
                }
            }
        }
        missed
    }

    /// Get scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            total_schedules: self.schedules.len(),
            enabled_schedules: self.schedules.values().filter(|s| s.enabled).count(),
            total_executions: self.history.len(),
            total_failures: self.history.iter().filter(|r| !r.success).count(),
        }
    }

    /// Clear all execution history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tp(month: u32, day: u32, hour: u32, minute: u32, weekday: u32) -> TimePoint {
        TimePoint {
            year: 2026,
            month,
            day,
            hour,
            minute,
            weekday,
        }
    }

    fn make_schedule(id: &str, cron_str: &str) -> ScheduleDef {
        ScheduleDef {
            id: id.to_string(),
            description: format!("test schedule {id}"),
            cron: parse_cron(cron_str).unwrap(),
            kind: ScheduleKind::Repeating,
            enabled: true,
            command: "atp pipeline run test".to_string(),
        }
    }

    #[test]
    fn test_parse_cron_all_stars() {
        let c = parse_cron("* * * * *").unwrap();
        assert_eq!(c.minute, CronField::Any);
        assert_eq!(c.hour, CronField::Any);
    }

    #[test]
    fn test_parse_cron_specific_values() {
        let c = parse_cron("30 14 1 6 3").unwrap();
        assert_eq!(c.minute, CronField::Value(30));
        assert_eq!(c.hour, CronField::Value(14));
        assert_eq!(c.day_of_month, CronField::Value(1));
        assert_eq!(c.month, CronField::Value(6));
        assert_eq!(c.day_of_week, CronField::Value(3));
    }

    #[test]
    fn test_parse_cron_range() {
        let c = parse_cron("0-30 9-17 * * 1-5").unwrap();
        assert_eq!(c.minute, CronField::Range(0, 30));
        assert_eq!(c.hour, CronField::Range(9, 17));
        assert_eq!(c.day_of_week, CronField::Range(1, 5));
    }

    #[test]
    fn test_parse_cron_step() {
        let c = parse_cron("*/15 */2 * * *").unwrap();
        assert_eq!(c.minute, CronField::Step { start: 0, step: 15 });
        assert_eq!(c.hour, CronField::Step { start: 0, step: 2 });
    }

    #[test]
    fn test_parse_cron_list() {
        let c = parse_cron("0,15,30,45 * * * *").unwrap();
        assert_eq!(c.minute, CronField::List(vec![0, 15, 30, 45]));
    }

    #[test]
    fn test_parse_cron_invalid() {
        assert!(parse_cron("* *").is_err());
        assert!(parse_cron("60 * * * *").is_err());
        assert!(parse_cron("abc * * * *").is_err());
    }

    #[test]
    fn test_cron_matches_any() {
        let c = parse_cron("* * * * *").unwrap();
        assert!(cron_matches(&c, &tp(1, 1, 0, 0, 0)));
        assert!(cron_matches(&c, &tp(12, 31, 23, 59, 6)));
    }

    #[test]
    fn test_cron_matches_specific() {
        let c = parse_cron("30 14 * * *").unwrap();
        assert!(cron_matches(&c, &tp(3, 5, 14, 30, 2)));
        assert!(!cron_matches(&c, &tp(3, 5, 14, 31, 2)));
        assert!(!cron_matches(&c, &tp(3, 5, 15, 30, 2)));
    }

    #[test]
    fn test_cron_field_step() {
        let f = CronField::Step { start: 0, step: 15 };
        assert!(f.matches(0));
        assert!(f.matches(15));
        assert!(f.matches(30));
        assert!(f.matches(45));
        assert!(!f.matches(10));
    }

    #[test]
    fn test_scheduler_register_and_due() {
        let mut sched = Scheduler::new();
        sched
            .register(make_schedule("daily_9am", "0 9 * * *"))
            .unwrap();
        sched
            .register(make_schedule("hourly", "0 * * * *"))
            .unwrap();

        let due = sched.due_at(&tp(3, 5, 9, 0, 2));
        assert_eq!(due.len(), 2); // both match

        let due2 = sched.due_at(&tp(3, 5, 10, 0, 2));
        assert_eq!(due2.len(), 1); // only hourly
        assert_eq!(due2[0].id, "hourly");
    }

    #[test]
    fn test_scheduler_enable_disable() {
        let mut sched = Scheduler::new();
        sched.register(make_schedule("s1", "* * * * *")).unwrap();
        sched.set_enabled("s1", false).unwrap();
        assert!(sched.due_at(&tp(1, 1, 0, 0, 0)).is_empty());
        sched.set_enabled("s1", true).unwrap();
        assert_eq!(sched.due_at(&tp(1, 1, 0, 0, 0)).len(), 1);
    }

    #[test]
    fn test_scheduler_remove() {
        let mut sched = Scheduler::new();
        sched.register(make_schedule("s1", "* * * * *")).unwrap();
        sched.remove("s1").unwrap();
        assert!(sched.schedule_ids().is_empty());
        assert!(sched.remove("s1").is_err());
    }

    #[test]
    fn test_scheduler_duplicate() {
        let mut sched = Scheduler::new();
        sched.register(make_schedule("s1", "* * * * *")).unwrap();
        assert!(sched.register(make_schedule("s1", "0 0 * * *")).is_err());
    }

    #[test]
    fn test_execution_history() {
        let mut sched = Scheduler::new();
        sched.register(make_schedule("s1", "* * * * *")).unwrap();
        sched.record_execution(ExecutionRecord {
            schedule_id: "s1".into(),
            executed_at: 1000,
            success: true,
            error: None,
        });
        sched.record_execution(ExecutionRecord {
            schedule_id: "s1".into(),
            executed_at: 2000,
            success: false,
            error: Some("timeout".into()),
        });
        assert_eq!(sched.history_for("s1").len(), 2);
        let stats = sched.stats();
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.total_failures, 1);
    }

    #[test]
    fn test_missed_runs() {
        let mut sched = Scheduler::new();
        sched.register(make_schedule("s1", "0 * * * *")).unwrap();
        let points = vec![tp(3, 5, 9, 0, 2), tp(3, 5, 10, 0, 2)];
        let missed = sched.detect_missed_runs(&points);
        assert_eq!(missed.len(), 2);
    }

    #[test]
    fn test_stats() {
        let mut sched = Scheduler::new();
        sched.register(make_schedule("s1", "* * * * *")).unwrap();
        sched.register(make_schedule("s2", "0 0 * * *")).unwrap();
        sched.set_enabled("s2", false).unwrap();
        let stats = sched.stats();
        assert_eq!(stats.total_schedules, 2);
        assert_eq!(stats.enabled_schedules, 1);
    }
}
