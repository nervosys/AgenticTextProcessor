//! AQL — ATP Query Language
//!
//! A unified, human-and-agent-friendly query language for text processing
//! that replaces the fragmented regex-based syntaxes of grep, sed, and awk.
//!
//! # Design Principles
//!
//! 1. **Readable**: English keyword-based verbs, not cryptic single-letter flags
//! 2. **Unambiguous**: No delimiter-dependent parsing, no flag overloading
//! 3. **Composable**: Pipeline stages with `|`, same syntax throughout
//! 4. **Regex-optional**: Literal `"strings"` by default, regex opt-in with `/pattern/`
//! 5. **Error-precise**: Parser rejects invalid queries with clear messages
//! 6. **Round-trippable**: Every AST can be explained in natural language
//!
//! # Quick Reference
//!
//! ```text
//! find "hello"                              # literal search
//! find /fn\s+\w+/                           # regex search
//! find "error" ignore_case                  # case-insensitive
//! replace "old" with "new" all              # global replace
//! find "TODO" | count                       # pipeline
//! select fields 1, 3 | sort by field 1     # field processing
//! filter field 2 > 100                      # numeric filter
//! delete lines matching "^#"               # delete matching lines
//! ```

use crate::engine::pipeline::{PipelineData, PipelineLine};
use crate::output::{PipelineResults, PipelineStageResult};
use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::Instant;

// ─── AST Types ──────────────────────────────────────────────────────────────

/// A complete AQL pipeline: one or more stages separated by `|`.
#[derive(Debug, Clone)]
pub struct AqlPipeline {
    pub stages: Vec<AqlStage>,
}

/// A single pipeline stage.
#[derive(Debug, Clone)]
pub enum AqlStage {
    /// Search for lines matching a pattern (grep equivalent)
    Find {
        pattern: AqlPattern,
        modifiers: Vec<AqlModifier>,
    },
    /// Replace matched text with new text (sed s/// equivalent)
    Replace {
        pattern: AqlPattern,
        replacement: String,
        modifiers: Vec<AqlModifier>,
    },
    /// Delete lines matching a pattern (sed d equivalent)
    Delete { pattern: AqlPattern },
    /// Insert text before lines matching a pattern
    InsertBefore { text: String, anchor: AqlPattern },
    /// Insert text after lines matching a pattern
    InsertAfter { text: String, anchor: AqlPattern },
    /// Select specific fields from each line (awk print $N equivalent)
    Select { fields: Vec<usize> },
    /// Filter lines by a condition on fields
    Filter { condition: AqlCondition },
    /// Sort lines
    Sort {
        by_field: Option<usize>,
        descending: bool,
        numeric: bool,
    },
    /// Deduplicate lines
    Unique { by_field: Option<usize> },
    /// Take first N lines
    Take(usize),
    /// Skip first N lines
    Skip(usize),
    /// Take last N lines
    Last(usize),
    /// Count lines (optionally matching a pattern)
    Count { pattern: Option<AqlPattern> },
    /// Compute an aggregation
    Aggregate { function: AqlAggFunc },
    /// Set the field separator for subsequent stages
    SetSeparator(String),
    /// Assign a value to a variable: `let x = "value"` or `let x = count`
    Let { name: String, value: AqlExpr },
    /// Conditional: `if field 1 > 10 then take 5 else take 10 end`
    IfElse {
        condition: AqlCondition,
        then_stages: Vec<AqlStage>,
        else_stages: Vec<AqlStage>,
    },
    /// Group by a field and aggregate: `group by field 1 aggregate count, sum 2`
    GroupBy {
        field: usize,
        aggregates: Vec<AqlAggFunc>,
    },
    /// Define a reusable function: `def my_func = find "x" | count`
    Define { name: String, body: Vec<AqlStage> },
    /// Call a user-defined function: `call my_func`
    Call { name: String },
}

/// Pattern types — explicit about literal vs regex.
#[derive(Debug, Clone)]
pub enum AqlPattern {
    /// Literal string match: `"hello"`
    Literal(String),
    /// Regex match: `/pattern/`
    Regex(String),
}

/// Modifiers that scope or alter stage behavior.
#[derive(Debug, Clone)]
pub enum AqlModifier {
    /// Case-insensitive matching. Keywords: `ignore_case`, `nocase`
    CaseInsensitive,
    /// Match whole words only. Keywords: `whole_word`, `word`
    WholeWord,
    /// Invert match. Keywords: `invert`, `not`
    Invert,
    /// Apply globally (all occurrences). Keywords: `all`, `global`
    Global,
    /// Multiline matching. Keyword: `multiline`
    Multiline,
    /// Context lines around matches. Keyword: `context N`
    Context(usize),
    /// Maximum number of results. Keyword: `max N`
    Max(usize),
    /// Show only matched text. Keyword: `only_match`
    OnlyMatching,
    /// Restrict to files matching a glob. Keyword: `in "*.rs"`
    InFiles(String),
    /// Apply only between lines matching two patterns. Keyword: `between "start" and "end"`
    Between(AqlPattern, AqlPattern),
    /// Apply only within a line range. Keyword: `in lines N to M`
    InLines(usize, usize),
}

/// Conditions for filter stages.
#[derive(Debug, Clone)]
pub enum AqlCondition {
    FieldContains(usize, String),
    FieldEquals(usize, String),
    FieldNotEquals(usize, String),
    FieldMatches(usize, String),
    FieldGreater(usize, f64),
    FieldLess(usize, f64),
    FieldNotEmpty(usize),
    LineContains(String),
    LineMatches(String),
    And(Box<AqlCondition>, Box<AqlCondition>),
    Or(Box<AqlCondition>, Box<AqlCondition>),
}

/// Aggregation functions.
#[derive(Debug, Clone)]
pub enum AqlAggFunc {
    Count,
    Sum(usize),
    Avg(usize),
    Min(usize),
    Max(usize),
    Distinct(usize),
    Freq(usize),
}

/// Expression for variable assignments.
#[derive(Debug, Clone)]
pub enum AqlExpr {
    /// A literal string value
    StringLiteral(String),
    /// An integer value
    IntLiteral(usize),
    /// A float value
    FloatLiteral(f64),
    /// Reference a variable by name
    Variable(String),
    /// The result of a pipeline (inline sub-pipeline)
    Pipeline(Vec<AqlStage>),
}

// ─── Stage Description ──────────────────────────────────────────────────────

impl AqlStage {
    /// Machine-readable stage type name.
    pub fn stage_name(&self) -> &str {
        match self {
            AqlStage::Find { .. } => "Find",
            AqlStage::Replace { .. } => "Replace",
            AqlStage::Delete { .. } => "Delete",
            AqlStage::InsertBefore { .. } => "InsertBefore",
            AqlStage::InsertAfter { .. } => "InsertAfter",
            AqlStage::Select { .. } => "Select",
            AqlStage::Filter { .. } => "Filter",
            AqlStage::Sort { .. } => "Sort",
            AqlStage::Unique { .. } => "Unique",
            AqlStage::Take(_) => "Take",
            AqlStage::Skip(_) => "Skip",
            AqlStage::Last(_) => "Last",
            AqlStage::Count { .. } => "Count",
            AqlStage::Aggregate { .. } => "Aggregate",
            AqlStage::SetSeparator(_) => "SetSeparator",
            AqlStage::Let { .. } => "Let",
            AqlStage::IfElse { .. } => "IfElse",
            AqlStage::GroupBy { .. } => "GroupBy",
            AqlStage::Define { .. } => "Define",
            AqlStage::Call { .. } => "Call",
        }
    }

    /// Human-readable description of what this stage does.
    pub fn describe(&self) -> String {
        match self {
            AqlStage::Find { pattern, modifiers } => {
                let mut desc = format!("Search for {}", describe_pattern(pattern));
                append_modifier_desc(modifiers, &mut desc);
                desc
            }
            AqlStage::Replace {
                pattern,
                replacement,
                modifiers,
            } => {
                let mut desc = format!(
                    "Replace {} with \"{}\"",
                    describe_pattern(pattern),
                    replacement
                );
                append_modifier_desc(modifiers, &mut desc);
                desc
            }
            AqlStage::Delete { pattern } => {
                format!("Delete lines matching {}", describe_pattern(pattern))
            }
            AqlStage::InsertBefore { text, anchor } => {
                format!(
                    "Insert \"{}\" before lines matching {}",
                    text,
                    describe_pattern(anchor)
                )
            }
            AqlStage::InsertAfter { text, anchor } => {
                format!(
                    "Insert \"{}\" after lines matching {}",
                    text,
                    describe_pattern(anchor)
                )
            }
            AqlStage::Select { fields } => {
                let f: Vec<String> = fields.iter().map(|f| f.to_string()).collect();
                format!("Select fields {}", f.join(", "))
            }
            AqlStage::Filter { condition } => {
                format!("Filter where {}", describe_condition(condition))
            }
            AqlStage::Sort {
                by_field,
                descending,
                numeric,
            } => {
                let field = by_field
                    .map(|f| format!("field {f}"))
                    .unwrap_or_else(|| "line content".to_string());
                let dir = if *descending {
                    "descending"
                } else {
                    "ascending"
                };
                let num = if *numeric { " (numeric)" } else { "" };
                format!("Sort by {field} {dir}{num}")
            }
            AqlStage::Unique { by_field } => {
                let field = by_field
                    .map(|f| format!("field {f}"))
                    .unwrap_or_else(|| "line content".to_string());
                format!("Deduplicate by {field}")
            }
            AqlStage::Take(n) => format!("Take first {n} records"),
            AqlStage::Skip(n) => format!("Skip first {n} records"),
            AqlStage::Last(n) => format!("Take last {n} records"),
            AqlStage::Count { pattern } => match pattern {
                Some(p) => format!("Count lines matching {}", describe_pattern(p)),
                None => "Count all lines".to_string(),
            },
            AqlStage::Aggregate { function } => {
                format!("Aggregate: {}", describe_agg(function))
            }
            AqlStage::SetSeparator(sep) => {
                format!("Set field separator to \"{}\"", sep)
            }
            AqlStage::Let { name, value } => {
                format!("Assign variable \"{name}\" = {}", describe_expr(value))
            }
            AqlStage::IfElse {
                condition,
                then_stages,
                else_stages,
            } => {
                let then_desc: Vec<String> = then_stages.iter().map(|s| s.describe()).collect();
                let else_desc: Vec<String> = else_stages.iter().map(|s| s.describe()).collect();
                if else_stages.is_empty() {
                    format!(
                        "If {} then [{}]",
                        describe_condition(condition),
                        then_desc.join(" | ")
                    )
                } else {
                    format!(
                        "If {} then [{}] else [{}]",
                        describe_condition(condition),
                        then_desc.join(" | "),
                        else_desc.join(" | ")
                    )
                }
            }
            AqlStage::GroupBy { field, aggregates } => {
                let aggs: Vec<String> = aggregates.iter().map(describe_agg).collect();
                format!("Group by field {field}, aggregate: {}", aggs.join(", "))
            }
            AqlStage::Define { name, body } => {
                let body_desc: Vec<String> = body.iter().map(|s| s.describe()).collect();
                format!("Define function \"{name}\" = [{}]", body_desc.join(" | "))
            }
            AqlStage::Call { name } => {
                format!("Call function \"{name}\"")
            }
        }
    }
}

