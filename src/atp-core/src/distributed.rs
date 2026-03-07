//! Distributed pipelines — scatter/gather AQL execution across nodes.
//!
//! Extends the remote execution model with proper distributed pipeline
//! semantics: partitioning input, scatter to workers, gather results,
//! and merge/reduce across nodes.
//!
//! # Architecture
//!
//! - [`PartitionStrategy`]: How to split work across nodes
//! - [`ScatterGatherPlan`]: A mapped execution plan
//! - [`NodeAssignment`]: Which node processes which partition
//! - [`DistributedPipeline`]: Orchestrates the entire scatter/gather flow
//! - [`MergeStrategy`]: How to combine results from all nodes
//!
//! # Example
//!
//! ```text
//! atp distributed run 'grep "ERROR" | freq' \
//!     --hosts node1,node2,node3 \
//!     --partition by-file \
//!     --merge concat-sort
//! ```

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// How to partition work across remote nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartitionStrategy {
    /// Each node processes a subset of files (round-robin assignment).
    ByFile,
    /// Each node processes a range of lines from all files.
    ByLineRange,
    /// Each node processes a random sample of the input.
    BySample,
    /// Duplicate the full input to every node (broadcast).
    Broadcast,
    /// Partition by file type / extension.
    ByFileType,
}

impl PartitionStrategy {
    /// Get a description of this strategy.
    pub fn description(&self) -> &'static str {
        match self {
            PartitionStrategy::ByFile => "Round-robin file assignment",
            PartitionStrategy::ByLineRange => "Line range partitioning",
            PartitionStrategy::BySample => "Random sampling",
            PartitionStrategy::Broadcast => "Full broadcast to all nodes",
            PartitionStrategy::ByFileType => "Group by file extension",
        }
    }
}

/// How to merge/reduce results from multiple nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    /// Concatenate all outputs.
    Concat,
    /// Concatenate and sort.
    ConcatSort,
    /// Concatenate and deduplicate.
    ConcatDedup,
    /// Sum numeric results.
    Sum,
    /// Average numeric results.
    Average,
    /// Take the first non-empty result.
    FirstNonEmpty,
    /// Merge frequency maps (add counts).
    MergeFrequency,
}

impl MergeStrategy {
    /// Apply this merge strategy to a list of node outputs.
    pub fn apply(&self, outputs: &[NodeResult]) -> Vec<String> {
        let all_lines: Vec<&str> = outputs
            .iter()
            .flat_map(|o| o.output_lines.iter().map(|s| s.as_str()))
            .collect();

        match self {
            MergeStrategy::Concat => all_lines.iter().map(|s| s.to_string()).collect(),
            MergeStrategy::ConcatSort => {
                let mut sorted: Vec<String> = all_lines.iter().map(|s| s.to_string()).collect();
                sorted.sort();
                sorted
            }
            MergeStrategy::ConcatDedup => {
                let mut deduped: Vec<String> = all_lines.iter().map(|s| s.to_string()).collect();
                deduped.sort();
                deduped.dedup();
                deduped
            }
            MergeStrategy::Sum => {
                let total: f64 = all_lines
                    .iter()
                    .filter_map(|l| l.trim().parse::<f64>().ok())
                    .sum();
                vec![total.to_string()]
            }
            MergeStrategy::Average => {
                let nums: Vec<f64> = all_lines
                    .iter()
                    .filter_map(|l| l.trim().parse::<f64>().ok())
                    .collect();
                if nums.is_empty() {
                    vec!["0".into()]
                } else {
                    let avg = nums.iter().sum::<f64>() / nums.len() as f64;
                    vec![avg.to_string()]
                }
            }
            MergeStrategy::FirstNonEmpty => {
                for o in outputs {
                    if !o.output_lines.is_empty() {
                        return o.output_lines.clone();
                    }
                }
                Vec::new()
            }
            MergeStrategy::MergeFrequency => {
                // Expects lines like "count\tvalue" or "count value"
                let mut freq: HashMap<String, u64> = HashMap::new();
                for line in &all_lines {
                    let parts: Vec<&str> = line.splitn(2, ['\t', ' ']).collect();
                    if parts.len() == 2 {
                        if let Ok(count) = parts[0].trim().parse::<u64>() {
                            *freq.entry(parts[1].to_string()).or_default() += count;
                        }
                    }
                }
                let mut entries: Vec<(u64, String)> =
                    freq.into_iter().map(|(k, v)| (v, k)).collect();
                entries.sort_by(|a, b| b.0.cmp(&a.0));
                entries
                    .iter()
                    .map(|(count, val)| format!("{}\t{}", count, val))
                    .collect()
            }
        }
    }
}

