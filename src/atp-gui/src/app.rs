//! ATP GUI application using egui.

use atp_core::engine::awk::{AwkConfig, AwkEngine};
use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::engine::pipeline::Pipeline;
use atp_core::engine::sed::{parse_substitution, SedConfig, SedEngine, TransformCommand};
use atp_core::output::{format_transform_human, FileScope, PatternType, SearchMatch};
use atp_core::traversal::Walker;
use eframe::egui;
use std::path::PathBuf;

#[derive(PartialEq)]
enum GuiTab {
    Search,
    Transform,
    Analyze,
    Pipeline,
    Ontology,
}

pub struct AtpGuiApp {
    active_tab: GuiTab,

    // Search state
    search_pattern: String,
    search_case_insensitive: bool,
    search_whole_word: bool,
    search_literal: bool,
    search_context_lines: usize,
    search_include_glob: String,
    search_results: Vec<SearchMatch>,
    search_total: usize,
    search_files_matched: usize,

    // Transform state
    transform_expression: String,
    transform_pattern: String,
    transform_replacement: String,
    transform_global: bool,
    transform_preview: String,
    transform_backup: bool,
    transform_backup_ext: String,
    transform_confirm_apply: bool,
    transform_last_applied: String,

    // Analyze state
    analyze_separator: String,
    analyze_output: String,

    // Pipeline state
    pipeline_dsl: String,
    pipeline_output: String,

    // Common state
    working_dir: String,
    status_message: String,
    max_depth: usize,
}

