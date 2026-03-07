//! # Fingerprint — Content Fingerprinting
//!
//! Provides near-duplicate detection via SimHash, MinHash, n-gram
//! shingling, rolling hash, and Jaccard similarity.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A content fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    pub method: String,
    pub bits: u64,
    pub hex: String,
}

/// Shingle — a contiguous n-gram of tokens.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shingle(pub Vec<String>);

/// Similarity result between two documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Similarity {
    pub method: String,
    pub score: f64,
}

/// Options for fingerprinting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintOptions {
    /// Shingle size (number of tokens per shingle).
    pub shingle_size: usize,
    /// Number of hash functions for MinHash.
    pub num_hashes: usize,
}

impl Default for FingerprintOptions {
    fn default() -> Self {
        Self {
            shingle_size: 3,
            num_hashes: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Tokenisation (simple word split)
// ---------------------------------------------------------------------------

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace().map(|w| w.to_lowercase()).collect()
}

// ---------------------------------------------------------------------------
// Shingling
// ---------------------------------------------------------------------------

/// Generate n-gram shingles from text.
pub fn shingle(text: &str, n: usize) -> Vec<Shingle> {
    let tokens = tokenize(text);
    if tokens.len() < n || n == 0 {
        return Vec::new();
    }
    tokens.windows(n).map(|w| Shingle(w.to_vec())).collect()
}

/// Generate shingle set (deduplicated).
pub fn shingle_set(text: &str, n: usize) -> BTreeSet<String> {
    shingle(text, n)
        .into_iter()
        .map(|s| s.0.join(" "))
        .collect()
}

// ---------------------------------------------------------------------------
// Hash helpers
// ---------------------------------------------------------------------------

/// Simple deterministic hash (FNV-1a 64-bit).
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Hash with a seed (for MinHash family).
fn seeded_hash(data: &[u8], seed: u64) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325 ^ seed;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ---------------------------------------------------------------------------
// SimHash
// ---------------------------------------------------------------------------

/// Compute SimHash (Charikar, 2002) — 64-bit locality-sensitive hash.
///
/// Shingles are hashed, then each bit position is voted on by all hashes.
pub fn simhash(text: &str, shingle_size: usize) -> Fingerprint {
    let shingles = shingle(text, shingle_size);
    let mut v = [0i64; 64];
    for s in &shingles {
        let h = fnv1a(s.0.join(" ").as_bytes());
        for (bit, v_val) in v.iter_mut().enumerate() {
            if (h >> bit) & 1 == 1 {
                *v_val += 1;
            } else {
                *v_val -= 1;
            }
        }
    }
    let mut bits: u64 = 0;
    for (bit, v_val) in v.iter().enumerate() {
        if *v_val > 0 {
            bits |= 1 << bit;
        }
    }
    Fingerprint {
        method: "simhash".into(),
        bits,
        hex: format!("{bits:016x}"),
    }
}

/// Hamming distance between two 64-bit fingerprints.
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// SimHash similarity (1.0 - hamming_distance / 64).
pub fn simhash_similarity(a: u64, b: u64) -> f64 {
    1.0 - hamming_distance(a, b) as f64 / 64.0
}

// ---------------------------------------------------------------------------
// MinHash
// ---------------------------------------------------------------------------

/// Compute MinHash signature for a set of shingles.
pub fn minhash(text: &str, opts: &FingerprintOptions) -> Vec<u64> {
    let set = shingle_set(text, opts.shingle_size);
    let mut sig = vec![u64::MAX; opts.num_hashes];
    for item in &set {
        let data = item.as_bytes();
        for (i, sig_val) in sig.iter_mut().enumerate() {
            let h = seeded_hash(data, i as u64);
            if h < *sig_val {
                *sig_val = h;
            }
        }
    }
    sig
}

/// Estimate Jaccard similarity from two MinHash signatures.
pub fn minhash_similarity(sig_a: &[u64], sig_b: &[u64]) -> f64 {
    if sig_a.len() != sig_b.len() || sig_a.is_empty() {
        return 0.0;
    }
    let matches = sig_a
        .iter()
        .zip(sig_b.iter())
        .filter(|(a, b)| a == b)
        .count();
    matches as f64 / sig_a.len() as f64
}

// ---------------------------------------------------------------------------
// Jaccard similarity (exact)
// ---------------------------------------------------------------------------

/// Exact Jaccard similarity on shingle sets.
pub fn jaccard_similarity(text_a: &str, text_b: &str, shingle_size: usize) -> Similarity {
    let set_a = shingle_set(text_a, shingle_size);
    let set_b = shingle_set(text_b, shingle_size);
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    let score = if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    };
    Similarity {
        method: "jaccard".into(),
        score,
    }
}

// ---------------------------------------------------------------------------
// Rolling hash (Rabin-like)
// ---------------------------------------------------------------------------

/// Compute rolling hash values over `window_size`-byte windows.
pub fn rolling_hashes(data: &[u8], window_size: usize) -> Vec<u64> {
    if data.len() < window_size || window_size == 0 {
        return Vec::new();
    }
    let base: u64 = 257;
    let modulus: u64 = (1 << 61) - 1; // Mersenne prime
                                      // Compute base^(window_size-1) mod modulus
    let mut pow = 1u64;
    for _ in 0..window_size.saturating_sub(1) {
        pow = pow.wrapping_mul(base) % modulus;
    }
    // Initial hash
    let mut h: u64 = 0;
    for &b in &data[..window_size] {
        h = (h.wrapping_mul(base).wrapping_add(b as u64)) % modulus;
    }
    let mut hashes = Vec::with_capacity(data.len() - window_size + 1);
    hashes.push(h);
    for i in 1..=data.len() - window_size {
        h = h
            .wrapping_add(modulus)
            .wrapping_sub((data[i - 1] as u64).wrapping_mul(pow) % modulus)
            % modulus;
        h = (h
            .wrapping_mul(base)
            .wrapping_add(data[i + window_size - 1] as u64))
            % modulus;
        hashes.push(h);
    }
    hashes
}

/// Find content-defined chunk boundaries using rolling hash.
pub fn chunk_boundaries(data: &[u8], window_size: usize, mask: u64) -> Vec<usize> {
    let hashes = rolling_hashes(data, window_size);
    let mut boundaries = Vec::new();
    for (i, &h) in hashes.iter().enumerate() {
        if h & mask == 0 {
            boundaries.push(i + window_size);
        }
    }
    boundaries
}

// ---------------------------------------------------------------------------
// Duplicate detection
// ---------------------------------------------------------------------------

/// Batch duplicate detection: returns groups of near-duplicate indices.
pub fn find_duplicates(
    documents: &[&str],
    opts: &FingerprintOptions,
    threshold: f64,
) -> Vec<Vec<usize>> {
    let sigs: Vec<Vec<u64>> = documents.iter().map(|d| minhash(d, opts)).collect();
    let n = sigs.len();
    let mut visited = vec![false; n];
    let mut groups = Vec::new();
    for i in 0..n {
        if visited[i] {
            continue;
        }
        let mut group = vec![i];
        for j in (i + 1)..n {
            if visited[j] {
                continue;
            }
            let sim = minhash_similarity(&sigs[i], &sigs[j]);
            if sim >= threshold {
                group.push(j);
                visited[j] = true;
            }
        }
        if group.len() > 1 {
            groups.push(group);
        }
        visited[i] = true;
    }
    groups
}

/// Compute a frequency map of tokens (for TF-IDF style weighting).
pub fn term_frequency(text: &str) -> HashMap<String, usize> {
    let mut freq = HashMap::new();
    for tok in tokenize(text) {
        *freq.entry(tok).or_insert(0) += 1;
    }
    freq
}

/// Summary report for multiple fingerprints.
pub fn summary_report(documents: &[&str], opts: &FingerprintOptions) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Documents: {}", documents.len()));
    lines.push(format!("Shingle size: {}", opts.shingle_size));
    lines.push(format!("MinHash functions: {}", opts.num_hashes));
    let groups = find_duplicates(documents, opts, 0.5);
    lines.push(format!("Duplicate groups (≥50% similar): {}", groups.len()));
    for (i, g) in groups.iter().enumerate() {
        lines.push(format!("  Group {}: {:?}", i + 1, g));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shingle_basic() {
        let shingles = shingle("the quick brown fox", 2);
        assert_eq!(shingles.len(), 3);
        assert_eq!(shingles[0].0, vec!["the", "quick"]);
    }

    #[test]
    fn test_shingle_empty() {
        assert!(shingle("hello", 3).is_empty());
        assert!(shingle("", 1).is_empty());
    }

    #[test]
    fn test_shingle_set_dedup() {
        let set = shingle_set("a b c a b c", 2);
        // "a b", "b c", "c a" — 3 unique shingles
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_simhash_identical() {
        let fp1 = simhash("the quick brown fox jumps over the lazy dog", 3);
        let fp2 = simhash("the quick brown fox jumps over the lazy dog", 3);
        assert_eq!(fp1.bits, fp2.bits);
    }

    #[test]
    fn test_simhash_similar() {
        let fp1 = simhash("the quick brown fox jumps over the lazy dog", 2);
        let fp2 = simhash("the quick brown fox leaps over the lazy dog", 2);
        let dist = hamming_distance(fp1.bits, fp2.bits);
        assert!(
            dist < 20,
            "similar texts should have low hamming distance, got {dist}"
        );
    }

    #[test]
    fn test_hamming_distance() {
        assert_eq!(hamming_distance(0b1010, 0b1001), 2);
        assert_eq!(hamming_distance(0, 0), 0);
    }

    #[test]
    fn test_simhash_similarity_fn() {
        let s = simhash_similarity(0, 0);
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_minhash_identical() {
        let opts = FingerprintOptions::default();
        let sig_a = minhash("the quick brown fox jumps", &opts);
        let sig_b = minhash("the quick brown fox jumps", &opts);
        let sim = minhash_similarity(&sig_a, &sig_b);
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_minhash_different() {
        let opts = FingerprintOptions::default();
        let sig_a = minhash("alpha beta gamma delta", &opts);
        let sig_b = minhash("one two three four five six", &opts);
        let sim = minhash_similarity(&sig_a, &sig_b);
        assert!(
            sim < 0.5,
            "disjoint texts should have low similarity, got {sim}"
        );
    }

    #[test]
    fn test_jaccard_identical() {
        let sim = jaccard_similarity("a b c d e", "a b c d e", 2);
        assert!((sim.score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaccard_disjoint() {
        let sim = jaccard_similarity("alpha beta gamma", "one two three four", 2);
        assert!((sim.score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rolling_hashes() {
        let data = b"abcdefgh";
        let hashes = rolling_hashes(data, 4);
        assert_eq!(hashes.len(), 5); // 8 - 4 + 1
    }

    #[test]
    fn test_rolling_hashes_short() {
        assert!(rolling_hashes(b"ab", 5).is_empty());
    }

    #[test]
    fn test_chunk_boundaries() {
        let data = b"a]xz8bq2c]xz8";
        let bounds = chunk_boundaries(data, 3, 0x1); // mask = low bit
                                                     // Some positions will match — just check it returns something valid
        for &b in &bounds {
            assert!(b <= data.len());
        }
    }

    #[test]
    fn test_find_duplicates() {
        let docs = vec![
            "the quick brown fox jumps over the lazy dog",
            "the quick brown fox jumps over the lazy cat",
            "completely different text about something else entirely",
        ];
        let opts = FingerprintOptions {
            shingle_size: 2,
            num_hashes: 50,
        };
        let groups = find_duplicates(&docs, &opts, 0.5);
        assert!(!groups.is_empty(), "first two docs should be grouped");
        assert!(groups[0].contains(&0));
        assert!(groups[0].contains(&1));
    }

    #[test]
    fn test_term_frequency() {
        let freq = term_frequency("the cat sat on the mat the");
        assert_eq!(freq["the"], 3);
        assert_eq!(freq["cat"], 1);
    }

    #[test]
    fn test_summary_report() {
        let docs = vec!["hello world", "hello world again"];
        let opts = FingerprintOptions::default();
        let report = summary_report(&docs, &opts);
        assert!(report.contains("Documents: 2"));
    }
}