/// Assignment of a partition to a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAssignment {
    /// Node identifier (host string).
    pub node: String,
    /// Files assigned to this node.
    pub files: Vec<PathBuf>,
    /// Optional line range (start, end) for line-range partitioning.
    pub line_range: Option<(usize, usize)>,
    /// Partition index (0-based).
    pub partition_index: usize,
}

/// Result from a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// Node identifier.
    pub node: String,
    /// Partition index.
    pub partition_index: usize,
    /// Output lines.
    pub output_lines: Vec<String>,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
    /// Files processed.
    pub files_processed: usize,
    /// Whether execution succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// A scatter/gather execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScatterGatherPlan {
    /// The AQL query to execute.
    pub query: String,
    /// How input is partitioned.
    pub partition: PartitionStrategy,
    /// How results are merged.
    pub merge: MergeStrategy,
    /// Node assignments.
    pub assignments: Vec<NodeAssignment>,
    /// Total files to process.
    pub total_files: usize,
}

impl ScatterGatherPlan {
    /// Create a new scatter/gather plan.
    pub fn new(
        query: String,
        nodes: Vec<String>,
        files: Vec<PathBuf>,
        partition: PartitionStrategy,
        merge: MergeStrategy,
    ) -> Result<Self> {
        if nodes.is_empty() {
            bail!("At least one node is required");
        }

        let total_files = files.len();
        let assignments = partition_files(&nodes, &files, partition);

        Ok(Self {
            query,
            partition,
            merge,
            assignments,
            total_files,
        })
    }

    /// Get the number of nodes in this plan.
    pub fn node_count(&self) -> usize {
        self.assignments.len()
    }

    /// Summarize the plan.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "Scatter/Gather Plan: {} files → {} nodes\n",
            self.total_files,
            self.node_count()
        ));
        s.push_str(&format!(
            "Partition: {:?} ({})\n",
            self.partition,
            self.partition.description()
        ));
        s.push_str(&format!("Merge: {:?}\n", self.merge));
        s.push_str(&format!("Query: {}\n", self.query));
        for a in &self.assignments {
            s.push_str(&format!(
                "  [{}] {} → {} files\n",
                a.partition_index,
                a.node,
                a.files.len()
            ));
        }
        s
    }
}

/// Result of a distributed pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedResult {
    /// The execution plan.
    pub plan: ScatterGatherPlan,
    /// Per-node results.
    pub node_results: Vec<NodeResult>,
    /// Merged final output.
    pub merged_output: Vec<String>,
    /// Total duration in milliseconds.
    pub total_duration_ms: u64,
    /// Number of nodes that succeeded.
    pub nodes_succeeded: usize,
    /// Number of nodes that failed.
    pub nodes_failed: usize,
}

/// Distributed pipeline executor.
pub struct DistributedPipeline {
    /// Maximum concurrent node executions.
    pub max_concurrent: usize,
    /// Timeout per node in milliseconds.
    pub node_timeout_ms: u64,
    /// Whether to continue on individual node failure.
    pub continue_on_error: bool,
}

impl Default for DistributedPipeline {
    fn default() -> Self {
        Self {
            max_concurrent: 8,
            node_timeout_ms: 60_000,
            continue_on_error: true,
        }
    }
}

impl DistributedPipeline {
    /// Create a new distributed pipeline executor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max concurrent nodes.
    pub fn with_max_concurrent(mut self, n: usize) -> Self {
        self.max_concurrent = n;
        self
    }

