//! Awk engine — structured record processing and aggregation.
//!
//! Provides field-based text processing with:
//! - Configurable field separators
//! - Pattern-action rules
//! - Built-in aggregation functions
//! - Strongly typed output records
//! - Streaming mode for memory-efficient processing of large files

use crate::output::{AggregationValue, AnalysisRecord, AnalysisResults};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// A pattern-action rule for the awk engine.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Pattern to match (regex). None = match all lines.
    pub pattern: Option<String>,
    /// Which fields to extract (1-based). Empty = all fields.
    pub select_fields: Vec<usize>,
    /// Computed fields: name → expression
    pub computed_fields: Vec<ComputedField>,
    /// Condition to filter records (applied after field extraction).
    pub condition: Option<Condition>,
}

/// A computed field derived from other fields.
#[derive(Debug, Clone)]
pub struct ComputedField {
    pub name: String,
    pub expression: FieldExpression,
}

/// Simple expressions for computed fields.
#[derive(Debug, Clone)]
pub enum FieldExpression {
    /// Reference a field by index (1-based)
    Field(usize),
    /// Concatenate multiple fields with a separator
    Concat(Vec<usize>, String),
    /// String length of a field
    Length(usize),
    /// Uppercase a field
    Upper(usize),
    /// Lowercase a field
    Lower(usize),
    /// Substring of a field
    Substr(usize, usize, Option<usize>),
    /// Literal value
    Literal(String),
    /// Replace first match of pattern in field (like awk sub())
    Sub(usize, String, String),
    /// Replace all matches of pattern in field (like awk gsub())
    Gsub(usize, String, String),
    /// Match field against regex, return matched text (like awk match())
    Match(usize, String),
    /// Split field by separator, return Nth part (like awk split())
    Split(usize, String, usize),
    /// Record number (NR)
    NR,
    /// Number of fields in current record (NF)
    NF,
}

/// Conditions for filtering records.
#[derive(Debug, Clone)]
pub enum Condition {
    /// Field equals value
    Equals(usize, String),
    /// Field does not equal value
    NotEquals(usize, String),
    /// Field contains substring
    Contains(usize, String),
    /// Field matches regex
    Matches(usize, String),
    /// Numeric comparison: field > value
    GreaterThan(usize, f64),
    /// Numeric comparison: field < value
    LessThan(usize, f64),
    /// Field is not empty
    NotEmpty(usize),
    /// Logical AND
    And(Box<Condition>, Box<Condition>),
    /// Logical OR
    Or(Box<Condition>, Box<Condition>),
}

/// Aggregation to compute over all records.
#[derive(Debug, Clone)]
pub enum Aggregation {
    Count,
    Sum(usize),
    Average(usize),
    Min(usize),
    Max(usize),
    Distinct(usize),
    Frequency(usize),
    CountWhere(Condition),
}

/// Configuration for the awk engine.
#[derive(Debug, Clone)]
pub struct AwkConfig {
    /// Field separator (regex). Default is whitespace.
    pub field_separator: String,
    /// Output field separator. Default is tab.
    pub output_separator: String,
    /// Rules to apply.
    pub rules: Vec<Rule>,
    /// Aggregations to compute.
    pub aggregations: Vec<(String, Aggregation)>,
    /// Whether to include header line.
    pub has_header: bool,
    /// Skip first N lines.
    pub skip_lines: usize,
    /// Maximum records to process.
    pub max_records: Option<usize>,
}

impl Default for AwkConfig {
    fn default() -> Self {
        Self {
            field_separator: r"\s+".to_string(),
            output_separator: "\t".to_string(),
            rules: Vec::new(),
            aggregations: Vec::new(),
            has_header: false,
            skip_lines: 0,
            max_records: None,
        }
    }
}

/// The awk processing engine.
pub struct AwkEngine {
    config: AwkConfig,
    separator_re: Regex,
}

impl AwkEngine {
    pub fn new(config: AwkConfig) -> Result<Self> {
        let separator_re = Regex::new(&config.field_separator)
            .with_context(|| format!("Invalid field separator: {}", config.field_separator))?;
        Ok(Self {
            config,
            separator_re,
        })
    }

    /// Process a single file.
    pub fn process_file(&self, path: &Path) -> Result<Vec<AnalysisRecord>> {
        self.process_file_with_nr_offset(path, 0)
    }

    /// Process a single file with an NR offset (for multi-file NR continuity).
    fn process_file_with_nr_offset(
        &self,
        path: &Path,
        nr_offset: usize,
    ) -> Result<Vec<AnalysisRecord>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let file_str = path.display().to_string();
        let mut records = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut record_count = 0usize;

