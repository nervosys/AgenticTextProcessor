//! # Macro Engine — Text Macro System
//!
//! Record/replay transformation sequences, parameterized macros,
//! variable substitution, conditional expansion, and macro libraries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single transformation step inside a macro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MacroStep {
    /// Replace all occurrences of `pattern` with `replacement`.
    Replace { pattern: String, replacement: String },
    /// Prepend text.
    Prepend(String),
    /// Append text.
    Append(String),
    /// Convert to upper case.
    Upper,
    /// Convert to lower case.
    Lower,
    /// Trim whitespace.
    Trim,
    /// Delete all occurrences of `pattern`.
    Delete(String),
    /// Insert text at byte offset.
    InsertAt { offset: usize, text: String },
    /// Wrap each line with prefix/suffix.
    WrapLines { prefix: String, suffix: String },
    /// Apply a sub-macro by name.
    Call(String),
    /// Conditional: if text contains `needle`, apply `steps`, else `otherwise`.
    If {
        needle: String,
        steps: Vec<MacroStep>,
        otherwise: Vec<MacroStep>,
    },
}

/// A named, parameterized macro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub description: String,
    pub params: Vec<String>,
    pub steps: Vec<MacroStep>,
}

/// Macro execution context (variables & sub-macros).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroContext {
    pub variables: HashMap<String, String>,
    pub macros: HashMap<String, Macro>,
}

/// Result of running a macro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroResult {
    pub output: String,
    pub steps_applied: usize,
}

/// A recorder that captures transformations for replay.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroRecorder {
    steps: Vec<MacroStep>,
    recording: bool,
}

// ---------------------------------------------------------------------------
// Variable substitution
// ---------------------------------------------------------------------------

