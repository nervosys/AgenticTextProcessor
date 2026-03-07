// ---------------------------------------------------------------------------
// fuzzy.rs — Fuzzy string matching
// ---------------------------------------------------------------------------
//
// Jaro-Winkler distance, trigram similarity, Levenshtein automaton,
// ranked fuzzy search with scoring, best-match selection.
// ---------------------------------------------------------------------------

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Jaro similarity
// ---------------------------------------------------------------------------

/// Jaro similarity between two strings (0.0 = no match, 1.0 = identical).
pub fn jaro(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();
    if la == 0 || lb == 0 {
        return 0.0;
    }
    let window = (la.max(lb) / 2).saturating_sub(1);
    let mut a_matched = vec![false; la];
    let mut b_matched = vec![false; lb];
    let mut matches = 0usize;
    for (i, &ac) in a_chars.iter().enumerate() {
        let lo = i.saturating_sub(window);
        let hi = (i + window + 1).min(lb);
        for j in lo..hi {
            if !b_matched[j] && b_chars[j] == ac {
                a_matched[i] = true;
                b_matched[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for (i, _) in a_chars.iter().enumerate().filter(|(i, _)| a_matched[*i]) {
        while !b_matched[k] {
            k += 1;
        }
        if a_chars[i] != b_chars[k] {
            transpositions += 1;
        }
        k += 1;
    }
    let m = matches as f64;
    (m / la as f64 + m / lb as f64 + (m - transpositions as f64 / 2.0) / m) / 3.0
}

// ---------------------------------------------------------------------------
// Jaro-Winkler
// ---------------------------------------------------------------------------

/// Jaro-Winkler similarity — boosts score when a common prefix exists.
///
/// `prefix_weight` is typically 0.1. The boost is limited to prefix length 4.
pub fn jaro_winkler(a: &str, b: &str, prefix_weight: f64) -> f64 {
    let j = jaro(a, b);
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let prefix_len = a_chars
        .iter()
        .zip(b_chars.iter())
        .take(4)
        .take_while(|(x, y)| x == y)
        .count();
    j + prefix_len as f64 * prefix_weight * (1.0 - j)
}

/// Jaro-Winkler with default prefix weight (0.1).
pub fn jaro_winkler_default(a: &str, b: &str) -> f64 {
    jaro_winkler(a, b, 0.1)
}

// ---------------------------------------------------------------------------
// Trigram similarity
// ---------------------------------------------------------------------------

/// Extract character trigrams from a string (padded with spaces).
pub fn trigrams(s: &str) -> Vec<String> {
    let padded = format!("  {s} ");
    let chars: Vec<char> = padded.chars().collect();
    if chars.len() < 3 {
        return vec![padded];
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Trigram (Jaccard-based) similarity between two strings.
pub fn trigram_similarity(a: &str, b: &str) -> f64 {
    let ta: HashSet<String> = trigrams(&a.to_lowercase()).into_iter().collect();
    let tb: HashSet<String> = trigrams(&b.to_lowercase()).into_iter().collect();
    let intersection = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

// ---------------------------------------------------------------------------
// Levenshtein automaton (bounded edit distance)
// ---------------------------------------------------------------------------

/// Bounded Levenshtein distance — returns `None` if distance exceeds `max_dist`.
pub fn bounded_levenshtein(a: &str, b: &str, max_dist: usize) -> Option<usize> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();
    if la.abs_diff(lb) > max_dist {
        return None;
    }
    let mut prev: Vec<usize> = (0..=lb).collect();
    let mut curr = vec![0usize; lb + 1];
    for i in 1..=la {
        curr[0] = i;
        let mut min_in_row = curr[0];
        for j in 1..=lb {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            min_in_row = min_in_row.min(curr[j]);
        }
        if min_in_row > max_dist {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    if prev[lb] <= max_dist {
        Some(prev[lb])
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Fuzzy search types
// ---------------------------------------------------------------------------

/// A scored match from fuzzy search.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// The matched candidate string.
    pub text: String,
    /// Index of the candidate in the input list.
    pub index: usize,
    /// Combined similarity score (0.0–1.0).
    pub score: f64,
    /// Jaro-Winkler component.
    pub jaro_winkler: f64,
    /// Trigram component.
    pub trigram: f64,
    /// Edit distance (if within threshold).
    pub edit_distance: Option<usize>,
}

/// Configuration for fuzzy search.
#[derive(Debug, Clone)]
pub struct FuzzyConfig {
    /// Maximum edit distance for bounded Levenshtein.
    pub max_edit_distance: usize,
    /// Weight for Jaro-Winkler score (default 0.5).
    pub jw_weight: f64,
    /// Weight for trigram score (default 0.3).
    pub trigram_weight: f64,
    /// Weight for edit-distance-derived score (default 0.2).
    pub edit_weight: f64,
    /// Minimum combined score to include in results.
    pub min_score: f64,
    /// Maximum number of results to return (0 = unlimited).
    pub max_results: usize,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            max_edit_distance: 3,
            jw_weight: 0.5,
            trigram_weight: 0.3,
            edit_weight: 0.2,
            min_score: 0.3,
            max_results: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Ranked fuzzy search
// ---------------------------------------------------------------------------

/// Score a single candidate against a query.
pub fn score(query: &str, candidate: &str, cfg: &FuzzyConfig) -> FuzzyMatch {
    let q = query.to_lowercase();
    let c = candidate.to_lowercase();
    let jw = jaro_winkler_default(&q, &c);
    let tg = trigram_similarity(&q, &c);
    let ed = bounded_levenshtein(&q, &c, cfg.max_edit_distance);
    let ed_score = ed
        .map(|d| 1.0 - d as f64 / (cfg.max_edit_distance as f64 + 1.0))
        .unwrap_or(0.0);
    let combined = cfg.jw_weight * jw + cfg.trigram_weight * tg + cfg.edit_weight * ed_score;
    FuzzyMatch {
        text: candidate.to_string(),
        index: 0,
        score: combined,
        jaro_winkler: jw,
        trigram: tg,
        edit_distance: ed,
    }
}

/// Search `candidates` for fuzzy matches to `query`, ranked by score.
pub fn fuzzy_search(query: &str, candidates: &[&str], cfg: &FuzzyConfig) -> Vec<FuzzyMatch> {
    let mut results: Vec<FuzzyMatch> = candidates
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let mut m = score(query, c, cfg);
            m.index = i;
            m
        })
        .filter(|m| m.score >= cfg.min_score)
        .collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if cfg.max_results > 0 && results.len() > cfg.max_results {
        results.truncate(cfg.max_results);
    }
    results
}

/// Return the single best match, if any.
pub fn best_match(
    query: &str,
    candidates: &[&str],
    cfg: &FuzzyConfig,
) -> Option<FuzzyMatch> {
    fuzzy_search(query, candidates, cfg).into_iter().next()
}

/// Quick fuzzy search with default configuration.
pub fn search(query: &str, candidates: &[&str]) -> Vec<FuzzyMatch> {
    fuzzy_search(query, candidates, &FuzzyConfig::default())
}

// ---------------------------------------------------------------------------
// Containment / substring score
// ---------------------------------------------------------------------------

/// Returns 1.0 if `needle` is a substring of `haystack` (case-insensitive), else 0.0.
pub fn contains_score(needle: &str, haystack: &str) -> f64 {
    if haystack.to_lowercase().contains(&needle.to_lowercase()) {
        1.0
    } else {
        0.0
    }
}

/// Combined score: max of fuzzy score and containment bonus.
pub fn hybrid_score(query: &str, candidate: &str, cfg: &FuzzyConfig) -> f64 {
    let fs = score(query, candidate, cfg).score;
    let cs = contains_score(query, candidate);
    fs.max(cs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaro_identical() {
        assert!((jaro("hello", "hello") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaro_empty() {
        assert!((jaro("", "hello")).abs() < f64::EPSILON);
        assert!((jaro("hello", "")).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaro_similar() {
        let s = jaro("martha", "marhta");
        assert!(s > 0.94 && s < 0.95);
    }

    #[test]
    fn test_jaro_winkler_boost() {
        let j = jaro("martha", "marhta");
        let jw = jaro_winkler_default("martha", "marhta");
        assert!(jw >= j);
    }

    #[test]
    fn test_jaro_winkler_identical() {
        assert!((jaro_winkler_default("test", "test") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trigrams_basic() {
        let t = trigrams("ab");
        assert!(!t.is_empty());
    }

    #[test]
    fn test_trigram_similarity_identical() {
        let s = trigram_similarity("hello", "hello");
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trigram_similarity_different() {
        let s = trigram_similarity("hello", "world");
        assert!(s < 0.5);
    }

    #[test]
    fn test_bounded_levenshtein_within() {
        assert_eq!(bounded_levenshtein("kitten", "sitting", 3), Some(3));
    }

    #[test]
    fn test_bounded_levenshtein_exceeds() {
        assert_eq!(bounded_levenshtein("abc", "xyz", 1), None);
    }

    #[test]
    fn test_bounded_levenshtein_zero() {
        assert_eq!(bounded_levenshtein("same", "same", 0), Some(0));
    }

    #[test]
    fn test_score_identical() {
        let m = score("rust", "rust", &FuzzyConfig::default());
        assert!((m.score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_fuzzy_search_ranking() {
        let candidates = vec!["rust", "dust", "trust", "ruby", "python"];
        let results = search("rust", &candidates);
        assert!(!results.is_empty());
        assert_eq!(results[0].text, "rust");
    }

    #[test]
    fn test_best_match() {
        let candidates = vec!["apple", "application", "apply", "banana"];
        let m = best_match("app", &candidates, &FuzzyConfig::default());
        assert!(m.is_some());
    }

    #[test]
    fn test_fuzzy_search_min_score_filter() {
        let cfg = FuzzyConfig {
            min_score: 0.99,
            ..Default::default()
        };
        let candidates = vec!["hello", "world"];
        let results = fuzzy_search("xyz", &candidates, &cfg);
        assert!(results.is_empty());
    }

    #[test]
    fn test_contains_score_hit() {
        assert!((contains_score("ello", "Hello World") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_contains_score_miss() {
        assert!((contains_score("xyz", "Hello World")).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hybrid_score_substring() {
        let s = hybrid_score("test", "this is a test string", &FuzzyConfig::default());
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_max_results_limit() {
        let cfg = FuzzyConfig {
            max_results: 2,
            min_score: 0.0,
            ..Default::default()
        };
        let candidates = vec!["a", "b", "c", "d", "e"];
        let results = fuzzy_search("a", &candidates, &cfg);
        assert!(results.len() <= 2);
    }
}