        for (idx, line) in lines.iter().enumerate().skip(self.config.skip_lines) {
            let line_num = idx + 1;
            record_count += 1;

            if let Some(max) = self.config.max_records {
                if records.len() >= max {
                    break;
                }
            }

            // Split into fields
            let fields: Vec<String> = self
                .separator_re
                .split(line.trim())
                .map(|s| s.to_string())
                .collect();

            let nf = fields.len();
            let nr = nr_offset + record_count;

            // Apply rules
            for rule in &self.config.rules {
                // Check pattern match
                if let Some(ref pattern) = rule.pattern {
                    let re = Regex::new(pattern)?;
                    if !re.is_match(line) {
                        continue;
                    }
                }

                // Apply condition
                if let Some(ref condition) = rule.condition {
                    if !self.evaluate_condition(condition, &fields) {
                        continue;
                    }
                }

                // Select fields
                let selected: Vec<String> = if rule.select_fields.is_empty() {
                    fields.clone()
                } else {
                    rule.select_fields
                        .iter()
                        .map(|&i| fields.get(i.saturating_sub(1)).cloned().unwrap_or_default())
                        .collect()
                };

                // Compute fields
                let mut computed = BTreeMap::new();
                for cf in &rule.computed_fields {
                    let val = self.evaluate_expression(&cf.expression, &fields, nr, nf);
                    computed.insert(cf.name.clone(), serde_json::Value::String(val));
                }

                records.push(AnalysisRecord {
                    source_file: file_str.clone(),
                    source_line: line_num,
                    nr,
                    nf,
                    fields: selected,
                    computed,
                });
            }
        }

