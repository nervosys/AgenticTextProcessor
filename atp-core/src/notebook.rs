//! Notebook / literate mode — markdown with embedded AQL code blocks.
//!
//! Parse, execute, and render markdown documents that contain AQL queries.
//! Supports inline results, streaming output, and rich formatting.
//!
//! # Format
//!
//! ````markdown
//! # Analysis Report
//!
//! ```aql
//! select "*.log" | grep "ERROR" | count
//! ```
//!
//! Some commentary between cells.
//!
//! ```aql {name="errors_by_file"}
//! select "*.log" | grep "ERROR" | freq
//! ```
//! ````
//!
//! Each `aql` fenced code block is a *cell*. Cells can have metadata
//! in `{key=value}` attributes. Results are inserted after the cell
//! as `aql-output` blocks when rendered.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single cell in a notebook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    /// Cell index (0-based).
    pub index: usize,
    /// Cell kind.
    pub kind: CellKind,
    /// Raw source text.
    pub source: String,
    /// Cell metadata / attributes.
    pub metadata: HashMap<String, String>,
    /// Execution output (populated after execution).
    pub output: Option<CellOutput>,
}

/// Cell type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellKind {
    /// Markdown prose.
    Markdown,
    /// AQL query.
    Aql,
    /// Raw output block (generated, not user-authored).
    Output,
}

/// Output from executing a cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellOutput {
    /// Text output.
    pub text: String,
    /// Number of result lines.
    pub line_count: usize,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
    /// Whether execution succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// A parsed notebook document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    /// Source file path.
    pub path: Option<PathBuf>,
    /// All cells in order.
    pub cells: Vec<NotebookCell>,
    /// Document-level metadata.
    pub metadata: HashMap<String, String>,
    /// Title extracted from first H1 heading.
    pub title: Option<String>,
}

impl Notebook {
    /// Parse a markdown string into a Notebook.
    pub fn parse(source: &str) -> Self {
        let mut cells = Vec::new();
        let mut current_text = String::new();
        let mut in_code_block = false;
        let mut code_lang = String::new();
        let mut code_attrs: HashMap<String, String> = HashMap::new();
        let mut code_content = String::new();
        let mut title: Option<String> = None;

        for line in source.lines() {
            if !in_code_block && line.starts_with("```") {
                // Start of code block
                let after_ticks = line.trim_start_matches('`');
                let (lang, attrs) = parse_fence_info(after_ticks);

                if lang == "aql" || lang == "aql-output" {
                    // Flush markdown
                    if !current_text.is_empty() {
                        if title.is_none() {
                            title = extract_title(&current_text);
                        }
                        cells.push(NotebookCell {
                            index: cells.len(),
                            kind: CellKind::Markdown,
                            source: current_text.clone(),
                            metadata: HashMap::new(),
                            output: None,
                        });
                        current_text.clear();
                    }
                    in_code_block = true;
                    code_lang = lang;
                    code_attrs = attrs;
                    code_content.clear();
                } else {
                    // Non-AQL code block — treat as markdown pass-through
                    current_text.push_str(line);
                    current_text.push('\n');
                }
            } else if in_code_block && line.starts_with("```") && line.trim() == "```" {
                // End of AQL code block
                in_code_block = false;
                let kind = if code_lang == "aql-output" {
                    CellKind::Output
                } else {
                    CellKind::Aql
                };
                cells.push(NotebookCell {
                    index: cells.len(),
                    kind,
                    source: code_content.trim_end().to_string(),
                    metadata: code_attrs.clone(),
                    output: None,
                });
                code_content.clear();
                code_attrs.clear();
            } else if in_code_block {
                code_content.push_str(line);
                code_content.push('\n');
            } else {
                current_text.push_str(line);
                current_text.push('\n');
            }
        }

        // Flush remaining markdown
        if !current_text.is_empty() {
            if title.is_none() {
                title = extract_title(&current_text);
            }
            cells.push(NotebookCell {
                index: cells.len(),
                kind: CellKind::Markdown,
                source: current_text,
                metadata: HashMap::new(),
                output: None,
            });
        }

        // If still in a code block (unterminated), flush it
        if in_code_block && !code_content.is_empty() {
            cells.push(NotebookCell {
                index: cells.len(),
                kind: CellKind::Aql,
                source: code_content.trim_end().to_string(),
                metadata: code_attrs,
                output: None,
            });
        }

        Self {
            path: None,
            cells,
            metadata: HashMap::new(),
            title,
        }
    }

    /// Load a notebook from a file.
    pub fn load(path: &Path) -> Result<Self> {
        let source =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut nb = Self::parse(&source);
        nb.path = Some(path.to_path_buf());
        Ok(nb)
    }