impl AqlPipeline {
    /// Generate a human-readable explanation of every stage.
    pub fn explain(&self) -> Vec<String> {
        self.stages
            .iter()
            .enumerate()
            .map(|(i, s)| format!("Stage {}: {}", i + 1, s.describe()))
            .collect()
    }
}

fn describe_pattern(p: &AqlPattern) -> String {
    match p {
        AqlPattern::Literal(s) => format!("\"{}\" (literal)", s),
        AqlPattern::Regex(r) => format!("/{r}/ (regex)"),
    }
}

fn describe_condition(c: &AqlCondition) -> String {
    match c {
        AqlCondition::FieldContains(i, v) => format!("field {i} contains \"{v}\""),
        AqlCondition::FieldEquals(i, v) => format!("field {i} equals \"{v}\""),
        AqlCondition::FieldNotEquals(i, v) => format!("field {i} != \"{v}\""),
        AqlCondition::FieldMatches(i, p) => format!("field {i} matches /{p}/"),
        AqlCondition::FieldGreater(i, v) => format!("field {i} > {v}"),
        AqlCondition::FieldLess(i, v) => format!("field {i} < {v}"),
        AqlCondition::FieldNotEmpty(i) => format!("field {i} is not empty"),
        AqlCondition::LineContains(v) => format!("line contains \"{v}\""),
        AqlCondition::LineMatches(p) => format!("line matches /{p}/"),
        AqlCondition::And(a, b) => {
            format!("{} and {}", describe_condition(a), describe_condition(b))
        }
        AqlCondition::Or(a, b) => {
            format!("{} or {}", describe_condition(a), describe_condition(b))
        }
    }
}

fn describe_agg(f: &AqlAggFunc) -> String {
    match f {
        AqlAggFunc::Count => "count".to_string(),
        AqlAggFunc::Sum(i) => format!("sum of field {i}"),
        AqlAggFunc::Avg(i) => format!("average of field {i}"),
        AqlAggFunc::Min(i) => format!("minimum of field {i}"),
        AqlAggFunc::Max(i) => format!("maximum of field {i}"),
        AqlAggFunc::Distinct(i) => format!("distinct values of field {i}"),
        AqlAggFunc::Freq(i) => format!("frequency of field {i}"),
    }
}

fn describe_expr(e: &AqlExpr) -> String {
    match e {
        AqlExpr::StringLiteral(s) => format!("\"{s}\""),
        AqlExpr::IntLiteral(n) => n.to_string(),
        AqlExpr::FloatLiteral(f) => f.to_string(),
        AqlExpr::Variable(v) => format!("${v}"),
        AqlExpr::Pipeline(stages) => {
            let descs: Vec<String> = stages.iter().map(|s| s.describe()).collect();
            format!("[{}]", descs.join(" | "))
        }
    }
}

fn append_modifier_desc(modifiers: &[AqlModifier], desc: &mut String) {
    for m in modifiers {
        match m {
            AqlModifier::CaseInsensitive => desc.push_str(", case-insensitive"),
            AqlModifier::WholeWord => desc.push_str(", whole word"),
            AqlModifier::Invert => desc.push_str(", inverted"),
            AqlModifier::Global => desc.push_str(", all occurrences"),
            AqlModifier::Multiline => desc.push_str(", multiline"),
            AqlModifier::Context(n) => desc.push_str(&format!(", {n} context lines")),
            AqlModifier::Max(n) => desc.push_str(&format!(", max {n} results")),
            AqlModifier::OnlyMatching => desc.push_str(", only matched text"),
            AqlModifier::InFiles(g) => desc.push_str(&format!(", in files \"{g}\"")),
            AqlModifier::Between(s, e) => desc.push_str(&format!(
                ", between {} and {}",
                describe_pattern(s),
                describe_pattern(e)
            )),
            AqlModifier::InLines(s, e) => desc.push_str(&format!(", lines {s}–{e}")),
        }
    }
}

// ─── Token Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Keyword(Kw),
    StringLit(String),
    RegexLit(String),
    Identifier(String),
    Integer(usize),
    Float(f64),
    Pipe,
    Comma,
    Greater,
    Less,
    NotEqual,
    Assign,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kw {
    // Stage verbs
    Find,
    Replace,
    With,
    Delete,
    Lines,
    Matching,
    Insert,
    Before,
    After,
    Select,
    Fields,
    Filter,
    Sort,
    By,
    Field,
    Line,
    Asc,
    Desc,
    Numeric,
    Unique,
    Take,
    First,
    Skip,
    Last,
    Count,
    Aggregate,
    // Aggregation functions
    Sum,
    Avg,
    Min,
    Max,
    Distinct,
    Freq,
    // Configuration
    Set,
    Separator,
    // Modifiers
    In,
    All, // also: global, every
    IgnoreCase,
    WholeWord,
    Invert,
    Multiline,
    Context,
    OnlyMatch,
    Between,
    // Logical
    And,
    Or,
    To,
    // Comparison operators (keyword form)
    Contains,
    Equals,
    Matches,
    NotEmpty,
    // AQL v2: variables, control flow, grouping, functions
    Let,
    If,
    Then,
    Else,
    End,
    Group,
    Def,
    Call,
}

impl Kw {
    fn from_word(s: &str) -> Option<Kw> {
        match s {
            "find" | "search" => Some(Kw::Find),
            "replace" => Some(Kw::Replace),
            "with" => Some(Kw::With),
            "delete" | "remove" => Some(Kw::Delete),
            "lines" => Some(Kw::Lines),
            "matching" => Some(Kw::Matching),
            "insert" | "add" => Some(Kw::Insert),
            "before" => Some(Kw::Before),
            "after" => Some(Kw::After),
            "select" | "pick" => Some(Kw::Select),
            "fields" => Some(Kw::Fields),
            "filter" | "where" => Some(Kw::Filter),
            "sort" | "order" => Some(Kw::Sort),
            "by" => Some(Kw::By),
            "field" => Some(Kw::Field),
            "line" => Some(Kw::Line),
            "asc" | "ascending" => Some(Kw::Asc),
            "desc" | "descending" => Some(Kw::Desc),
            "numeric" => Some(Kw::Numeric),
            "unique" | "uniq" | "deduplicate" | "dedup" => Some(Kw::Unique),
            "take" | "head" => Some(Kw::Take),
            "first" => Some(Kw::First),
            "skip" => Some(Kw::Skip),
            "last" | "tail" => Some(Kw::Last),
            "count" => Some(Kw::Count),
            "aggregate" | "agg" => Some(Kw::Aggregate),
            "sum" => Some(Kw::Sum),
            "avg" | "average" | "mean" => Some(Kw::Avg),
            "min" | "minimum" => Some(Kw::Min),
            "max" | "maximum" => Some(Kw::Max),
            "distinct" => Some(Kw::Distinct),
            "freq" | "frequency" => Some(Kw::Freq),
            "set" => Some(Kw::Set),
            "separator" | "sep" | "delim" | "delimiter" => Some(Kw::Separator),
            "in" => Some(Kw::In),
            "all" | "global" | "every" => Some(Kw::All),
            "ignore_case" | "ignorecase" | "nocase" | "case_insensitive" => Some(Kw::IgnoreCase),
            "whole_word" | "wholeword" | "word" => Some(Kw::WholeWord),
            "invert" | "not" => Some(Kw::Invert),
            "multiline" => Some(Kw::Multiline),
            "context" | "ctx" => Some(Kw::Context),
            "only_match" | "onlymatch" | "only_matching" => Some(Kw::OnlyMatch),
            "between" => Some(Kw::Between),
            "and" => Some(Kw::And),
            "or" => Some(Kw::Or),
            "to" => Some(Kw::To),
            "contains" | "has" => Some(Kw::Contains),
            "equals" | "eq" | "is" => Some(Kw::Equals),
            "matches" => Some(Kw::Matches),
            "not_empty" | "notempty" | "present" => Some(Kw::NotEmpty),
            "let" => Some(Kw::Let),
            "if" => Some(Kw::If),
            "then" => Some(Kw::Then),
            "else" => Some(Kw::Else),
            "end" => Some(Kw::End),
            "group" | "groupby" | "group_by" => Some(Kw::Group),
            "def" | "define" | "fn" | "func" => Some(Kw::Def),
            "call" | "invoke" => Some(Kw::Call),
            _ => None,
        }
    }
}

// ─── Tokenizer ──────────────────────────────────────────────────────────────

