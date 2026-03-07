//! Explain command — describe what a command will do before execution.

use anyhow::Result;
use clap::Args;

use atp_core::output::*;

#[derive(Args)]
pub struct ExplainArgs {
    /// The full atp command to explain (e.g. "search 'pattern' src/")
    pub command_string: Vec<String>,
}

pub fn execute(args: ExplainArgs, format: Format) -> Result<()> {
    let cmd_str = args.command_string.join(" ");
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();

    // Skip "atp" prefix if present
    let parts = if parts.first().copied() == Some("atp") {
        &parts[1..]
    } else {
        &parts[..]
    };

    let (_command, description, will_modify, steps, warnings) = match parts.first().copied() {
        Some("search") | Some("s") | Some("grep") | Some("find") => {
            let pattern = parts.get(1).unwrap_or(&"<pattern>");
            (
                "search",
                format!("Search for pattern '{}' in files", pattern),
                false,
                vec![
                    ExplainStep {
                        order: 1,
                        action: "resolve_scope".into(),
                        description: "Determine which files to search based on paths and glob filters".into(),
                    },
                    ExplainStep {
                        order: 2,
                        action: "compile_pattern".into(),
                        description: format!("Compile regex pattern '{}'", pattern),
                    },
                    ExplainStep {
                        order: 3,
                        action: "search_files".into(),
                        description: "Search each file for matches, collecting line numbers, columns, and context".into(),
                    },
                    ExplainStep {
                        order: 4,
                        action: "format_output".into(),
                        description: "Format results as strongly typed SearchResults".into(),
                    },
                ],
                vec![],
            )
        }
        Some("transform") | Some("t") | Some("sed") | Some("replace") => {
            let has_in_place = parts.contains(&"--in-place");
            let warnings = if has_in_place {
                vec![
                    "This command will modify files on disk. Use --backup to create backups."
                        .to_string(),
                ]
            } else {
                vec![
                    "Dry-run mode: no files will be modified. Use --in-place to apply changes."
                        .to_string(),
                ]
            };
            (
                "transform",
                "Apply text transformation to files".to_string(),
                has_in_place,
                vec![
                    ExplainStep {
                        order: 1,
                        action: "resolve_scope".into(),
                        description: "Determine which files to transform".into(),
                    },
                    ExplainStep {
                        order: 2,
                        action: "compile_transform".into(),
                        description: "Parse and validate the transform expression".into(),
                    },
                    ExplainStep {
                        order: 3,
                        action: "apply_transforms".into(),
                        description: if has_in_place {
                            "Apply transformations to files (IN-PLACE)".into()
                        } else {
                            "Compute transformations (DRY RUN — no files modified)".into()
                        },
                    },
                    ExplainStep {
                        order: 4,
                        action: "format_output".into(),
                        description: "Format results as strongly typed TransformResults with diffs"
                            .into(),
                    },
                ],
                warnings,
            )
        }
        Some("analyze") | Some("a") | Some("awk") => (
            "analyze",
            "Analyze text with field-based processing".to_string(),
            false,
            vec![
                ExplainStep {
                    order: 1,
                    action: "resolve_scope".into(),
                    description: "Determine which files to analyze".into(),
                },
                ExplainStep {
                    order: 2,
                    action: "configure_fields".into(),
                    description: "Set up field separator and selection".into(),
                },
                ExplainStep {
                    order: 3,
                    action: "process_records".into(),
                    description: "Process each line as a record, extracting and computing fields"
                        .into(),
                },
                ExplainStep {
                    order: 4,
                    action: "aggregate".into(),
                    description: "Compute any requested aggregations".into(),
                },
            ],
            vec![],
        ),
        Some("pipeline") | Some("pipe") => (
            "pipeline",
            "Execute multi-stage processing pipeline".to_string(),
            false,
            vec![
                ExplainStep {
                    order: 1,
                    action: "parse_pipeline".into(),
                    description: "Parse pipeline DSL or definition file".into(),
                },
                ExplainStep {
                    order: 2,
                    action: "validate_stages".into(),
                    description: "Validate each stage's configuration and type compatibility"
                        .into(),
                },
                ExplainStep {
                    order: 3,
                    action: "execute_stages".into(),
                    description:
                        "Execute each pipeline stage sequentially, passing data between stages"
                            .into(),
                },
            ],
            vec![],
        ),
        Some("query") | Some("q") | Some("aql") | Some("run") => {
            let query_str = parts[1..].join(" ");
            let (description, steps) =
                if let Ok(pipeline) = atp_core::engine::aql::parse(&query_str) {
                    let explanation = pipeline.explain();
                    let steps: Vec<ExplainStep> = explanation
                        .iter()
                        .enumerate()
                        .map(|(i, desc)| ExplainStep {
                            order: i + 1,
                            action: pipeline.stages[i].stage_name().to_string(),
                            description: desc.clone(),
                        })
                        .collect();
                    (
                        format!(
                            "Execute AQL pipeline with {} stage(s): {}",
                            pipeline.stages.len(),
                            query_str
                        ),
                        steps,
                    )
                } else {
                    (
                        format!("Execute AQL query: {}", query_str),
                        vec![ExplainStep {
                            order: 1,
                            action: "parse_aql".into(),
                            description: "Parse and execute the AQL query pipeline".into(),
                        }],
                    )
                };
            ("query", description, false, steps, vec![])
        }
        Some("scope") | Some("ls") | Some("files") => (
            "scope",
            "List files matching the current scope configuration".to_string(),
            false,
            vec![
                ExplainStep {
                    order: 1,
                    action: "resolve_scope".into(),
                    description:
                        "Walk the directory tree applying include/exclude globs and depth limits"
                            .into(),
                },
                ExplainStep {
                    order: 2,
                    action: "collect_files".into(),
                    description: "Collect matching files sorted deterministically".into(),
                },
                ExplainStep {
                    order: 3,
                    action: "format_output".into(),
                    description: "Output file list with sizes and count".into(),
                },
            ],
            vec![],
        ),
        Some("validate") | Some("check") => (
            "validate",
            "Validate input syntax without executing".to_string(),
            false,
            vec![
                ExplainStep {
                    order: 1,
                    action: "detect_type".into(),
                    description: "Determine input type: pattern, expression, pipeline, or aql"
                        .into(),
                },
                ExplainStep {
                    order: 2,
                    action: "parse_input".into(),
                    description: "Parse the input and check for syntax errors".into(),
                },
                ExplainStep {
                    order: 3,
                    action: "report".into(),
                    description: "Report validity, errors, and normalized form".into(),
                },
            ],
            vec![],
        ),
        Some("context") | Some("ctx") => (
            "context",
            "Extract smart context window around a target line".to_string(),
            false,
            vec![
                ExplainStep {
                    order: 1,
                    action: "read_file".into(),
                    description: "Read the target file contents".into(),
                },
                ExplainStep {
                    order: 2,
                    action: "detect_scope".into(),
                    description:
                        "Determine the context boundary (function, block, indent, or lines)".into(),
                },
                ExplainStep {
                    order: 3,
                    action: "extract_context".into(),
                    description: "Extract lines within the detected scope".into(),
                },
            ],
            vec![],
        ),
        Some("ontology") | Some("onto") | Some("capabilities") | Some("schema") => (
            "ontology",
            "Output the machine-readable ATP ontology for agent discovery".to_string(),
            false,
            vec![
                ExplainStep {
                    order: 1,
                    action: "build_ontology".into(),
                    description:
                        "Assemble the complete ontology (capabilities, commands, types, errors)"
                            .into(),
                },
                ExplainStep {
                    order: 2,
                    action: "filter".into(),
                    description: "Apply --command or --section filters if specified".into(),
                },
                ExplainStep {
                    order: 3,
                    action: "format_output".into(),
                    description: "Serialize to the requested output format".into(),
                },
            ],
            vec![],
        ),
        _ => {
            anyhow::bail!(
                "Cannot explain unknown command: '{}'. \
                 Available commands: search, transform, analyze, pipeline, query, \
                 scope, validate, context, ontology",
                parts.first().unwrap_or(&"<empty>")
            );
        }
    };

    let result = ExplainResult {
        command: cmd_str,
        description,
        will_modify_files: will_modify,
        estimated_files_affected: 0,
        steps,
        warnings,
    };

    match format {
        Format::Human => {
            println!("Command: {}", result.command);
            println!("Description: {}", result.description);
            println!("Modifies files: {}", result.will_modify_files);
            println!("\nSteps:");
            for step in &result.steps {
                println!("  {}. [{}] {}", step.order, step.action, step.description);
            }
            if !result.warnings.is_empty() {
                println!("\nWarnings:");
                for w in &result.warnings {
                    println!("  ⚠ {w}");
                }
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
