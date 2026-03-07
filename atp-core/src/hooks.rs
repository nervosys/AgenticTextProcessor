//! Event system and lifecycle hooks.
//!
//! Pub/sub event bus with pre/post pipeline stage hooks, on-match/on-error/
//! on-complete callbacks, and hook chaining. Enables extensibility without
//! modifying core pipeline logic.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Event types emitted during pipeline execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    /// Pipeline is about to start.
    PipelineStart,
    /// Pipeline finished successfully.
    PipelineComplete,
    /// Pipeline failed with an error.
    PipelineError,
    /// A stage is about to execute.
    StageStart,
    /// A stage completed successfully.
    StageComplete,
    /// A stage failed.
    StageError,
    /// A search match was found.
    MatchFound,
    /// A transform was applied.
    TransformApplied,
    /// A custom/user-defined event.
    Custom(String),
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PipelineStart => write!(f, "pipeline.start"),
            Self::PipelineComplete => write!(f, "pipeline.complete"),
            Self::PipelineError => write!(f, "pipeline.error"),
            Self::StageStart => write!(f, "stage.start"),
            Self::StageComplete => write!(f, "stage.complete"),
            Self::StageError => write!(f, "stage.error"),
            Self::MatchFound => write!(f, "match.found"),
            Self::TransformApplied => write!(f, "transform.applied"),
            Self::Custom(name) => write!(f, "custom.{name}"),
        }
    }
}

/// Payload for events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// What kind of event.
    pub kind: EventKind,
    /// Timestamp (epoch seconds).
    pub timestamp: u64,
    /// Stage index (if applicable).
    pub stage_index: Option<usize>,
    /// Stage label (if applicable).
    pub stage_label: Option<String>,
    /// Free-form data associated with the event.
    pub data: BTreeMap<String, String>,
}

impl Event {
    /// Create a new event of the given kind.
    pub fn new(kind: EventKind) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            kind,
            timestamp,
            stage_index: None,
            stage_label: None,
            data: BTreeMap::new(),
        }
    }

    /// Builder: set stage info.
    pub fn with_stage(mut self, index: usize, label: &str) -> Self {
        self.stage_index = Some(index);
        self.stage_label = Some(label.to_string());
        self
    }

    /// Builder: add a data field.
    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.data.insert(key.to_string(), value.to_string());
        self
    }
}

/// Hook priority — lower value runs first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HookPriority {
    First = 0,
    Early = 25,
    #[default]
    Normal = 50,
    Late = 75,
    Last = 100,
}

/// Outcome from a hook execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookOutcome {
    /// Continue processing subsequent hooks and the event.
    Continue,
    /// Skip remaining hooks for this event.
    Skip,
    /// Abort the pipeline (for pre-hooks).
    Abort(String),
}

/// A registered hook definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDef {
    /// Unique hook name.
    pub name: String,
    /// Which event(s) this hooks into.
    pub events: Vec<EventKind>,
    /// Execution priority.
    pub priority: HookPriority,
    /// Whether this hook is enabled.
    pub enabled: bool,
    /// Description.
    pub description: String,
}

/// Stats about the event bus.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookStats {
    pub total_hooks: usize,
    pub total_events_emitted: u64,
    pub events_by_kind: BTreeMap<String, u64>,
}

// ---------------------------------------------------------------------------
// Hook handler (boxed closure)
// ---------------------------------------------------------------------------

/// Type-erased hook handler.
pub type HookHandler = Arc<dyn Fn(&Event) -> HookOutcome + Send + Sync>;

struct RegisteredHook {
    def: HookDef,
    handler: HookHandler,
}

impl fmt::Debug for RegisteredHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredHook")
            .field("def", &self.def)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

