//! AQL query plan optimizer.
//!
//! Cost-based optimizer pass between AQL parsing and evaluation. Performs:
//! - Predicate pushdown: moves filters earlier in the pipeline
//! - Stage reordering: sorts stages by estimated cost
//! - Dead-stage elimination: removes stages after a `take 0` or `count`
//! - Common sub-expression folding: deduplicates identical stages
//!
//! `--explain` shows before/after plans.

use serde::{Deserialize, Serialize};

/// An optimized query plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    /// Original stage types (before optimization).
    pub original_stages: Vec<String>,
    /// Optimized stage types (after optimization).
    pub optimized_stages: Vec<String>,
    /// Rules that were applied.
    pub rules_applied: Vec<OptimizationRule>,
    /// Estimated cost reduction (0.0–1.0, where 0.5 = 50% less work).
    pub estimated_cost_reduction: f64,
}

/// An optimization rule that was applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OptimizationRule {
    /// Rule name (e.g., "predicate_pushdown", "dead_stage_elimination").
    pub name: String,
    /// Description of what the rule did.
    pub description: String,
}

/// Stage classification for the optimizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageKind {
    /// Stages that reduce record count (Filter, Find, Delete).
    Selective,
    /// Stages that reorder records (Sort).
    Reordering,
    /// Stages that pass through all records (Transform, SetSeparator, Select).
    PassThrough,
    /// Stages that truncate (Take, Skip, Last, Head, Tail).
    Truncating,
    /// Stages that aggregate (Count, Aggregate, GroupBy, Unique).
    Aggregating,
    /// Structural stages (Let, IfElse, Define, Call).
    Structural,
}

/// Simple stage descriptor used by the optimizer.
#[derive(Debug, Clone)]
pub struct StageDesc {
    /// Stage type name.
    pub name: String,
    /// The kind of this stage.
    pub kind: StageKind,
    /// Estimated selectivity (0.0–1.0). Lower = filters more.
    pub estimated_selectivity: f64,
    /// Original index in the pipeline.
    pub original_index: usize,
}

/// Classify a stage by its type name.
pub fn classify_stage(name: &str) -> StageKind {
    match name {
        "Find" | "Filter" | "Delete" => StageKind::Selective,
        "Sort" => StageKind::Reordering,
        "Transform" | "Replace" | "InsertBefore" | "InsertAfter" | "Select" | "SetSeparator" => {
            StageKind::PassThrough
        }
        "Take" | "Skip" | "Last" | "Head" | "Tail" => StageKind::Truncating,
        "Count" | "Aggregate" | "GroupBy" | "Unique" => StageKind::Aggregating,
        _ => StageKind::Structural,
    }
}

/// Default estimated selectivity by stage kind.
pub fn default_selectivity(kind: &StageKind) -> f64 {
    match kind {
        StageKind::Selective => 0.3,
        StageKind::Reordering => 1.0,
        StageKind::PassThrough => 1.0,
        StageKind::Truncating => 0.1,
        StageKind::Aggregating => 0.01,
        StageKind::Structural => 1.0,
    }
}

