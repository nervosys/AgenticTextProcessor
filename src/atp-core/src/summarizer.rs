// ---------------------------------------------------------------------------
// summarizer.rs — Extractive text summarization
// ---------------------------------------------------------------------------
//
// TF-IDF sentence scoring, TextRank graph-based ranking, lead-sentence
// heuristic, word/sentence budgets, keyword extraction.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A scored sentence from the original document.
#[derive(Debug, Clone)]
pub struct ScoredSentence {
    /// Original index (0-based) of the sentence.
    pub index: usize,
    /// The sentence text.
    pub text: String,
    /// Relevance score.
    pub score: f64,
}

/// Summary result.
#[derive(Debug, Clone)]
pub struct Summary {
    /// The selected sentences in document order.
    pub sentences: Vec<ScoredSentence>,
    /// Total word count of the summary.
    pub word_count: usize,
    /// Compression ratio (summary words / original words).
    pub compression: f64,
}

/// An extracted keyword with score.
#[derive(Debug, Clone)]
pub struct Keyword {
    /// The keyword.
    pub word: String,
    /// TF-IDF score.
    pub score: f64,
}

/// Summarization strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// TF-IDF based sentence scoring.
    TfIdf,
    /// TextRank (graph-based).
    TextRank,
    /// Lead sentences (first N).
    Lead,
}

/// Configuration for summarization.
#[derive(Debug, Clone)]
pub struct SummaryConfig {
    /// Strategy to use.
    pub strategy: Strategy,
    /// Maximum number of sentences in summary (0 = unlimited).
    pub max_sentences: usize,
    /// Maximum word count for summary (0 = unlimited).
    pub max_words: usize,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            strategy: Strategy::TfIdf,
            max_sentences: 5,
            max_words: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split text into sentences (simple heuristic).
pub fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if (b == b'.' || b == b'!' || b == b'?')
            && (i + 1 >= bytes.len() || bytes[i + 1] == b' ' || bytes[i + 1] == b'\n')
        {
            let s = text[start..=i].trim();
            if !s.is_empty() {
                sentences.push(s);
            }
            start = i + 1;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        sentences.push(tail);
    }
    sentences
}

/// Tokenize into lowercase words.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Simple stop-word check (top ~30 English stop words).
fn is_stop_word(w: &str) -> bool {
    matches!(
        w,
        "the"
            | "a"
            | "an"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "have"
            | "has"
            | "had"
            | "do"
            | "does"
            | "did"
            | "will"
            | "would"
            | "shall"
            | "should"
            | "may"
            | "might"
            | "can"
            | "could"
            | "and"
            | "or"
            | "but"
            | "in"
            | "on"
            | "at"
            | "to"
            | "for"
            | "of"
            | "with"
            | "by"
            | "from"
            | "it"
            | "this"
            | "that"
            | "not"
            | "no"
            | "if"
            | "so"
            | "as"
    )
}

// ---------------------------------------------------------------------------
// TF-IDF
// ---------------------------------------------------------------------------

/// Compute term frequency for a list of words.
pub fn term_frequency(words: &[String]) -> HashMap<String, f64> {
    let mut tf: HashMap<String, usize> = HashMap::new();
    for w in words {
        if !is_stop_word(w) {
            *tf.entry(w.clone()).or_default() += 1;
        }
    }
    let total = words.len().max(1) as f64;
    tf.into_iter().map(|(k, v)| (k, v as f64 / total)).collect()
}

/// Compute inverse document frequency across a set of sentence word lists.
pub fn inverse_document_frequency(docs: &[Vec<String>]) -> HashMap<String, f64> {
    let n = docs.len().max(1) as f64;
    let mut df: HashMap<String, usize> = HashMap::new();
    for doc in docs {
        let unique: std::collections::HashSet<&String> = doc.iter().collect();
        for w in unique {
            if !is_stop_word(w) {
                *df.entry(w.clone()).or_default() += 1;
            }
        }
    }
    df.into_iter()
        .map(|(k, v)| (k, (n / v as f64).ln() + 1.0))
        .collect()
}