    /// Set node timeout.
    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.node_timeout_ms = ms;
        self
    }

    /// Set continue-on-error.
    pub fn with_continue_on_error(mut self, b: bool) -> Self {
        self.continue_on_error = b;
        self
    }

    /// Execute a distributed plan.
    ///
    /// Note: This is a local simulation. Real execution would use
    /// [`crate::remote::RemoteExecutor`] to dispatch work via SSH.
    pub fn execute(&self, plan: &ScatterGatherPlan) -> Result<DistributedResult> {
        let start = Instant::now();
        let mut node_results = Vec::new();
        let mut nodes_succeeded = 0usize;
        let nodes_failed = 0usize;

        for assignment in &plan.assignments {
            let node_start = Instant::now();

            // Simulate per-node execution
            let output_lines: Vec<String> = assignment
                .files
                .iter()
                .map(|f| format!("[{}] processed: {}", assignment.node, f.display()))
                .collect();

            node_results.push(NodeResult {
                node: assignment.node.clone(),
                partition_index: assignment.partition_index,
                output_lines,
                duration_ms: node_start.elapsed().as_millis() as u64,
                files_processed: assignment.files.len(),
                success: true,
                error: None,
            });
            nodes_succeeded += 1;
        }

        // Merge results
        let merged_output = plan.merge.apply(&node_results);

        Ok(DistributedResult {
            plan: plan.clone(),
            node_results,
            merged_output,
            total_duration_ms: start.elapsed().as_millis() as u64,
            nodes_succeeded,
            nodes_failed,
        })
    }
}

// ─── partition helpers ──────────────────────────────────────────────────────

fn partition_files(
    nodes: &[String],
    files: &[PathBuf],
    strategy: PartitionStrategy,
) -> Vec<NodeAssignment> {
    match strategy {
        PartitionStrategy::ByFile => partition_round_robin(nodes, files),
        PartitionStrategy::ByFileType => partition_by_extension(nodes, files),
        PartitionStrategy::Broadcast => partition_broadcast(nodes, files),
        PartitionStrategy::ByLineRange => {
            // For line ranges, each node gets all files but a different line range
            // This is a placeholder — real implementation reads file sizes
            partition_round_robin(nodes, files)
        }
        PartitionStrategy::BySample => {
            // Random sample — placeholder uses round-robin
            partition_round_robin(nodes, files)
        }
    }
}

fn partition_round_robin(nodes: &[String], files: &[PathBuf]) -> Vec<NodeAssignment> {
    let mut assignments: Vec<NodeAssignment> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| NodeAssignment {
            node: node.clone(),
            files: Vec::new(),
            line_range: None,
            partition_index: i,
        })
        .collect();

    for (i, file) in files.iter().enumerate() {
        assignments[i % nodes.len()].files.push(file.clone());
    }

    assignments
}

fn partition_broadcast(nodes: &[String], files: &[PathBuf]) -> Vec<NodeAssignment> {
    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| NodeAssignment {
            node: node.clone(),
            files: files.to_vec(),
            line_range: None,
            partition_index: i,
        })
        .collect()
}