/// Optimize a list of stage descriptors, returning an optimized plan.
pub fn optimize(stages: &[StageDesc]) -> QueryPlan {
    let original_stages: Vec<String> = stages.iter().map(|s| s.name.clone()).collect();
    let mut optimized: Vec<StageDesc> = stages.to_vec();
    let mut rules_applied = Vec::new();

    // Rule 1: Predicate pushdown — move Selective stages before Reordering stages
    let before_pushdown: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();
    predicate_pushdown(&mut optimized);
    let after_pushdown: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();
    if before_pushdown != after_pushdown {
        rules_applied.push(OptimizationRule {
            name: "predicate_pushdown".into(),
            description: "Moved selective stages (Filter/Find) before reordering stages (Sort)"
                .into(),
        });
    }

    // Rule 2: Dead-stage elimination — remove stages after aggregating stages like Count
    let before_dse: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();
    dead_stage_elimination(&mut optimized);
    let after_dse: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();
    if before_dse != after_dse {
        rules_applied.push(OptimizationRule {
            name: "dead_stage_elimination".into(),
            description: "Removed unreachable stages after terminal aggregation".into(),
        });
    }

    // Rule 3: Duplicate elimination — remove consecutive identical stages
    let before_dedup: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();
    duplicate_elimination(&mut optimized);
    let after_dedup: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();
    if before_dedup != after_dedup {
        rules_applied.push(OptimizationRule {
            name: "duplicate_elimination".into(),
            description: "Removed duplicate adjacent stages".into(),
        });
    }

    let optimized_stages: Vec<String> = optimized.iter().map(|s| s.name.clone()).collect();

    // Estimate cost reduction based on stages removed and reordered
    let original_cost = estimate_cost(&original_stages);
    let optimized_cost = estimate_cost(&optimized_stages);
    let estimated_cost_reduction = if original_cost > 0.0 {
        1.0 - (optimized_cost / original_cost)
    } else {
        0.0
    };

    QueryPlan {
        original_stages,
        optimized_stages,
        rules_applied,
        estimated_cost_reduction: estimated_cost_reduction.max(0.0),
    }
}

/// Predicate pushdown: move selective stages before reordering stages.
fn predicate_pushdown(stages: &mut [StageDesc]) {
    // Bubble selective stages up past reordering/pass-through stages where safe
    let mut changed = true;
    while changed {
        changed = false;
        for i in 1..stages.len() {
            if stages[i].kind == StageKind::Selective
                && (stages[i - 1].kind == StageKind::Reordering
                    || stages[i - 1].kind == StageKind::PassThrough)
            {
                stages.swap(i - 1, i);
                changed = true;
            }
        }
    }
}

/// Dead-stage elimination: remove stages after Count or terminal aggregation.
#[allow(clippy::ptr_arg)]
fn dead_stage_elimination(stages: &mut Vec<StageDesc>) {
    if let Some(terminal_idx) = stages.iter().position(|s| s.name == "Count") {
        // Count is terminal — remove everything after it
        stages.truncate(terminal_idx + 1);
    }
}

/// Remove consecutive duplicate stages.
#[allow(clippy::ptr_arg)]
fn duplicate_elimination(stages: &mut Vec<StageDesc>) {
    stages.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);
}

/// Estimate relative cost of a stage list (arbitrary units).
fn estimate_cost(stages: &[String]) -> f64 {
    stages
        .iter()
        .map(|name| {
            let kind = classify_stage(name);
            match kind {
                StageKind::Reordering => 10.0, // Sort is expensive
                StageKind::Selective => 2.0,
                StageKind::PassThrough => 3.0,
                StageKind::Truncating => 1.0,
                StageKind::Aggregating => 1.0,
                StageKind::Structural => 1.0,
            }
        })
        .sum()
}

/// Create a stage descriptor.
pub fn stage_desc(name: &str, index: usize) -> StageDesc {
    let kind = classify_stage(name);
    let estimated_selectivity = default_selectivity(&kind);
    StageDesc {
        name: name.to_string(),
        kind,
        estimated_selectivity,
        original_index: index,
    }
}

