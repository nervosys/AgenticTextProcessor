//! Application state and logic for the TUI.

use crate::file_browser::FileBrowser;
use atp_core::code_intel::{CodeIntelligence, Language, StructuralQuery, Symbol, SymbolKind};
use atp_core::engine::awk::{AwkConfig, AwkEngine};
use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::engine::pipeline::Pipeline;
use atp_core::engine::sed::{SedConfig, SedEngine};
use atp_core::output::{PatternType, SearchMatch};
use atp_core::traversal::Walker;
use std::path::PathBuf;

/// Active tab in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Search,
    Transform,
    Analyze,
    Pipeline,
    Aql,
    Symbols,
    /// v2: Browse the incremental file index.
    Index,
    /// v2: Interactive AQL debugger (DAP).
    Debug,
    /// v2: Notebook / literate `.atp.md` viewer.
    Notebook,
    /// v2: Distributed pipeline dashboard.
    Distributed,
}

impl Tab {
    #[allow(dead_code)]
    pub fn title(&self) -> &str {
        match self {
            Tab::Search => "Search",
            Tab::Transform => "Transform",
            Tab::Analyze => "Analyze",
            Tab::Pipeline => "Pipeline",
            Tab::Aql => "AQL",
            Tab::Symbols => "Symbols",
            Tab::Index => "Index",
            Tab::Debug => "Debug",
            Tab::Notebook => "Notebook",
            Tab::Distributed => "Distributed",
        }
    }
}

const TABS: [Tab; 10] = [
    Tab::Search,
    Tab::Transform,
    Tab::Analyze,
    Tab::Pipeline,
    Tab::Aql,
    Tab::Symbols,
    Tab::Index,
    Tab::Debug,
    Tab::Notebook,
    Tab::Distributed,
];

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Input,
    Results,
    FileBrowser,
}

/// Command suggestions for auto-completion.
const SUGGESTIONS: &[&str] = &[
    "search ",
    "transform ",
    "analyze ",
    "pipeline ",
    "ontology",
    "explain ",
    "scope ",
    "validate ",
    "context ",
    "find ",
    "replace ",
    "count",
    "sort",
    "unique",
    "head ",
    "tail ",
    "fields ",
    "where ",
    "group_by ",
    "fn:*",
    "struct:*",
    "enum:*",
    "trait:*",
    "class:*",
    "--literal",
    "--case-insensitive",
    "--whole-word",
    "--context ",
    "--include ",
    "--exclude ",
    "--max-depth ",
    "--format json",
    "--format human",
    "--global",
    "--in-place",
    "--dry-run",
    "--separator ",
    "--fields ",
    "--aggregate ",
];

pub struct App {
    pub active_tab: Tab,
    pub input: String,
    pub cursor_pos: usize,
    pub results: Vec<SearchMatch>,
    pub selected_result: usize,
    pub status_message: String,
    pub show_help: bool,
    pub suggestion: Option<String>,
    #[allow(dead_code)]
    pub scroll_offset: usize,
    pub working_dir: PathBuf,
    pub explainer_text: String,
    pub file_browser: FileBrowser,
    pub focus: Focus,
    pub symbols: Vec<Symbol>,
    pub selected_symbol: usize,
    pub aql_history: Vec<String>,
    pub history_pos: Option<usize>,
    pub preview_content: String,
    /// v2: Cached index entries for the Index tab.
    pub index_entries: Vec<String>,
    /// v2: Debug output lines for the Debug tab.
    pub debug_output: Vec<String>,
    /// v2: Parsed notebook cells for the Notebook tab.
    pub notebook_cells: Vec<NotebookCell>,
    /// v2: Distributed pipeline node statuses.
    pub distributed_nodes: Vec<NodeStatus>,
}

/// A parsed cell from a literate `.atp.md` notebook.
#[derive(Debug, Clone)]
pub struct NotebookCell {
    pub kind: CellKind,
    pub content: String,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Markdown,
    Aql,
}