/// The central event bus with hook registration and dispatch.
#[derive(Clone)]
pub struct EventBus {
    hooks: Arc<Mutex<Vec<RegisteredHook>>>,
    log: Arc<Mutex<Vec<Event>>>,
    stats: Arc<Mutex<HookStats>>,
}

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus")
            .field(
                "hooks_count",
                &self.hooks.lock().map(|h| h.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl EventBus {
    /// Create a new empty event bus.
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(Mutex::new(Vec::new())),
            log: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(HookStats::default())),
        }
    }

    /// Register a hook with a handler function.
    pub fn register(
        &self,
        def: HookDef,
        handler: impl Fn(&Event) -> HookOutcome + Send + Sync + 'static,
    ) {
        let mut hooks = self.hooks.lock().unwrap();
        hooks.push(RegisteredHook {
            def,
            handler: Arc::new(handler),
        });
        // Keep sorted by priority
        hooks.sort_by_key(|h| h.def.priority);
        let mut stats = self.stats.lock().unwrap();
        stats.total_hooks = hooks.len();
    }

    /// Unregister a hook by name.
    pub fn unregister(&self, name: &str) -> bool {
        let mut hooks = self.hooks.lock().unwrap();
        let before = hooks.len();
        hooks.retain(|h| h.def.name != name);
        let removed = hooks.len() < before;
        if removed {
            let mut stats = self.stats.lock().unwrap();
            stats.total_hooks = hooks.len();
        }
        removed
    }

    /// Emit an event, dispatching to all matching hooks.
    /// Returns the final outcome (Continue, Skip, or Abort).
    pub fn emit(&self, event: &Event) -> HookOutcome {
        // Record event
        {
            let mut log = self.log.lock().unwrap();
            log.push(event.clone());
        }
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_events_emitted += 1;
            *stats
                .events_by_kind
                .entry(event.kind.to_string())
                .or_insert(0) += 1;
        }

        let hooks = self.hooks.lock().unwrap();
        for hook in hooks.iter() {
            if !hook.def.enabled {
                continue;
            }
            if !hook.def.events.contains(&event.kind) {
                continue;
            }
            let outcome = (hook.handler)(event);
            match outcome {
                HookOutcome::Continue => {}
                HookOutcome::Skip => return HookOutcome::Skip,
                HookOutcome::Abort(ref _msg) => return outcome,
            }
        }
        HookOutcome::Continue
    }

    /// Get all registered hook definitions.
    pub fn hook_defs(&self) -> Vec<HookDef> {
        self.hooks
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.def.clone())
            .collect()
    }

    /// Get the event log.
    pub fn event_log(&self) -> Vec<Event> {
        self.log.lock().unwrap().clone()
    }

    /// Clear the event log.
    pub fn clear_log(&self) {
        self.log.lock().unwrap().clear();
    }

    /// Get stats.
    pub fn stats(&self) -> HookStats {
        self.stats.lock().unwrap().clone()
    }

    /// Enable/disable a hook by name.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> bool {
        let mut hooks = self.hooks.lock().unwrap();
        for h in hooks.iter_mut() {
            if h.def.name == name {
                h.def.enabled = enabled;
                return true;
            }
        }
        false
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Convenience builders
// ---------------------------------------------------------------------------

/// Create a simple logging hook that records event data.
pub fn logging_hook(name: &str, events: Vec<EventKind>) -> HookDef {
    HookDef {
        name: name.to_string(),
        events,
        priority: HookPriority::Last,
        enabled: true,
        description: format!("Logging hook: {name}"),
    }
}

/// Create a pre-stage validation hook definition.
pub fn pre_stage_hook(name: &str) -> HookDef {
    HookDef {
        name: name.to_string(),
        events: vec![EventKind::StageStart],
        priority: HookPriority::Early,
        enabled: true,
        description: format!("Pre-stage hook: {name}"),
    }
}

