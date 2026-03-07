//! # Sampler — Statistical Sampling from Text
//!
//! Reservoir sampling, stratified sampling, systematic sampling,
//! random line selection, and weighted selection from text lines.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Sampling method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SampleMethod {
    /// Reservoir sampling (Vitter's Algorithm R).
    Reservoir,
    /// Systematic: every k-th item.
    Systematic,
    /// Stratified: divide into strata, sample from each.
    Stratified,
    /// Pure random without replacement.
    Random,
    /// Weighted selection (lines weighted by length or custom weights).
    Weighted,
}

/// A sampled item with provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    /// 0-based line index in the original text.
    pub index: usize,
    /// The sampled line text.
    pub text: String,
    /// Optional stratum label (for stratified sampling).
    pub stratum: Option<String>,
}

/// Options for sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleOptions {
    /// Number of items to sample.
    pub count: usize,
    /// Random seed (deterministic when set).
    pub seed: u64,
    /// Number of strata (for stratified sampling).
    pub strata: usize,
    /// Step interval (for systematic sampling).
    pub step: usize,
}

impl Default for SampleOptions {
    fn default() -> Self {
        Self {
            count: 10,
            seed: 42,
            strata: 4,
            step: 5,
        }
    }
}

/// Summary of a sampling operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleSummary {
    pub method: String,
    pub population_size: usize,
    pub sample_size: usize,
    pub coverage_pct: f64,
}

// ---------------------------------------------------------------------------
// Deterministic PRNG (xorshift64)
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Random index in 0..n.
    fn usize_below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Random f64 in [0, 1).
    fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ---------------------------------------------------------------------------
// Sampling algorithms
// ---------------------------------------------------------------------------

/// Reservoir sampling (Algorithm R) — sample `k` items from a stream.
pub fn reservoir(lines: &[&str], k: usize, seed: u64) -> Vec<Sample> {
    if k == 0 || lines.is_empty() {
        return Vec::new();
    }
    let k = k.min(lines.len());
    let mut rng = Rng::new(seed);
    let mut reservoir: Vec<Sample> = lines[..k]
        .iter()
        .enumerate()
        .map(|(i, &l)| Sample {
            index: i,
            text: l.to_string(),
            stratum: None,
        })
        .collect();
    for (i, line) in lines.iter().enumerate().skip(k) {
        let j = rng.usize_below(i + 1);
        if j < k {
            reservoir[j] = Sample {
                index: i,
                text: line.to_string(),
                stratum: None,
            };
        }
    }
    reservoir
}

/// Systematic sampling — take every `step`-th line starting from a random offset.
pub fn systematic(lines: &[&str], step: usize, seed: u64) -> Vec<Sample> {
    if lines.is_empty() || step == 0 {
        return Vec::new();
    }
    let mut rng = Rng::new(seed);
    let start = rng.usize_below(step.min(lines.len()));
    let mut samples = Vec::new();
    let mut i = start;
    while i < lines.len() {
        samples.push(Sample {
            index: i,
            text: lines[i].to_string(),
            stratum: None,
        });
        i += step;
    }
    samples
}

/// Stratified sampling — divide into `num_strata` groups, sample proportionally.
pub fn stratified(lines: &[&str], count: usize, num_strata: usize, seed: u64) -> Vec<Sample> {
    if lines.is_empty() || count == 0 || num_strata == 0 {
        return Vec::new();
    }
    let strata_size = lines.len().div_ceil(num_strata);
    let per_stratum = count.div_ceil(num_strata);
    let mut rng = Rng::new(seed);
    let mut samples = Vec::new();

    for s in 0..num_strata {
        let start = s * strata_size;
        let end = (start + strata_size).min(lines.len());
        if start >= end {
            break;
        }
        let stratum_lines: Vec<usize> = (start..end).collect();
        let take = per_stratum.min(stratum_lines.len());
        // Fisher-Yates partial shuffle
        let mut indices = stratum_lines;
        for i in 0..take {
            let j = i + rng.usize_below(indices.len() - i);
            indices.swap(i, j);
        }
        for &idx in &indices[..take] {
            samples.push(Sample {
                index: idx,
                text: lines[idx].to_string(),
                stratum: Some(format!("stratum-{}", s + 1)),
            });
        }
    }
    samples.truncate(count);
    samples
}

/// Random sampling without replacement.
pub fn random_sample(lines: &[&str], count: usize, seed: u64) -> Vec<Sample> {
    if lines.is_empty() || count == 0 {
        return Vec::new();
    }
    let k = count.min(lines.len());
    let mut rng = Rng::new(seed);
    let mut indices: Vec<usize> = (0..lines.len()).collect();
    // Fisher-Yates partial shuffle
    for i in 0..k {
        let j = i + rng.usize_below(indices.len() - i);
        indices.swap(i, j);
    }
    indices[..k]
        .iter()
        .map(|&i| Sample {
            index: i,
            text: lines[i].to_string(),
            stratum: None,
        })
        .collect()
}

/// Weighted sampling — lines weighted by their byte length.
pub fn weighted_sample(lines: &[&str], count: usize, seed: u64) -> Vec<Sample> {
    weighted_sample_with(lines, count, seed, |l| l.len() as f64 + 1.0)
}

