//! Priority task queue with dependency DAG.
//!
//! Schedules pipeline tasks based on priority and dependency ordering.
//! Supports topological scheduling, configurable concurrency limits,
//! retry with exponential backoff, and cancellation tokens.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique identifier for a task in the queue.
pub type TaskId = String;

/// Task priority — lower numerical value = higher priority.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
    Background = 4,
}

/// Current state of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// Waiting for dependencies to complete.
    Pending,
    /// All dependencies met, eligible for execution.
    Ready,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Failed after all retries exhausted.
    Failed,
    /// Cancelled by the user or a downstream failure.
    Cancelled,
}

/// Retry policy for failed tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 = no retries).
    pub max_retries: u32,
    /// Initial backoff duration.
    pub initial_backoff: Duration,
    /// Backoff multiplier (e.g. 2.0 for exponential).
    pub multiplier: f64,
    /// Maximum backoff duration cap.
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            multiplier: 2.0,
            max_backoff: Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// Compute the backoff delay for the given attempt (0-based).
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return self.initial_backoff;
        }
        let factor = self.multiplier.powi(attempt as i32);
        let nanos = (self.initial_backoff.as_secs_f64() * factor * 1e9) as u64;
        let d = Duration::from_nanos(nanos);
        if d > self.max_backoff {
            self.max_backoff
        } else {
            d
        }
    }
}

/// A single task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDef {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable label.
    pub label: String,
    /// Priority level.
    pub priority: Priority,
    /// IDs of tasks that must complete before this one.
    pub depends_on: Vec<TaskId>,
    /// Retry policy.
    pub retry_policy: RetryPolicy,
    /// Arbitrary payload (e.g. serialized pipeline stage).
    pub payload: String,
}

/// Runtime state for one task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub id: TaskId,
    pub state: TaskState,
    pub attempts: u32,
    pub last_error: Option<String>,
    #[serde(skip)]
    pub started_at: Option<Instant>,
    pub elapsed_ms: u64,
}

/// Queue-level statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    pub total: usize,
    pub pending: usize,
    pub ready: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

/// Configuration for the task queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum number of tasks executing concurrently.
    pub max_concurrency: usize,
    /// Whether to cancel dependents when a task fails.
    pub cancel_dependents_on_failure: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 4,
            cancel_dependents_on_failure: true,
        }
    }
}

/// A cancel handle that can be shared across threads.
#[derive(Debug, Clone)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl Default for CancelHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Dependency graph helpers
// ---------------------------------------------------------------------------

/// Errors specific to the task queue.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TaskQueueError {
    #[error("task not found: {0}")]
    NotFound(TaskId),
    #[error("duplicate task id: {0}")]
    Duplicate(TaskId),
    #[error("dependency cycle detected involving: {0:?}")]
    CycleDetected(Vec<TaskId>),
    #[error("unknown dependency {dep} for task {task}")]
    UnknownDep { task: TaskId, dep: TaskId },
    #[error("queue cancelled")]
    Cancelled,
}

/// Perform a topological sort on task definitions. Returns ordered task IDs
/// or an error if a cycle is detected.
pub fn topological_sort(tasks: &[TaskDef]) -> Result<Vec<TaskId>, TaskQueueError> {
    let ids: BTreeSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
    // Validate all deps exist
    for t in tasks {
        for dep in &t.depends_on {
            if !ids.contains(dep.as_str()) {
                return Err(TaskQueueError::UnknownDep {
                    task: t.id.clone(),
                    dep: dep.clone(),
                });
            }
        }
    }

    let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in tasks {
        in_degree.entry(t.id.as_str()).or_insert(0);
        for dep in &t.depends_on {
            *in_degree.entry(t.id.as_str()).or_insert(0) += 1;
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(t.id.as_str());
        }
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut order = Vec::with_capacity(tasks.len());

    while let Some(id) = queue.pop_front() {
        order.push(id.to_string());
        if let Some(deps) = dependents.get(id) {
            for &d in deps {
                if let Some(deg) = in_degree.get_mut(d) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(d);
                    }
                }
            }
        }
    }

    if order.len() != tasks.len() {
        // Remaining nodes form a cycle
        let cycle: Vec<TaskId> = in_degree
            .iter()
            .filter(|(_, &d)| d > 0)
            .map(|(&id, _)| id.to_string())
            .collect();
        return Err(TaskQueueError::CycleDetected(cycle));
    }

    Ok(order)
}

// ---------------------------------------------------------------------------
// TaskQueue
// ---------------------------------------------------------------------------

