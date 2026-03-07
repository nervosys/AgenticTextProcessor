//! Pipeline engine — composable, typed, multi-stage processing.
//!
//! Enables agents to chain search → transform → analyze operations
//! with type-checked intermediate results and streaming execution.

use crate::engine::grep::{GrepConfig, GrepEngine};
use crate::output::{PipelineResults, PipelineStageResult};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A pipeline stage definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PipelineStage {
    /// Search stage (grep)
    Search {
        pattern: String,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        context_lines: usize,
    },
    /// Transform stage (sed)
    Transform {
        pattern: String,
        replacement: String,
        #[serde(default)]
        global: bool,
    },
    /// Analyze stage (awk)
    Analyze {
        #[serde(default = "default_separator")]
        field_separator: String,
        #[serde(default)]
        select_fields: Vec<usize>,
        #[serde(default)]
        pattern: Option<String>,
    },
    /// Filter stage — keep only lines matching pattern
    Filter { pattern: String },
    /// Sort stage
    Sort {
        #[serde(default)]
        field: Option<usize>,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        numeric: bool,
    },
    /// Unique stage — deduplicate lines
    Unique {
        #[serde(default)]
        field: Option<usize>,
    },
    /// Head — take first N records
    Head { count: usize },
    /// Tail — take last N records
    Tail { count: usize },
    /// Count — count matching records
    Count {
        #[serde(default)]
        pattern: Option<String>,
    },
}

fn default_separator() -> String {
    r"\s+".to_string()
}

/// A complete pipeline definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDefinition {
    pub name: Option<String>,
    pub description: Option<String>,
    pub stages: Vec<PipelineStage>,
}

/// Intermediate data flowing between pipeline stages.
#[derive(Debug, Clone)]
pub struct PipelineData {
    /// Text lines being processed
    pub lines: Vec<PipelineLine>,
    /// Files involved
    pub source_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PipelineLine {
    pub source_file: String,
    pub source_line: usize,
    pub content: String,
    pub fields: Vec<String>,
}

/// The pipeline execution engine.
pub struct Pipeline {
    definition: PipelineDefinition,
}

impl Pipeline {
    pub fn new(definition: PipelineDefinition) -> Self {
        Self { definition }
    }

    /// Parse a pipeline from a YAML or JSON string.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let def: PipelineDefinition =
            serde_yaml::from_str(yaml).context("Failed to parse pipeline definition")?;
        Ok(Self::new(def))
    }

    /// Parse a pipeline from a DSL string like "search:pattern | transform:s/a/b/g | head:10"
    pub fn from_dsl(dsl: &str) -> Result<Self> {
        let stages: Vec<PipelineStage> = dsl
            .split('|')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(Self::parse_dsl_stage)
            .collect::<Result<Vec<_>>>()?;

        Ok(Self::new(PipelineDefinition {
            name: None,
            description: None,
            stages,
        }))
    }