        Ok(records)
    }

    /// Process multiple files and aggregate results.
    pub fn process_files(&self, paths: &[&Path]) -> Result<AnalysisResults> {
        let mut all_records = Vec::new();
        let mut nr_offset = 0usize;

        for path in paths {
            let file_records = self.process_file_with_nr_offset(path, nr_offset)?;
            nr_offset += file_records.len();
            all_records.extend(file_records);
        }

        // Compute aggregations
        let mut aggregation_results = BTreeMap::new();
        for (name, agg) in &self.config.aggregations {
            let value = self.compute_aggregation(agg, &all_records)?;
            aggregation_results.insert(name.clone(), value);
        }

        let records_processed = all_records.len();
        Ok(AnalysisResults {
            records_processed,
            output_records: all_records,
            aggregations: aggregation_results,
            field_separator: self.config.field_separator.clone(),
        })
    }

    // ── Streaming API ───────────────────────────────────────────────────

    /// Process a readable source in streaming mode (memory-efficient for large files).
    ///
    /// Reads input line-by-line without loading the entire content into memory.
    pub fn process_reader<R: Read>(
        &self,
        reader: R,
        source_name: &str,
        nr_offset: usize,
    ) -> Result<Vec<AnalysisRecord>> {
        let buf = BufReader::new(reader);
        let mut records = Vec::new();
        let mut record_count = 0usize;

        for (idx, line_result) in buf.lines().enumerate().skip(self.config.skip_lines) {
            let line = line_result
                .with_context(|| format!("Failed to read line {} from {}", idx + 1, source_name))?;
            record_count += 1;

            if let Some(max) = self.config.max_records {
                if records.len() >= max {
                    break;
                }
            }

            let fields: Vec<String> = self
                .separator_re
                .split(line.trim())
                .map(|s| s.to_string())
                .collect();

            let nf = fields.len();
            let nr = nr_offset + record_count;
            let line_num = idx + 1;

            for rule in &self.config.rules {
                if let Some(ref pattern) = rule.pattern {
                    let re = Regex::new(pattern)?;
                    if !re.is_match(&line) {
                        continue;
                    }
                }

                if let Some(ref condition) = rule.condition {
                    if !self.evaluate_condition(condition, &fields) {
                        continue;
                    }
                }

                let selected: Vec<String> = if rule.select_fields.is_empty() {
                    fields.clone()
                } else {
                    rule.select_fields
                        .iter()
                        .map(|&i| fields.get(i.saturating_sub(1)).cloned().unwrap_or_default())
                        .collect()
                };

                let mut computed = BTreeMap::new();
                for cf in &rule.computed_fields {
                    let val = self.evaluate_expression(&cf.expression, &fields, nr, nf);
                    computed.insert(cf.name.clone(), serde_json::Value::String(val));
                }

                records.push(AnalysisRecord {
                    source_file: source_name.to_string(),
                    source_line: line_num,
                    nr,
                    nf,
                    fields: selected,
                    computed,
                });
            }
        }

        Ok(records)
    }

    /// Process a file in streaming mode (memory-efficient for large files).
    pub fn process_file_streaming(&self, path: &Path) -> Result<Vec<AnalysisRecord>> {
        let file = fs::File::open(path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;
        self.process_reader(file, &path.display().to_string(), 0)
    }

    /// Process multiple files in streaming mode.
    pub fn process_files_streaming(&self, paths: &[&Path]) -> Result<AnalysisResults> {
        let mut all_records = Vec::new();
        let mut nr_offset = 0usize;

        for path in paths {
            let file = fs::File::open(path)
                .with_context(|| format!("Failed to open file: {}", path.display()))?;
            let file_records = self.process_reader(file, &path.display().to_string(), nr_offset)?;
            nr_offset += file_records.len();
            all_records.extend(file_records);
        }

        let mut aggregation_results = BTreeMap::new();
        for (name, agg) in &self.config.aggregations {
            let value = self.compute_aggregation(agg, &all_records)?;
            aggregation_results.insert(name.clone(), value);
        }

        let records_processed = all_records.len();
        Ok(AnalysisResults {
            records_processed,
            output_records: all_records,
            aggregations: aggregation_results,
            field_separator: self.config.field_separator.clone(),
        })
    }

    fn evaluate_condition(&self, condition: &Condition, fields: &[String]) -> bool {
        match condition {
            Condition::Equals(idx, val) => fields
                .get(idx.saturating_sub(1))
                .map(|f| f == val)
                .unwrap_or(false),
            Condition::NotEquals(idx, val) => fields
                .get(idx.saturating_sub(1))
                .map(|f| f != val)
                .unwrap_or(true),
            Condition::Contains(idx, val) => fields
                .get(idx.saturating_sub(1))
                .map(|f| f.contains(val.as_str()))
                .unwrap_or(false),
            Condition::Matches(idx, pattern) => {
                if let Ok(re) = Regex::new(pattern) {
                    fields
                        .get(idx.saturating_sub(1))
                        .map(|f| re.is_match(f))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            Condition::GreaterThan(idx, val) => fields
                .get(idx.saturating_sub(1))
                .and_then(|f| f.parse::<f64>().ok())
                .map(|f| f > *val)
                .unwrap_or(false),
            Condition::LessThan(idx, val) => fields
                .get(idx.saturating_sub(1))
                .and_then(|f| f.parse::<f64>().ok())
                .map(|f| f < *val)
                .unwrap_or(false),
            Condition::NotEmpty(idx) => fields
                .get(idx.saturating_sub(1))
                .map(|f| !f.is_empty())
                .unwrap_or(false),
            Condition::And(a, b) => {
                self.evaluate_condition(a, fields) && self.evaluate_condition(b, fields)
            }
            Condition::Or(a, b) => {
                self.evaluate_condition(a, fields) || self.evaluate_condition(b, fields)
            }
        }
    }

    fn evaluate_expression(
        &self,
        expr: &FieldExpression,
        fields: &[String],
        nr: usize,
        nf: usize,
    ) -> String {
        match expr {
            FieldExpression::Field(idx) => fields
                .get(idx.saturating_sub(1))
                .cloned()
                .unwrap_or_default(),
            FieldExpression::Concat(indices, sep) => indices
                .iter()
                .map(|&i| fields.get(i.saturating_sub(1)).cloned().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(sep),
            FieldExpression::Length(idx) => fields
                .get(idx.saturating_sub(1))
                .map(|f| f.len().to_string())
                .unwrap_or_else(|| "0".to_string()),
            FieldExpression::Upper(idx) => fields
                .get(idx.saturating_sub(1))
                .map(|f| f.to_uppercase())
                .unwrap_or_default(),
            FieldExpression::Lower(idx) => fields
                .get(idx.saturating_sub(1))
                .map(|f| f.to_lowercase())
                .unwrap_or_default(),
            FieldExpression::Substr(idx, start, len) => {
                let field = fields
                    .get(idx.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                let s = *start;
                match len {
                    Some(l) => field.chars().skip(s).take(*l).collect(),
                    None => field.chars().skip(s).collect(),
                }
            }
            FieldExpression::Literal(val) => val.clone(),
            FieldExpression::Sub(idx, pattern, replacement) => {
                // Replace first match (like awk sub())
                let field = fields
                    .get(idx.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                if let Ok(re) = Regex::new(pattern) {
                    re.replace(&field, replacement.as_str()).to_string()
                } else {
                    field
                }
            }
            FieldExpression::Gsub(idx, pattern, replacement) => {
                // Replace all matches (like awk gsub())
                let field = fields
                    .get(idx.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                if let Ok(re) = Regex::new(pattern) {
                    re.replace_all(&field, replacement.as_str()).to_string()
                } else {
                    field
                }
            }
            FieldExpression::Match(idx, pattern) => {
                // Return matched text or empty (like awk match())
                let field = fields
                    .get(idx.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                if let Ok(re) = Regex::new(pattern) {
                    re.find(&field)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
            FieldExpression::Split(idx, separator, nth) => {
                // Split field by separator and return Nth part (like awk split())
                let field = fields
                    .get(idx.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();
                if let Ok(re) = Regex::new(separator) {
                    re.split(&field)
                        .nth(nth.saturating_sub(1))
                        .unwrap_or("")
                        .to_string()
                } else {
                    field
                }
            }
            FieldExpression::NR => nr.to_string(),
            FieldExpression::NF => nf.to_string(),
        }
    }

    fn compute_aggregation(
        &self,
        agg: &Aggregation,
        records: &[AnalysisRecord],
    ) -> Result<AggregationValue> {
        match agg {
            Aggregation::Count => Ok(AggregationValue::Count(records.len())),

            Aggregation::Sum(field_idx) => {
                let sum: f64 = records
                    .iter()
                    .filter_map(|r| {
                        r.fields
                            .get(field_idx.saturating_sub(1))
                            .and_then(|f| f.parse::<f64>().ok())
                    })
                    .sum();
                Ok(AggregationValue::Sum(sum))
            }

            Aggregation::Average(field_idx) => {
                let vals: Vec<f64> = records
                    .iter()
                    .filter_map(|r| {
                        r.fields
                            .get(field_idx.saturating_sub(1))
                            .and_then(|f| f.parse::<f64>().ok())
                    })
                    .collect();
                if vals.is_empty() {
                    Ok(AggregationValue::Average(0.0))
                } else {
                    let avg = vals.iter().sum::<f64>() / vals.len() as f64;
                    Ok(AggregationValue::Average(avg))
                }
            }

            Aggregation::Min(field_idx) => {
                let min = records
                    .iter()
                    .filter_map(|r| {
                        r.fields
                            .get(field_idx.saturating_sub(1))
                            .and_then(|f| f.parse::<f64>().ok())
                    })
                    .fold(f64::INFINITY, f64::min);
                Ok(AggregationValue::Min(min))
            }

            Aggregation::Max(field_idx) => {
                let max = records
                    .iter()
                    .filter_map(|r| {
                        r.fields
                            .get(field_idx.saturating_sub(1))
                            .and_then(|f| f.parse::<f64>().ok())
                    })
                    .fold(f64::NEG_INFINITY, f64::max);
                Ok(AggregationValue::Max(max))
            }

            Aggregation::Distinct(field_idx) => {
                let mut vals: Vec<String> = records
                    .iter()
                    .filter_map(|r| r.fields.get(field_idx.saturating_sub(1)).cloned())
                    .collect();
                vals.sort();
                vals.dedup();
                Ok(AggregationValue::Distinct(vals))
            }

            Aggregation::Frequency(field_idx) => {
                let mut freq = BTreeMap::new();
                for r in records {
                    if let Some(val) = r.fields.get(field_idx.saturating_sub(1)) {
                        *freq.entry(val.clone()).or_insert(0usize) += 1;
                    }
                }
                Ok(AggregationValue::Frequency(freq))
            }

            Aggregation::CountWhere(condition) => {
                let count = records
                    .iter()
                    .filter(|r| self.evaluate_condition(condition, &r.fields))
                    .count();
                Ok(AggregationValue::Count(count))
            }
        }
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

    fn default_rule() -> Rule {
        Rule {
            pattern: None,
            select_fields: vec![],
            computed_fields: vec![],
            condition: None,
        }
    }

    #[test]
    fn test_default_config_whitespace_separator() {
        let config = AwkConfig {
            rules: vec![default_rule()],
            ..Default::default()
        };
        assert_eq!(config.field_separator, r"\s+");
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("hello world\nfoo bar baz\n");
        let records = engine.process_file(tmp.path()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fields, vec!["hello", "world"]);
        assert_eq!(records[1].fields, vec!["foo", "bar", "baz"]);
        assert_eq!(records[0].nf, 2);
        assert_eq!(records[1].nf, 3);
    }

    #[test]
    fn test_csv_separator() {
        let config = AwkConfig {
            field_separator: ",".to_string(),
            rules: vec![default_rule()],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("a,b,c\n1,2,3\n");
        let records = engine.process_file(tmp.path()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fields, vec!["a", "b", "c"]);
        assert_eq!(records[1].fields, vec!["1", "2", "3"]);
    }

    #[test]
    fn test_select_fields() {
        let config = AwkConfig {
            rules: vec![Rule {
                pattern: None,
                select_fields: vec![1, 3],
                computed_fields: vec![],
                condition: None,
            }],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("a b c d\n1 2 3 4\n");
        let records = engine.process_file(tmp.path()).unwrap();
        assert_eq!(records[0].fields, vec!["a", "c"]);
        assert_eq!(records[1].fields, vec!["1", "3"]);
    }

    #[test]
    fn test_condition_equals() {
        let config = AwkConfig {
            field_separator: ",".to_string(),
            rules: vec![Rule {
                pattern: None,
                select_fields: vec![],
                computed_fields: vec![],
                condition: Some(Condition::Equals(1, "yes".to_string())),
            }],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("yes,keep\nno,drop\nyes,also keep\n");
        let records = engine.process_file(tmp.path()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fields, vec!["yes", "keep"]);
        assert_eq!(records[1].fields, vec!["yes", "also keep"]);
    }

    #[test]
    fn test_aggregation_count_and_sum() {
        let config = AwkConfig {
            field_separator: ",".to_string(),
            rules: vec![default_rule()],
            aggregations: vec![
                ("total".to_string(), Aggregation::Count),
                ("sum_f2".to_string(), Aggregation::Sum(2)),
            ],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("a,10\nb,20\nc,30\n");
        let path_refs: Vec<&Path> = vec![tmp.path()];
        let results = engine.process_files(&path_refs).unwrap();
        assert_eq!(results.records_processed, 3);
        match &results.aggregations["total"] {
            AggregationValue::Count(c) => assert_eq!(*c, 3),
            _ => panic!("Expected Count"),
        }
        match &results.aggregations["sum_f2"] {
            AggregationValue::Sum(s) => assert!((s - 60.0).abs() < f64::EPSILON),
            _ => panic!("Expected Sum"),
        }
    }

    #[test]
    fn test_process_files_multiple() {
        let config = AwkConfig {
            rules: vec![default_rule()],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp1 = write_tmp("a b\n");
        let tmp2 = write_tmp("c d\n");
        let path_refs: Vec<&Path> = vec![tmp1.path(), tmp2.path()];
        let results = engine.process_files(&path_refs).unwrap();
        assert_eq!(results.records_processed, 2);
        // NR should be 1-based sequential across files
        assert_eq!(results.output_records[0].nr, 1);
        assert_eq!(results.output_records[1].nr, 2);
    }

    #[test]
    fn test_empty_file() {
        let config = AwkConfig {
            rules: vec![default_rule()],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("");
        let records = engine.process_file(tmp.path()).unwrap();
        assert!(records.is_empty());
    }

    // ── Streaming tests ──────────────────────────────────────────────

    #[test]
    fn test_streaming_basic() {
        let config = AwkConfig {
            rules: vec![default_rule()],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let data = b"hello world\nfoo bar baz\n";
        let records = engine.process_reader(&data[..], "stdin", 0).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fields, vec!["hello", "world"]);
        assert_eq!(records[1].fields, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_streaming_csv() {
        let config = AwkConfig {
            field_separator: ",".to_string(),
            rules: vec![default_rule()],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let data = b"a,b,c\n1,2,3\n";
        let records = engine.process_reader(&data[..], "pipe", 0).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_streaming_file_matches_non_streaming() {
        let config = AwkConfig {
            rules: vec![default_rule()],
            ..Default::default()
        };
        let engine = AwkEngine::new(config).unwrap();
        let tmp = write_tmp("hello world\nfoo bar baz\n");
        let non_streaming = engine.process_file(tmp.path()).unwrap();
        let streaming = engine.process_file_streaming(tmp.path()).unwrap();
        assert_eq!(non_streaming.len(), streaming.len());
        for (a, b) in non_streaming.iter().zip(streaming.iter()) {
            assert_eq!(a.fields, b.fields);
            assert_eq!(a.nr, b.nr);
            assert_eq!(a.nf, b.nf);
        }
    }
}