/// Weighted sampling with a custom weight function.
pub fn weighted_sample_with<F>(lines: &[&str], count: usize, seed: u64, weight_fn: F) -> Vec<Sample>
where
    F: Fn(&str) -> f64,
{
    if lines.is_empty() || count == 0 {
        return Vec::new();
    }
    let k = count.min(lines.len());
    let mut rng = Rng::new(seed);
    let weights: Vec<f64> = lines.iter().map(|l| weight_fn(l).max(0.0)).collect();
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return random_sample(lines, count, seed);
    }
    let mut selected = Vec::new();
    let mut used = vec![false; lines.len()];
    for _ in 0..k {
        let mut target = rng.f64() * total;
        let mut chosen = 0;
        for (i, &w) in weights.iter().enumerate() {
            if used[i] {
                continue;
            }
            target -= w;
            if target <= 0.0 {
                chosen = i;
                break;
            }
            chosen = i;
        }
        used[chosen] = true;
        selected.push(Sample {
            index: chosen,
            text: lines[chosen].to_string(),
            stratum: None,
        });
    }
    selected
}

// ---------------------------------------------------------------------------
// Unified interface
// ---------------------------------------------------------------------------

/// Sample from text using the specified method.
pub fn sample(text: &str, method: SampleMethod, opts: &SampleOptions) -> Vec<Sample> {
    let lines: Vec<&str> = text.lines().collect();
    match method {
        SampleMethod::Reservoir => reservoir(&lines, opts.count, opts.seed),
        SampleMethod::Systematic => systematic(&lines, opts.step, opts.seed),
        SampleMethod::Stratified => stratified(&lines, opts.count, opts.strata, opts.seed),
        SampleMethod::Random => random_sample(&lines, opts.count, opts.seed),
        SampleMethod::Weighted => weighted_sample(&lines, opts.count, opts.seed),
    }
}

/// Generate a summary of the sampling.
pub fn summary(text: &str, method: SampleMethod, samples: &[Sample]) -> SampleSummary {
    let pop = text.lines().count();
    let n = samples.len();
    SampleSummary {
        method: format!("{method:?}"),
        population_size: pop,
        sample_size: n,
        coverage_pct: if pop > 0 {
            n as f64 / pop as f64 * 100.0
        } else {
            0.0
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_text() -> &'static str {
        "line 0\nline 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9"
    }

    #[test]
    fn test_reservoir_count() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = reservoir(&lines, 3, 42);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn test_reservoir_deterministic() {
        let lines: Vec<&str> = test_text().lines().collect();
        let a = reservoir(&lines, 3, 42);
        let b = reservoir(&lines, 3, 42);
        assert_eq!(
            a.iter().map(|s| s.index).collect::<Vec<_>>(),
            b.iter().map(|s| s.index).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_reservoir_large_k() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = reservoir(&lines, 100, 42);
        assert_eq!(s.len(), 10); // capped at population
    }

    #[test]
    fn test_systematic() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = systematic(&lines, 3, 1);
        assert!(!s.is_empty());
        // Check step is consistent
        if s.len() >= 2 {
            assert_eq!(s[1].index - s[0].index, 3);
        }
    }

    #[test]
    fn test_stratified() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = stratified(&lines, 4, 2, 42);
        assert_eq!(s.len(), 4);
        // Should have stratum labels
        assert!(s.iter().any(|x| x.stratum.as_deref() == Some("stratum-1")));
    }

    #[test]
    fn test_random_no_replacement() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = random_sample(&lines, 5, 42);
        assert_eq!(s.len(), 5);
        let indices: Vec<usize> = s.iter().map(|x| x.index).collect();
        let mut unique = indices.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 5, "no duplicates");
    }

    #[test]
    fn test_weighted() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = weighted_sample(&lines, 3, 42);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn test_weighted_custom() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = weighted_sample_with(
            &lines,
            2,
            42,
            |l| {
                if l.contains('0') {
                    100.0
                } else {
                    0.01
                }
            },
        );
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn test_sample_unified() {
        let opts = SampleOptions {
            count: 3,
            seed: 42,
            strata: 2,
            step: 3,
        };
        let s = sample(test_text(), SampleMethod::Random, &opts);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn test_summary() {
        let opts = SampleOptions::default();
        let s = sample(test_text(), SampleMethod::Reservoir, &opts);
        let sum = summary(test_text(), SampleMethod::Reservoir, &s);
        assert_eq!(sum.population_size, 10);
        assert_eq!(sum.sample_size, s.len());
    }

    #[test]
    fn test_empty_input() {
        let s = sample("", SampleMethod::Random, &SampleOptions::default());
        assert!(s.is_empty());
    }

    #[test]
    fn test_single_line() {
        let s = sample(
            "only",
            SampleMethod::Reservoir,
            &SampleOptions {
                count: 1,
                ..Default::default()
            },
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "only");
    }

    #[test]
    fn test_systematic_step_1() {
        let lines: Vec<&str> = test_text().lines().collect();
        let s = systematic(&lines, 1, 42);
        assert_eq!(s.len(), 10); // every line
    }

    #[test]
    fn test_method_labels() {
        let opts = SampleOptions::default();
        for method in [
            SampleMethod::Reservoir,
            SampleMethod::Systematic,
            SampleMethod::Stratified,
            SampleMethod::Random,
            SampleMethod::Weighted,
        ] {
            let s = sample(test_text(), method, &opts);
            let sum = summary(test_text(), method, &s);
            assert!(!sum.method.is_empty());
        }
    }

    #[test]
    fn test_rng_deterministic() {
        let mut a = Rng::new(12345);
        let mut b = Rng::new(12345);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}