    /// Count AQL cells.
    pub fn aql_cell_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| c.kind == CellKind::Aql)
            .count()
    }

    /// Count markdown cells.
    pub fn markdown_cell_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| c.kind == CellKind::Markdown)
            .count()
    }

    /// Get all AQL cells.
    pub fn aql_cells(&self) -> Vec<&NotebookCell> {
        self.cells
            .iter()
            .filter(|c| c.kind == CellKind::Aql)
            .collect()
    }

    /// Get a cell by index.
    pub fn cell(&self, index: usize) -> Option<&NotebookCell> {
        self.cells.get(index)
    }

    /// Get a mutable cell by index.
    pub fn cell_mut(&mut self, index: usize) -> Option<&mut NotebookCell> {
        self.cells.get_mut(index)
    }

    /// Execute a single AQL cell (mock — real execution uses AqlEngine).
    ///
    /// For real execution, call `execute_cell_with_engine()`.
    pub fn execute_cell(&mut self, index: usize) -> Result<()> {
        let cell = self
            .cells
            .get_mut(index)
            .with_context(|| format!("Cell index {} out of range", index))?;

        if cell.kind != CellKind::Aql {
            bail!("Cell {} is not an AQL cell", index);
        }

        let start = std::time::Instant::now();

        // Mock execution — produces a placeholder output
        let output_text = format!("[Notebook] Executed: {}", cell.source);
        let line_count = output_text.lines().count();

        cell.output = Some(CellOutput {
            text: output_text,
            line_count,
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            error: None,
        });

        Ok(())
    }

    /// Execute all AQL cells in order.
    pub fn execute_all(&mut self) -> Result<NotebookRunSummary> {
        let aql_indices: Vec<usize> = self
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind == CellKind::Aql)
            .map(|(i, _)| i)
            .collect();

        let mut summary = NotebookRunSummary {
            total_cells: aql_indices.len(),
            ..Default::default()
        };
        let start = std::time::Instant::now();

        for idx in &aql_indices {
            match self.execute_cell(*idx) {
                Ok(()) => summary.succeeded += 1,
                Err(e) => {
                    summary.failed += 1;
                    summary.errors.push(format!("Cell {}: {}", idx, e));
                }
            }
        }

        summary.total_duration_ms = start.elapsed().as_millis() as u64;
        Ok(summary)
    }

    /// Render the notebook back to markdown, inserting outputs after AQL cells.
    pub fn render(&self) -> String {
        let mut out = String::new();

        for cell in &self.cells {
            match cell.kind {
                CellKind::Markdown => {
                    out.push_str(&cell.source);
                    if !cell.source.ends_with('\n') {
                        out.push('\n');
                    }
                }
                CellKind::Aql => {
                    // Render attributes
                    let attrs = if cell.metadata.is_empty() {
                        String::new()
                    } else {
                        let pairs: Vec<String> = cell
                            .metadata
                            .iter()
                            .map(|(k, v)| format!("{}=\"{}\"", k, v))
                            .collect();
                        format!(" {{{}}}", pairs.join(" "))
                    };
                    out.push_str(&format!("```aql{}\n", attrs));
                    out.push_str(&cell.source);
                    out.push_str("\n```\n");

                    // Insert output if present
                    if let Some(ref output) = cell.output {
                        out.push_str("\n```aql-output\n");
                        out.push_str(&output.text);
                        out.push_str("\n```\n");
                    }
                    out.push('\n');
                }
                CellKind::Output => {
                    out.push_str("```aql-output\n");
                    out.push_str(&cell.source);
                    out.push_str("\n```\n");
                }
            }
        }

        out
    }

    /// Save the rendered notebook to a file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let rendered = self.render();
        std::fs::write(path, &rendered)
            .with_context(|| format!("writing notebook to {}", path.display()))?;
        Ok(())
    }
}

/// Summary of a notebook execution run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotebookRunSummary {
    /// Total AQL cells.
    pub total_cells: usize,
    /// Successfully executed.
    pub succeeded: usize,
    /// Failed.
    pub failed: usize,
    /// Total duration in milliseconds.
    pub total_duration_ms: u64,
    /// Error messages.
    pub errors: Vec<String>,
}

// ─── helpers ────────────────────────────────────────────────────────────────

