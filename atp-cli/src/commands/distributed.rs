//! CLI command: `atp distributed` — scatter/gather distributed AQL.

use anyhow::Result;
use atp_core::distributed::{
    DistributedPipeline, MergeStrategy, PartitionStrategy, ScatterGatherPlan,
};
use atp_core::output::Format;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct DistributedArgs {
    /// AQL query to execute
    pub query: String,

    /// Files / directories to process
    pub paths: Vec<String>,

    /// Comma-separated list of remote hosts
    #[arg(long)]
    pub hosts: String,

    /// Partition strategy: by-file, by-line-range, by-sample, broadcast, by-file-type
    #[arg(long, short = 'p', default_value = "by-file")]
    pub partition: String,

    /// Merge strategy: concat, concat-sort, concat-dedup, sum, average, first-non-empty, merge-frequency
    #[arg(long, short = 'm', default_value = "concat")]
    pub merge: String,

    /// Maximum concurrent node executions
    #[arg(long, default_value = "8")]
    pub max_concurrent: usize,

    /// Timeout per node in milliseconds
    #[arg(long, default_value = "60000")]
    pub timeout: u64,

    /// Show execution plan without running
    #[arg(long)]
    pub dry_run: bool,
}

fn parse_partition(s: &str) -> Result<PartitionStrategy> {
    match s {
        "by-file" => Ok(PartitionStrategy::ByFile),
        "by-line-range" => Ok(PartitionStrategy::ByLineRange),
        "by-sample" => Ok(PartitionStrategy::BySample),
        "broadcast" => Ok(PartitionStrategy::Broadcast),
        "by-file-type" => Ok(PartitionStrategy::ByFileType),
        other => anyhow::bail!("Unknown partition strategy: {}", other),
    }
}

fn parse_merge(s: &str) -> Result<MergeStrategy> {
    match s {
        "concat" => Ok(MergeStrategy::Concat),
        "concat-sort" => Ok(MergeStrategy::ConcatSort),
        "concat-dedup" => Ok(MergeStrategy::ConcatDedup),
        "sum" => Ok(MergeStrategy::Sum),
        "average" | "avg" => Ok(MergeStrategy::Average),
        "first-non-empty" => Ok(MergeStrategy::FirstNonEmpty),
        "merge-frequency" | "merge-freq" => Ok(MergeStrategy::MergeFrequency),
        other => anyhow::bail!("Unknown merge strategy: {}", other),
    }
}

pub fn execute(args: DistributedArgs, format: Format, _raw: bool) -> Result<()> {
    let nodes: Vec<String> = args
        .hosts
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let files: Vec<PathBuf> = args.paths.iter().map(PathBuf::from).collect();
    let partition = parse_partition(&args.partition)?;
    let merge = parse_merge(&args.merge)?;

    let plan = ScatterGatherPlan::new(args.query, nodes, files, partition, merge)?;

    if args.dry_run {
        match format {
            Format::Human => {
                print!("{}", plan.summary());
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            }
        }
        return Ok(());
    }

    let pipeline = DistributedPipeline::new()
        .with_max_concurrent(args.max_concurrent)
        .with_timeout(args.timeout);

    let result = pipeline.execute(&plan)?;

    match format {
        Format::Human => {
            println!(
                "Distributed execution: {} nodes ({} succeeded, {} failed) in {}ms",
                plan.node_count(),
                result.nodes_succeeded,
                result.nodes_failed,
                result.total_duration_ms
            );
            println!("\nMerged output ({} lines):", result.merged_output.len());
            for line in &result.merged_output {
                println!("{}", line);
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