struct Tokenizer {
    input: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                tokens.push(Token::Eof);
                break;
            }
            let ch = self.input[self.pos];
            match ch {
                '|' => {
                    tokens.push(Token::Pipe);
                    self.pos += 1;
                }
                ',' => {
                    tokens.push(Token::Comma);
                    self.pos += 1;
                }
                '>' => {
                    tokens.push(Token::Greater);
                    self.pos += 1;
                }
                '<' => {
                    tokens.push(Token::Less);
                    self.pos += 1;
                }
                '!' => {
                    if self.peek_at(1) == Some('=') {
                        tokens.push(Token::NotEqual);
                        self.pos += 2;
                    } else {
                        anyhow::bail!(
                            "Unexpected '!' at position {}. Did you mean '!='?",
                            self.pos
                        );
                    }
                }
                '=' => {
                    tokens.push(Token::Assign);
                    self.pos += 1;
                }
                '"' | '\'' => {
                    let s = self.read_string(ch)?;
                    tokens.push(Token::StringLit(s));
                }
                '/' => {
                    let r = self.read_regex()?;
                    tokens.push(Token::RegexLit(r));
                }
                c if c.is_ascii_digit() => {
                    let tok = self.read_number()?;
                    tokens.push(tok);
                }
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let word = self.read_word();
                    let lower = word.to_lowercase();
                    if let Some(kw) = Kw::from_word(&lower) {
                        tokens.push(Token::Keyword(kw));
                    } else {
                        // Unrecognized word: identifier (for variables / function names)
                        tokens.push(Token::Identifier(word));
                    }
                }
                _ => {
                    anyhow::bail!("Unexpected character '{}' at position {}", ch, self.pos);
                }
            }
        }
        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn read_string(&mut self, quote: char) -> Result<String> {
        self.pos += 1; // skip opening quote
        let mut s = String::new();
        loop {
            if self.pos >= self.input.len() {
                anyhow::bail!("Unterminated string literal (missing closing {quote})");
            }
            let ch = self.input[self.pos];
            if ch == quote {
                self.pos += 1;
                return Ok(s);
            }
            if ch == '\\' {
                self.pos += 1;
                if self.pos >= self.input.len() {
                    anyhow::bail!("Unterminated escape in string literal");
                }
                match self.input[self.pos] {
                    '"' => s.push('"'),
                    '\'' => s.push('\''),
                    '\\' => s.push('\\'),
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    other => {
                        s.push('\\');
                        s.push(other);
                    }
                }
                self.pos += 1;
            } else {
                s.push(ch);
                self.pos += 1;
            }
        }
    }

    fn read_regex(&mut self) -> Result<String> {
        self.pos += 1; // skip opening /
        let mut s = String::new();
        loop {
            if self.pos >= self.input.len() {
                anyhow::bail!("Unterminated regex literal (missing closing '/')");
            }
            let ch = self.input[self.pos];
            if ch == '/' {
                self.pos += 1;
                return Ok(s);
            }
            if ch == '\\' {
                self.pos += 1;
                if self.pos >= self.input.len() {
                    anyhow::bail!("Unterminated escape in regex literal");
                }
                s.push('\\');
                s.push(self.input[self.pos]);
                self.pos += 1;
            } else {
                s.push(ch);
                self.pos += 1;
            }
        }
    }

    fn read_number(&mut self) -> Result<Token> {
        let start = self.pos;
        let mut has_dot = false;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        let num_str: String = self.input[start..self.pos].iter().collect();
        if has_dot {
            let f: f64 = num_str
                .parse()
                .with_context(|| format!("Invalid number: {num_str}"))?;
            Ok(Token::Float(f))
        } else {
            let n: usize = num_str
                .parse()
                .with_context(|| format!("Invalid integer: {num_str}"))?;
            Ok(Token::Integer(n))
        }
    }

    fn read_word(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.input[start..self.pos].iter().collect()
    }
}