/// Score sentences by TF-IDF.
pub fn tfidf_score(text: &str) -> Vec<ScoredSentence> {
    let sents = split_sentences(text);
    let doc_words: Vec<Vec<String>> = sents.iter().map(|s| words(s)).collect();
    let idf = inverse_document_frequency(&doc_words);
    sents
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let ws = words(s);
            let tf = term_frequency(&ws);
            let score: f64 = tf
                .iter()
                .map(|(w, tf_val)| tf_val * idf.get(w).unwrap_or(&1.0))
                .sum();
            ScoredSentence {
                index: i,
                text: s.to_string(),
                score,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// TextRank
// ---------------------------------------------------------------------------

/// Cosine similarity between two word frequency vectors.
fn cosine_similarity(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for (k, v) in a {
        norm_a += v * v;
        if let Some(bv) = b.get(k) {
            dot += v * bv;
        }
    }
    for v in b.values() {
        norm_b += v * v;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        dot / denom
    }
}

/// TextRank-style sentence scoring.
///
/// Builds a similarity graph between sentences and iterates PageRank.
pub fn textrank_score(text: &str, iterations: usize, damping: f64) -> Vec<ScoredSentence> {
    let sents = split_sentences(text);
    let n = sents.len();
    if n == 0 {
        return Vec::new();
    }
    let tfs: Vec<HashMap<String, f64>> = sents.iter().map(|s| term_frequency(&words(s))).collect();

    // Build adjacency matrix
    let mut weights = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let sim = cosine_similarity(&tfs[i], &tfs[j]);
            weights[i][j] = sim;
            weights[j][i] = sim;
        }
    }

    // Row-normalize
    for row in &mut weights {
        let sum: f64 = row.iter().sum();
        if sum > 1e-12 {
            for v in row.iter_mut() {
                *v /= sum;
            }
        }
    }

    // Iterate
    let mut scores = vec![1.0 / n as f64; n];
    for _ in 0..iterations {
        let mut new_scores = vec![0.0f64; n];
        for i in 0..n {
            let mut s = 0.0;
            for j in 0..n {
                s += weights[j][i] * scores[j];
            }
            new_scores[i] = (1.0 - damping) / n as f64 + damping * s;
        }
        scores = new_scores;
    }

    sents
        .iter()
        .enumerate()
        .map(|(i, &s)| ScoredSentence {
            index: i,
            text: s.to_string(),
            score: scores[i],
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Summarize
// ---------------------------------------------------------------------------

/// Produce an extractive summary of `text`.
pub fn summarize(text: &str, config: &SummaryConfig) -> Summary {
    let total_words = words(text).len();
    let mut scored = match config.strategy {
        Strategy::TfIdf => tfidf_score(text),
        Strategy::TextRank => textrank_score(text, 20, 0.85),
        Strategy::Lead => {
            let sents = split_sentences(text);
            sents
                .into_iter()
                .enumerate()
                .map(|(i, s)| ScoredSentence {
                    index: i,
                    text: s.to_string(),
                    score: 1.0 / (i as f64 + 1.0),
                })
                .collect()
        }
    };

    // Sort by score descending
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Apply sentence limit
    if config.max_sentences > 0 && scored.len() > config.max_sentences {
        scored.truncate(config.max_sentences);
    }

    // Apply word budget
    if config.max_words > 0 {
        let mut budget = config.max_words;
        let mut kept = Vec::new();
        for s in &scored {
            let wc = words(&s.text).len();
            if wc <= budget {
                kept.push(s.clone());
                budget -= wc;
            }
        }
        scored = kept;
    }

    // Re-sort by document order
    scored.sort_by_key(|s| s.index);

    let wc: usize = scored.iter().map(|s| words(&s.text).len()).sum();
    let compression = if total_words > 0 {
        wc as f64 / total_words as f64
    } else {
        0.0
    };

    Summary {
        sentences: scored,
        word_count: wc,
        compression,
    }
}

/// Summarize with default config (TF-IDF, max 5 sentences).
pub fn summarize_default(text: &str) -> Summary {
    summarize(text, &SummaryConfig::default())
}

// ---------------------------------------------------------------------------
// Keyword extraction
// ---------------------------------------------------------------------------

/// Extract top-N keywords by TF-IDF score across the whole document.
pub fn extract_keywords(text: &str, top_n: usize) -> Vec<Keyword> {
    let ws = words(text);
    let tf = term_frequency(&ws);
    let sents = split_sentences(text);
    let doc_words: Vec<Vec<String>> = sents.iter().map(|s| words(s)).collect();
    let idf = inverse_document_frequency(&doc_words);

    let mut keywords: Vec<Keyword> = tf
        .iter()
        .map(|(w, tf_val)| Keyword {
            word: w.clone(),
            score: tf_val * idf.get(w).unwrap_or(&1.0),
        })
        .collect();
    keywords.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    keywords.truncate(top_n);
    keywords
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Rust is a systems programming language. It focuses on safety and performance. The borrow checker ensures memory safety. Rust has a growing community. Many developers love Rust for its reliability.";

    #[test]
    fn test_split_sentences() {
        let s = split_sentences(SAMPLE);
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn test_split_sentences_empty() {
        let s = split_sentences("");
        assert!(s.is_empty());
    }

    #[test]
    fn test_term_frequency() {
        let w = words("hello world hello");
        let tf = term_frequency(&w);
        assert!(tf.get("hello").unwrap() > tf.get("world").unwrap());
    }

    #[test]
    fn test_tfidf_score_count() {
        let scored = tfidf_score(SAMPLE);
        assert_eq!(scored.len(), 5);
    }

    #[test]
    fn test_textrank_score_count() {
        let scored = textrank_score(SAMPLE, 10, 0.85);
        assert_eq!(scored.len(), 5);
    }

    #[test]
    fn test_textrank_positive_scores() {
        let scored = textrank_score(SAMPLE, 10, 0.85);
        for s in &scored {
            assert!(s.score > 0.0);
        }
    }

    #[test]
    fn test_summarize_tfidf() {
        let cfg = SummaryConfig {
            strategy: Strategy::TfIdf,
            max_sentences: 2,
            max_words: 0,
        };
        let s = summarize(SAMPLE, &cfg);
        assert_eq!(s.sentences.len(), 2);
        assert!(s.compression > 0.0 && s.compression < 1.0);
    }

    #[test]
    fn test_summarize_textrank() {
        let cfg = SummaryConfig {
            strategy: Strategy::TextRank,
            max_sentences: 3,
            max_words: 0,
        };
        let s = summarize(SAMPLE, &cfg);
        assert!(s.sentences.len() <= 3);
    }

    #[test]
    fn test_summarize_lead() {
        let cfg = SummaryConfig {
            strategy: Strategy::Lead,
            max_sentences: 2,
            max_words: 0,
        };
        let s = summarize(SAMPLE, &cfg);
        assert_eq!(s.sentences.len(), 2);
        assert_eq!(s.sentences[0].index, 0);
    }

    #[test]
    fn test_summarize_word_budget() {
        let cfg = SummaryConfig {
            strategy: Strategy::TfIdf,
            max_sentences: 10,
            max_words: 10,
        };
        let s = summarize(SAMPLE, &cfg);
        assert!(s.word_count <= 10);
    }

    #[test]
    fn test_summarize_default() {
        let s = summarize_default(SAMPLE);
        assert!(!s.sentences.is_empty());
    }

    #[test]
    fn test_summarize_empty() {
        let s = summarize_default("");
        assert!(s.sentences.is_empty());
    }

    #[test]
    fn test_extract_keywords() {
        let kw = extract_keywords(SAMPLE, 3);
        assert_eq!(kw.len(), 3);
        assert!(kw[0].score >= kw[1].score);
    }

    #[test]
    fn test_keywords_contain_rust() {
        let kw = extract_keywords(SAMPLE, 5);
        assert!(kw.iter().any(|k| k.word == "rust"));
    }

    #[test]
    fn test_document_order_preserved() {
        let cfg = SummaryConfig {
            strategy: Strategy::TfIdf,
            max_sentences: 3,
            max_words: 0,
        };
        let s = summarize(SAMPLE, &cfg);
        for w in s.sentences.windows(2) {
            assert!(w[0].index < w[1].index);
        }
    }

    #[test]
    fn test_compression_ratio() {
        let s = summarize_default(SAMPLE);
        assert!(s.compression >= 0.0 && s.compression <= 1.0);
    }
}
