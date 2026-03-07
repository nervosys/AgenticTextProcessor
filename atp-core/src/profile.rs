//! Profile-guided optimization hints for pipeline execution.
//!
//! Instruments pipeline stage execution with per-stage timing, memory estimates,
//! and record counts. Generates optimization suggestions such as reordering
//! stages (e.g., "move filter before sort to reduce sort input by 80%").

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A single stage profile measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageProfile {
    /// Stage index in the pipeline.
    pub stage_index: usize,
    /// Stage type name (e.g., "Search", "Filter", "Sort").
    pub stage_type: String,
    /// Wall-clock duration of this stage.
    pub duration: Duration,
    /// Number of records entering this stage.
    pub records_in: usize,
    /// Number of records leaving this stage.
    pub records_out: usize,
    /// Selectivity: records_out / records_in (0.0–1.0).
    pub selectivity: f64,
    /// Estimated memory usage in bytes.
    pub estimated_memory_bytes: u64,
}

/// A complete pipeline profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineProfile {
    /// Per-stage measurements.
    pub stages: Vec<StageProfile>,
    /// Total wall-clock duration.
    pub total_duration: Duration,
    /// Total records processed (input to first stage).
    pub total_records: usize,
    /// Optimization suggestions.
    pub suggestions: Vec<OptimizationSuggestion>,
}

/// An optimization suggestion generated from profile data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Severity: "info", "warning", "critical"
    pub severity: SuggestionSeverity,
    /// Human-readable description of the suggestion.
    pub message: String,
    /// Estimated speedup if applied (e.g., "2.5x").
    pub estimated_speedup: Option<String>,
    /// Suggested action: e.g., "move_stage", "remove_stage", "add_stage"
    pub action: SuggestedAction,
}

/// Severity level for suggestions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionSeverity {
    Info,
    Warning,
    Critical,
}

/// Concrete action the user could take.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum SuggestedAction {
    /// Move a stage to a different position.
    MoveStage { from_index: usize, to_index: usize },
    /// Remove an unnecessary stage.
    RemoveStage { index: usize },
    /// A general textual recommendation.
    Advice { detail: String },
}

/// Analyze a set of stage profiles and generate optimization suggestions.
pub fn analyze_profile(stages: &[StageProfile]) -> Vec<OptimizationSuggestion> {
    let mut suggestions = Vec::new();

    // Rule 1: Filter-before-sort — if a Filter appears after a Sort,
    // and the filter has low selectivity, suggest reordering.
    for (i, stage) in stages.iter().enumerate() {
        if stage.stage_type == "Sort" {
            // Look for a Filter that follows this Sort
            for (j, stage_j) in stages.iter().enumerate().skip(i + 1) {
                if stage_j.stage_type == "Filter" && stage_j.selectivity < 0.5 {
                    let reduction_pct = ((1.0 - stage_j.selectivity) * 100.0) as u32;
                    suggestions.push(OptimizationSuggestion {
                        severity: SuggestionSeverity::Warning,
                        message: format!(
                            "Move Filter (stage {}) before Sort (stage {}) to reduce sort input by {}%",
                            j, i, reduction_pct
                        ),
                        estimated_speedup: Some(format!(
                            "{:.1}x",
                            1.0 / stage_j.selectivity
                        )),
                        action: SuggestedAction::MoveStage {
                            from_index: j,
                            to_index: i,
                        },
                    });
                }
            }
        }
    }

    // Rule 2: Dead stage — if a stage has 0 output records and isn't the last stage
    for (i, stage) in stages.iter().enumerate() {
        if stage.records_out == 0 && i < stages.len() - 1 {
            suggestions.push(OptimizationSuggestion {
                severity: SuggestionSeverity::Critical,
                message: format!(
                    "Stage {} ({}) produces 0 records — all subsequent stages are wasted work",
                    i, stage.stage_type
                ),
                estimated_speedup: None,
                action: SuggestedAction::Advice {
                    detail: "Consider removing stages after the empty stage or fixing the stage's pattern".into(),
                },
            });
        }
    }

    // Rule 3: Expensive identity stage — if a stage takes >50% of total time
    // but doesn't reduce record count
    let total_ns: u128 = stages.iter().map(|s| s.duration.as_nanos()).sum();
    if total_ns > 0 {
        for (i, stage) in stages.iter().enumerate() {
            let pct = (stage.duration.as_nanos() as f64 / total_ns as f64) * 100.0;
            if pct > 50.0 && stage.selectivity > 0.99 {
                suggestions.push(OptimizationSuggestion {
                    severity: SuggestionSeverity::Info,
                    message: format!(
                        "Stage {} ({}) takes {:.0}% of total time but passes through {:.0}% of records — consider if it's necessary",
                        i, stage.stage_type, pct, stage.selectivity * 100.0
                    ),
                    estimated_speedup: None,
                    action: SuggestedAction::Advice {
                        detail: format!("Stage consumes {:.0}% of pipeline time with minimal filtering", pct),
                    },
                });
            }
        }
    }

    // Rule 4: High-selectivity stage late in pipeline — suggest moving earlier
    for (i, stage) in stages.iter().enumerate() {
        if i > 0 && stage.selectivity < 0.2 && stage.stage_type == "Filter" {
            // Check if earlier stages had high selectivity (passed most records through)
            let earlier_high_pass = stages[..i]
                .iter()
                .any(|s| s.selectivity > 0.9 && s.stage_type != "Filter");
            if earlier_high_pass {
                suggestions.push(OptimizationSuggestion {
                    severity: SuggestionSeverity::Info,
                    message: format!(
                        "Filter at stage {} removes {:.0}% of records — moving it earlier would reduce work for preceding stages",
                        i,
                        (1.0 - stage.selectivity) * 100.0
                    ),
                    estimated_speedup: Some(format!("{:.1}x", 1.0 / stage.selectivity)),
                    action: SuggestedAction::MoveStage {
                        from_index: i,
                        to_index: 0,
                    },
                });
            }
        }
    }

    suggestions
}