/// Create a post-stage hook definition.
pub fn post_stage_hook(name: &str) -> HookDef {
    HookDef {
        name: name.to_string(),
        events: vec![EventKind::StageComplete],
        priority: HookPriority::Normal,
        enabled: true,
        description: format!("Post-stage hook: {name}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_and_log() {
        let bus = EventBus::new();
        let event = Event::new(EventKind::PipelineStart);
        let outcome = bus.emit(&event);
        assert_eq!(outcome, HookOutcome::Continue);
        assert_eq!(bus.event_log().len(), 1);
    }

    #[test]
    fn test_hook_fires() {
        let bus = EventBus::new();
        let flag = Arc::new(Mutex::new(false));
        let flag_clone = flag.clone();
        bus.register(
            logging_hook("test", vec![EventKind::PipelineStart]),
            move |_| {
                *flag_clone.lock().unwrap() = true;
                HookOutcome::Continue
            },
        );
        bus.emit(&Event::new(EventKind::PipelineStart));
        assert!(*flag.lock().unwrap());
    }

    #[test]
    fn test_hook_does_not_fire_for_other_events() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0u32));
        let count_clone = count.clone();
        bus.register(
            logging_hook("test", vec![EventKind::PipelineStart]),
            move |_| {
                *count_clone.lock().unwrap() += 1;
                HookOutcome::Continue
            },
        );
        bus.emit(&Event::new(EventKind::PipelineComplete));
        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn test_hook_skip() {
        let bus = EventBus::new();
        bus.register(
            HookDef {
                name: "skip_hook".into(),
                events: vec![EventKind::StageStart],
                priority: HookPriority::First,
                enabled: true,
                description: "Skips".into(),
            },
            |_| HookOutcome::Skip,
        );
        let count = Arc::new(Mutex::new(0u32));
        let count_clone = count.clone();
        bus.register(
            HookDef {
                name: "after_hook".into(),
                events: vec![EventKind::StageStart],
                priority: HookPriority::Normal,
                enabled: true,
                description: "After".into(),
            },
            move |_| {
                *count_clone.lock().unwrap() += 1;
                HookOutcome::Continue
            },
        );
        let outcome = bus.emit(&Event::new(EventKind::StageStart));
        assert_eq!(outcome, HookOutcome::Skip);
        assert_eq!(*count.lock().unwrap(), 0); // Second hook should NOT fire
    }

    #[test]
    fn test_hook_abort() {
        let bus = EventBus::new();
        bus.register(pre_stage_hook("blocker"), |_| {
            HookOutcome::Abort("blocked".into())
        });
        let outcome = bus.emit(&Event::new(EventKind::StageStart));
        assert!(matches!(outcome, HookOutcome::Abort(_)));
    }

    #[test]
    fn test_unregister() {
        let bus = EventBus::new();
        bus.register(logging_hook("temp", vec![EventKind::PipelineStart]), |_| {
            HookOutcome::Continue
        });
        assert_eq!(bus.hook_defs().len(), 1);
        assert!(bus.unregister("temp"));
        assert_eq!(bus.hook_defs().len(), 0);
        assert!(!bus.unregister("nonexistent"));
    }

    #[test]
    fn test_enable_disable() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0u32));
        let count_clone = count.clone();
        bus.register(
            logging_hook("toggleable", vec![EventKind::PipelineStart]),
            move |_| {
                *count_clone.lock().unwrap() += 1;
                HookOutcome::Continue
            },
        );

        bus.emit(&Event::new(EventKind::PipelineStart));
        assert_eq!(*count.lock().unwrap(), 1);

        bus.set_enabled("toggleable", false);
        bus.emit(&Event::new(EventKind::PipelineStart));
        assert_eq!(*count.lock().unwrap(), 1); // Still 1

        bus.set_enabled("toggleable", true);
        bus.emit(&Event::new(EventKind::PipelineStart));
        assert_eq!(*count.lock().unwrap(), 2);
    }

    #[test]
    fn test_priority_ordering() {
        let bus = EventBus::new();
        let order = Arc::new(Mutex::new(Vec::new()));

        let o1 = order.clone();
        bus.register(
            HookDef {
                name: "late".into(),
                events: vec![EventKind::PipelineStart],
                priority: HookPriority::Late,
                enabled: true,
                description: "late".into(),
            },
            move |_| {
                o1.lock().unwrap().push("late");
                HookOutcome::Continue
            },
        );

        let o2 = order.clone();
        bus.register(
            HookDef {
                name: "early".into(),
                events: vec![EventKind::PipelineStart],
                priority: HookPriority::Early,
                enabled: true,
                description: "early".into(),
            },
            move |_| {
                o2.lock().unwrap().push("early");
                HookOutcome::Continue
            },
        );

        bus.emit(&Event::new(EventKind::PipelineStart));
        let result = order.lock().unwrap().clone();
        assert_eq!(result, vec!["early", "late"]);
    }

    #[test]
    fn test_event_with_stage() {
        let event = Event::new(EventKind::StageStart)
            .with_stage(0, "grep")
            .with_data("pattern", "foo");
        assert_eq!(event.stage_index, Some(0));
        assert_eq!(event.stage_label.as_deref(), Some("grep"));
        assert_eq!(event.data.get("pattern").unwrap(), "foo");
    }

    #[test]
    fn test_stats() {
        let bus = EventBus::new();
        bus.register(logging_hook("s", vec![EventKind::PipelineStart]), |_| {
            HookOutcome::Continue
        });
        bus.emit(&Event::new(EventKind::PipelineStart));
        bus.emit(&Event::new(EventKind::PipelineStart));
        bus.emit(&Event::new(EventKind::PipelineComplete));

        let stats = bus.stats();
        assert_eq!(stats.total_hooks, 1);
        assert_eq!(stats.total_events_emitted, 3);
        assert_eq!(stats.events_by_kind.get("pipeline.start"), Some(&2));
    }

    #[test]
    fn test_clear_log() {
        let bus = EventBus::new();
        bus.emit(&Event::new(EventKind::PipelineStart));
        assert_eq!(bus.event_log().len(), 1);
        bus.clear_log();
        assert!(bus.event_log().is_empty());
    }

    #[test]
    fn test_custom_event() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0u32));
        let count_clone = count.clone();
        bus.register(
            HookDef {
                name: "custom_handler".into(),
                events: vec![EventKind::Custom("my.event".into())],
                priority: HookPriority::Normal,
                enabled: true,
                description: "custom".into(),
            },
            move |_| {
                *count_clone.lock().unwrap() += 1;
                HookOutcome::Continue
            },
        );
        bus.emit(&Event::new(EventKind::Custom("my.event".into())));
        bus.emit(&Event::new(EventKind::Custom("other.event".into())));
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn test_multiple_events_per_hook() {
        let bus = EventBus::new();
        let count = Arc::new(Mutex::new(0u32));
        let count_clone = count.clone();
        bus.register(
            logging_hook(
                "multi",
                vec![EventKind::PipelineStart, EventKind::PipelineComplete],
            ),
            move |_| {
                *count_clone.lock().unwrap() += 1;
                HookOutcome::Continue
            },
        );
        bus.emit(&Event::new(EventKind::PipelineStart));
        bus.emit(&Event::new(EventKind::PipelineComplete));
        bus.emit(&Event::new(EventKind::PipelineError));
        assert_eq!(*count.lock().unwrap(), 2);
    }
}