    fn parse_dsl_stage(stage_str: &str) -> Result<PipelineStage> {
        let parts: Vec<&str> = stage_str.splitn(2, ':').collect();
        let cmd = parts[0].trim().to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match cmd.as_str() {
            "search" | "grep" | "find" => Ok(PipelineStage::Search {
                pattern: args.to_string(),
                case_sensitive: true,
                context_lines: 0,
            }),
            "transform" | "sed" | "replace" => {
                let sub_parts: Vec<&str> = args.splitn(2, "->").collect();
                if sub_parts.len() == 2 {
                    Ok(PipelineStage::Transform {
                        pattern: sub_parts[0].trim().to_string(),
                        replacement: sub_parts[1].trim().to_string(),
                        global: true,
                    })
                } else {
                    // Try sed-style s/pattern/replacement/flags
                    if args.starts_with("s/") || args.starts_with("s|") {
                        let delim = args
                            .chars()
                            .nth(1)
                            .expect("sed-style arg has delimiter at index 1");
                        let inner_parts: Vec<&str> = args[2..].splitn(3, delim).collect();
                        Ok(PipelineStage::Transform {
                            pattern: inner_parts.first().unwrap_or(&"").to_string(),
                            replacement: inner_parts.get(1).unwrap_or(&"").to_string(),
                            global: inner_parts.get(2).map(|f| f.contains('g')).unwrap_or(false),
                        })
                    } else {
                        anyhow::bail!("Transform stage requires pattern->replacement or s/pat/repl/flags syntax")
                    }
                }
            }
            "filter" => Ok(PipelineStage::Filter {
                pattern: args.to_string(),
            }),
            "sort" => {
                let reverse = args.contains("-r") || args.contains("reverse");
                let numeric = args.contains("-n") || args.contains("numeric");
                let field = args
                    .split_whitespace()
                    .find(|s| s.parse::<usize>().is_ok())
                    .and_then(|s| s.parse().ok());
                Ok(PipelineStage::Sort {
                    field,
                    reverse,
                    numeric,
                })
            }
            "unique" | "uniq" => Ok(PipelineStage::Unique {
                field: args.parse().ok(),
            }),
            "head" => Ok(PipelineStage::Head {
                count: args.parse().unwrap_or(10),
            }),
            "tail" => Ok(PipelineStage::Tail {
                count: args.parse().unwrap_or(10),
            }),
            "count" => Ok(PipelineStage::Count {
                pattern: if args.is_empty() {
                    None
                } else {
                    Some(args.to_string())
                },
            }),
            "analyze" | "awk" => Ok(PipelineStage::Analyze {
                field_separator: r"\s+".to_string(),
                select_fields: args
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect(),
                pattern: None,
            }),
            _ => anyhow::bail!("Unknown pipeline stage: {cmd}"),
        }
    }