/// Build a profile from raw stage measurements.
pub fn build_profile(stages: Vec<StageProfile>) -> PipelineProfile {
    let total_duration: Duration = stages.iter().map(|s| s.duration).sum();
    let total_records = stages.first().map(|s| s.records_in).unwrap_or(0);
    let suggestions = analyze_profile(&stages);

    PipelineProfile {
        stages,
        total_duration,
        total_records,
        suggestions,
    }
}

/// Create a stage profile measurement.
pub fn measure_stage(
    stage_index: usize,
    stage_type: &str,
    duration: Duration,
    records_in: usize,
    records_out: usize,
) -> StageProfile {
    let selectivity = if records_in > 0 {
        records_out as f64 / records_in as f64
    } else {
        1.0
    };
    // Rough memory estimate: ~64 bytes per record
    let estimated_memory_bytes = (records_in.max(records_out) as u64) * 64;

    StageProfile {
        stage_index,
        stage_type: stage_type.to_string(),
        duration,
        records_in,
        records_out,
        selectivity,
        estimated_memory_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_stage() {
        let p = measure_stage(0, "Search", Duration::from_millis(10), 1000, 100);
        assert_eq!(p.stage_index, 0);
        assert_eq!(p.stage_type, "Search");
        assert!((p.selectivity - 0.1).abs() < 0.001);
        assert_eq!(p.estimated_memory_bytes, 1000 * 64);
    }

    #[test]
    fn test_measure_stage_zero_input() {
        let p = measure_stage(0, "Search", Duration::from_millis(1), 0, 0);
        assert!((p.selectivity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_build_profile() {
        let stages = vec![
            measure_stage(0, "Search", Duration::from_millis(10), 1000, 500),
            measure_stage(1, "Filter", Duration::from_millis(5), 500, 50),
        ];
        let profile = build_profile(stages);
        assert_eq!(profile.total_records, 1000);
        assert_eq!(profile.total_duration, Duration::from_millis(15));
        assert_eq!(profile.stages.len(), 2);
    }

    #[test]
    fn test_filter_before_sort_suggestion() {
        let stages = vec![
            measure_stage(0, "Search", Duration::from_millis(10), 1000, 1000),
            measure_stage(1, "Sort", Duration::from_millis(50), 1000, 1000),
            measure_stage(2, "Filter", Duration::from_millis(5), 1000, 100), // 90% reduction!
        ];
        let suggestions = analyze_profile(&stages);
        assert!(suggestions.iter().any(|s| matches!(
            &s.action,
            SuggestedAction::MoveStage {
                from_index: 2,
                to_index: 1
            }
        )));
    }

    #[test]
    fn test_dead_stage_suggestion() {
        let stages = vec![
            measure_stage(0, "Search", Duration::from_millis(10), 1000, 0),
            measure_stage(1, "Sort", Duration::from_millis(50), 0, 0),
        ];
        let suggestions = analyze_profile(&stages);
        assert!(suggestions
            .iter()
            .any(|s| s.severity == SuggestionSeverity::Critical));
        assert!(suggestions.iter().any(|s| s.message.contains("0 records")));
    }

    #[test]
    fn test_no_suggestions_for_optimal_pipeline() {
        let stages = vec![
            measure_stage(0, "Filter", Duration::from_millis(2), 1000, 100),
            measure_stage(1, "Sort", Duration::from_millis(5), 100, 100),
            measure_stage(2, "Head", Duration::from_millis(1), 100, 10),
        ];
        let suggestions = analyze_profile(&stages);
        // Should have no filter-before-sort suggestions (already correct order)
        assert!(!suggestions
            .iter()
            .any(|s| matches!(&s.action, SuggestedAction::MoveStage { .. })));
    }

    #[test]
    fn test_expensive_identity_suggestion() {
        let stages = vec![
            measure_stage(0, "Transform", Duration::from_millis(100), 1000, 1000),
            measure_stage(1, "Head", Duration::from_millis(1), 1000, 10),
        ];
        let suggestions = analyze_profile(&stages);
        assert!(suggestions
            .iter()
            .any(|s| s.severity == SuggestionSeverity::Info && s.message.contains("takes")));
    }

    #[test]
    fn test_late_selective_filter() {
        let stages = vec![
            measure_stage(0, "Search", Duration::from_millis(10), 1000, 950),
            measure_stage(1, "Sort", Duration::from_millis(50), 950, 950),
            measure_stage(2, "Filter", Duration::from_millis(5), 950, 50),
        ];
        let suggestions = analyze_profile(&stages);
        // Should suggest moving filter before sort AND moving filter earlier
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_profile_serialization() {
        let profile = build_profile(vec![measure_stage(
            0,
            "Search",
            Duration::from_millis(10),
            100,
            50,
        )]);
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("Search"));
        assert!(json.contains("selectivity"));
    }
}