impl AtpGuiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        Self {
            active_tab: GuiTab::Search,
            search_pattern: String::new(),
            search_case_insensitive: false,
            search_whole_word: false,
            search_literal: false,
            search_context_lines: 0,
            search_include_glob: String::new(),
            search_results: Vec::new(),
            search_total: 0,
            search_files_matched: 0,
            transform_expression: String::new(),
            transform_pattern: String::new(),
            transform_replacement: String::new(),
            transform_global: true,
            transform_preview: String::new(),
            transform_backup: true,
            transform_backup_ext: "bak".to_string(),
            transform_confirm_apply: false,
            transform_last_applied: String::new(),
            analyze_separator: String::new(),
            analyze_output: String::new(),
            pipeline_dsl: String::new(),
            pipeline_output: String::new(),
            working_dir: cwd,
            status_message: "Ready".to_string(),
            max_depth: 10,
        }
    }

    fn run_search(&mut self) {
        if self.search_pattern.is_empty() {
            return;
        }

        let config = GrepConfig {
            pattern: self.search_pattern.clone(),
            pattern_type: if self.search_literal {
                PatternType::Literal
            } else {
                PatternType::Regex
            },
            case_sensitive: !self.search_case_insensitive,
            whole_word: self.search_whole_word,
            context_before: self.search_context_lines,
            context_after: self.search_context_lines,
            max_matches: Some(200),
            ..Default::default()
        };

        let engine = match GrepEngine::new(config) {
            Ok(e) => e,
            Err(e) => {
                self.status_message = format!("Pattern error: {e}");
                return;
            }
        };

        let scope = FileScope {
            roots: vec![PathBuf::from(&self.working_dir)],
            include_globs: if self.search_include_glob.is_empty() {
                vec![]
            } else {
                vec![self.search_include_glob.clone()]
            },
            max_depth: Some(self.max_depth),
            respect_gitignore: true,
            ..Default::default()
        };

        let walker = match Walker::new(scope) {
            Ok(w) => w,
            Err(e) => {
                self.status_message = format!("Scope error: {e}");
                return;
            }
        };

        let files = match walker.collect_files() {
            Ok(f) => f,
            Err(e) => {
                self.status_message = format!("File error: {e}");
                return;
            }
        };

        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.search_files(&path_refs) {
            Ok(results) => {
                self.status_message = format!(
                    "Found {} matches in {} files ({} files scanned)",
                    results.total_matches,
                    results.files_with_matches,
                    files.len()
                );
                self.search_total = results.total_matches;
                self.search_files_matched = results.files_with_matches;
                self.search_results = results.matches;
            }
            Err(e) => {
                self.status_message = format!("Search error: {e}");
            }
        }
    }

    fn run_transform_preview(&mut self) {
        let cmd = if !self.transform_expression.is_empty() {
            match parse_substitution(&self.transform_expression) {
                Ok(c) => c,
                Err(e) => {
                    self.transform_preview = format!("Error: {e}");
                    return;
                }
            }
        } else if !self.transform_pattern.is_empty() {
            TransformCommand::Substitute {
                pattern: self.transform_pattern.clone(),
                replacement: self.transform_replacement.clone(),
                global: self.transform_global,
                case_insensitive: false,
            }
        } else {
            self.transform_preview = "Enter a pattern or expression".to_string();
            return;
        };

        let config = SedConfig {
            commands: vec![cmd],
            dry_run: true,
            ..Default::default()
        };

        let scope = FileScope {
            roots: vec![PathBuf::from(&self.working_dir)],
            max_depth: Some(self.max_depth),
            respect_gitignore: true,
            ..Default::default()
        };

        let walker = match Walker::new(scope) {
            Ok(w) => w,
            Err(e) => {
                self.transform_preview = format!("Scope error: {e}");
                return;
            }
        };

        let files = match walker.collect_files() {
            Ok(f) => f,
            Err(e) => {
                self.transform_preview = format!("File error: {e}");
                return;
            }
        };

        let engine = SedEngine::new(config);
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.transform_files(&path_refs) {
            Ok(results) => {
                self.transform_preview = format_transform_human(&results, false);
                self.status_message = format!(
                    "Preview: {} changes in {} files",
                    results.total_changes, results.files_modified
                );
            }
            Err(e) => {
                self.transform_preview = format!("Transform error: {e}");
            }
        }
    }

    fn run_transform_apply(&mut self) {
        let cmd = if !self.transform_expression.is_empty() {
            match parse_substitution(&self.transform_expression) {
                Ok(c) => c,
                Err(e) => {
                    self.transform_preview = format!("Error: {e}");
                    return;
                }
            }
        } else if !self.transform_pattern.is_empty() {
            TransformCommand::Substitute {
                pattern: self.transform_pattern.clone(),
                replacement: self.transform_replacement.clone(),
                global: self.transform_global,
                case_insensitive: false,
            }
        } else {
            self.transform_preview = "Enter a pattern or expression".to_string();
            return;
        };

        let config = SedConfig {
            commands: vec![cmd],
            dry_run: false,
            in_place: true,
            backup_extension: if self.transform_backup {
                Some(self.transform_backup_ext.clone())
            } else {
                None
            },
            ..Default::default()
        };

        let scope = FileScope {
            roots: vec![PathBuf::from(&self.working_dir)],
            max_depth: Some(self.max_depth),
            respect_gitignore: true,
            ..Default::default()
        };

        let walker = match Walker::new(scope) {
            Ok(w) => w,
            Err(e) => {
                self.transform_preview = format!("Scope error: {e}");
                return;
            }
        };

        let files = match walker.collect_files() {
            Ok(f) => f,
            Err(e) => {
                self.transform_preview = format!("File error: {e}");
                return;
            }
        };

        let engine = SedEngine::new(config);
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.transform_files(&path_refs) {
            Ok(results) => {
                self.transform_last_applied = format_transform_human(&results, false);
                self.transform_preview = format!(
                    "✅ Applied {} changes in {} files{}",
                    results.total_changes,
                    results.files_modified,
                    if self.transform_backup {
                        format!(" (backups with .{} extension)", self.transform_backup_ext)
                    } else {
                        String::new()
                    }
                );
                self.status_message = format!(
                    "Applied: {} changes in {} files",
                    results.total_changes, results.files_modified
                );
            }
            Err(e) => {
                self.transform_preview = format!("Apply error: {e}");
            }
        }
        self.transform_confirm_apply = false;
    }

    fn collect_scope_files(&self) -> Result<Vec<PathBuf>, String> {
        let scope = FileScope {
            roots: vec![PathBuf::from(&self.working_dir)],
            max_depth: Some(self.max_depth),
            respect_gitignore: true,
            ..Default::default()
        };
        let walker = Walker::new(scope).map_err(|e| format!("Scope error: {e}"))?;
        walker
            .collect_files()
            .map_err(|e| format!("File error: {e}"))
    }

    fn run_analyze(&mut self) {
        let sep = if self.analyze_separator.is_empty() {
            r"\s+".to_string()
        } else {
            regex::escape(&self.analyze_separator)
        };
        let config = AwkConfig {
            field_separator: sep,
            ..Default::default()
        };
        let engine = match AwkEngine::new(config) {
            Ok(e) => e,
            Err(e) => {
                self.analyze_output = format!("Config error: {e}");
                return;
            }
        };
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.analyze_output = e;
                return;
            }
        };
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match engine.process_files(&path_refs) {
            Ok(results) => {
                self.status_message = format!(
                    "Analyzed {} records from {} files",
                    results.records_processed,
                    files.len()
                );
                let mut output = String::new();
                for rec in results.output_records.iter().take(200) {
                    output.push_str(&format!(
                        "{}:{} | {}\n",
                        rec.source_file,
                        rec.source_line,
                        rec.fields.join("\t")
                    ));
                }
                if !results.aggregations.is_empty() {
                    output.push_str("\n--- Aggregations ---\n");
                    for (name, val) in &results.aggregations {
                        output.push_str(&format!("{name}: {val:?}\n"));
                    }
                }
                self.analyze_output = output;
            }
            Err(e) => self.analyze_output = format!("Analyze error: {e}"),
        }
    }

    fn run_pipeline(&mut self) {
        if self.pipeline_dsl.is_empty() {
            self.pipeline_output = "Enter a pipeline DSL expression".to_string();
            return;
        }
        let pipeline = match Pipeline::from_dsl(&self.pipeline_dsl) {
            Ok(p) => p,
            Err(e) => {
                self.pipeline_output = format!("Parse error: {e}");
                return;
            }
        };
        let files = match self.collect_scope_files() {
            Ok(f) => f,
            Err(e) => {
                self.pipeline_output = e;
                return;
            }
        };
        let path_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
        match pipeline.execute(&path_refs) {
            Ok(results) => {
                let mut output = String::new();
                for stage in &results.stages {
                    output.push_str(&format!(
                        "Stage {} [{}]: {}→{} records ({}ms)\n",
                        stage.stage_index,
                        stage.stage_type,
                        stage.records_in,
                        stage.records_out,
                        stage.duration_ms
                    ));
                }
                output.push_str(&format!("\nTotal: {}ms\n\n", results.total_duration_ms));
                output.push_str(
                    &serde_json::to_string_pretty(&results.final_output)
                        .unwrap_or_else(|_| results.final_output.to_string()),
                );
                self.status_message = format!(
                    "Pipeline: {} stages, {}ms",
                    results.stages.len(),
                    results.total_duration_ms
                );
                self.pipeline_output = output;
            }
            Err(e) => self.pipeline_output = format!("Pipeline error: {e}"),
        }
    }
}

