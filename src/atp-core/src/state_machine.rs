//! # Programmable State Machine
//!
//! Define states, transitions, guards, and actions. Run the machine
//! against input lines, emit events, detect deadlocks, and export
//! the graph in DOT format.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique state identifier.
pub type StateId = String;

/// Unique event/trigger identifier.
pub type EventId = String;

/// A guard predicate on input text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Guard {
    /// Always passes.
    Always,
    /// Input line contains substring.
    Contains(String),
    /// Input line matches regex.
    Regex(String),
    /// Input line starts with prefix.
    StartsWith(String),
    /// Input line ends with suffix.
    EndsWith(String),
    /// Logical AND of guards.
    All(Vec<Guard>),
    /// Logical OR of guards.
    Any(Vec<Guard>),
    /// Negate a guard.
    Not(Box<Guard>),
}

impl Guard {
    /// Evaluate the guard against an input line.
    pub fn evaluate(&self, input: &str) -> bool {
        match self {
            Guard::Always => true,
            Guard::Contains(s) => input.contains(s.as_str()),
            Guard::Regex(pat) => regex::Regex::new(pat)
                .map(|re| re.is_match(input))
                .unwrap_or(false),
            Guard::StartsWith(p) => input.starts_with(p.as_str()),
            Guard::EndsWith(s) => input.ends_with(s.as_str()),
            Guard::All(gs) => gs.iter().all(|g| g.evaluate(input)),
            Guard::Any(gs) => gs.iter().any(|g| g.evaluate(input)),
            Guard::Not(g) => !g.evaluate(input),
        }
    }
}

/// Action to perform on a machine transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MachineAction {
    /// Emit a named event.
    Emit(EventId),
    /// Set a context variable.
    SetVar(String, String),
    /// Increment a counter variable.
    Increment(String),
    /// Log a message.
    Log(String),
    /// Collect the input line in a named buffer.
    Collect(String),
    /// No operation.
    Noop,
}

/// A transition between states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: StateId,
    pub to: StateId,
    pub guard: Guard,
    pub actions: Vec<MachineAction>,
    /// Human-readable label.
    pub label: Option<String>,
}

/// State definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDef {
    pub id: StateId,
    /// Actions run on entry.
    pub on_enter: Vec<MachineAction>,
    /// Actions run on exit.
    pub on_exit: Vec<MachineAction>,
    /// Is this an accepting/final state?
    pub accepting: bool,
}

/// The machine definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineSpec {
    pub name: String,
    pub states: Vec<StateDef>,
    pub transitions: Vec<Transition>,
    pub initial: StateId,
}

/// Runtime execution context for a machine instance.
#[derive(Debug, Clone)]
pub struct MachineInstance {
    pub spec: MachineSpec,
    pub current: StateId,
    pub variables: BTreeMap<String, String>,
    pub events: Vec<EmittedEvent>,
    pub buffers: BTreeMap<String, Vec<String>>,
    pub step_count: usize,
    pub history: Vec<StateId>,
}

/// An event emitted during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmittedEvent {
    pub event: EventId,
    pub step: usize,
    pub from: StateId,
    pub to: StateId,
    pub input: String,
}

/// Result summary after processing all inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub final_state: StateId,
    pub accepting: bool,
    pub steps: usize,
    pub events: Vec<EmittedEvent>,
    pub variables: BTreeMap<String, String>,
    pub history: Vec<StateId>,
}

/// Machine analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineAnalysis {
    pub state_count: usize,
    pub transition_count: usize,
    pub reachable: BTreeSet<String>,
    pub unreachable: BTreeSet<String>,
    pub dead_ends: BTreeSet<String>,
    pub has_cycles: bool,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

impl MachineSpec {
    pub fn new(name: &str, initial: &str) -> Self {
        Self {
            name: name.into(),
            states: Vec::new(),
            transitions: Vec::new(),
            initial: initial.into(),
        }
    }

    pub fn add_state(&mut self, id: &str, accepting: bool) -> &mut Self {
        self.states.push(StateDef {
            id: id.into(),
            on_enter: Vec::new(),
            on_exit: Vec::new(),
            accepting,
        });
        self
    }

    pub fn add_state_with_actions(
        &mut self,
        id: &str,
        accepting: bool,
        on_enter: Vec<MachineAction>,
        on_exit: Vec<MachineAction>,
    ) -> &mut Self {
        self.states.push(StateDef {
            id: id.into(),
            on_enter,
            on_exit,
            accepting,
        });
        self
    }