/// The priority task queue with dependency resolution.
#[derive(Debug)]
pub struct TaskQueue {
    config: QueueConfig,
    tasks: BTreeMap<TaskId, TaskDef>,
    statuses: BTreeMap<TaskId, TaskStatus>,
    order: Vec<TaskId>,
    cancel: CancelHandle,
}

impl TaskQueue {
    /// Create a new queue from task definitions.
    pub fn new(tasks: Vec<TaskDef>, config: QueueConfig) -> Result<Self, TaskQueueError> {
        // Check duplicates
        let mut seen = BTreeSet::new();
        for t in &tasks {
            if !seen.insert(t.id.clone()) {
                return Err(TaskQueueError::Duplicate(t.id.clone()));
            }
        }

        let order = topological_sort(&tasks)?;

        let mut task_map = BTreeMap::new();
        let mut statuses = BTreeMap::new();
        for t in tasks {
            statuses.insert(
                t.id.clone(),
                TaskStatus {
                    id: t.id.clone(),
                    state: TaskState::Pending,
                    attempts: 0,
                    last_error: None,
                    started_at: None,
                    elapsed_ms: 0,
                },
            );
            task_map.insert(t.id.clone(), t);
        }

        Ok(Self {
            config,
            tasks: task_map,
            statuses,
            order,
            cancel: CancelHandle::new(),
        })
    }

    /// Get a cancel handle for this queue.
    pub fn cancel_handle(&self) -> CancelHandle {
        self.cancel.clone()
    }

    /// Get queue statistics.
    pub fn stats(&self) -> QueueStats {
        let mut s = QueueStats {
            total: self.statuses.len(),
            ..Default::default()
        };
        for status in self.statuses.values() {
            match status.state {
                TaskState::Pending => s.pending += 1,
                TaskState::Ready => s.ready += 1,
                TaskState::Running => s.running += 1,
                TaskState::Completed => s.completed += 1,
                TaskState::Failed => s.failed += 1,
                TaskState::Cancelled => s.cancelled += 1,
            }
        }
        s
    }

    /// Get the topological execution order.
    pub fn execution_order(&self) -> &[TaskId] {
        &self.order
    }

    /// Get the status of a specific task.
    pub fn task_status(&self, id: &str) -> Option<&TaskStatus> {
        self.statuses.get(id)
    }

    /// Get all task statuses.
    pub fn all_statuses(&self) -> Vec<&TaskStatus> {
        self.statuses.values().collect()
    }

    /// Check whether all dependencies of a task are completed.
    fn deps_met(&self, id: &str) -> bool {
        if let Some(task) = self.tasks.get(id) {
            task.depends_on.iter().all(|dep| {
                self.statuses
                    .get(dep)
                    .map(|s| s.state == TaskState::Completed)
                    .unwrap_or(false)
            })
        } else {
            false
        }
    }