/// Status of a node in a distributed pipeline.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NodeStatus {
    pub id: String,
    pub stage: String,
    pub state: NodeState,
    pub records_processed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NodeState {
    Idle,
    Running,
    Done,
    Error,
}

impl App {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            active_tab: Tab::Search,
            input: String::new(),
            cursor_pos: 0,
            results: Vec::new(),
            selected_result: 0,
            status_message: "F1=Help │ Tab=Mode │ Ctrl+B=Browser │ Ctrl+Q=Quit".to_string(),
            show_help: false,
            suggestion: None,
            scroll_offset: 0,
            working_dir: cwd.clone(),
            explainer_text: String::new(),
            file_browser: FileBrowser::new(&cwd),
            focus: Focus::Input,
            symbols: Vec::new(),
            selected_symbol: 0,
            aql_history: Vec::new(),
            history_pos: None,
            preview_content: String::new(),
            index_entries: Vec::new(),
            debug_output: Vec::new(),
            notebook_cells: Vec::new(),
            distributed_nodes: Vec::new(),
        }
    }

    pub fn next_tab(&mut self) {
        let idx = TABS.iter().position(|&t| t == self.active_tab).unwrap_or(0);
        self.active_tab = TABS[(idx + 1) % TABS.len()];
        self.update_explainer();
    }

    pub fn prev_tab(&mut self) {
        let idx = TABS.iter().position(|&t| t == self.active_tab).unwrap_or(0);
        self.active_tab = TABS[(idx + TABS.len() - 1) % TABS.len()];
        self.update_explainer();
    }

    pub fn on_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
        self.update_suggestion();
        self.update_explainer();

        // Live search on every keystroke in search mode
        if self.active_tab == Tab::Search && self.input.len() >= 2 {
            self.execute_search();
        }
    }

    pub fn on_backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input.remove(self.cursor_pos);
            self.update_suggestion();
            self.update_explainer();
        }
    }

    pub fn on_enter(&mut self) {
        match self.active_tab {
            Tab::Search => self.execute_search(),
            Tab::Transform => self.execute_transform(),
            Tab::Analyze => self.execute_analyze(),
            Tab::Pipeline => self.execute_pipeline(),
            Tab::Aql => self.execute_aql(),
            Tab::Symbols => self.execute_symbols(),
            Tab::Index => self.execute_index(),
            Tab::Debug => self.execute_debug(),
            Tab::Notebook => self.execute_notebook(),
            Tab::Distributed => self.execute_distributed(),
        }
    }

    /// Toggle file browser visibility.
    pub fn toggle_file_browser(&mut self) {
        self.file_browser.visible = !self.file_browser.visible;
    }

    /// Cycle focus between Input → Results → FileBrowser.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Input => Focus::Results,
            Focus::Results => {
                if self.file_browser.visible {
                    Focus::FileBrowser
                } else {
                    Focus::Input
                }
            }
            Focus::FileBrowser => Focus::Input,
        };
    }

    /// Handle file browser selection — load preview or set scope.
    pub fn on_file_browser_enter(&mut self) {
        let items = self.file_browser.root.flatten_visible();
        if let Some((_, node)) = items.get(self.file_browser.selected) {
            if node.is_dir {
                self.file_browser.toggle_expand();
            } else {
                // Preview file content
                if let Ok(content) = std::fs::read_to_string(&node.path) {
                    let lines: Vec<&str> = content.lines().take(100).collect();
                    self.preview_content = lines.join("\n");
                    self.status_message = format!("Preview: {}", node.path.display());
                }
            }
        }
    }

    pub fn on_up(&mut self) {
        if self.selected_result > 0 {
            self.selected_result -= 1;
        }
    }

    pub fn on_down(&mut self) {
        if self.selected_result + 1 < self.results.len() {
            self.selected_result += 1;
        }
    }

    pub fn on_page_up(&mut self) {
        self.selected_result = self.selected_result.saturating_sub(10);
    }

    pub fn on_page_down(&mut self) {
        self.selected_result =
            (self.selected_result + 10).min(self.results.len().saturating_sub(1));
    }

    pub fn accept_suggestion(&mut self) {
        if let Some(ref suggestion) = self.suggestion.clone() {
            self.input = suggestion.clone();
            self.cursor_pos = self.input.len();
            self.suggestion = None;
        }
    }

    fn update_suggestion(&mut self) {
        if self.input.is_empty() {
            self.suggestion = None;
            return;
        }
        self.suggestion = SUGGESTIONS
            .iter()
            .find(|s| s.starts_with(&self.input) && **s != self.input)
            .map(|s| s.to_string());
    }

    fn update_explainer(&mut self) {
        self.explainer_text = match self.active_tab {
            Tab::Search => {
                if self.input.is_empty() {
                    "Type a regex pattern to search. Results update live as you type.".to_string()
                } else {
                    match regex::Regex::new(&self.input) {
                        Ok(_) => format!(
                            "Valid regex: '{}' — searching in {}",
                            self.input,
                            self.working_dir.display()
                        ),
                        Err(e) => format!("Invalid regex: {e}"),
                    }
                }
            }
            Tab::Transform => {
                if self.input.is_empty() {
                    "Enter a sed-style expression: s/pattern/replacement/flags".to_string()
                } else if self.input.starts_with("s/") || self.input.starts_with("s|") {
                    match atp_core::engine::sed::parse_substitution(&self.input) {
                        Ok(_) => format!("Valid expression: '{}'", self.input),
                        Err(e) => format!("Invalid expression: {e}"),
                    }
                } else {
                    "Expression should start with 's/' (e.g. 's/old/new/g')".to_string()
                }
            }
            Tab::Analyze => {
                "Enter field separator and selection. Default separator: whitespace".to_string()
            }
            Tab::Pipeline => {
                if self.input.is_empty() {
                    "Enter pipeline DSL: stage1 | stage2 | stage3".to_string()
                } else {
                    match atp_core::engine::pipeline::Pipeline::from_dsl(&self.input) {
                        Ok(pipe) => {
                            let explain = pipe.explain();
                            format!(
                                "Pipeline ({} stages):\n{}",
                                explain.len(),
                                explain.join("\n")
                            )
                        }
                        Err(e) => format!("Invalid pipeline: {e}"),
                    }
                }
            }
            Tab::Aql => {
                if self.input.is_empty() {
                    "AQL — unified query language. Examples:\n  find \"error\" ignore_case | count\n  replace \"old\" with \"new\" all\n  find \"TODO\" | sort | unique".to_string()
                } else {
                    match atp_core::engine::aql::parse(&self.input) {
                        Ok(pipeline) => format!(
                            "Valid AQL: {} stage(s). Press Enter to execute.",
                            pipeline.stages.len()
                        ),
                        Err(e) => format!("AQL error: {e}"),
                    }
                }
            }
            Tab::Symbols => {
                if self.input.is_empty() {
                    "Code intelligence — extract symbols from source files.\nFilter: fn:name, struct:pattern, enum:*, or just a name pattern.".to_string()
                } else {
                    format!("Symbol query: '{}' — press Enter to search", self.input)
                }
            }
            Tab::Index => {
                if self.input.is_empty() {
                    "File index browser — type a glob or substring to filter indexed files.\nPress Enter to refresh the index.".to_string()
                } else {
                    format!(
                        "Index filter: '{}' — press Enter to search index",
                        self.input
                    )
                }
            }
            Tab::Debug => {
                if self.input.is_empty() {
                    "AQL Debugger — type an AQL query and press Enter to step-debug.\nShows each pipeline stage's intermediate result.".to_string()
                } else {
                    format!(
                        "Debug AQL: '{}' — press Enter to trace execution",
                        self.input
                    )
                }
            }
            Tab::Notebook => {
                if self.input.is_empty() {
                    "Notebook viewer — enter path to an .atp.md file to load.\nDisplays markdown prose and evaluated AQL cells.".to_string()
                } else {
                    format!("Load notebook: '{}' — press Enter to open", self.input)
                }
            }
            Tab::Distributed => {
                if self.input.is_empty() {
                    "Distributed pipeline dashboard — enter pipeline DSL and target nodes.\nFormat: pipeline_dsl @node1,node2,...".to_string()
                } else {
                    format!("Distributed: '{}' — press Enter to dispatch", self.input)
                }
            }
        };
    }

    fn execute_search(&mut self) {
        if self.input.is_empty() {
            return;
        }

        let config = GrepConfig {
            pattern: self.input.clone(),
            pattern_type: PatternType::Regex,
            case_sensitive: true,
            context_before: 1,
            context_after: 1,
            max_matches: Some(100),
            ..Default::default()
        };

        let engine = match GrepEngine::new(config) {
            Ok(e) => e,
            Err(e) => {
                self.status_message = format!("Pattern error: {e}");
                return;
            }
        };

        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };

        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.search_files(&path_refs) {
            Ok(results) => {
                self.status_message = format!(
                    "{} matches in {} files",
                    results.total_matches, results.files_with_matches
                );
                self.results = results.matches;
                self.selected_result = 0;
            }
            Err(e) => {
                self.status_message = format!("Search error: {e}");
            }
        }
    }

    fn collect_scope_files(&self) -> Result<Vec<PathBuf>, String> {
        let scope = atp_core::output::FileScope {
            roots: vec![self.working_dir.clone()],
            max_depth: Some(5),
            respect_gitignore: true,
            ..Default::default()
        };
        let walker = Walker::new(scope).map_err(|e| format!("Scope error: {e}"))?;
        walker
            .collect_files()
            .map_err(|e| format!("File error: {e}"))
    }

    fn execute_transform(&mut self) {
        if self.input.is_empty() {
            self.status_message = "Enter a sed expression: s/pattern/replacement/flags".to_string();
            return;
        }
        let cmd = match atp_core::engine::sed::parse_substitution(&self.input) {
            Ok(c) => c,
            Err(e) => {
                self.status_message = format!("Parse error: {e}");
                return;
            }
        };
        let config = SedConfig {
            commands: vec![cmd],
            dry_run: true,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.transform_files(&path_refs) {
            Ok(results) => {
                self.status_message = format!(
                    "{} changes in {} files (dry-run)",
                    results.total_changes, results.files_modified
                );
                self.results = results
                    .changes
                    .iter()
                    .flat_map(|fc| {
                        fc.changes.iter().map(|c| SearchMatch {
                            file: fc.file.clone(),
                            line_number: c.line_number,
                            column_start: 1,
                            column_end: 1,
                            line_content: format!("- {}", c.original),
                            matched_text: format!("+ {}", c.replacement),
                            context_before: vec![],
                            context_after: vec![],
                            byte_offset: 0,
                        })
                    })
                    .collect();
                self.selected_result = 0;
            }
            Err(e) => self.status_message = format!("Transform error: {e}"),
        }
    }

    fn execute_analyze(&mut self) {
        let config = AwkConfig {
            field_separator: if self.input.is_empty() {
                r"\s+".to_string()
            } else {
                regex::escape(&self.input)
            },
            ..Default::default()
        };
        let engine = match AwkEngine::new(config) {
            Ok(e) => e,
            Err(e) => {
                self.status_message = format!("Config error: {e}");
                return;
            }
        };
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.process_files(&path_refs) {
            Ok(results) => {
                self.status_message = format!("{} records processed", results.records_processed);
                self.results = results
                    .output_records
                    .iter()
                    .take(200)
                    .map(|rec| SearchMatch {
                        file: rec.source_file.clone(),
                        line_number: rec.source_line,
                        column_start: 1,
                        column_end: 1,
                        line_content: rec.fields.join("\t"),
                        matched_text: format!("NR={} NF={}", rec.nr, rec.nf),
                        context_before: vec![],
                        context_after: vec![],
                        byte_offset: 0,
                    })
                    .collect();
                self.selected_result = 0;
            }
            Err(e) => self.status_message = format!("Analyze error: {e}"),
        }
    }

    fn execute_pipeline(&mut self) {
        if self.input.is_empty() {
            self.status_message = "Enter pipeline DSL: stage1 | stage2 | stage3".to_string();
            return;
        }
        let pipeline = match Pipeline::from_dsl(&self.input) {
            Ok(p) => p,
            Err(e) => {
                self.status_message = format!("Pipeline parse error: {e}");
                return;
            }
        };
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match pipeline.execute(&path_refs) {
            Ok(results) => {
                let stage_summary: Vec<String> = results
                    .stages
                    .iter()
                    .map(|s| {
                        format!(
                            "{}: {}→{} ({}ms)",
                            s.stage_type, s.records_in, s.records_out, s.duration_ms
                        )
                    })
                    .collect();
                self.status_message = format!(
                    "Pipeline: {} stages, {}ms total | {}",
                    results.stages.len(),
                    results.total_duration_ms,
                    stage_summary.join(" | ")
                );
                // Show final output as result entries
                if let serde_json::Value::Array(items) = &results.final_output {
                    self.results = items
                        .iter()
                        .enumerate()
                        .take(200)
                        .map(|(i, item)| SearchMatch {
                            file: String::new(),
                            line_number: i + 1,
                            column_start: 1,
                            column_end: 1,
                            line_content: item.to_string(),
                            matched_text: String::new(),
                            context_before: vec![],
                            context_after: vec![],
                            byte_offset: 0,
                        })
                        .collect();
                } else {
                    self.results = vec![SearchMatch {
                        file: String::new(),
                        line_number: 1,
                        column_start: 1,
                        column_end: 1,
                        line_content: results.final_output.to_string(),
                        matched_text: String::new(),
                        context_before: vec![],
                        context_after: vec![],
                        byte_offset: 0,
                    }];
                }
                self.selected_result = 0;
            }
            Err(e) => self.status_message = format!("Pipeline error: {e}"),
        }
    }

    pub fn tab_index(&self) -> usize {
        TABS.iter().position(|&t| t == self.active_tab).unwrap_or(0)
    }

    fn execute_aql(&mut self) {
        if self.input.is_empty() {
            self.status_message = "Enter an AQL query, e.g.: find \"TODO\" | count".to_string();
            return;
        }
        // Parse and execute AQL
        match atp_core::engine::aql::parse(&self.input) {
            Ok(pipeline) => {
                let mut engine = atp_core::engine::aql::AqlEngine::new();
                let files = match self.collect_scope_files() {
                    Ok(f) => f,
                    Err(e) => {
                        self.status_message = e;
                        return;
                    }
                };
                let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
                match engine.execute(&pipeline, &path_refs) {
                    Ok(results) => {
                        // Save to history
                        self.aql_history.push(self.input.clone());
                        self.history_pos = None;

                        self.status_message = format!(
                            "AQL: {} stages, {}ms",
                            results.stages.len(),
                            results.total_duration_ms,
                        );
                        // Show final output
                        if let serde_json::Value::Array(items) = &results.final_output {
                            self.results = items
                                .iter()
                                .enumerate()
                                .take(500)
                                .map(|(i, item)| SearchMatch {
                                    file: String::new(),
                                    line_number: i + 1,
                                    column_start: 1,
                                    column_end: 1,
                                    line_content: item
                                        .as_str()
                                        .unwrap_or(&item.to_string())
                                        .to_string(),
                                    matched_text: String::new(),
                                    context_before: vec![],
                                    context_after: vec![],
                                    byte_offset: 0,
                                })
                                .collect();
                        } else {
                            self.results = vec![SearchMatch {
                                file: String::new(),
                                line_number: 1,
                                column_start: 1,
                                column_end: 1,
                                line_content: results.final_output.to_string(),
                                matched_text: String::new(),
                                context_before: vec![],
                                context_after: vec![],
                                byte_offset: 0,
                            }];
                        }
                        self.selected_result = 0;
                    }
                    Err(e) => self.status_message = format!("AQL error: {e}"),
                }
            }
            Err(e) => self.status_message = format!("AQL parse error: {e}"),
        }
    }

    fn execute_symbols(&mut self) {
        let ci = CodeIntelligence::new();
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };

        // Build query from input
        let query = if self.input.is_empty() {
            StructuralQuery {
                kind: None,
                name_pattern: None,
                language: None,
                visibility: None,
                doc_contains: None,
            }
        } else {
            // Parse simple filter: "fn:pattern", "struct:pattern", or just "pattern"
            let (kind, name_pat) = if let Some((prefix, pat)) = self.input.split_once(':') {
                let kind = match prefix.to_lowercase().as_str() {
                    "fn" | "function" => Some(SymbolKind::Function),
                    "struct" => Some(SymbolKind::Struct),
                    "enum" => Some(SymbolKind::Enum),
                    "trait" => Some(SymbolKind::Trait),
                    "mod" | "module" => Some(SymbolKind::Module),
                    "const" => Some(SymbolKind::Constant),
                    "type" => Some(SymbolKind::Type),
                    "class" => Some(SymbolKind::Class),
                    "method" => Some(SymbolKind::Method),
                    "import" | "use" => Some(SymbolKind::Import),
                    _ => None,
                };
                (kind, Some(pat.to_string()))
            } else {
                (None, Some(self.input.clone()))
            };

            StructuralQuery {
                kind,
                name_pattern: name_pat,
                language: None,
                visibility: None,
                doc_contains: None,
            }
        };

        let mut all_symbols = Vec::new();
        for file in &files {
            if Language::from_path(file) != Language::Unknown {
                if let Ok(source) = std::fs::read_to_string(file) {
                    let file_str = file.to_string_lossy().to_string();
                    let symbols = ci.query_symbols(&source, &file_str, &query);
                    all_symbols.extend(symbols);
                }
            }
        }

        self.status_message = format!("{} symbols found", all_symbols.len());

        // Convert symbols to SearchMatch for display
        self.results = all_symbols
            .iter()
            .take(500)
            .map(|sym| SearchMatch {
                file: sym.file.clone(),
                line_number: sym.start_line,
                column_start: 1,
                column_end: 1,
                line_content: format!("{:?} {} {}", sym.kind, sym.name, &sym.signature),
                matched_text: sym.visibility.clone().unwrap_or_default(),
                context_before: vec![],
                context_after: vec![],
                byte_offset: 0,
            })
            .collect();
        self.symbols = all_symbols;
        self.selected_result = 0;
        self.selected_symbol = 0;
    }

    // ── v2 tab handlers ────────────────────────────────────────

    /// Index tab: list files from the working directory that match the filter.
    fn execute_index(&mut self) {
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = e;
                return;
            }
        };
        let filter = self.input.to_lowercase();
        let entries: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .filter(|name| filter.is_empty() || name.to_lowercase().contains(&filter))
            .collect();
        self.status_message = format!("{} indexed files", entries.len());
        self.results = entries
            .iter()
            .enumerate()
            .take(500)
            .map(|(i, path)| SearchMatch {
                file: path.clone(),
                line_number: i + 1,
                column_start: 1,
                column_end: 1,
                line_content: path.clone(),
                matched_text: String::new(),
                context_before: vec![],
                context_after: vec![],
                byte_offset: 0,
            })
            .collect();
        self.index_entries = entries;
        self.selected_result = 0;
    }

    /// Debug tab: parse AQL and show per-stage trace output.
    fn execute_debug(&mut self) {
        if self.input.is_empty() {
            self.status_message = "Enter an AQL query to debug.".to_string();
            return;
        }
        match atp_core::engine::aql::parse(&self.input) {
            Ok(pipeline) => {
                let files = match self.collect_scope_files() {
                    Ok(f) => f,
                    Err(e) => {
                        self.status_message = e;
                        return;
                    }
                };
                let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
                let mut engine = atp_core::engine::aql::AqlEngine::new();
                match engine.execute(&pipeline, &path_refs) {
                    Ok(results) => {
                        let mut output = Vec::new();
                        for (i, stage) in results.stages.iter().enumerate() {
                            output.push(format!(
                                "Stage {}: {} — in={} out={} ({}ms)",
                                i + 1,
                                stage.stage_type,
                                stage.records_in,
                                stage.records_out,
                                stage.duration_ms,
                            ));
                        }
                        self.status_message =
                            format!("Debug: {} stages traced", results.stages.len());
                        self.debug_output = output.clone();
                        self.results = output
                            .iter()
                            .enumerate()
                            .map(|(i, line)| SearchMatch {
                                file: String::new(),
                                line_number: i + 1,
                                column_start: 1,
                                column_end: 1,
                                line_content: line.clone(),
                                matched_text: String::new(),
                                context_before: vec![],
                                context_after: vec![],
                                byte_offset: 0,
                            })
                            .collect();
                        self.selected_result = 0;
                    }
                    Err(e) => self.status_message = format!("Debug error: {e}"),
                }
            }
            Err(e) => self.status_message = format!("AQL parse error: {e}"),
        }
    }

    /// Notebook tab: load and parse an `.atp.md` file.
    fn execute_notebook(&mut self) {
        if self.input.is_empty() {
            self.status_message = "Enter path to an .atp.md file.".to_string();
            return;
        }
        let path = std::path::Path::new(&self.input);
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                self.status_message = format!("Cannot read file: {e}");
                return;
            }
        };

        let mut cells = Vec::new();
        let mut in_aql = false;
        let mut md_buf = String::new();
        let mut aql_buf = String::new();

        for line in content.lines() {
            if line.trim_start().starts_with("```aql") && !in_aql {
                // Flush markdown
                if !md_buf.is_empty() {
                    cells.push(NotebookCell {
                        kind: CellKind::Markdown,
                        content: std::mem::take(&mut md_buf),
                        output: None,
                    });
                }
                in_aql = true;
                aql_buf.clear();
            } else if line.trim_start().starts_with("```") && in_aql {
                // End of AQL block — evaluate
                let output = match atp_core::engine::aql::parse(aql_buf.trim()) {
                    Ok(pipeline) => {
                        let explain = pipeline
                            .stages
                            .iter()
                            .map(|s| format!("{:?}", s))
                            .collect::<Vec<_>>()
                            .join(" | ");
                        Some(format!("✓ {} stage(s): {}", pipeline.stages.len(), explain))
                    }
                    Err(e) => Some(format!("✗ Error: {e}")),
                };
                cells.push(NotebookCell {
                    kind: CellKind::Aql,
                    content: std::mem::take(&mut aql_buf),
                    output,
                });
                in_aql = false;
            } else if in_aql {
                if !aql_buf.is_empty() {
                    aql_buf.push('\n');
                }
                aql_buf.push_str(line);
            } else {
                if !md_buf.is_empty() {
                    md_buf.push('\n');
                }
                md_buf.push_str(line);
            }
        }
        // Flush trailing markdown
        if !md_buf.is_empty() {
            cells.push(NotebookCell {
                kind: CellKind::Markdown,
                content: md_buf,
                output: None,
            });
        }

        self.status_message = format!(
            "Notebook: {} cells ({} AQL, {} markdown)",
            cells.len(),
            cells.iter().filter(|c| c.kind == CellKind::Aql).count(),
            cells
                .iter()
                .filter(|c| c.kind == CellKind::Markdown)
                .count(),
        );
        self.results = cells
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let prefix = match cell.kind {
                    CellKind::Markdown => "[MD]",
                    CellKind::Aql => "[AQL]",
                };
                SearchMatch {
                    file: String::new(),
                    line_number: i + 1,
                    column_start: 1,
                    column_end: 1,
                    line_content: format!(
                        "{} {}",
                        prefix,
                        cell.content.lines().next().unwrap_or("")
                    ),
                    matched_text: cell.output.clone().unwrap_or_default(),
                    context_before: vec![],
                    context_after: vec![],
                    byte_offset: 0,
                }
            })
            .collect();
        self.notebook_cells = cells;
        self.selected_result = 0;
    }

    /// Distributed tab: simulate dispatching pipeline stages to nodes.
    fn execute_distributed(&mut self) {
        if self.input.is_empty() {
            self.status_message =
                "Enter: pipeline_dsl @node1,node2 (e.g. find \"x\" | count @a,b)".to_string();
            return;
        }
        // Split input at '@' to get pipeline DSL and node list.
        let (dsl, nodes_str) = if let Some(at) = self.input.rfind('@') {
            (&self.input[..at], &self.input[at + 1..])
        } else {
            (self.input.as_str(), "local")
        };
        let node_ids: Vec<&str> = nodes_str.split(',').map(|s| s.trim()).collect();

        let pipeline = match Pipeline::from_dsl(dsl.trim()) {
            Ok(p) => p,
            Err(e) => {
                self.status_message = format!("Pipeline error: {e}");
                return;
            }
        };
        let stages = pipeline.explain();

        // Simulate: assign stages round-robin to nodes.
        let mut statuses: Vec<NodeStatus> = Vec::new();
        for (i, stage_desc) in stages.iter().enumerate() {
            let node = node_ids[i % node_ids.len()];
            statuses.push(NodeStatus {
                id: node.to_string(),
                stage: stage_desc.clone(),
                state: NodeState::Done,
                records_processed: 0,
            });
        }

        self.status_message = format!(
            "Distributed: {} stages across {} nodes (simulated)",
            stages.len(),
            node_ids.len()
        );
        self.results = statuses
            .iter()
            .enumerate()
            .map(|(i, ns)| SearchMatch {
                file: String::new(),
                line_number: i + 1,
                column_start: 1,
                column_end: 1,
                line_content: format!("[{:?}] {} → {}", ns.state, ns.id, ns.stage,),
                matched_text: String::new(),
                context_before: vec![],
                context_after: vec![],
                byte_offset: 0,
            })
            .collect();
        self.distributed_nodes = statuses;
        self.selected_result = 0;
    }

    /// Navigate AQL history up.
    pub fn history_up(&mut self) {
        if self.aql_history.is_empty() {
            return;
        }
        let pos = match self.history_pos {
            Some(p) if p > 0 => p - 1,
            Some(p) => p,
            None => self.aql_history.len() - 1,
        };
        self.history_pos = Some(pos);
        self.input = self.aql_history[pos].clone();
        self.cursor_pos = self.input.len();
    }

    /// Navigate AQL history down.
    pub fn history_down(&mut self) {
        if let Some(pos) = self.history_pos {
            if pos + 1 < self.aql_history.len() {
                self.history_pos = Some(pos + 1);
                self.input = self.aql_history[pos + 1].clone();
            } else {
                self.history_pos = None;
                self.input.clear();
            }
            self.cursor_pos = self.input.len();
        }
    }
}