fn partition_by_extension(nodes: &[String], files: &[PathBuf]) -> Vec<NodeAssignment> {
    // Group files by extension, then assign groups to nodes
    let mut ext_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in files {
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "none".into());
        ext_groups.entry(ext).or_default().push(file.clone());
    }

    let mut assignments: Vec<NodeAssignment> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| NodeAssignment {
            node: node.clone(),
            files: Vec::new(),
            line_range: None,
            partition_index: i,
        })
        .collect();

    for (i, (_ext, group_files)) in ext_groups.into_iter().enumerate() {
        assignments[i % nodes.len()].files.extend(group_files);
    }

    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_strategy_description() {
        assert!(!PartitionStrategy::ByFile.description().is_empty());
        assert!(!PartitionStrategy::Broadcast.description().is_empty());
    }

    #[test]
    fn test_merge_concat() {
        let results = vec![
            NodeResult {
                node: "a".into(),
                partition_index: 0,
                output_lines: vec!["line1".into(), "line2".into()],
                duration_ms: 0,
                files_processed: 1,
                success: true,
                error: None,
            },
            NodeResult {
                node: "b".into(),
                partition_index: 1,
                output_lines: vec!["line3".into()],
                duration_ms: 0,
                files_processed: 1,
                success: true,
                error: None,
            },
        ];
        let merged = MergeStrategy::Concat.apply(&results);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_concat_sort() {
        let results = vec![NodeResult {
            node: "a".into(),
            partition_index: 0,
            output_lines: vec!["banana".into(), "apple".into()],
            duration_ms: 0,
            files_processed: 0,
            success: true,
            error: None,
        }];
        let merged = MergeStrategy::ConcatSort.apply(&results);
        assert_eq!(merged, vec!["apple", "banana"]);
    }

    #[test]
    fn test_merge_concat_dedup() {
        let results = vec![
            NodeResult {
                node: "a".into(),
                partition_index: 0,
                output_lines: vec!["x".into(), "y".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
            NodeResult {
                node: "b".into(),
                partition_index: 1,
                output_lines: vec!["y".into(), "z".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
        ];
        let merged = MergeStrategy::ConcatDedup.apply(&results);
        assert_eq!(merged, vec!["x", "y", "z"]);
    }

    #[test]
    fn test_merge_sum() {
        let results = vec![
            NodeResult {
                node: "a".into(),
                partition_index: 0,
                output_lines: vec!["10".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
            NodeResult {
                node: "b".into(),
                partition_index: 1,
                output_lines: vec!["20".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
        ];
        let merged = MergeStrategy::Sum.apply(&results);
        assert_eq!(merged, vec!["30"]);
    }

    #[test]
    fn test_merge_average() {
        let results = vec![
            NodeResult {
                node: "a".into(),
                partition_index: 0,
                output_lines: vec!["10".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
            NodeResult {
                node: "b".into(),
                partition_index: 1,
                output_lines: vec!["20".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
        ];
        let merged = MergeStrategy::Average.apply(&results);
        assert_eq!(merged, vec!["15"]);
    }

    #[test]
    fn test_merge_frequency() {
        let results = vec![
            NodeResult {
                node: "a".into(),
                partition_index: 0,
                output_lines: vec!["5\tERROR".into(), "3\tWARN".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
            NodeResult {
                node: "b".into(),
                partition_index: 1,
                output_lines: vec!["7\tERROR".into(), "2\tINFO".into()],
                duration_ms: 0,
                files_processed: 0,
                success: true,
                error: None,
            },
        ];
        let merged = MergeStrategy::MergeFrequency.apply(&results);
        assert_eq!(merged[0], "12\tERROR"); // 5+7
        assert!(merged.len() == 3);
    }

    #[test]
    fn test_partition_round_robin() {
        let nodes = vec!["a".into(), "b".into()];
        let files: Vec<PathBuf> = (0..5)
            .map(|i| PathBuf::from(format!("file{}.txt", i)))
            .collect();
        let assignments = partition_round_robin(&nodes, &files);
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].files.len(), 3); // 0, 2, 4
        assert_eq!(assignments[1].files.len(), 2); // 1, 3
    }

    #[test]
    fn test_partition_broadcast() {
        let nodes = vec!["a".into(), "b".into()];
        let files = vec![PathBuf::from("f1.txt"), PathBuf::from("f2.txt")];
        let assignments = partition_broadcast(&nodes, &files);
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].files.len(), 2);
        assert_eq!(assignments[1].files.len(), 2);
    }

    #[test]
    fn test_scatter_gather_plan() {
        let plan = ScatterGatherPlan::new(
            "grep ERROR | count".into(),
            vec!["node1".into(), "node2".into()],
            vec![
                PathBuf::from("a.txt"),
                PathBuf::from("b.txt"),
                PathBuf::from("c.txt"),
            ],
            PartitionStrategy::ByFile,
            MergeStrategy::Sum,
        )
        .unwrap();

        assert_eq!(plan.node_count(), 2);
        assert_eq!(plan.total_files, 3);
        let summary = plan.summary();
        assert!(summary.contains("3 files"));
        assert!(summary.contains("2 nodes"));
    }

    #[test]
    fn test_scatter_gather_plan_no_nodes() {
        let result = ScatterGatherPlan::new(
            "query".into(),
            vec![],
            vec![PathBuf::from("f.txt")],
            PartitionStrategy::ByFile,
            MergeStrategy::Concat,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_distributed_pipeline_execute() {
        let plan = ScatterGatherPlan::new(
            "grep ERROR".into(),
            vec!["n1".into(), "n2".into()],
            vec![PathBuf::from("a.log"), PathBuf::from("b.log")],
            PartitionStrategy::ByFile,
            MergeStrategy::Concat,
        )
        .unwrap();

        let pipeline = DistributedPipeline::new();
        let result = pipeline.execute(&plan).unwrap();
        assert_eq!(result.nodes_succeeded, 2);
        assert_eq!(result.nodes_failed, 0);
        assert!(!result.merged_output.is_empty());
    }

    #[test]
    fn test_distributed_pipeline_builder() {
        let p = DistributedPipeline::new()
            .with_max_concurrent(4)
            .with_timeout(5000)
            .with_continue_on_error(false);
        assert_eq!(p.max_concurrent, 4);
        assert_eq!(p.node_timeout_ms, 5000);
        assert!(!p.continue_on_error);
    }
}