// ─── Parser ─────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect_keyword(&mut self, kw: Kw) -> Result<()> {
        match self.advance() {
            Token::Keyword(k) if k == kw => Ok(()),
            other => anyhow::bail!("Expected '{:?}', got {:?}", kw, other),
        }
    }

    fn try_keyword(&mut self, kw: Kw) -> bool {
        if matches!(self.peek(), Token::Keyword(k) if *k == kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_string(&mut self) -> Result<String> {
        match self.advance() {
            Token::StringLit(s) => Ok(s),
            other => anyhow::bail!("Expected quoted string, got {:?}", other),
        }
    }

    fn expect_integer(&mut self) -> Result<usize> {
        match self.advance() {
            Token::Integer(n) => Ok(n),
            Token::Float(f) => Ok(f as usize),
            other => anyhow::bail!("Expected integer, got {:?}", other),
        }
    }

    fn expect_number(&mut self) -> Result<f64> {
        match self.advance() {
            Token::Integer(n) => Ok(n as f64),
            Token::Float(f) => Ok(f),
            Token::StringLit(s) => s
                .parse::<f64>()
                .with_context(|| format!("Expected number, got string \"{s}\"")),
            other => anyhow::bail!("Expected number, got {:?}", other),
        }
    }

    // ── Pipeline ────────────────────────────────────────────────────────

    fn parse_pipeline(&mut self) -> Result<AqlPipeline> {
        let mut stages = vec![self.parse_stage()?];
        while matches!(self.peek(), Token::Pipe) {
            self.advance(); // consume |
                            // Stop pipeline at Eof, else, end (for sub-pipelines in if/def)
            if matches!(
                self.peek(),
                Token::Eof | Token::Keyword(Kw::Else) | Token::Keyword(Kw::End)
            ) {
                break;
            }
            stages.push(self.parse_stage()?);
        }
        Ok(AqlPipeline { stages })
    }

    /// Parse stages until hitting Eof, Else, End, or Pipe-followed-by-end-marker.
    fn parse_sub_pipeline(&mut self) -> Result<Vec<AqlStage>> {
        let mut stages = vec![self.parse_stage()?];
        while matches!(self.peek(), Token::Pipe) {
            // Check if next after pipe is a terminator
            if self.pos + 1 < self.tokens.len() {
                match &self.tokens[self.pos + 1] {
                    Token::Keyword(Kw::Else) | Token::Keyword(Kw::End) | Token::Eof => break,
                    _ => {}
                }
            }
            self.advance(); // consume |
            stages.push(self.parse_stage()?);
        }
        Ok(stages)
    }

    fn parse_stage(&mut self) -> Result<AqlStage> {
        match self.peek().clone() {
            Token::Keyword(Kw::Find) => self.parse_find(),
            Token::Keyword(Kw::Replace) => self.parse_replace(),
            Token::Keyword(Kw::Delete) => self.parse_delete(),
            Token::Keyword(Kw::Insert) => self.parse_insert(),
            Token::Keyword(Kw::Select) => self.parse_select(),
            Token::Keyword(Kw::Filter) => self.parse_filter(),
            Token::Keyword(Kw::Sort) => self.parse_sort(),
            Token::Keyword(Kw::Unique) => self.parse_unique(),
            Token::Keyword(Kw::Take) | Token::Keyword(Kw::First) => self.parse_take(),
            Token::Keyword(Kw::Skip) => self.parse_skip(),
            Token::Keyword(Kw::Last) => self.parse_last(),
            Token::Keyword(Kw::Count) => self.parse_count(),
            Token::Keyword(Kw::Aggregate) => self.parse_aggregate(),
            Token::Keyword(Kw::Set) => self.parse_set(),
            Token::Keyword(Kw::Let) => self.parse_let(),
            Token::Keyword(Kw::If) => self.parse_if(),
            Token::Keyword(Kw::Group) => self.parse_group_by(),
            Token::Keyword(Kw::Def) => self.parse_def(),
            Token::Keyword(Kw::Call) => self.parse_call(),
            other => anyhow::bail!(
                "Expected stage keyword (find, replace, delete, insert, select, \
                 filter, sort, unique, take, skip, last, count, aggregate, set, \
                 let, if, group, def, call), got {:?}",
                other
            ),
        }
    }

    // ── Pattern ─────────────────────────────────────────────────────────

    fn parse_pattern(&mut self) -> Result<AqlPattern> {
        match self.advance() {
            Token::StringLit(s) => Ok(AqlPattern::Literal(s)),
            Token::RegexLit(r) => Ok(AqlPattern::Regex(r)),
            other => anyhow::bail!("Expected pattern (\"string\" or /regex/), got {:?}", other),
        }
    }

    // ── Stages ──────────────────────────────────────────────────────────

    fn parse_find(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'find'
        let pattern = self.parse_pattern()?;
        let modifiers = self.parse_modifiers()?;
        Ok(AqlStage::Find { pattern, modifiers })
    }

    fn parse_replace(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'replace'
        let pattern = self.parse_pattern()?;
        self.expect_keyword(Kw::With)?;
        let replacement = self.expect_string()?;
        let modifiers = self.parse_modifiers()?;
        Ok(AqlStage::Replace {
            pattern,
            replacement,
            modifiers,
        })
    }

    fn parse_delete(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'delete'
                        // Optional syntactic sugar: 'lines' 'matching'
        self.try_keyword(Kw::Lines);
        self.try_keyword(Kw::Matching);
        let pattern = self.parse_pattern()?;
        Ok(AqlStage::Delete { pattern })
    }

    fn parse_insert(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'insert'
        let text = self.expect_string()?;
        match self.advance() {
            Token::Keyword(Kw::Before) => {
                let anchor = self.parse_pattern()?;
                Ok(AqlStage::InsertBefore { text, anchor })
            }
            Token::Keyword(Kw::After) => {
                let anchor = self.parse_pattern()?;
                Ok(AqlStage::InsertAfter { text, anchor })
            }
            other => anyhow::bail!(
                "Expected 'before' or 'after' in insert stage, got {:?}",
                other
            ),
        }
    }

    fn parse_select(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'select'
        self.try_keyword(Kw::Fields); // optional 'fields'
        let mut fields = vec![self.expect_integer()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            fields.push(self.expect_integer()?);
        }
        Ok(AqlStage::Select { fields })
    }

    fn parse_filter(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'filter'
        let condition = self.parse_condition()?;
        Ok(AqlStage::Filter { condition })
    }

    fn parse_sort(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'sort'
        let mut by_field = None;
        let mut descending = false;
        let mut numeric = false;

        if self.try_keyword(Kw::By) {
            if self.try_keyword(Kw::Field) {
                by_field = Some(self.expect_integer()?);
            } else {
                self.try_keyword(Kw::Line); // 'by line' = sort by full line content
            }
        }
        if self.try_keyword(Kw::Desc) {
            descending = true;
        } else {
            self.try_keyword(Kw::Asc);
        }
        if self.try_keyword(Kw::Numeric) {
            numeric = true;
        }
        Ok(AqlStage::Sort {
            by_field,
            descending,
            numeric,
        })
    }

    fn parse_unique(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'unique'
        let mut by_field = None;
        if self.try_keyword(Kw::By) {
            self.expect_keyword(Kw::Field)?;
            by_field = Some(self.expect_integer()?);
        }
        Ok(AqlStage::Unique { by_field })
    }

    fn parse_take(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'take' or 'first'
        let n = self.expect_integer()?;
        Ok(AqlStage::Take(n))
    }

    fn parse_skip(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'skip'
        let n = self.expect_integer()?;
        Ok(AqlStage::Skip(n))
    }

    fn parse_last(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'last'
        let n = self.expect_integer()?;
        Ok(AqlStage::Last(n))
    }

    fn parse_count(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'count'
        let pattern = if matches!(self.peek(), Token::StringLit(_) | Token::RegexLit(_)) {
            Some(self.parse_pattern()?)
        } else {
            None
        };
        Ok(AqlStage::Count { pattern })
    }

    fn parse_aggregate(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'aggregate'
        let function = self.parse_agg_func()?;
        Ok(AqlStage::Aggregate { function })
    }

    fn parse_set(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'set'
        self.expect_keyword(Kw::Separator)?;
        let sep = self.expect_string()?;
        Ok(AqlStage::SetSeparator(sep))
    }

    // ── AQL v2: Variables, Control Flow, Grouping, Functions ────────────

    /// Parse `let <name> = <expr>`.
    /// Expr can be a string, integer, float, or identifier (variable ref).
    fn parse_let(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'let'
        let name = match self.advance() {
            Token::Identifier(s) => s,
            Token::StringLit(s) => s,
            other => anyhow::bail!("Expected variable name after 'let', got {:?}", other),
        };
        // expect '='
        match self.advance() {
            Token::Assign => {}
            other => anyhow::bail!("Expected '=' after variable name, got {:?}", other),
        }
        let value = match self.peek().clone() {
            Token::StringLit(_) => {
                if let Token::StringLit(s) = self.advance() {
                    AqlExpr::StringLiteral(s)
                } else {
                    unreachable!()
                }
            }
            Token::Integer(_) => {
                if let Token::Integer(n) = self.advance() {
                    AqlExpr::IntLiteral(n)
                } else {
                    unreachable!()
                }
            }
            Token::Float(_) => {
                if let Token::Float(f) = self.advance() {
                    AqlExpr::FloatLiteral(f)
                } else {
                    unreachable!()
                }
            }
            Token::Identifier(_) => {
                if let Token::Identifier(s) = self.advance() {
                    AqlExpr::Variable(s)
                } else {
                    unreachable!()
                }
            }
            _ => anyhow::bail!("Expected value after '=' in let statement"),
        };
        Ok(AqlStage::Let { name, value })
    }

    /// Parse `if <condition> then <stages> [else <stages>] end`.
    fn parse_if(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'if'
        let condition = self.parse_condition()?;
        self.expect_keyword(Kw::Then)?;
        let then_stages = self.parse_sub_pipeline()?;
        let else_stages = if matches!(self.peek(), Token::Keyword(Kw::Else)) {
            self.advance(); // consume 'else'
            self.parse_sub_pipeline()?
        } else {
            Vec::new()
        };
        self.expect_keyword(Kw::End)?;
        Ok(AqlStage::IfElse {
            condition,
            then_stages,
            else_stages,
        })
    }

    /// Parse `group by field <N> aggregate <func> [, <func>]*`.
    fn parse_group_by(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'group'
        self.try_keyword(Kw::By);
        self.expect_keyword(Kw::Field)?;
        let field = self.expect_integer()?;
        self.expect_keyword(Kw::Aggregate)?;
        let mut aggregates = vec![self.parse_agg_func()?];
        while matches!(self.peek(), Token::Comma) {
            self.advance(); // consume ','
            aggregates.push(self.parse_agg_func()?);
        }
        Ok(AqlStage::GroupBy { field, aggregates })
    }

    /// Parse `def <name> = <stages> end`.
    fn parse_def(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'def'
        let name = match self.advance() {
            Token::Identifier(s) => s,
            Token::StringLit(s) => s,
            other => anyhow::bail!("Expected function name after 'def', got {:?}", other),
        };
        match self.advance() {
            Token::Assign => {}
            other => anyhow::bail!("Expected '=' after function name, got {:?}", other),
        }
        let body = self.parse_sub_pipeline()?;
        self.expect_keyword(Kw::End)?;
        Ok(AqlStage::Define { name, body })
    }

    /// Parse `call <name>`.
    fn parse_call(&mut self) -> Result<AqlStage> {
        self.advance(); // consume 'call'
        let name = match self.advance() {
            Token::Identifier(s) => s,
            Token::StringLit(s) => s,
            other => anyhow::bail!("Expected function name after 'call', got {:?}", other),
        };
        Ok(AqlStage::Call { name })
    }

    // ── Aggregation Functions ───────────────────────────────────────────

    fn parse_agg_func(&mut self) -> Result<AqlAggFunc> {
        match self.advance() {
            Token::Keyword(Kw::Count) => Ok(AqlAggFunc::Count),
            Token::Keyword(Kw::Sum) => {
                self.try_keyword(Kw::Field);
                Ok(AqlAggFunc::Sum(self.expect_integer()?))
            }
            Token::Keyword(Kw::Avg) => {
                self.try_keyword(Kw::Field);
                Ok(AqlAggFunc::Avg(self.expect_integer()?))
            }
            Token::Keyword(Kw::Min) => {
                self.try_keyword(Kw::Field);
                Ok(AqlAggFunc::Min(self.expect_integer()?))
            }
            Token::Keyword(Kw::Max) => {
                self.try_keyword(Kw::Field);
                Ok(AqlAggFunc::Max(self.expect_integer()?))
            }
            Token::Keyword(Kw::Distinct) => {
                self.try_keyword(Kw::Field);
                Ok(AqlAggFunc::Distinct(self.expect_integer()?))
            }
            Token::Keyword(Kw::Freq) => {
                self.try_keyword(Kw::Field);
                Ok(AqlAggFunc::Freq(self.expect_integer()?))
            }
            other => anyhow::bail!(
                "Expected aggregation (count, sum, avg, min, max, distinct, freq), got {:?}",
                other
            ),
        }
    }

    // ── Conditions ──────────────────────────────────────────────────────

    fn parse_condition(&mut self) -> Result<AqlCondition> {
        let left = self.parse_simple_condition()?;
        match self.peek() {
            Token::Keyword(Kw::And) => {
                self.advance();
                let right = self.parse_condition()?;
                Ok(AqlCondition::And(Box::new(left), Box::new(right)))
            }
            Token::Keyword(Kw::Or) => {
                self.advance();
                let right = self.parse_condition()?;
                Ok(AqlCondition::Or(Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_simple_condition(&mut self) -> Result<AqlCondition> {
        match self.peek().clone() {
            Token::Keyword(Kw::Field) => {
                self.advance();
                let idx = self.expect_integer()?;
                self.parse_field_op(idx)
            }
            Token::Keyword(Kw::Line) => {
                self.advance();
                self.parse_line_op()
            }
            other => anyhow::bail!(
                "Expected 'field N' or 'line' to start condition, got {:?}",
                other
            ),
        }
    }

    fn parse_field_op(&mut self, idx: usize) -> Result<AqlCondition> {
        match self.advance() {
            Token::Keyword(Kw::Contains) => {
                let val = self.expect_string()?;
                Ok(AqlCondition::FieldContains(idx, val))
            }
            Token::Keyword(Kw::Equals) => {
                let val = self.expect_string()?;
                Ok(AqlCondition::FieldEquals(idx, val))
            }
            Token::Keyword(Kw::Matches) => {
                let pat = match self.advance() {
                    Token::StringLit(s) => s,
                    Token::RegexLit(r) => r,
                    other => anyhow::bail!("Expected pattern after 'matches', got {:?}", other),
                };
                Ok(AqlCondition::FieldMatches(idx, pat))
            }
            Token::Keyword(Kw::NotEmpty) => Ok(AqlCondition::FieldNotEmpty(idx)),
            Token::Greater => {
                let n = self.expect_number()?;
                Ok(AqlCondition::FieldGreater(idx, n))
            }
            Token::Less => {
                let n = self.expect_number()?;
                Ok(AqlCondition::FieldLess(idx, n))
            }
            Token::NotEqual => {
                let val = self.expect_string()?;
                Ok(AqlCondition::FieldNotEquals(idx, val))
            }
            other => anyhow::bail!(
                "Expected comparison (contains, equals, matches, >, <, !=, not_empty) \
                 after 'field {idx}', got {:?}",
                other
            ),
        }
    }

    fn parse_line_op(&mut self) -> Result<AqlCondition> {
        match self.advance() {
            Token::Keyword(Kw::Contains) => {
                let val = self.expect_string()?;
                Ok(AqlCondition::LineContains(val))
            }
            Token::Keyword(Kw::Matches) => {
                let pat = match self.advance() {
                    Token::StringLit(s) => s,
                    Token::RegexLit(r) => r,
                    other => anyhow::bail!("Expected pattern after 'matches', got {:?}", other),
                };
                Ok(AqlCondition::LineMatches(pat))
            }
            other => anyhow::bail!(
                "Expected 'contains' or 'matches' after 'line', got {:?}",
                other
            ),
        }
    }

    // ── Modifiers ───────────────────────────────────────────────────────

    fn parse_modifiers(&mut self) -> Result<Vec<AqlModifier>> {
        let mut modifiers = Vec::new();
        loop {
            match self.peek() {
                Token::Keyword(Kw::IgnoreCase) => {
                    self.advance();
                    modifiers.push(AqlModifier::CaseInsensitive);
                }
                Token::Keyword(Kw::WholeWord) => {
                    self.advance();
                    modifiers.push(AqlModifier::WholeWord);
                }
                Token::Keyword(Kw::Invert) => {
                    self.advance();
                    modifiers.push(AqlModifier::Invert);
                }
                Token::Keyword(Kw::All) => {
                    self.advance();
                    modifiers.push(AqlModifier::Global);
                }
                Token::Keyword(Kw::Multiline) => {
                    self.advance();
                    modifiers.push(AqlModifier::Multiline);
                }
                Token::Keyword(Kw::OnlyMatch) => {
                    self.advance();
                    modifiers.push(AqlModifier::OnlyMatching);
                }
                Token::Keyword(Kw::Context) => {
                    self.advance();
                    let n = self.expect_integer()?;
                    modifiers.push(AqlModifier::Context(n));
                }
                Token::Keyword(Kw::Max) => {
                    self.advance();
                    let n = self.expect_integer()?;
                    modifiers.push(AqlModifier::Max(n));
                }
                Token::Keyword(Kw::In) => {
                    self.advance();
                    if self.try_keyword(Kw::Lines) {
                        let start = self.expect_integer()?;
                        self.expect_keyword(Kw::To)?;
                        let end = self.expect_integer()?;
                        modifiers.push(AqlModifier::InLines(start, end));
                    } else if matches!(self.peek(), Token::Keyword(Kw::All)) {
                        self.advance(); // 'in all' = no filter (default)
                    } else {
                        let glob = self.expect_string()?;
                        modifiers.push(AqlModifier::InFiles(glob));
                    }
                }
                Token::Keyword(Kw::Between) => {
                    self.advance();
                    let start = self.parse_pattern()?;
                    self.expect_keyword(Kw::And)?;
                    let end = self.parse_pattern()?;
                    modifiers.push(AqlModifier::Between(start, end));
                }
                _ => break,
            }
        }
        Ok(modifiers)
    }
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Parse an AQL query string into a pipeline AST.
///
/// Returns a fully validated `AqlPipeline` ready for execution or explanation.
///
/// # Examples
///
/// ```
/// use atp_core::engine::aql;
///
/// let pipeline = aql::parse(r#"find "hello" ignore_case | count"#).unwrap();
/// assert_eq!(pipeline.stages.len(), 2);
///
/// let explanation = pipeline.explain();
/// assert!(explanation[0].contains("Search for"));
/// assert!(explanation[1].contains("Count"));
/// ```
pub fn parse(input: &str) -> Result<AqlPipeline> {
    let mut tokenizer = Tokenizer::new(input);
    let tokens = tokenizer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_pipeline()
}

// ─── Evaluator ──────────────────────────────────────────────────────────────

/// AQL execution engine.
///
/// Executes AQL pipelines against files, producing typed `PipelineResults`.
/// Bridges the AQL AST to the existing grep/sed/awk engines internally.
pub struct AqlEngine {
    /// Current field separator (changeable via `set separator` stage)
    separator: String,
    /// Variable environment for `let` assignments
    variables: HashMap<String, String>,
    /// User-defined function registry for `def`/`call`
    functions: HashMap<String, Vec<AqlStage>>,
}

impl AqlEngine {
    pub fn new() -> Self {
        Self {
            separator: r"\s+".to_string(),
            variables: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    /// Execute an AQL pipeline on the given files.
    pub fn execute(&mut self, pipeline: &AqlPipeline, paths: &[&Path]) -> Result<PipelineResults> {
        // Initialize pipeline data from files
        let mut data = PipelineData {
            lines: Vec::new(),
            source_files: paths.iter().map(|p| p.to_path_buf()).collect(),
        };

        for path in paths {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read: {}", path.display()))?;
            let file_str = path.display().to_string();
            for (idx, line) in content.lines().enumerate() {
                data.lines.push(PipelineLine {
                    source_file: file_str.clone(),
                    source_line: idx + 1,
                    content: line.to_string(),
                    fields: Vec::new(),
                });
            }
        }

        self.run_pipeline(pipeline, data)
    }

    /// Execute an AQL pipeline on pre-loaded data (e.g., from stdin).
    pub fn execute_on_data(
        &mut self,
        pipeline: &AqlPipeline,
        data: PipelineData,
    ) -> Result<PipelineResults> {
        self.run_pipeline(pipeline, data)
    }

    /// Shared pipeline execution: runs each stage sequentially on the data.
    fn run_pipeline(
        &mut self,
        pipeline: &AqlPipeline,
        mut data: PipelineData,
    ) -> Result<PipelineResults> {
        let total_start = Instant::now();
        let mut stage_results = Vec::new();

        // Execute each stage sequentially
        for (stage_idx, stage) in pipeline.stages.iter().enumerate() {
            let stage_start = Instant::now();
            let records_in = data.lines.len();

            data = self
                .execute_stage(stage, data)
                .with_context(|| format!("AQL pipeline failed at stage {}", stage_idx + 1))?;

            stage_results.push(PipelineStageResult {
                stage_index: stage_idx,
                stage_type: stage.stage_name().to_string(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
                records_in,
                records_out: data.lines.len(),
            });
        }

        let final_lines: Vec<serde_json::Value> = data
            .lines
            .iter()
            .map(|l| {
                serde_json::json!({
                    "file": l.source_file,
                    "line": l.source_line,
                    "content": l.content,
                    "fields": l.fields,
                })
            })
            .collect();

        Ok(PipelineResults {
            stages: stage_results,
            final_output: serde_json::Value::Array(final_lines),
            total_duration_ms: total_start.elapsed().as_millis() as u64,
        })
    }

    fn execute_stage(&mut self, stage: &AqlStage, mut data: PipelineData) -> Result<PipelineData> {
        match stage {
            AqlStage::Find { pattern, modifiers } => self.exec_find(pattern, modifiers, data),

            AqlStage::Replace {
                pattern,
                replacement,
                modifiers,
            } => self.exec_replace(pattern, replacement, modifiers, data),

            AqlStage::Delete { pattern } => {
                let re = self.compile_pattern(pattern, false)?;
                data.lines.retain(|line| !re.is_match(&line.content));
                Ok(data)
            }

            AqlStage::InsertBefore { text, anchor } => {
                let re = self.compile_pattern(anchor, false)?;
                let mut new_lines = Vec::new();
                for line in data.lines {
                    if re.is_match(&line.content) {
                        new_lines.push(PipelineLine {
                            source_file: line.source_file.clone(),
                            source_line: 0,
                            content: text.clone(),
                            fields: Vec::new(),
                        });
                    }
                    new_lines.push(line);
                }
                data.lines = new_lines;
                Ok(data)
            }

            AqlStage::InsertAfter { text, anchor } => {
                let re = self.compile_pattern(anchor, false)?;
                let mut new_lines = Vec::new();
                for line in data.lines {
                    let matched = re.is_match(&line.content);
                    let sf = if matched {
                        Some(line.source_file.clone())
                    } else {
                        None
                    };
                    new_lines.push(line);
                    if let Some(sf) = sf {
                        new_lines.push(PipelineLine {
                            source_file: sf,
                            source_line: 0,
                            content: text.clone(),
                            fields: Vec::new(),
                        });
                    }
                }
                data.lines = new_lines;
                Ok(data)
            }

            AqlStage::Select { fields } => {
                let sep_re = Regex::new(&self.separator)?;
                for line in &mut data.lines {
                    let all: Vec<String> =
                        sep_re.split(&line.content).map(|s| s.to_string()).collect();
                    line.fields = fields
                        .iter()
                        .map(|&i| all.get(i.saturating_sub(1)).cloned().unwrap_or_default())
                        .collect();
                    line.content = line.fields.join("\t");
                }
                Ok(data)
            }

            AqlStage::Filter { condition } => {
                let sep_re = Regex::new(&self.separator)?;
                data.lines.retain(|line| {
                    let fields: Vec<String> =
                        sep_re.split(&line.content).map(|s| s.to_string()).collect();
                    self.eval_condition(condition, &fields, &line.content)
                });
                Ok(data)
            }

            AqlStage::Sort {
                by_field,
                descending,
                numeric,
            } => {
                let sep_re = Regex::new(&self.separator)?;
                let desc = *descending;
                let num = *numeric;
                let bf = *by_field;
                data.lines.sort_by(|a, b| {
                    let a_val = Self::field_value(a, bf, &sep_re);
                    let b_val = Self::field_value(b, bf, &sep_re);
                    let cmp = if num {
                        let an = a_val.parse::<f64>().unwrap_or(0.0);
                        let bn = b_val.parse::<f64>().unwrap_or(0.0);
                        an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a_val.cmp(&b_val)
                    };
                    if desc {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                });
                Ok(data)
            }

            AqlStage::Unique { by_field } => {
                let sep_re = Regex::new(&self.separator)?;
                let bf = *by_field;
                let mut seen = HashSet::new();
                data.lines
                    .retain(|line| seen.insert(Self::field_value(line, bf, &sep_re)));
                Ok(data)
            }

            AqlStage::Take(n) => {
                data.lines.truncate(*n);
                Ok(data)
            }

            AqlStage::Skip(n) => {
                if *n < data.lines.len() {
                    data.lines = data.lines.split_off(*n);
                } else {
                    data.lines.clear();
                }
                Ok(data)
            }

            AqlStage::Last(n) => {
                let len = data.lines.len();
                if *n < len {
                    data.lines = data.lines.split_off(len - n);
                }
                Ok(data)
            }

            AqlStage::Count { pattern } => {
                let count = if let Some(pat) = pattern {
                    let re = self.compile_pattern(pat, false)?;
                    data.lines
                        .iter()
                        .filter(|l| re.is_match(&l.content))
                        .count()
                } else {
                    data.lines.len()
                };
                data.lines = vec![PipelineLine {
                    source_file: String::new(),
                    source_line: 0,
                    content: count.to_string(),
                    fields: vec![count.to_string()],
                }];
                Ok(data)
            }

            AqlStage::Aggregate { function } => {
                let result = self.exec_aggregate(function, &data)?;
                data.lines = vec![PipelineLine {
                    source_file: String::new(),
                    source_line: 0,
                    content: result,
                    fields: Vec::new(),
                }];
                Ok(data)
            }

            AqlStage::SetSeparator(sep) => {
                self.separator = sep.clone();
                Ok(data)
            }

            AqlStage::Let { name, value } => {
                let resolved = match value {
                    AqlExpr::StringLiteral(s) => s.clone(),
                    AqlExpr::IntLiteral(n) => n.to_string(),
                    AqlExpr::FloatLiteral(f) => f.to_string(),
                    AqlExpr::Variable(v) => self.variables.get(v).cloned().unwrap_or_default(),
                    AqlExpr::Pipeline(stages) => {
                        // Run the sub-pipeline on a clone of the data
                        let sub_pipeline = AqlPipeline {
                            stages: stages.clone(),
                        };
                        let sub_result = self.run_pipeline(&sub_pipeline, data.clone())?;
                        // The result is the final output as a string
                        if let Some(line) = sub_result
                            .final_output
                            .as_array()
                            .and_then(|a| a.first())
                            .and_then(|v| v.get("content"))
                            .and_then(|v| v.as_str())
                        {
                            line.to_string()
                        } else {
                            sub_result.final_output.to_string()
                        }
                    }
                };
                self.variables.insert(name.clone(), resolved);
                Ok(data)
            }

            AqlStage::IfElse {
                condition,
                then_stages,
                else_stages,
            } => {
                // Evaluate condition on the first line or a synthetic line
                let sep_re = Regex::new(&self.separator)?;
                let matches_condition = if let Some(first) = data.lines.first() {
                    let fields: Vec<String> = sep_re
                        .split(&first.content)
                        .map(|s| s.to_string())
                        .collect();
                    self.eval_condition(condition, &fields, &first.content)
                } else {
                    false
                };

                let stages = if matches_condition {
                    then_stages
                } else {
                    else_stages
                };

                for stage in stages {
                    data = self.execute_stage(stage, data)?;
                }
                Ok(data)
            }

            AqlStage::GroupBy { field, aggregates } => {
                let sep_re = Regex::new(&self.separator)?;
                let f = *field;

                // Group lines by field value
                let mut groups: HashMap<String, Vec<&PipelineLine>> = HashMap::new();
                let mut order: Vec<String> = Vec::new();
                for line in &data.lines {
                    let key = Self::field_value(line, Some(f), &sep_re);
                    if !groups.contains_key(&key) {
                        order.push(key.clone());
                    }
                    groups.entry(key).or_default().push(line);
                }

                // Compute aggregates per group
                let mut result_lines = Vec::new();
                for key in &order {
                    let group_lines = &groups[key];
                    let mut parts = vec![key.clone()];
                    for agg in aggregates {
                        let val = match agg {
                            AqlAggFunc::Count => group_lines.len().to_string(),
                            AqlAggFunc::Sum(af) => {
                                let sum: f64 = group_lines
                                    .iter()
                                    .map(|l| {
                                        Self::field_value(l, Some(*af), &sep_re)
                                            .parse::<f64>()
                                            .unwrap_or(0.0)
                                    })
                                    .sum();
                                sum.to_string()
                            }
                            AqlAggFunc::Avg(af) => {
                                let vals: Vec<f64> = group_lines
                                    .iter()
                                    .map(|l| {
                                        Self::field_value(l, Some(*af), &sep_re)
                                            .parse::<f64>()
                                            .unwrap_or(0.0)
                                    })
                                    .collect();
                                if vals.is_empty() {
                                    "0".to_string()
                                } else {
                                    let avg: f64 = vals.iter().sum::<f64>() / vals.len() as f64;
                                    avg.to_string()
                                }
                            }
                            AqlAggFunc::Min(af) => group_lines
                                .iter()
                                .map(|l| {
                                    Self::field_value(l, Some(*af), &sep_re)
                                        .parse::<f64>()
                                        .unwrap_or(f64::MAX)
                                })
                                .fold(f64::MAX, f64::min)
                                .to_string(),
                            AqlAggFunc::Max(af) => group_lines
                                .iter()
                                .map(|l| {
                                    Self::field_value(l, Some(*af), &sep_re)
                                        .parse::<f64>()
                                        .unwrap_or(f64::MIN)
                                })
                                .fold(f64::MIN, f64::max)
                                .to_string(),
                            AqlAggFunc::Distinct(af) => {
                                let unique: HashSet<String> = group_lines
                                    .iter()
                                    .map(|l| Self::field_value(l, Some(*af), &sep_re))
                                    .collect();
                                unique.len().to_string()
                            }
                            AqlAggFunc::Freq(af) => {
                                let mut freq: HashMap<String, usize> = HashMap::new();
                                for l in group_lines {
                                    *freq
                                        .entry(Self::field_value(l, Some(*af), &sep_re))
                                        .or_default() += 1;
                                }
                                let pairs: Vec<String> =
                                    freq.iter().map(|(k, v)| format!("{k}:{v}")).collect();
                                pairs.join(";")
                            }
                        };
                        parts.push(val);
                    }
                    result_lines.push(PipelineLine {
                        source_file: String::new(),
                        source_line: 0,
                        content: parts.join("\t"),
                        fields: parts,
                    });
                }
                data.lines = result_lines;
                Ok(data)
            }

            AqlStage::Define { name, body } => {
                self.functions.insert(name.clone(), body.clone());
                Ok(data)
            }

            AqlStage::Call { name } => {
                let body = self
                    .functions
                    .get(name)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Undefined function: {name}"))?;
                for stage in &body {
                    data = self.execute_stage(stage, data)?;
                }
                Ok(data)
            }
        }
    }

    // ── Find ────────────────────────────────────────────────────────────

    fn exec_find(
        &self,
        pattern: &AqlPattern,
        modifiers: &[AqlModifier],
        data: PipelineData,
    ) -> Result<PipelineData> {
        let case_insensitive = modifiers
            .iter()
            .any(|m| matches!(m, AqlModifier::CaseInsensitive));
        let invert = modifiers.iter().any(|m| matches!(m, AqlModifier::Invert));
        let whole_word = modifiers
            .iter()
            .any(|m| matches!(m, AqlModifier::WholeWord));
        let only_matching = modifiers
            .iter()
            .any(|m| matches!(m, AqlModifier::OnlyMatching));
        let max_matches = modifiers.iter().find_map(|m| {
            if let AqlModifier::Max(n) = m {
                Some(*n)
            } else {
                None
            }
        });
        let line_range = modifiers.iter().find_map(|m| {
            if let AqlModifier::InLines(s, e) = m {
                Some((*s, *e))
            } else {
                None
            }
        });

        // Build regex
        let mut regex_str = match pattern {
            AqlPattern::Literal(s) => regex::escape(s),
            AqlPattern::Regex(r) => r.clone(),
        };
        if whole_word {
            regex_str = format!(r"\b{}\b", regex_str);
        }
        let re = RegexBuilder::new(&regex_str)
            .case_insensitive(case_insensitive)
            .build()
            .with_context(|| format!("Invalid pattern: {regex_str}"))?;

        // Compute between-mask
        let between_mask = self.compute_between_mask(modifiers, &data)?;

        // Filter
        let mut result_lines = Vec::new();
        let mut match_count = 0usize;

        for (idx, line) in data.lines.into_iter().enumerate() {
            // Check between mask
            if let Some(ref mask) = between_mask {
                if !mask.get(idx).copied().unwrap_or(false) {
                    continue;
                }
            }
            // Check line range
            if let Some((start, end)) = line_range {
                if line.source_line < start || line.source_line > end {
                    continue;
                }
            }

            let matches = re.is_match(&line.content);
            let include = if invert { !matches } else { matches };

            if include {
                match_count += 1;
                if let Some(max) = max_matches {
                    if match_count > max {
                        break;
                    }
                }
                let mut out_line = line;
                if only_matching && !invert {
                    if let Some(mat) = re.find(&out_line.content) {
                        out_line.content = mat.as_str().to_string();
                    }
                }
                result_lines.push(out_line);
            }
        }

        Ok(PipelineData {
            lines: result_lines,
            source_files: Vec::new(),
        })
    }

    // ── Replace ─────────────────────────────────────────────────────────

    fn exec_replace(
        &self,
        pattern: &AqlPattern,
        replacement: &str,
        modifiers: &[AqlModifier],
        mut data: PipelineData,
    ) -> Result<PipelineData> {
        let case_insensitive = modifiers
            .iter()
            .any(|m| matches!(m, AqlModifier::CaseInsensitive));
        let global = modifiers.iter().any(|m| matches!(m, AqlModifier::Global));
        let line_range = modifiers.iter().find_map(|m| {
            if let AqlModifier::InLines(s, e) = m {
                Some((*s, *e))
            } else {
                None
            }
        });

        let re = self.compile_pattern(pattern, case_insensitive)?;
        let between_mask = self.compute_between_mask(modifiers, &data)?;

        for (idx, line) in data.lines.iter_mut().enumerate() {
            // Check scoping
            if let Some(ref mask) = between_mask {
                if !mask.get(idx).copied().unwrap_or(false) {
                    continue;
                }
            }
            if let Some((start, end)) = line_range {
                if line.source_line < start || line.source_line > end {
                    continue;
                }
            }
            // Apply replacement
            if global {
                line.content = re.replace_all(&line.content, replacement).to_string();
            } else {
                line.content = re.replace(&line.content, replacement).to_string();
            }
        }
        Ok(data)
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn compile_pattern(&self, pattern: &AqlPattern, case_insensitive: bool) -> Result<Regex> {
        let regex_str = match pattern {
            AqlPattern::Literal(s) => regex::escape(s),
            AqlPattern::Regex(r) => r.clone(),
        };
        RegexBuilder::new(&regex_str)
            .case_insensitive(case_insensitive)
            .build()
            .with_context(|| format!("Invalid pattern: {regex_str}"))
    }

    fn compute_between_mask(
        &self,
        modifiers: &[AqlModifier],
        data: &PipelineData,
    ) -> Result<Option<Vec<bool>>> {
        let between = modifiers.iter().find_map(|m| {
            if let AqlModifier::Between(s, e) = m {
                Some((s, e))
            } else {
                None
            }
        });
        let (start_pat, end_pat) = match between {
            Some((s, e)) => (s, e),
            None => return Ok(None),
        };
        let start_re = self.compile_pattern(start_pat, false)?;
        let end_re = self.compile_pattern(end_pat, false)?;
        let mut mask = vec![false; data.lines.len()];
        let mut inside = false;
        for (idx, line) in data.lines.iter().enumerate() {
            if !inside {
                if start_re.is_match(&line.content) {
                    inside = true;
                    mask[idx] = true;
                }
            } else {
                mask[idx] = true;
                if end_re.is_match(&line.content) {
                    inside = false;
                }
            }
        }
        Ok(Some(mask))
    }

    fn field_value(line: &PipelineLine, by_field: Option<usize>, sep_re: &Regex) -> String {
        match by_field {
            Some(idx) if idx > 0 => {
                let fields: Vec<&str> = sep_re.split(&line.content).collect();
                fields.get(idx.saturating_sub(1)).unwrap_or(&"").to_string()
            }
            _ => line.content.clone(),
        }
    }

    fn eval_condition(&self, cond: &AqlCondition, fields: &[String], line: &str) -> bool {
        match cond {
            AqlCondition::FieldContains(i, v) => fields
                .get(i.saturating_sub(1))
                .map(|f| f.contains(v.as_str()))
                .unwrap_or(false),
            AqlCondition::FieldEquals(i, v) => fields
                .get(i.saturating_sub(1))
                .map(|f| f == v)
                .unwrap_or(false),
            AqlCondition::FieldNotEquals(i, v) => fields
                .get(i.saturating_sub(1))
                .map(|f| f != v)
                .unwrap_or(true),
            AqlCondition::FieldMatches(i, p) => {
                if let Ok(re) = Regex::new(p) {
                    fields
                        .get(i.saturating_sub(1))
                        .map(|f| re.is_match(f))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            AqlCondition::FieldGreater(i, v) => fields
                .get(i.saturating_sub(1))
                .and_then(|f| f.parse::<f64>().ok())
                .map(|f| f > *v)
                .unwrap_or(false),
            AqlCondition::FieldLess(i, v) => fields
                .get(i.saturating_sub(1))
                .and_then(|f| f.parse::<f64>().ok())
                .map(|f| f < *v)
                .unwrap_or(false),
            AqlCondition::FieldNotEmpty(i) => fields
                .get(i.saturating_sub(1))
                .map(|f| !f.is_empty())
                .unwrap_or(false),
            AqlCondition::LineContains(v) => line.contains(v.as_str()),
            AqlCondition::LineMatches(p) => {
                if let Ok(re) = Regex::new(p) {
                    re.is_match(line)
                } else {
                    false
                }
            }
            AqlCondition::And(a, b) => {
                self.eval_condition(a, fields, line) && self.eval_condition(b, fields, line)
            }
            AqlCondition::Or(a, b) => {
                self.eval_condition(a, fields, line) || self.eval_condition(b, fields, line)
            }
        }
    }

    fn exec_aggregate(&self, func: &AqlAggFunc, data: &PipelineData) -> Result<String> {
        let sep_re = Regex::new(&self.separator)?;
        match func {
            AqlAggFunc::Count => Ok(data.lines.len().to_string()),

            AqlAggFunc::Sum(fi) => {
                let sum: f64 = data
                    .lines
                    .iter()
                    .filter_map(|l| {
                        let fields: Vec<&str> = sep_re.split(&l.content).collect();
                        fields.get(fi.saturating_sub(1))?.parse::<f64>().ok()
                    })
                    .sum();
                Ok(sum.to_string())
            }

            AqlAggFunc::Avg(fi) => {
                let vals: Vec<f64> = data
                    .lines
                    .iter()
                    .filter_map(|l| {
                        let fields: Vec<&str> = sep_re.split(&l.content).collect();
                        fields.get(fi.saturating_sub(1))?.parse::<f64>().ok()
                    })
                    .collect();
                if vals.is_empty() {
                    Ok("0".to_string())
                } else {
                    Ok((vals.iter().sum::<f64>() / vals.len() as f64).to_string())
                }
            }

            AqlAggFunc::Min(fi) => {
                let min = data
                    .lines
                    .iter()
                    .filter_map(|l| {
                        let fields: Vec<&str> = sep_re.split(&l.content).collect();
                        fields.get(fi.saturating_sub(1))?.parse::<f64>().ok()
                    })
                    .fold(f64::INFINITY, f64::min);
                Ok(min.to_string())
            }

            AqlAggFunc::Max(fi) => {
                let max = data
                    .lines
                    .iter()
                    .filter_map(|l| {
                        let fields: Vec<&str> = sep_re.split(&l.content).collect();
                        fields.get(fi.saturating_sub(1))?.parse::<f64>().ok()
                    })
                    .fold(f64::NEG_INFINITY, f64::max);
                Ok(max.to_string())
            }

            AqlAggFunc::Distinct(fi) => {
                let mut vals: Vec<String> = data
                    .lines
                    .iter()
                    .filter_map(|l| {
                        let fields: Vec<String> =
                            sep_re.split(&l.content).map(String::from).collect();
                        fields.get(fi.saturating_sub(1)).cloned()
                    })
                    .collect();
                vals.sort();
                vals.dedup();
                Ok(vals.join(", "))
            }

            AqlAggFunc::Freq(fi) => {
                let mut freq = std::collections::BTreeMap::new();
                for l in &data.lines {
                    let fields: Vec<String> = sep_re.split(&l.content).map(String::from).collect();
                    if let Some(val) = fields.get(fi.saturating_sub(1)) {
                        *freq.entry(val.clone()).or_insert(0usize) += 1;
                    }
                }
                Ok(serde_json::to_string(&freq).unwrap_or_default())
            }
        }
    }
}

impl Default for AqlEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    // ── Tokenizer ───────────────────────────────────────────────────────

    #[test]
    fn test_tokenize_find_literal() {
        let mut t = Tokenizer::new(r#"find "hello""#);
        let tokens = t.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Kw::Find),
                Token::StringLit("hello".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_tokenize_regex() {
        let mut t = Tokenizer::new(r#"find /fn\s+\w+/"#);
        let tokens = t.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Kw::Find),
                Token::RegexLit(r"fn\s+\w+".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_tokenize_pipeline() {
        let mut t = Tokenizer::new(r#"find "error" ignore_case | count"#);
        let tokens = t.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Kw::Find),
                Token::StringLit("error".into()),
                Token::Keyword(Kw::IgnoreCase),
                Token::Pipe,
                Token::Keyword(Kw::Count),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_tokenize_single_quotes() {
        let mut t = Tokenizer::new("find 'hello world'");
        let tokens = t.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Kw::Find),
                Token::StringLit("hello world".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_tokenize_numbers_and_operators() {
        let mut t = Tokenizer::new("filter field 3 > 100");
        let tokens = t.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Keyword(Kw::Filter),
                Token::Keyword(Kw::Field),
                Token::Integer(3),
                Token::Greater,
                Token::Integer(100),
                Token::Eof,
            ]
        );
    }

    // ── Parser ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_simple_find() {
        let p = parse(r#"find "hello""#).unwrap();
        assert_eq!(p.stages.len(), 1);
        match &p.stages[0] {
            AqlStage::Find {
                pattern: AqlPattern::Literal(s),
                modifiers,
            } => {
                assert_eq!(s, "hello");
                assert!(modifiers.is_empty());
            }
            _ => panic!("Expected Find stage"),
        }
    }

    #[test]
    fn test_parse_find_with_modifiers() {
        let p = parse(r#"find "error" ignore_case whole_word max 10"#).unwrap();
        match &p.stages[0] {
            AqlStage::Find { modifiers, .. } => {
                assert_eq!(modifiers.len(), 3);
                assert!(modifiers
                    .iter()
                    .any(|m| matches!(m, AqlModifier::CaseInsensitive)));
                assert!(modifiers
                    .iter()
                    .any(|m| matches!(m, AqlModifier::WholeWord)));
                assert!(modifiers.iter().any(|m| matches!(m, AqlModifier::Max(10))));
            }
            _ => panic!("Expected Find stage"),
        }
    }

    #[test]
    fn test_parse_replace() {
        let p = parse(r#"replace "old" with "new" all"#).unwrap();
        match &p.stages[0] {
            AqlStage::Replace {
                pattern: AqlPattern::Literal(s),
                replacement,
                modifiers,
            } => {
                assert_eq!(s, "old");
                assert_eq!(replacement, "new");
                assert!(modifiers.iter().any(|m| matches!(m, AqlModifier::Global)));
            }
            _ => panic!("Expected Replace stage"),
        }
    }

    #[test]
    fn test_parse_pipeline() {
        let p = parse(r#"find "TODO" | sort desc | take 5"#).unwrap();
        assert_eq!(p.stages.len(), 3);
        assert!(matches!(&p.stages[0], AqlStage::Find { .. }));
        assert!(matches!(
            &p.stages[1],
            AqlStage::Sort {
                descending: true,
                ..
            }
        ));
        assert!(matches!(&p.stages[2], AqlStage::Take(5)));
    }

    #[test]
    fn test_parse_select() {
        let p = parse("select fields 1, 3, 5").unwrap();
        match &p.stages[0] {
            AqlStage::Select { fields } => assert_eq!(*fields, vec![1, 3, 5]),
            _ => panic!("Expected Select"),
        }
    }

    #[test]
    fn test_parse_filter() {
        let p = parse("filter field 3 > 100").unwrap();
        match &p.stages[0] {
            AqlStage::Filter {
                condition: AqlCondition::FieldGreater(3, val),
            } => assert_eq!(*val, 100.0),
            _ => panic!("Expected Filter with FieldGreater"),
        }
    }

    #[test]
    fn test_parse_complex_pipeline() {
        let p = parse(
            r#"find "error" ignore_case | set separator "|" | select fields 1, 3 | sort by field 1 desc numeric | unique | take 10"#,
        )
        .unwrap();
        assert_eq!(p.stages.len(), 6);
    }

    #[test]
    fn test_parse_between_modifier() {
        let p = parse(r#"replace "old" with "new" all between "START" and "END""#).unwrap();
        match &p.stages[0] {
            AqlStage::Replace { modifiers, .. } => {
                assert!(modifiers
                    .iter()
                    .any(|m| matches!(m, AqlModifier::Between(..))));
            }
            _ => panic!("Expected Replace"),
        }
    }

    #[test]
    fn test_parse_delete() {
        let p = parse(r#"delete lines matching "^#""#).unwrap();
        assert!(matches!(&p.stages[0], AqlStage::Delete { .. }));
    }

    #[test]
    fn test_parse_insert() {
        let p = parse(r#"insert "// TODO" before "unsafe""#).unwrap();
        assert!(matches!(&p.stages[0], AqlStage::InsertBefore { .. }));
    }

    #[test]
    fn test_parse_aggregate() {
        let p = parse("aggregate sum field 3").unwrap();
        match &p.stages[0] {
            AqlStage::Aggregate {
                function: AqlAggFunc::Sum(3),
            } => {}
            _ => panic!("Expected Aggregate Sum(3)"),
        }
    }

    #[test]
    fn test_parse_errors() {
        // Missing pattern after 'find'
        assert!(parse("find").is_err());
        // Missing 'with' keyword in replace
        assert!(parse(r#"replace "old" "new""#).is_err());
        // Unknown stage keyword
        assert!(parse("unknownstage").is_err());
    }

    // ── Explain ─────────────────────────────────────────────────────────

    #[test]
    fn test_explain() {
        let p = parse(r#"find "error" ignore_case | count"#).unwrap();
        let steps = p.explain();
        assert_eq!(steps.len(), 2);
        assert!(steps[0].contains("Search for"));
        assert!(steps[0].contains("case-insensitive"));
        assert!(steps[1].contains("Count"));
    }

    // ── Evaluator ───────────────────────────────────────────────────────

    #[test]
    fn test_eval_find() {
        let f = temp("hello world\nfoo bar\nhello again\n");
        let pipeline = parse(r#"find "hello""#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        assert_eq!(results.stages[0].records_out, 2);
    }

    #[test]
    fn test_eval_find_regex() {
        let f = temp("fn main() {}\npub fn new() {}\nstruct Foo {}\n");
        let pipeline = parse(r#"find /fn\s+\w+/"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        assert_eq!(results.stages[0].records_out, 2);
    }

    #[test]
    fn test_eval_replace() {
        let f = temp("hello world\nfoo bar\nhello again\n");
        let pipeline = parse(r#"replace "hello" with "HI" all"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // All 3 lines preserved
        assert_eq!(results.stages[0].records_out, 3);
        // Check content is replaced
        if let serde_json::Value::Array(arr) = &results.final_output {
            assert_eq!(arr[0]["content"].as_str().unwrap(), "HI world");
            assert_eq!(arr[1]["content"].as_str().unwrap(), "foo bar");
            assert_eq!(arr[2]["content"].as_str().unwrap(), "HI again");
        }
    }

    #[test]
    fn test_eval_pipeline_find_count() {
        let f = temp("error line 1\ninfo line 2\nerror line 3\nwarn line 4\n");
        let pipeline = parse(r#"find "error" | count"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        if let serde_json::Value::Array(arr) = &results.final_output {
            assert_eq!(arr[0]["content"].as_str().unwrap(), "2");
        } else {
            panic!("Expected array output");
        }
    }

    #[test]
    fn test_eval_sort_and_take() {
        let f = temp("cherry\napple\nbanana\ndate\n");
        let pipeline = parse(r#"sort | take 2"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        if let serde_json::Value::Array(arr) = &results.final_output {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0]["content"].as_str().unwrap(), "apple");
            assert_eq!(arr[1]["content"].as_str().unwrap(), "banana");
        }
    }

    #[test]
    fn test_eval_unique() {
        let f = temp("apple\nbanana\napple\ncherry\nbanana\n");
        let pipeline = parse("unique").unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        assert_eq!(results.stages[0].records_out, 3);
    }

    #[test]
    fn test_eval_select_fields() {
        let f = temp("alice 30 engineer\nbob 25 artist\n");
        let pipeline = parse("select fields 1, 3").unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        if let serde_json::Value::Array(arr) = &results.final_output {
            assert_eq!(arr[0]["content"].as_str().unwrap(), "alice\tengineer");
            assert_eq!(arr[1]["content"].as_str().unwrap(), "bob\tartist");
        }
    }

    #[test]
    fn test_eval_filter() {
        let f = temp("name,age\nalice,30\nbob,25\ncharlie,35\n");
        let pipeline = parse(r#"set separator "," | filter field 2 > 28"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // "name,age" has "age" which is not > 28, so filtered out
        // alice,30 → 30 > 28 ✓, bob,25 → 25 > 28 ✗, charlie,35 → 35 > 28 ✓
        assert_eq!(results.stages[1].records_out, 2);
    }

    #[test]
    fn test_eval_delete() {
        let f = temp("keep this\n# comment\nkeep this too\n# another comment\n");
        let pipeline = parse("delete \"# \"").unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        assert_eq!(results.stages[0].records_out, 2);
    }

    #[test]
    fn test_eval_between() {
        let f = temp("before\nSTART\nmiddle1\nmiddle2\nEND\nafter\n");
        let pipeline = parse(r#"find "middle" between "START" and "END""#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        assert_eq!(results.stages[0].records_out, 2);
    }

    // ---- AQL v2 tests ----

    #[test]
    fn test_parse_let() {
        let pipeline = parse(r#"let x = "hello""#).unwrap();
        assert_eq!(pipeline.stages.len(), 1);
        match &pipeline.stages[0] {
            AqlStage::Let { name, value } => {
                assert_eq!(name, "x");
                match value {
                    AqlExpr::StringLiteral(s) => assert_eq!(s, "hello"),
                    _ => panic!("Expected StringLiteral"),
                }
            }
            _ => panic!("Expected Let stage"),
        }
    }

    #[test]
    fn test_parse_let_integer() {
        let pipeline = parse("let n = 42").unwrap();
        match &pipeline.stages[0] {
            AqlStage::Let { name, value } => {
                assert_eq!(name, "n");
                match value {
                    AqlExpr::IntLiteral(v) => assert_eq!(*v, 42),
                    _ => panic!("Expected IntLiteral"),
                }
            }
            _ => panic!("Expected Let stage"),
        }
    }

    #[test]
    fn test_eval_let_variable() {
        let f = temp("hello world\nfoo bar\n");
        let pipeline = parse(r#"let x = "hello" | find "hello""#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // The let stage just stores a variable, passes data through
        assert_eq!(results.stages[0].records_out, 2);
        // find "hello" matches 1 line
        assert_eq!(results.stages[1].records_out, 1);
    }

    #[test]
    fn test_parse_if_else() {
        let pipeline = parse(r#"if line contains "x" then find "a" else find "b" end"#).unwrap();
        assert_eq!(pipeline.stages.len(), 1);
        match &pipeline.stages[0] {
            AqlStage::IfElse {
                condition,
                then_stages,
                else_stages,
            } => {
                assert!(matches!(condition, AqlCondition::LineContains(_)));
                assert_eq!(then_stages.len(), 1);
                assert_eq!(else_stages.len(), 1);
            }
            _ => panic!("Expected IfElse stage"),
        }
    }

    #[test]
    fn test_eval_if_then_branch() {
        let f = temp("hello world\nfoo bar\nbaz hello\n");
        let pipeline = parse(r#"if line contains "hello" then find "world" end"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // First line is "hello world" which contains "hello" → then branch
        // find "world" matches "hello world" only
        assert_eq!(results.stages[0].records_out, 1);
    }

    #[test]
    fn test_eval_if_else_branch() {
        let f = temp("no match here\nfoo bar\nbaz\n");
        let pipeline = parse(r#"if line contains "NOPE" then find "x" else count end"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // First line doesn't contain "NOPE" → else branch → count
        assert_eq!(results.stages[0].records_out, 1);
    }

    #[test]
    fn test_parse_group_by() {
        let pipeline = parse("group by field 1 aggregate count").unwrap();
        assert_eq!(pipeline.stages.len(), 1);
        match &pipeline.stages[0] {
            AqlStage::GroupBy { field, aggregates } => {
                assert_eq!(*field, 1);
                assert_eq!(aggregates.len(), 1);
            }
            _ => panic!("Expected GroupBy stage"),
        }
    }

    #[test]
    fn test_eval_group_by_count() {
        let f = temp("a\tb\na\tc\nb\td\na\te\n");
        let pipeline = parse("group by field 1 aggregate count").unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // Groups: a(3), b(1) → 2 output lines
        assert_eq!(results.stages[0].records_out, 2);
    }

    #[test]
    fn test_eval_group_by_multiple_aggregates() {
        let f = temp("a\t10\na\t20\nb\t30\n");
        let pipeline = parse("group by field 1 aggregate count, sum 2").unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // Groups: a(count=2, sum=30), b(count=1, sum=30) → 2 output lines
        assert_eq!(results.stages[0].records_out, 2);
    }

    #[test]
    fn test_parse_def_and_call() {
        let pipeline = parse(r#"def myfunc = find "hello" | count end | call myfunc"#).unwrap();
        assert_eq!(pipeline.stages.len(), 2);
        match &pipeline.stages[0] {
            AqlStage::Define { name, body } => {
                assert_eq!(name, "myfunc");
                assert_eq!(body.len(), 2);
            }
            _ => panic!("Expected Define stage"),
        }
        match &pipeline.stages[1] {
            AqlStage::Call { name } => assert_eq!(name, "myfunc"),
            _ => panic!("Expected Call stage"),
        }
    }

    #[test]
    fn test_eval_def_and_call() {
        let f = temp("hello world\nfoo bar\nhello again\n");
        let pipeline = parse(r#"def myfunc = find "hello" end | call myfunc"#).unwrap();
        let mut engine = AqlEngine::new();
        let results = engine.execute(&pipeline, &[f.path()]).unwrap();
        // def just stores the function, passes data through
        assert_eq!(results.stages[0].records_out, 3);
        // call myfunc runs find "hello" → matches 2 lines
        assert_eq!(results.stages[1].records_out, 2);
    }

    #[test]
    fn test_explain_aql_v2_stages() {
        let pipeline = parse(r#"let x = "hi" | if line contains "a" then count else find "b" end | group by field 1 aggregate count | def f = count end | call f"#).unwrap();
        let explanation = pipeline.explain();
        let text = explanation.join("\n");
        assert!(text.contains("Assign variable"));
        assert!(text.contains("If"));
        assert!(text.contains("Group by"));
        assert!(text.contains("Define function"));
        assert!(text.contains("Call function"));
    }
}