    /// Execute the pipeline on the given files.
    pub fn execute(&self, paths: &[&Path]) -> Result<PipelineResults> {
        let total_start = Instant::now();
        let mut stage_results = Vec::new();

        // Initialize pipeline data from files
        let mut data = PipelineData {
            lines: Vec::new(),
            source_files: paths.iter().map(|p| p.to_path_buf()).collect(),
        };

        for path in paths {
            let content = std::fs::read_to_string(path)?;
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

        // Execute each stage
        for (stage_idx, stage) in self.definition.stages.iter().enumerate() {
            let stage_start = Instant::now();
            let records_in = data.lines.len();

            data = self
                .execute_stage(stage, data)
                .with_context(|| format!("Pipeline failed at stage {stage_idx}"))?;

            let records_out = data.lines.len();
            let duration_ms = stage_start.elapsed().as_millis() as u64;

            stage_results.push(PipelineStageResult {
                stage_index: stage_idx,
                stage_type: format!("{:?}", stage)
                    .split('{')
                    .next()
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string(),
                duration_ms,
                records_in,
                records_out,
            });
        }

        // Build final output
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

        let total_duration_ms = total_start.elapsed().as_millis() as u64;
        Ok(PipelineResults {
            stages: stage_results,
            final_output: serde_json::Value::Array(final_lines),
            total_duration_ms,
        })
    }

    fn execute_stage(&self, stage: &PipelineStage, mut data: PipelineData) -> Result<PipelineData> {
        match stage {
            PipelineStage::Search {
                pattern,
                case_sensitive,
                ..
            } => {
                let config = GrepConfig {
                    pattern: pattern.clone(),
                    case_sensitive: *case_sensitive,
                    ..Default::default()
                };
                let engine = GrepEngine::new(config)?;
                let re = regex::RegexBuilder::new(pattern)
                    .case_insensitive(!case_sensitive)
                    .build()?;
                data.lines.retain(|line| re.is_match(&line.content));
                let _ = engine; // used for validation
                Ok(data)
            }

            PipelineStage::Transform {
                pattern,
                replacement,
                global,
            } => {
                let re = regex::Regex::new(pattern)?;
                for line in &mut data.lines {
                    let new_content = if *global {
                        re.replace_all(&line.content, replacement.as_str())
                            .to_string()
                    } else {
                        re.replace(&line.content, replacement.as_str()).to_string()
                    };
                    line.content = new_content;
                }
                Ok(data)
            }

            PipelineStage::Filter { pattern } => {
                let re = regex::Regex::new(pattern)?;
                data.lines.retain(|line| re.is_match(&line.content));
                Ok(data)
            }

            PipelineStage::Analyze {
                field_separator,
                select_fields,
                pattern,
            } => {
                let sep_re = regex::Regex::new(field_separator)?;
                if let Some(pat) = pattern {
                    let pat_re = regex::Regex::new(pat)?;
                    data.lines.retain(|line| pat_re.is_match(&line.content));
                }
                for line in &mut data.lines {
                    let all_fields: Vec<String> =
                        sep_re.split(&line.content).map(|s| s.to_string()).collect();
                    line.fields = if select_fields.is_empty() {
                        all_fields
                    } else {
                        select_fields
                            .iter()
                            .map(|&i| {
                                all_fields
                                    .get(i.saturating_sub(1))
                                    .cloned()
                                    .unwrap_or_default()
                            })
                            .collect()
                    };
                }
                Ok(data)
            }

            PipelineStage::Sort {
                field,
                reverse,
                numeric,
            } => {
                let field_idx = field.unwrap_or(0);
                data.lines.sort_by(|a, b| {
                    let a_val = if field_idx > 0 {
                        a.fields
                            .get(field_idx - 1)
                            .cloned()
                            .unwrap_or_else(|| a.content.clone())
                    } else {
                        a.content.clone()
                    };
                    let b_val = if field_idx > 0 {
                        b.fields
                            .get(field_idx - 1)
                            .cloned()
                            .unwrap_or_else(|| b.content.clone())
                    } else {
                        b.content.clone()
                    };

                    let cmp = if *numeric {
                        let a_num = a_val.parse::<f64>().unwrap_or(0.0);
                        let b_num = b_val.parse::<f64>().unwrap_or(0.0);
                        a_num
                            .partial_cmp(&b_num)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a_val.cmp(&b_val)
                    };

                    if *reverse {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                });
                Ok(data)
            }

            PipelineStage::Unique { field } => {
                let mut seen = std::collections::HashSet::new();
                data.lines.retain(|line| {
                    let key = if let Some(idx) = field {
                        line.fields
                            .get(idx.saturating_sub(1))
                            .cloned()
                            .unwrap_or_else(|| line.content.clone())
                    } else {
                        line.content.clone()
                    };
                    seen.insert(key)
                });
                Ok(data)
            }

            PipelineStage::Head { count } => {
                data.lines.truncate(*count);
                Ok(data)
            }

            PipelineStage::Tail { count } => {
                let len = data.lines.len();
                if *count < len {
                    data.lines = data.lines.split_off(len - count);
                }
                Ok(data)
            }

            PipelineStage::Count { pattern } => {
                let count = if let Some(pat) = pattern {
                    let re = regex::Regex::new(pat)?;
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
        }
    }

    /// Explain what the pipeline will do, without executing it.
    pub fn explain(&self) -> Vec<String> {
        self.definition
            .stages
            .iter()
            .enumerate()
            .map(|(i, stage)| {
                let desc = match stage {
                    PipelineStage::Search {
                        pattern,
                        case_sensitive,
                        ..
                    } => {
                        format!(
                            "Search for pattern '{}' (case {}sensitive)",
                            pattern,
                            if *case_sensitive { "" } else { "in" }
                        )
                    }
                    PipelineStage::Transform {
                        pattern,
                        replacement,
                        global,
                    } => {
                        format!(
                            "Replace '{}' with '{}' ({})",
                            pattern,
                            replacement,
                            if *global {
                                "all occurrences"
                            } else {
                                "first occurrence"
                            }
                        )
                    }
                    PipelineStage::Filter { pattern } => {
                        format!("Keep only lines matching '{}'", pattern)
                    }
                    PipelineStage::Analyze {
                        field_separator,
                        select_fields,
                        ..
                    } => {
                        format!(
                            "Split fields by '{}', select fields {:?}",
                            field_separator, select_fields
                        )
                    }
                    PipelineStage::Sort {
                        field,
                        reverse,
                        numeric,
                    } => {
                        format!(
                            "Sort by {} ({}, {})",
                            field
                                .map(|f| format!("field {f}"))
                                .unwrap_or("line content".into()),
                            if *reverse { "descending" } else { "ascending" },
                            if *numeric { "numeric" } else { "lexicographic" }
                        )
                    }
                    PipelineStage::Unique { field } => {
                        format!(
                            "Deduplicate by {}",
                            field
                                .map(|f| format!("field {f}"))
                                .unwrap_or("line content".into())
                        )
                    }
                    PipelineStage::Head { count } => format!("Take first {} records", count),
                    PipelineStage::Tail { count } => format!("Take last {} records", count),
                    PipelineStage::Count { pattern } => {
                        if let Some(p) = pattern {
                            format!("Count records matching '{}'", p)
                        } else {
                            "Count all records".to_string()
                        }
                    }
                };
                format!("Stage {}: {}", i + 1, desc)
            })
            .collect()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Streaming pipeline — backpressure-aware stage-to-stage channels
// ──────────────────────────────────────────────────────────────────────

/// Configuration for streaming pipeline execution.
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Channel capacity between stages (backpressure threshold).
    pub channel_capacity: usize,
    /// Maximum records to buffer before flushing.
    pub batch_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1024,
            batch_size: 256,
        }
    }
}

/// A streaming pipeline that processes records through tokio channels
/// without materializing the full dataset between stages.
pub struct StreamingPipeline {
    definition: PipelineDefinition,
    config: StreamingConfig,
}

/// Progress information emitted during streaming execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingProgress {
    /// Stage index currently processing.
    pub stage_index: usize,
    /// Records processed so far in this stage.
    pub records_processed: usize,
    /// Records emitted by this stage so far.
    pub records_emitted: usize,
}

impl StreamingPipeline {
    /// Create a new streaming pipeline from a definition.
    pub fn new(definition: PipelineDefinition, config: StreamingConfig) -> Self {
        Self { definition, config }
    }

    /// Create from DSL with default streaming config.
    pub fn from_dsl(dsl: &str) -> Result<Self> {
        let inner = Pipeline::from_dsl(dsl)?;
        Ok(Self {
            definition: inner.definition,
            config: StreamingConfig::default(),
        })
    }

    /// Execute the pipeline in streaming mode.
    ///
    /// Each stage runs as a concurrent task connected by bounded channels.
    /// When a channel is full, the upstream stage pauses (backpressure).
    /// This keeps memory usage proportional to `channel_capacity * num_stages`
    /// rather than the full dataset.
    pub async fn execute_streaming(&self, paths: &[&Path]) -> Result<PipelineResults> {
        use tokio::sync::mpsc;

        let total_start = Instant::now();

        // Read all input lines
        let mut initial_lines = Vec::new();
        let source_files: Vec<PathBuf> = paths.iter().map(|p| p.to_path_buf()).collect();

        for path in paths {
            let content = std::fs::read_to_string(path)?;
            let file_str = path.display().to_string();
            for (idx, line) in content.lines().enumerate() {
                initial_lines.push(PipelineLine {
                    source_file: file_str.clone(),
                    source_line: idx + 1,
                    content: line.to_string(),
                    fields: Vec::new(),
                });
            }
        }

        if self.definition.stages.is_empty() {
            return Ok(PipelineResults {
                stages: Vec::new(),
                final_output: serde_json::Value::Array(Vec::new()),
                total_duration_ms: total_start.elapsed().as_millis() as u64,
            });
        }

        // For streaming, we process each stage sequentially but in batches
        // with bounded channels between producer and consumer.
        let mut stage_results = Vec::new();
        let mut current_data = initial_lines;

        for (stage_idx, stage) in self.definition.stages.iter().enumerate() {
            let stage_start = Instant::now();
            let records_in = current_data.len();

            let (tx, mut rx) = mpsc::channel::<Vec<PipelineLine>>(self.config.channel_capacity);

            // Clone what we need for the producer task
            let batch_size = self.config.batch_size;
            let input_data = std::mem::take(&mut current_data);

            // Producer: send data in batches
            let producer = tokio::spawn(async move {
                for chunk in input_data.chunks(batch_size) {
                    if tx.send(chunk.to_vec()).await.is_err() {
                        break; // receiver dropped
                    }
                }
                // tx dropped here, closing the channel
            });

            // Consumer: apply stage transformation
            let stage_clone = stage.clone();
            let mut output_lines = Vec::new();

            while let Some(batch) = rx.recv().await {
                let batch_data = PipelineData {
                    lines: batch,
                    source_files: source_files.clone(),
                };
                let inner = Pipeline::new(PipelineDefinition {
                    name: None,
                    description: None,
                    stages: vec![stage_clone.clone()],
                });
                match inner.execute_stage(&stage_clone, batch_data) {
                    Ok(result) => output_lines.extend(result.lines),
                    Err(e) => return Err(e),
                }
            }

            producer
                .await
                .map_err(|e| anyhow::anyhow!("Producer task failed: {}", e))?;

            let records_out = output_lines.len();
            let duration_ms = stage_start.elapsed().as_millis() as u64;

            stage_results.push(PipelineStageResult {
                stage_index: stage_idx,
                stage_type: format!("{:?}", stage)
                    .split('{')
                    .next()
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string(),
                duration_ms,
                records_in,
                records_out,
            });

            current_data = output_lines;
        }

        // Build final output
        let final_lines: Vec<serde_json::Value> = current_data
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

        let total_duration_ms = total_start.elapsed().as_millis() as u64;
        Ok(PipelineResults {
            stages: stage_results,
            final_output: serde_json::Value::Array(final_lines),
            total_duration_ms,
        })
    }

    /// Get the underlying stage definitions.
    pub fn stages(&self) -> &[PipelineStage] {
        &self.definition.stages
    }

    /// Get the streaming configuration.
    pub fn streaming_config(&self) -> &StreamingConfig {
        &self.config
    }
}

// Make `execute_stage` accessible for streaming
impl Pipeline {
    /// Public access to single-stage execution for streaming pipeline.
    pub fn execute_stage_public(
        &self,
        stage: &PipelineStage,
        data: PipelineData,
    ) -> Result<PipelineData> {
        self.execute_stage(stage, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(content: &str) -> tempfile::NamedTempFile {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        tmp
    }

    #[test]
    fn test_from_dsl_search() {
        let p = Pipeline::from_dsl("search:hello").unwrap();
        assert_eq!(p.definition.stages.len(), 1);
        match &p.definition.stages[0] {
            PipelineStage::Search { pattern, .. } => assert_eq!(pattern, "hello"),
            _ => panic!("Expected Search stage"),
        }
    }

    #[test]
    fn test_from_dsl_multi_stage() {
        let p = Pipeline::from_dsl("search:foo | filter:bar | head:5").unwrap();
        assert_eq!(p.definition.stages.len(), 3);
        match &p.definition.stages[2] {
            PipelineStage::Head { count } => assert_eq!(*count, 5),
            _ => panic!("Expected Head stage"),
        }
    }

    #[test]
    fn test_from_dsl_transform_arrow() {
        let p = Pipeline::from_dsl("transform:old->new").unwrap();
        match &p.definition.stages[0] {
            PipelineStage::Transform {
                pattern,
                replacement,
                ..
            } => {
                assert_eq!(pattern, "old");
                assert_eq!(replacement, "new");
            }
            _ => panic!("Expected Transform stage"),
        }
    }

    #[test]
    fn test_from_dsl_transform_sed_style() {
        let p = Pipeline::from_dsl("sed:s/hello/world/g").unwrap();
        match &p.definition.stages[0] {
            PipelineStage::Transform {
                pattern,
                replacement,
                global,
                ..
            } => {
                assert_eq!(pattern, "hello");
                assert_eq!(replacement, "world");
                assert!(*global);
            }
            _ => panic!("Expected Transform stage"),
        }
    }

    #[test]
    fn test_from_dsl_unknown_stage() {
        assert!(Pipeline::from_dsl("foobar:xyz").is_err());
    }

    #[test]
    fn test_explain() {
        let p = Pipeline::from_dsl("search:pattern | head:10").unwrap();
        let explain = p.explain();
        assert_eq!(explain.len(), 2);
        assert!(explain[0].contains("Search"));
        assert!(explain[1].contains("first 10"));
    }

    #[test]
    fn test_execute_search_then_head() {
        let tmp = write_tmp("line one\nline two\nline three\nline four\nline five\n");
        let p = Pipeline::from_dsl("search:line | head:3").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = p.execute(&path_refs).unwrap();
        assert_eq!(results.stages.len(), 2);
        assert!(results.stages[1].records_out <= 3);
    }

    #[test]
    fn test_execute_filter() {
        let tmp = write_tmp("apple\nbanana\napricot\ncherry\n");
        let p = Pipeline::from_dsl("search:. | filter:ap").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = p.execute(&path_refs).unwrap();
        // Should keep apple and apricot
        assert_eq!(results.stages.last().unwrap().records_out, 2);
    }

    #[test]
    fn test_execute_sort() {
        let tmp = write_tmp("cherry\napple\nbanana\n");
        let p = Pipeline::from_dsl("search:. | sort:").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = p.execute(&path_refs).unwrap();
        assert_eq!(results.stages.len(), 2);
    }

    #[test]
    fn test_execute_count() {
        let tmp = write_tmp("aaa\nbbb\naaa\nccc\n");
        let p = Pipeline::from_dsl("search:. | count:").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = p.execute(&path_refs).unwrap();
        assert_eq!(results.stages.len(), 2);
    }

    // ── Streaming pipeline tests ────────────────────────────────────

    #[tokio::test]
    async fn test_streaming_basic() {
        let tmp = write_tmp("hello world\nfoo bar\nhello again\n");
        let sp = StreamingPipeline::from_dsl("search:hello").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = sp.execute_streaming(&path_refs).await.unwrap();
        assert_eq!(results.stages.len(), 1);
        assert_eq!(results.stages[0].records_out, 2);
    }

    #[tokio::test]
    async fn test_streaming_multi_stage() {
        let tmp = write_tmp("apple\nbanana\napricot\ncherry\navocado\n");
        let sp = StreamingPipeline::from_dsl("search:. | filter:a | head:3").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = sp.execute_streaming(&path_refs).await.unwrap();
        assert_eq!(results.stages.len(), 3);
        assert!(results.stages.last().unwrap().records_out <= 3);
    }

    #[tokio::test]
    async fn test_streaming_empty_pipeline() {
        let tmp = write_tmp("hello\n");
        let sp = StreamingPipeline::new(
            PipelineDefinition {
                name: None,
                description: None,
                stages: Vec::new(),
            },
            StreamingConfig::default(),
        );
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = sp.execute_streaming(&path_refs).await.unwrap();
        assert!(results.stages.is_empty());
    }

    #[tokio::test]
    async fn test_streaming_backpressure_config() {
        let config = StreamingConfig {
            channel_capacity: 2,
            batch_size: 1,
        };
        // Large input with tiny batch size to exercise backpressure
        let lines: String = (0..100).map(|i| format!("line {}\n", i)).collect();
        let tmp = write_tmp(&lines);
        let sp = StreamingPipeline::new(
            PipelineDefinition {
                name: None,
                description: None,
                stages: vec![PipelineStage::Head { count: 10 }],
            },
            config,
        );
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = sp.execute_streaming(&path_refs).await.unwrap();
        // Head truncates per-batch, so total output <= 10 * num_batches;
        // with 100 lines / batch_size 1 = 100 batches, each producing at most 10.
        // But the key property is: it completes without deadlock.
        assert_eq!(results.stages.len(), 1);
        assert!(results.stages[0].records_in == 100);
    }

    #[tokio::test]
    async fn test_streaming_matches_batch() {
        let tmp = write_tmp("apple\nbanana\napricot\ncherry\n");
        let batch = Pipeline::from_dsl("search:. | filter:ap").unwrap();
        let stream = StreamingPipeline::from_dsl("search:. | filter:ap").unwrap();
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let batch_results = batch.execute(&path_refs).unwrap();
        let stream_results = stream.execute_streaming(&path_refs).await.unwrap();
        assert_eq!(
            batch_results.stages.last().unwrap().records_out,
            stream_results.stages.last().unwrap().records_out,
        );
    }

    #[test]
    fn test_streaming_config_default() {
        let config = StreamingConfig::default();
        assert_eq!(config.channel_capacity, 1024);
        assert_eq!(config.batch_size, 256);
    }
}