/// Parse fence info line: `aql {name="foo" cache=true}` → ("aql", {name: "foo", cache: "true"})
fn parse_fence_info(info: &str) -> (String, HashMap<String, String>) {
    let info = info.trim();
    let mut attrs = HashMap::new();

    // Split at first `{`
    if let Some(brace_pos) = info.find('{') {
        let lang = info[..brace_pos].trim().to_string();
        let attr_str = info[brace_pos..].trim_matches(|c| c == '{' || c == '}');

        // Simple key=value parser (handles key="value" and key=value)
        for part in attr_str.split_whitespace() {
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].to_string();
                let val = part[eq_pos + 1..].trim_matches('"').to_string();
                attrs.insert(key, val);
            }
        }

        (lang, attrs)
    } else {
        (info.to_string(), attrs)
    }
}

/// Extract title from first H1 line.
fn extract_title(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return Some(heading.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_notebook() {
        let md = r#"# My Report

Some intro text.

```aql
select "*.log" | grep "ERROR" | count
```

More text here.
"#;
        let nb = Notebook::parse(md);
        assert_eq!(nb.title.as_deref(), Some("My Report"));
        assert_eq!(nb.cells.len(), 3);
        assert_eq!(nb.cells[0].kind, CellKind::Markdown);
        assert_eq!(nb.cells[1].kind, CellKind::Aql);
        assert_eq!(nb.cells[2].kind, CellKind::Markdown);
        assert_eq!(nb.aql_cell_count(), 1);
        assert_eq!(nb.markdown_cell_count(), 2);
    }

    #[test]
    fn test_parse_multiple_aql_cells() {
        let md = r#"```aql
select "*.rs" | count
```

```aql
select "*.py" | count
```
"#;
        let nb = Notebook::parse(md);
        assert_eq!(nb.aql_cell_count(), 2);
        assert_eq!(nb.aql_cells()[0].source, "select \"*.rs\" | count");
        assert_eq!(nb.aql_cells()[1].source, "select \"*.py\" | count");
    }

    #[test]
    fn test_parse_fence_info() {
        let (lang, attrs) = parse_fence_info("aql {name=\"errors\" cache=true}");
        assert_eq!(lang, "aql");
        assert_eq!(attrs.get("name").unwrap(), "errors");
        assert_eq!(attrs.get("cache").unwrap(), "true");
    }

    #[test]
    fn test_parse_fence_info_no_attrs() {
        let (lang, attrs) = parse_fence_info("aql");
        assert_eq!(lang, "aql");
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_cell_metadata() {
        let md = "```aql {name=\"test\"}\nselect \"*\" | count\n```\n";
        let nb = Notebook::parse(md);
        let aql = nb.aql_cells();
        assert_eq!(aql.len(), 1);
        assert_eq!(aql[0].metadata.get("name").unwrap(), "test");
    }

    #[test]
    fn test_execute_cell() {
        let md = "```aql\nselect \"*.rs\" | count\n```\n";
        let mut nb = Notebook::parse(md);
        nb.execute_cell(0).unwrap();
        let output = nb.cells[0].output.as_ref().unwrap();
        assert!(output.success);
        assert!(output.text.contains("Executed"));
    }

    #[test]
    fn test_execute_all() {
        let md = "```aql\nq1\n```\n\n```aql\nq2\n```\n";
        let mut nb = Notebook::parse(md);
        let summary = nb.execute_all().unwrap();
        assert_eq!(summary.total_cells, 2);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn test_execute_markdown_cell_errors() {
        let md = "# Title\n\n```aql\nquery\n```\n";
        let mut nb = Notebook::parse(md);
        // Cell 0 is markdown, should fail
        assert!(nb.execute_cell(0).is_err());
    }

    #[test]
    fn test_render_round_trip() {
        let md = "```aql\nselect \"*.rs\" | count\n```\n";
        let nb = Notebook::parse(md);
        let rendered = nb.render();
        assert!(rendered.contains("```aql\n"));
        assert!(rendered.contains("select \"*.rs\" | count"));
        assert!(rendered.contains("```\n"));
    }

    #[test]
    fn test_render_with_output() {
        let md = "```aql\nquery\n```\n";
        let mut nb = Notebook::parse(md);
        nb.execute_cell(0).unwrap();
        let rendered = nb.render();
        assert!(rendered.contains("```aql-output\n"));
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(
            extract_title("# Hello World\nfoo"),
            Some("Hello World".into())
        );
        assert_eq!(extract_title("no heading"), None);
    }

    #[test]
    fn test_notebook_cell_accessors() {
        let md = "# Title\n\n```aql\nq\n```\n";
        let nb = Notebook::parse(md);
        assert!(nb.cell(0).is_some());
        assert!(nb.cell(99).is_none());
    }

    #[test]
    fn test_run_summary_default() {
        let s = NotebookRunSummary::default();
        assert_eq!(s.total_cells, 0);
        assert_eq!(s.succeeded, 0);
        assert_eq!(s.failed, 0);
    }
}