/// Substitute `{{var}}` placeholders from a variable map.
pub fn substitute(text: &str, vars: &HashMap<String, String>) -> String {
    let mut result = text.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

/// Substitute parameters: `$1`, `$2`, … from a positional slice.
pub fn substitute_positional(text: &str, params: &[&str]) -> String {
    let mut result = text.to_string();
    for (i, val) in params.iter().enumerate() {
        let placeholder = format!("${}", i + 1);
        result = result.replace(&placeholder, val);
    }
    result
}

// ---------------------------------------------------------------------------
// Step execution
// ---------------------------------------------------------------------------

fn apply_step(text: &str, step: &MacroStep, ctx: &MacroContext, depth: usize) -> String {
    if depth > 64 {
        return text.to_string(); // guard against infinite recursion
    }
    match step {
        MacroStep::Replace { pattern, replacement } => {
            let pat = substitute(pattern, &ctx.variables);
            let rep = substitute(replacement, &ctx.variables);
            text.replace(&pat, &rep)
        }
        MacroStep::Prepend(s) => {
            let s = substitute(s, &ctx.variables);
            format!("{s}{text}")
        }
        MacroStep::Append(s) => {
            let s = substitute(s, &ctx.variables);
            format!("{text}{s}")
        }
        MacroStep::Upper => text.to_uppercase(),
        MacroStep::Lower => text.to_lowercase(),
        MacroStep::Trim => text.trim().to_string(),
        MacroStep::Delete(pat) => {
            let pat = substitute(pat, &ctx.variables);
            text.replace(&pat, "")
        }
        MacroStep::InsertAt { offset, text: ins } => {
            let ins = substitute(ins, &ctx.variables);
            let off = (*offset).min(text.len());
            let mut s = text.to_string();
            s.insert_str(off, &ins);
            s
        }
        MacroStep::WrapLines { prefix, suffix } => {
            let pfx = substitute(prefix, &ctx.variables);
            let sfx = substitute(suffix, &ctx.variables);
            text.lines()
                .map(|l| format!("{pfx}{l}{sfx}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
        MacroStep::Call(name) => {
            if let Some(m) = ctx.macros.get(name) {
                run_steps(text, &m.steps, ctx, depth + 1)
            } else {
                text.to_string()
            }
        }
        MacroStep::If {
            needle,
            steps,
            otherwise,
        } => {
            let needle = substitute(needle, &ctx.variables);
            if text.contains(&needle) {
                run_steps(text, steps, ctx, depth + 1)
            } else {
                run_steps(text, otherwise, ctx, depth + 1)
            }
        }
    }
}

fn run_steps(text: &str, steps: &[MacroStep], ctx: &MacroContext, depth: usize) -> String {
    let mut current = text.to_string();
    for step in steps {
        current = apply_step(&current, step, ctx, depth);
    }
    current
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Execute a sequence of steps on `text`.
pub fn execute(text: &str, steps: &[MacroStep], ctx: &MacroContext) -> MacroResult {
    let output = run_steps(text, steps, ctx, 0);
    MacroResult {
        output,
        steps_applied: steps.len(),
    }
}

/// Execute a named macro from the context.
pub fn run_macro(text: &str, name: &str, ctx: &MacroContext) -> Result<MacroResult, String> {
    let m = ctx
        .macros
        .get(name)
        .ok_or_else(|| format!("macro not found: {name}"))?;
    Ok(execute(text, &m.steps, ctx))
}

/// Execute a named macro with positional parameters.
pub fn run_macro_with(
    text: &str,
    name: &str,
    params: &[&str],
    ctx: &MacroContext,
) -> Result<MacroResult, String> {
    let m = ctx
        .macros
        .get(name)
        .ok_or_else(|| format!("macro not found: {name}"))?;
    // Merge positional params into context variables
    let mut ctx2 = ctx.clone();
    for (i, val) in params.iter().enumerate() {
        ctx2.variables
            .insert(format!("${}", i + 1), (*val).to_string());
    }
    Ok(execute(text, &m.steps, &ctx2))
}

// ---------------------------------------------------------------------------
// Macro Context helpers
// ---------------------------------------------------------------------------

impl MacroContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a macro.
    pub fn register(&mut self, m: Macro) {
        self.macros.insert(m.name.clone(), m);
    }

    /// Set a variable.
    pub fn set_var(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_string(), value.to_string());
    }

    /// List registered macro names.
    pub fn list_macros(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.macros.keys().map(String::as_str).collect();
        names.sort();
        names
    }
}

// ---------------------------------------------------------------------------
// Recorder
// ---------------------------------------------------------------------------

impl MacroRecorder {
    /// Create a new recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start recording.
    pub fn start(&mut self) {
        self.recording = true;
        self.steps.clear();
    }

    /// Stop recording.
    pub fn stop(&mut self) {
        self.recording = false;
    }

    /// Record a step (only while recording).
    pub fn record(&mut self, step: MacroStep) {
        if self.recording {
            self.steps.push(step);
        }
    }

    /// Get recorded steps.
    pub fn steps(&self) -> &[MacroStep] {
        &self.steps
    }

    /// Convert recorded steps into a named `Macro`.
    pub fn to_macro(&self, name: &str, description: &str) -> Macro {
        Macro {
            name: name.to_string(),
            description: description.to_string(),
            params: Vec::new(),
            steps: self.steps.clone(),
        }
    }

    /// Is the recorder active?
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Clear recorded steps.
    pub fn clear(&mut self) {
        self.steps.clear();
    }
}

// ---------------------------------------------------------------------------
// Built-in macro library
// ---------------------------------------------------------------------------

/// Create a built-in "quote" macro that wraps text in quotes.
pub fn builtin_quote() -> Macro {
    Macro {
        name: "quote".into(),
        description: "Wrap text in double quotes".into(),
        params: Vec::new(),
        steps: vec![
            MacroStep::Prepend("\"".into()),
            MacroStep::Append("\"".into()),
        ],
    }
}

/// Create a built-in "slugify" macro: lowercase + replace spaces with hyphens.
pub fn builtin_slugify() -> Macro {
    Macro {
        name: "slugify".into(),
        description: "Slugify text".into(),
        params: Vec::new(),
        steps: vec![
            MacroStep::Lower,
            MacroStep::Trim,
            MacroStep::Replace {
                pattern: " ".into(),
                replacement: "-".into(),
            },
        ],
    }
}

/// Create a built-in "comment" macro that prefixes lines with `// `.
pub fn builtin_comment() -> Macro {
    Macro {
        name: "comment".into(),
        description: "Comment out lines with //".into(),
        params: Vec::new(),
        steps: vec![MacroStep::WrapLines {
            prefix: "// ".into(),
            suffix: String::new(),
        }],
    }
}

/// Register all built-in macros into a context.
pub fn register_builtins(ctx: &mut MacroContext) {
    ctx.register(builtin_quote());
    ctx.register(builtin_slugify());
    ctx.register(builtin_comment());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> MacroContext {
        let mut c = MacroContext::new();
        register_builtins(&mut c);
        c
    }

    #[test]
    fn test_substitute() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "World".into());
        assert_eq!(substitute("Hello {{name}}!", &vars), "Hello World!");
    }

    #[test]
    fn test_substitute_positional() {
        assert_eq!(
            substitute_positional("$1 likes $2", &["Alice", "Rust"]),
            "Alice likes Rust"
        );
    }

    #[test]
    fn test_replace_step() {
        let ctx = ctx();
        let result = execute("foo bar", &[MacroStep::Replace {
            pattern: "foo".into(),
            replacement: "baz".into(),
        }], &ctx);
        assert_eq!(result.output, "baz bar");
    }

    #[test]
    fn test_prepend_append() {
        let ctx = ctx();
        let result = execute(
            "hello",
            &[MacroStep::Prepend("[".into()), MacroStep::Append("]".into())],
            &ctx,
        );
        assert_eq!(result.output, "[hello]");
    }

    #[test]
    fn test_upper_lower() {
        let ctx = ctx();
        assert_eq!(execute("hello", &[MacroStep::Upper], &ctx).output, "HELLO");
        assert_eq!(execute("HELLO", &[MacroStep::Lower], &ctx).output, "hello");
    }

    #[test]
    fn test_trim() {
        let ctx = ctx();
        assert_eq!(execute("  hi  ", &[MacroStep::Trim], &ctx).output, "hi");
    }

    #[test]
    fn test_delete() {
        let ctx = ctx();
        assert_eq!(
            execute("abcXdef", &[MacroStep::Delete("X".into())], &ctx).output,
            "abcdef"
        );
    }

    #[test]
    fn test_insert_at() {
        let ctx = ctx();
        let result = execute(
            "hello world",
            &[MacroStep::InsertAt {
                offset: 5,
                text: ",".into(),
            }],
            &ctx,
        );
        assert_eq!(result.output, "hello, world");
    }

    #[test]
    fn test_wrap_lines() {
        let ctx = ctx();
        let result = execute(
            "a\nb",
            &[MacroStep::WrapLines {
                prefix: "> ".into(),
                suffix: " <".into(),
            }],
            &ctx,
        );
        assert_eq!(result.output, "> a <\n> b <");
    }

    #[test]
    fn test_call_builtin_quote() {
        let ctx = ctx();
        let result = execute("hello", &[MacroStep::Call("quote".into())], &ctx);
        assert_eq!(result.output, "\"hello\"");
    }

    #[test]
    fn test_run_macro() {
        let ctx = ctx();
        let result = run_macro("  Hello World  ", "slugify", &ctx).unwrap();
        assert_eq!(result.output, "hello-world");
    }

    #[test]
    fn test_conditional_if() {
        let ctx = ctx();
        let steps = vec![MacroStep::If {
            needle: "error".into(),
            steps: vec![MacroStep::Upper],
            otherwise: vec![MacroStep::Lower],
        }];
        assert_eq!(execute("an error occurred", &steps, &ctx).output, "AN ERROR OCCURRED");
        assert_eq!(execute("all good", &steps, &ctx).output, "all good");
    }

    #[test]
    fn test_recorder() {
        let mut rec = MacroRecorder::new();
        rec.start();
        assert!(rec.is_recording());
        rec.record(MacroStep::Upper);
        rec.record(MacroStep::Trim);
        rec.stop();
        assert!(!rec.is_recording());
        assert_eq!(rec.steps().len(), 2);
        let m = rec.to_macro("test", "test macro");
        assert_eq!(m.name, "test");
    }

    #[test]
    fn test_context_list_macros() {
        let ctx = ctx();
        let names = ctx.list_macros();
        assert!(names.contains(&"quote"));
        assert!(names.contains(&"slugify"));
        assert!(names.contains(&"comment"));
    }

    #[test]
    fn test_builtin_comment() {
        let ctx = ctx();
        let result = run_macro("fn main() {}\nreturn 0;", "comment", &ctx).unwrap();
        assert_eq!(result.output, "// fn main() {}\n// return 0;");
    }

    #[test]
    fn test_variable_in_steps() {
        let mut ctx = ctx();
        ctx.set_var("old", "world");
        ctx.set_var("new", "Rust");
        let result = execute(
            "hello world",
            &[MacroStep::Replace {
                pattern: "{{old}}".into(),
                replacement: "{{new}}".into(),
            }],
            &ctx,
        );
        assert_eq!(result.output, "hello Rust");
    }
}