    pub fn add_transition(
        &mut self,
        from: &str,
        to: &str,
        guard: Guard,
        actions: Vec<MachineAction>,
    ) -> &mut Self {
        self.transitions.push(Transition {
            from: from.into(),
            to: to.into(),
            guard,
            actions,
            label: None,
        });
        self
    }

    pub fn add_labeled_transition(
        &mut self,
        from: &str,
        to: &str,
        guard: Guard,
        actions: Vec<MachineAction>,
        label: &str,
    ) -> &mut Self {
        self.transitions.push(Transition {
            from: from.into(),
            to: to.into(),
            guard,
            actions,
            label: Some(label.into()),
        });
        self
    }

    /// Build a runnable instance.
    pub fn instantiate(&self) -> MachineInstance {
        MachineInstance {
            spec: self.clone(),
            current: self.initial.clone(),
            variables: BTreeMap::new(),
            events: Vec::new(),
            buffers: BTreeMap::new(),
            step_count: 0,
            history: vec![self.initial.clone()],
        }
    }

    /// Find a state definition by id.
    pub fn find_state(&self, id: &str) -> Option<&StateDef> {
        self.states.iter().find(|s| s.id == id)
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

impl MachineInstance {
    /// Process a single input line; returns whether a transition fired.
    pub fn step(&mut self, input: &str) -> bool {
        let current = self.current.clone();
        // Find first matching transition
        let transition = self
            .spec
            .transitions
            .iter()
            .find(|t| t.from == current && t.guard.evaluate(input));

        if let Some(t) = transition.cloned() {
            // On-exit actions for old state
            let exit_actions = self
                .spec
                .find_state(&current)
                .map(|sd| sd.on_exit.clone())
                .unwrap_or_default();
            for action in &exit_actions {
                self.execute_action(action, input, &current, &t.to);
            }

            // Transition actions
            for action in &t.actions {
                self.execute_action(action, input, &current, &t.to);
            }

            // Move to new state
            self.current = t.to.clone();
            self.history.push(self.current.clone());

            // On-enter actions for new state
            if let Some(state_def) = self.spec.find_state(&t.to).cloned() {
                for action in &state_def.on_enter {
                    self.execute_action(action, input, &current, &t.to);
                }
            }

            self.step_count += 1;
            true
        } else {
            self.step_count += 1;
            false
        }
    }

    fn execute_action(&mut self, action: &MachineAction, input: &str, from: &str, to: &str) {
        match action {
            MachineAction::Emit(event) => {
                self.events.push(EmittedEvent {
                    event: event.clone(),
                    step: self.step_count,
                    from: from.into(),
                    to: to.into(),
                    input: input.into(),
                });
            }
            MachineAction::SetVar(key, val) => {
                self.variables.insert(key.clone(), val.clone());
            }
            MachineAction::Increment(key) => {
                let current = self
                    .variables
                    .get(key)
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0);
                self.variables
                    .insert(key.clone(), (current + 1).to_string());
            }
            MachineAction::Log(_msg) => { /* no-op in library context */ }
            MachineAction::Collect(buffer) => {
                self.buffers
                    .entry(buffer.clone())
                    .or_default()
                    .push(input.into());
            }
            MachineAction::Noop => {}
        }
    }

    /// Process many input lines.
    pub fn run(&mut self, inputs: &[&str]) -> RunResult {
        for input in inputs {
            self.step(input);
        }
        self.result()
    }

    /// Current result snapshot.
    pub fn result(&self) -> RunResult {
        let accepting = self
            .spec
            .find_state(&self.current)
            .map(|s| s.accepting)
            .unwrap_or(false);
        RunResult {
            final_state: self.current.clone(),
            accepting,
            steps: self.step_count,
            events: self.events.clone(),
            variables: self.variables.clone(),
            history: self.history.clone(),
        }
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.current = self.spec.initial.clone();
        self.variables.clear();
        self.events.clear();
        self.buffers.clear();
        self.step_count = 0;
        self.history = vec![self.spec.initial.clone()];
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Analyse the machine graph: reachability, dead ends, cycles.
pub fn analyze_machine(spec: &MachineSpec) -> MachineAnalysis {
    let all_states: BTreeSet<String> = spec.states.iter().map(|s| s.id.clone()).collect();

    // Reachability via BFS
    let mut reachable = BTreeSet::new();
    let mut queue = vec![spec.initial.clone()];
    while let Some(state) = queue.pop() {
        if reachable.insert(state.clone()) {
            for t in &spec.transitions {
                if t.from == state && !reachable.contains(&t.to) {
                    queue.push(t.to.clone());
                }
            }
        }
    }

    let unreachable: BTreeSet<String> = all_states.difference(&reachable).cloned().collect();

    // Dead ends: states with no outgoing transitions
    let has_outgoing: BTreeSet<String> = spec.transitions.iter().map(|t| t.from.clone()).collect();
    let dead_ends: BTreeSet<String> = all_states
        .iter()
        .filter(|s| !has_outgoing.contains(*s))
        .cloned()
        .collect();

    // Cycle detection via DFS
    let has_cycles = detect_cycles(spec);

    MachineAnalysis {
        state_count: all_states.len(),
        transition_count: spec.transitions.len(),
        reachable,
        unreachable,
        dead_ends,
        has_cycles,
    }
}

fn detect_cycles(spec: &MachineSpec) -> bool {
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &spec.transitions {
        adj.entry(t.from.clone()).or_default().push(t.to.clone());
    }
    let mut visited = BTreeSet::new();
    let mut on_stack = BTreeSet::new();
    for state in spec.states.iter().map(|s| &s.id) {
        if dfs_cycle(state, &adj, &mut visited, &mut on_stack) {
            return true;
        }
    }
    false
}

fn dfs_cycle(
    node: &str,
    adj: &BTreeMap<String, Vec<String>>,
    visited: &mut BTreeSet<String>,
    on_stack: &mut BTreeSet<String>,
) -> bool {
    if on_stack.contains(node) {
        return true;
    }
    if visited.contains(node) {
        return false;
    }
    visited.insert(node.to_string());
    on_stack.insert(node.to_string());
    if let Some(neighbors) = adj.get(node) {
        for n in neighbors {
            if dfs_cycle(n, adj, visited, on_stack) {
                return true;
            }
        }
    }
    on_stack.remove(node);
    false
}

// ---------------------------------------------------------------------------
// DOT export
// ---------------------------------------------------------------------------

/// Export the machine to Graphviz DOT format.
pub fn to_dot(spec: &MachineSpec) -> String {
    let mut out = String::new();
    out.push_str(&format!("digraph \"{}\" {{\n", spec.name));
    out.push_str("  rankdir=LR;\n");
    out.push_str("  \"\" [shape=none];\n");
    out.push_str(&format!("  \"\" -> \"{}\";\n", spec.initial));

    for state in &spec.states {
        let shape = if state.accepting {
            "doublecircle"
        } else {
            "circle"
        };
        out.push_str(&format!("  \"{}\" [shape={}];\n", state.id, shape));
    }

    for t in &spec.transitions {
        let label = t.label.as_deref().unwrap_or("");
        out.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            t.from, t.to, label
        ));
    }

    out.push_str("}\n");
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn traffic_light() -> MachineSpec {
        let mut spec = MachineSpec::new("traffic_light", "red");
        spec.add_state("red", false);
        spec.add_state("green", false);
        spec.add_state("yellow", false);
        spec.add_transition("red", "green", Guard::Contains("go".into()), vec![]);
        spec.add_transition(
            "green",
            "yellow",
            Guard::Contains("slow".into()),
            vec![MachineAction::Emit("warning".into())],
        );
        spec.add_transition("yellow", "red", Guard::Contains("stop".into()), vec![]);
        spec
    }

    #[test]
    fn test_basic_transitions() {
        let spec = traffic_light();
        let mut m = spec.instantiate();
        assert_eq!(m.current, "red");
        m.step("go");
        assert_eq!(m.current, "green");
        m.step("slow down");
        assert_eq!(m.current, "yellow");
        m.step("stop");
        assert_eq!(m.current, "red");
    }

    #[test]
    fn test_no_transition() {
        let spec = traffic_light();
        let mut m = spec.instantiate();
        let fired = m.step("invalid");
        assert!(!fired);
        assert_eq!(m.current, "red");
    }

    #[test]
    fn test_events_emitted() {
        let spec = traffic_light();
        let mut m = spec.instantiate();
        m.step("go");
        m.step("slow");
        assert_eq!(m.events.len(), 1);
        assert_eq!(m.events[0].event, "warning");
    }

    #[test]
    fn test_run_multiple() {
        let spec = traffic_light();
        let mut m = spec.instantiate();
        let result = m.run(&["go", "slow", "stop"]);
        assert_eq!(result.final_state, "red");
        assert_eq!(result.steps, 3);
        assert_eq!(result.history.len(), 4); // initial + 3 transitions
    }

    #[test]
    fn test_variables() {
        let mut spec = MachineSpec::new("counter", "start");
        spec.add_state("start", false);
        spec.add_state("counting", false);
        spec.add_transition(
            "start",
            "counting",
            Guard::Always,
            vec![
                MachineAction::SetVar("status".into(), "active".into()),
                MachineAction::Increment("count".into()),
            ],
        );
        spec.add_transition(
            "counting",
            "counting",
            Guard::Always,
            vec![MachineAction::Increment("count".into())],
        );

        let mut m = spec.instantiate();
        m.run(&["a", "b", "c"]);
        assert_eq!(m.variables["status"], "active");
        assert_eq!(m.variables["count"], "3");
    }

    #[test]
    fn test_collect_buffer() {
        let mut spec = MachineSpec::new("collector", "idle");
        spec.add_state("idle", false);
        spec.add_state("active", false);
        spec.add_transition("idle", "active", Guard::Contains("start".into()), vec![]);
        spec.add_transition(
            "active",
            "active",
            Guard::Always,
            vec![MachineAction::Collect("lines".into())],
        );

        let mut m = spec.instantiate();
        m.run(&["start", "line1", "line2"]);
        assert_eq!(m.buffers["lines"], vec!["line1", "line2"]);
    }

    #[test]
    fn test_guard_regex() {
        let g = Guard::Regex(r"^\d{3}$".into());
        assert!(g.evaluate("404"));
        assert!(!g.evaluate("hello"));
    }

    #[test]
    fn test_guard_composite() {
        let g = Guard::All(vec![
            Guard::Contains("error".into()),
            Guard::Not(Box::new(Guard::Contains("ignore".into()))),
        ]);
        assert!(g.evaluate("critical error"));
        assert!(!g.evaluate("ignore error"));
    }

    #[test]
    fn test_accepting_state() {
        let mut spec = MachineSpec::new("accept_test", "a");
        spec.add_state("a", false);
        spec.add_state("b", true);
        spec.add_transition("a", "b", Guard::Always, vec![]);

        let mut m = spec.instantiate();
        let r = m.run(&["x"]);
        assert!(r.accepting);
    }

    #[test]
    fn test_analyze_reachability() {
        let mut spec = MachineSpec::new("test", "a");
        spec.add_state("a", false);
        spec.add_state("b", false);
        spec.add_state("c", false); // unreachable
        spec.add_transition("a", "b", Guard::Always, vec![]);

        let analysis = analyze_machine(&spec);
        assert!(analysis.reachable.contains("a"));
        assert!(analysis.reachable.contains("b"));
        assert!(analysis.unreachable.contains("c"));
    }

    #[test]
    fn test_analyze_dead_ends() {
        let mut spec = MachineSpec::new("test", "a");
        spec.add_state("a", false);
        spec.add_state("b", false); // dead end
        spec.add_transition("a", "b", Guard::Always, vec![]);

        let analysis = analyze_machine(&spec);
        assert!(analysis.dead_ends.contains("b"));
    }

    #[test]
    fn test_analyze_cycles() {
        let spec = traffic_light();
        let analysis = analyze_machine(&spec);
        assert!(analysis.has_cycles);
    }

    #[test]
    fn test_no_cycles() {
        let mut spec = MachineSpec::new("linear", "a");
        spec.add_state("a", false);
        spec.add_state("b", false);
        spec.add_state("c", true);
        spec.add_transition("a", "b", Guard::Always, vec![]);
        spec.add_transition("b", "c", Guard::Always, vec![]);

        let analysis = analyze_machine(&spec);
        assert!(!analysis.has_cycles);
    }

    #[test]
    fn test_dot_export() {
        let spec = traffic_light();
        let dot = to_dot(&spec);
        assert!(dot.contains("digraph"));
        assert!(dot.contains("\"red\""));
        assert!(dot.contains("\"green\""));
        assert!(dot.contains("->"));
    }

    #[test]
    fn test_reset() {
        let spec = traffic_light();
        let mut m = spec.instantiate();
        m.run(&["go", "slow"]);
        assert_eq!(m.current, "yellow");
        m.reset();
        assert_eq!(m.current, "red");
        assert!(m.events.is_empty());
        assert_eq!(m.step_count, 0);
    }

    #[test]
    fn test_on_enter_on_exit() {
        let mut spec = MachineSpec::new("test", "a");
        spec.add_state_with_actions(
            "a",
            false,
            vec![],
            vec![MachineAction::Emit("exit_a".into())],
        );
        spec.add_state_with_actions(
            "b",
            false,
            vec![MachineAction::Emit("enter_b".into())],
            vec![],
        );
        spec.add_transition("a", "b", Guard::Always, vec![]);

        let mut m = spec.instantiate();
        m.step("x");
        let events: Vec<&str> = m.events.iter().map(|e| e.event.as_str()).collect();
        assert!(events.contains(&"exit_a"));
        assert!(events.contains(&"enter_b"));
    }
}