impl eframe::App for AtpGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel with tabs
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, GuiTab::Search, "🔍 Search");
                ui.selectable_value(&mut self.active_tab, GuiTab::Transform, "✏️ Transform");
                ui.selectable_value(&mut self.active_tab, GuiTab::Analyze, "📊 Analyze");
                ui.selectable_value(&mut self.active_tab, GuiTab::Pipeline, "🔗 Pipeline");
                ui.selectable_value(&mut self.active_tab, GuiTab::Ontology, "📋 Ontology");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("ATP — Agentic Text Processor");
                });
            });
        });

        // Bottom panel with status
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("v{}", atp_core::ATP_VERSION));
                });
            });
        });

        // Left panel with settings
        egui::SidePanel::left("settings")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                ui.label("Working Directory:");
                ui.text_edit_singleline(&mut self.working_dir);
                ui.add(egui::Slider::new(&mut self.max_depth, 1..=20).text("Max depth"));
                ui.separator();

                match self.active_tab {
                    GuiTab::Search => {
                        ui.heading("Search Options");
                        ui.label("Pattern:");
                        let response = ui.text_edit_singleline(&mut self.search_pattern);
                        ui.checkbox(&mut self.search_case_insensitive, "Case insensitive");
                        ui.checkbox(&mut self.search_whole_word, "Whole word");
                        ui.checkbox(&mut self.search_literal, "Literal (no regex)");
                        ui.add(
                            egui::Slider::new(&mut self.search_context_lines, 0..=10)
                                .text("Context lines"),
                        );
                        ui.label("Include glob:");
                        ui.text_edit_singleline(&mut self.search_include_glob);

                        if ui.button("🔍 Search").clicked() || response.lost_focus() {
                            self.run_search();
                        }
                    }
                    GuiTab::Transform => {
                        ui.heading("Transform Options");
                        ui.label("Expression (s/pat/repl/flags):");
                        ui.text_edit_singleline(&mut self.transform_expression);
                        ui.separator();
                        ui.label("— or —");
                        ui.label("Pattern:");
                        ui.text_edit_singleline(&mut self.transform_pattern);
                        ui.label("Replacement:");
                        ui.text_edit_singleline(&mut self.transform_replacement);
                        ui.checkbox(&mut self.transform_global, "Global (all occurrences)");

                        ui.horizontal(|ui| {
                            if ui.button("👁 Preview").clicked() {
                                self.run_transform_preview();
                            }

                            if !self.transform_confirm_apply && ui.button("⚡ Apply").clicked() {
                                self.transform_confirm_apply = true;
                            }
                        });

                        // Apply confirmation dialog
                        if self.transform_confirm_apply {
                            ui.separator();
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 180, 0),
                                "⚠ This will modify files in-place. Are you sure?",
                            );
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.transform_backup, "Create backups");
                                if self.transform_backup {
                                    ui.label("Extension:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.transform_backup_ext)
                                            .desired_width(50.0),
                                    );
                                }
                            });
                            ui.horizontal(|ui| {
                                if ui.button("✅ Confirm Apply").clicked() {
                                    self.run_transform_apply();
                                }
                                if ui.button("❌ Cancel").clicked() {
                                    self.transform_confirm_apply = false;
                                }
                            });
                        }
                    }
                    _ => match self.active_tab {
                        GuiTab::Analyze => {
                            ui.heading("Analyze Options");
                            ui.label("Field Separator (empty = whitespace):");
                            ui.text_edit_singleline(&mut self.analyze_separator);
                            if ui.button("📊 Analyze").clicked() {
                                self.run_analyze();
                            }
                        }
                        GuiTab::Pipeline => {
                            ui.heading("Pipeline DSL");
                            ui.label("Expression:");
                            ui.text_edit_multiline(&mut self.pipeline_dsl);
                            ui.label("Format: stage:args | stage:args");
                            if ui.button("▶ Execute").clicked() {
                                self.run_pipeline();
                            }
                        }
                        _ => {
                            ui.label("Select a mode from the tabs above.");
                        }
                    },
                }
            });

        // Central panel with results
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                GuiTab::Search => {
                    ui.heading(format!(
                        "Results: {} matches in {} files",
                        self.search_total, self.search_files_matched
                    ));
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for m in &self.search_results {
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    format!("{}:{}:{}", m.file, m.line_number, m.column_start),
                                );
                                // Highlight matched text
                                let before = &m.line_content[..m.column_start.saturating_sub(1)];
                                let matched = &m.matched_text;
                                let after_start =
                                    m.column_end.saturating_sub(1).min(m.line_content.len());
                                let after = &m.line_content[after_start..];
                                ui.label(before);
                                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), matched);
                                ui.label(after);
                            });
                        }
                    });
                }
                GuiTab::Transform => {
                    ui.heading("Transform Preview (Dry Run)");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace(&self.transform_preview);
                    });
                }
                GuiTab::Analyze => {
                    ui.heading("Analyze Results");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace(&self.analyze_output);
                    });
                }
                GuiTab::Pipeline => {
                    ui.heading("Pipeline Results");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace(&self.pipeline_output);
                    });
                }
                GuiTab::Ontology => {
                    ui.heading("ATP Ontology");
                    ui.separator();
                    let ontology = atp_core::ontology::build_ontology();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Capabilities");
                        for cap in &ontology.capabilities {
                            ui.group(|ui| {
                                ui.strong(&cap.name);
                                ui.label(&cap.description);
                                ui.horizontal(|ui| {
                                    ui.label("Tags:");
                                    for tag in &cap.semantic_tags {
                                        ui.code(tag);
                                    }
                                });
                            });
                        }

                        ui.separator();
                        ui.heading("Commands");
                        for cmd in &ontology.commands {
                            ui.group(|ui| {
                                ui.strong(format!("atp {}", cmd.name));
                                if !cmd.aliases.is_empty() {
                                    ui.label(format!("Aliases: {}", cmd.aliases.join(", ")));
                                }
                                ui.label(&cmd.description);
                                ui.label(format!(
                                    "Deterministic: {} | Modifies files: {}",
                                    cmd.deterministic, cmd.modifies_files
                                ));
                            });
                        }
                    });
                }
            }
        });
    }
}