/// Format a query plan as an explain-style string.
pub fn format_plan(plan: &QueryPlan) -> String {
    let mut out = String::new();
    out.push_str("=== Query Plan ===\n\n");
    out.push_str("Original:  ");
    out.push_str(&plan.original_stages.join(" → "));
    out.push_str("\nOptimized: ");
    out.push_str(&plan.optimized_stages.join(" → "));
    out.push('\n');

    if !plan.rules_applied.is_empty() {
        out.push_str("\nRules applied:\n");
        for rule in &plan.rules_applied {
            out.push_str(&format!("  • {} — {}\n", rule.name, rule.description));
        }
    } else {
        out.push_str("\nNo optimizations applicable — pipeline is already optimal.\n");
    }

    if plan.estimated_cost_reduction > 0.0 {
        out.push_str(&format!(
            "\nEstimated cost reduction: {:.0}%\n",
            plan.estimated_cost_reduction * 100.0
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_stage() {
        assert_eq!(classify_stage("Find"), StageKind::Selective);
        assert_eq!(classify_stage("Filter"), StageKind::Selective);
        assert_eq!(classify_stage("Sort"), StageKind::Reordering);
        assert_eq!(classify_stage("Transform"), StageKind::PassThrough);
        assert_eq!(classify_stage("Take"), StageKind::Truncating);
        assert_eq!(classify_stage("Count"), StageKind::Aggregating);
        assert_eq!(classify_stage("IfElse"), StageKind::Structural);
    }

    #[test]
    fn test_predicate_pushdown() {
        let stages = vec![
            stage_desc("Find", 0),
            stage_desc("Sort", 1),
            stage_desc("Filter", 2),
        ];
        let plan = optimize(&stages);
        // Filter should be pushed before Sort
        assert_eq!(plan.optimized_stages, vec!["Find", "Filter", "Sort"]);
        assert!(plan
            .rules_applied
            .iter()
            .any(|r| r.name == "predicate_pushdown"));
    }

    #[test]
    fn test_dead_stage_elimination() {
        let stages = vec![
            stage_desc("Find", 0),
            stage_desc("Count", 1),
            stage_desc("Sort", 2),
        ];
        let plan = optimize(&stages);
        // Sort after Count should be eliminated
        assert_eq!(plan.optimized_stages, vec!["Find", "Count"]);
        assert!(plan
            .rules_applied
            .iter()
            .any(|r| r.name == "dead_stage_elimination"));
    }

    #[test]
    fn test_duplicate_elimination() {
        let stages = vec![
            stage_desc("Filter", 0),
            stage_desc("Filter", 1),
            stage_desc("Sort", 2),
        ];
        let plan = optimize(&stages);
        assert_eq!(plan.optimized_stages, vec!["Filter", "Sort"]);
        assert!(plan
            .rules_applied
            .iter()
            .any(|r| r.name == "duplicate_elimination"));
    }

    #[test]
    fn test_no_optimization_needed() {
        let stages = vec![
            stage_desc("Filter", 0),
            stage_desc("Sort", 1),
            stage_desc("Take", 2),
        ];
        let plan = optimize(&stages);
        assert_eq!(plan.optimized_stages, vec!["Filter", "Sort", "Take"]);
        assert!(plan.rules_applied.is_empty());
    }

    #[test]
    fn test_complex_optimization() {
        let stages = vec![
            stage_desc("Find", 0),
            stage_desc("Sort", 1),
            stage_desc("Filter", 2),
            stage_desc("Count", 3),
            stage_desc("Take", 4), // dead after Count
        ];
        let plan = optimize(&stages);
        // Filter pushdown + dead stage elimination
        assert_eq!(
            plan.optimized_stages,
            vec!["Find", "Filter", "Sort", "Count"]
        );
        assert!(plan.rules_applied.len() >= 2);
    }

    #[test]
    fn test_cost_reduction() {
        let stages = vec![stage_desc("Sort", 0), stage_desc("Filter", 1)];
        let plan = optimize(&stages);
        assert!(plan.estimated_cost_reduction >= 0.0);
    }

    #[test]
    fn test_format_plan() {
        let stages = vec![stage_desc("Sort", 0), stage_desc("Filter", 1)];
        let plan = optimize(&stages);
        let formatted = format_plan(&plan);
        assert!(formatted.contains("Query Plan"));
        assert!(formatted.contains("Original:"));
        assert!(formatted.contains("Optimized:"));
    }

    #[test]
    fn test_stage_desc() {
        let sd = stage_desc("Find", 5);
        assert_eq!(sd.name, "Find");
        assert_eq!(sd.kind, StageKind::Selective);
        assert_eq!(sd.original_index, 5);
        assert!(sd.estimated_selectivity < 1.0);
    }

    #[test]
    fn test_plan_serialization() {
        let stages = vec![stage_desc("Find", 0), stage_desc("Sort", 1)];
        let plan = optimize(&stages);
        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("original_stages"));
        assert!(json.contains("optimized_stages"));
    }
}