    /// Refresh ready states: move Pending → Ready when deps are met.
    pub fn refresh_ready(&mut self) {
        let ids: Vec<TaskId> = self
            .statuses
            .iter()
            .filter(|(_, s)| s.state == TaskState::Pending)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if self.deps_met(&id) {
                if let Some(s) = self.statuses.get_mut(&id) {
                    s.state = TaskState::Ready;
                }
            }
        }
    }

    /// Get the next batch of tasks to execute (respecting concurrency limit).
    pub fn next_batch(&mut self) -> Vec<TaskId> {
        self.refresh_ready();

        let running = self
            .statuses
            .values()
            .filter(|s| s.state == TaskState::Running)
            .count();

        let available = self.config.max_concurrency.saturating_sub(running);

        // Pick ready tasks in topological + priority order
        let mut ready: Vec<&TaskDef> = self
            .order
            .iter()
            .filter_map(|id| {
                let s = self.statuses.get(id)?;
                if s.state == TaskState::Ready {
                    self.tasks.get(id)
                } else {
                    None
                }
            })
            .collect();

        ready.sort_by_key(|t| t.priority);

        let batch: Vec<TaskId> = ready
            .into_iter()
            .take(available)
            .map(|t| t.id.clone())
            .collect();

        // Mark them as running
        for id in &batch {
            if let Some(s) = self.statuses.get_mut(id) {
                s.state = TaskState::Running;
                s.started_at = Some(Instant::now());
                s.attempts += 1;
            }
        }

        batch
    }

    /// Mark a task as completed.
    pub fn complete(&mut self, id: &str) -> Result<(), TaskQueueError> {
        let status = self
            .statuses
            .get_mut(id)
            .ok_or_else(|| TaskQueueError::NotFound(id.to_string()))?;
        status.state = TaskState::Completed;
        if let Some(started) = status.started_at {
            status.elapsed_ms = started.elapsed().as_millis() as u64;
        }
        Ok(())
    }

    /// Mark a task as failed. If retries remain, requeue it as Pending.
    pub fn fail(&mut self, id: &str, error: &str) -> Result<(), TaskQueueError> {
        let task = self
            .tasks
            .get(id)
            .ok_or_else(|| TaskQueueError::NotFound(id.to_string()))?
            .clone();

        let status = self
            .statuses
            .get_mut(id)
            .ok_or_else(|| TaskQueueError::NotFound(id.to_string()))?;

        status.last_error = Some(error.to_string());

        if status.attempts <= task.retry_policy.max_retries {
            // Requeue
            status.state = TaskState::Pending;
        } else {
            status.state = TaskState::Failed;
            if let Some(started) = status.started_at {
                status.elapsed_ms = started.elapsed().as_millis() as u64;
            }
            // Optionally cancel dependents
            if self.config.cancel_dependents_on_failure {
                self.cancel_dependents(id);
            }
        }

        Ok(())
    }

    /// Cancel all tasks that transitively depend on the given task.
    fn cancel_dependents(&mut self, failed_id: &str) {
        let mut to_cancel = Vec::new();
        for (id, task) in &self.tasks {
            if task.depends_on.iter().any(|d| d == failed_id) {
                to_cancel.push(id.clone());
            }
        }
        for id in &to_cancel {
            if let Some(s) = self.statuses.get_mut(id) {
                if s.state != TaskState::Completed && s.state != TaskState::Failed {
                    s.state = TaskState::Cancelled;
                }
            }
            // Recursively cancel
            self.cancel_dependents(id);
        }
    }

    /// Run the queue to completion using a synchronous executor function.
    /// `exec_fn` receives the task definition and returns `Ok(())` on success
    /// or `Err(msg)` on failure.
    pub fn run_sync<F>(&mut self, mut exec_fn: F) -> Result<QueueStats, TaskQueueError>
    where
        F: FnMut(&TaskDef) -> Result<(), String>,
    {
        loop {
            if self.cancel.is_cancelled() {
                // Cancel all non-terminal tasks
                for s in self.statuses.values_mut() {
                    if s.state != TaskState::Completed && s.state != TaskState::Failed {
                        s.state = TaskState::Cancelled;
                    }
                }
                return Err(TaskQueueError::Cancelled);
            }

            let batch = self.next_batch();
            if batch.is_empty() {
                // Nothing ready — either everything is done, or there are failures
                break;
            }

            for id in batch {
                let task = self.tasks.get(&id).unwrap().clone();
                match exec_fn(&task) {
                    Ok(()) => {
                        self.complete(&id)?;
                    }
                    Err(e) => {
                        self.fail(&id, &e)?;
                    }
                }
            }
        }

        Ok(self.stats())
    }

    /// Check whether the queue is finished (no pending/ready/running tasks).
    pub fn is_finished(&self) -> bool {
        self.statuses.values().all(|s| {
            matches!(
                s.state,
                TaskState::Completed | TaskState::Failed | TaskState::Cancelled
            )
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, priority: Priority, deps: &[&str]) -> TaskDef {
        TaskDef {
            id: id.to_string(),
            label: format!("Task {id}"),
            priority,
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            retry_policy: RetryPolicy::default(),
            payload: String::new(),
        }
    }

    #[test]
    fn test_topological_sort_linear() {
        let tasks = vec![
            task("c", Priority::Normal, &["b"]),
            task("b", Priority::Normal, &["a"]),
            task("a", Priority::Normal, &[]),
        ];
        let order = topological_sort(&tasks).unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_topological_sort_diamond() {
        let tasks = vec![
            task("d", Priority::Normal, &["b", "c"]),
            task("b", Priority::Normal, &["a"]),
            task("c", Priority::Normal, &["a"]),
            task("a", Priority::Normal, &[]),
        ];
        let order = topological_sort(&tasks).unwrap();
        assert_eq!(order[0], "a");
        assert!(order.contains(&"b".to_string()));
        assert!(order.contains(&"c".to_string()));
        assert_eq!(order[3], "d");
    }

    #[test]
    fn test_cycle_detection() {
        let tasks = vec![
            task("a", Priority::Normal, &["c"]),
            task("b", Priority::Normal, &["a"]),
            task("c", Priority::Normal, &["b"]),
        ];
        let err = topological_sort(&tasks).unwrap_err();
        assert!(matches!(err, TaskQueueError::CycleDetected(_)));
    }

    #[test]
    fn test_unknown_dep() {
        let tasks = vec![task("a", Priority::Normal, &["missing"])];
        let err = topological_sort(&tasks).unwrap_err();
        assert!(matches!(err, TaskQueueError::UnknownDep { .. }));
    }

    #[test]
    fn test_duplicate_task() {
        let tasks = vec![
            task("a", Priority::Normal, &[]),
            task("a", Priority::Low, &[]),
        ];
        let err = TaskQueue::new(tasks, QueueConfig::default()).unwrap_err();
        assert!(matches!(err, TaskQueueError::Duplicate(_)));
    }

    #[test]
    fn test_run_sync_success() {
        let tasks = vec![
            task("a", Priority::Normal, &[]),
            task("b", Priority::Normal, &["a"]),
            task("c", Priority::Normal, &["b"]),
        ];
        let mut q = TaskQueue::new(tasks, QueueConfig::default()).unwrap();
        let stats = q.run_sync(|_| Ok(())).unwrap();
        assert_eq!(stats.completed, 3);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_run_sync_with_failure_cancels_dependents() {
        let tasks = vec![
            task("a", Priority::Normal, &[]),
            task("b", Priority::Normal, &["a"]),
            task("c", Priority::Normal, &["b"]),
        ];
        let config = QueueConfig {
            cancel_dependents_on_failure: true,
            ..Default::default()
        };
        let mut q = TaskQueue::new(tasks, config).unwrap();
        let stats = q
            .run_sync(|t| {
                if t.id == "a" {
                    Err("boom".into())
                } else {
                    Ok(())
                }
            })
            .unwrap();
        // a: 4 attempts (initial + 3 retries) then failed
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.cancelled, 2);
    }

    #[test]
    fn test_priority_ordering() {
        let tasks = vec![
            task("low", Priority::Low, &[]),
            task("high", Priority::High, &[]),
            task("crit", Priority::Critical, &[]),
        ];
        let config = QueueConfig {
            max_concurrency: 1,
            ..Default::default()
        };
        let mut q = TaskQueue::new(tasks, config).unwrap();
        let mut exec_order = Vec::new();
        q.run_sync(|t| {
            exec_order.push(t.id.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(exec_order[0], "crit");
        assert_eq!(exec_order[1], "high");
        assert_eq!(exec_order[2], "low");
    }

    #[test]
    fn test_retry_backoff() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff: Duration::from_millis(100),
            multiplier: 2.0,
            max_backoff: Duration::from_secs(5),
        };
        assert_eq!(policy.backoff_for(0), Duration::from_millis(100));
        assert_eq!(policy.backoff_for(1), Duration::from_millis(200));
        assert_eq!(policy.backoff_for(2), Duration::from_millis(400));
        // Check capping
        assert!(policy.backoff_for(100) <= Duration::from_secs(5));
    }

    #[test]
    fn test_cancel_handle() {
        let tasks = vec![
            task("a", Priority::Normal, &[]),
            task("b", Priority::Normal, &["a"]),
        ];
        let mut q = TaskQueue::new(tasks, QueueConfig::default()).unwrap();
        let handle = q.cancel_handle();
        handle.cancel();
        let err = q.run_sync(|_| Ok(())).unwrap_err();
        assert!(matches!(err, TaskQueueError::Cancelled));
    }

    #[test]
    fn test_stats() {
        let tasks = vec![
            task("a", Priority::Normal, &[]),
            task("b", Priority::Normal, &[]),
        ];
        let q = TaskQueue::new(tasks, QueueConfig::default()).unwrap();
        let s = q.stats();
        assert_eq!(s.total, 2);
        assert_eq!(s.pending, 2);
    }

    #[test]
    fn test_empty_queue() {
        let mut q = TaskQueue::new(vec![], QueueConfig::default()).unwrap();
        let stats = q.run_sync(|_| Ok(())).unwrap();
        assert_eq!(stats.total, 0);
        assert!(q.is_finished());
    }

    #[test]
    fn test_parallel_independent_tasks() {
        let tasks = vec![
            task("a", Priority::Normal, &[]),
            task("b", Priority::Normal, &[]),
            task("c", Priority::Normal, &[]),
        ];
        let config = QueueConfig {
            max_concurrency: 3,
            ..Default::default()
        };
        let mut q = TaskQueue::new(tasks, config).unwrap();
        // All three should be in the first batch
        let batch = q.next_batch();
        assert_eq!(batch.len(), 3);
    }
}
