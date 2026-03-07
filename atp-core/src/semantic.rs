// Semantic search — TF-IDF based similarity search for text files.
//
// Provides a lightweight "semantic" search capability that goes beyond
// exact string/regex matching. Uses TF-IDF (Term Frequency × Inverse
// Document Frequency) cosine similarity to find lines or documents
// that are conceptually similar to a query, even if no exact match exists.
//
// This is not a neural embedding model — it uses bag-of-words statistics.
// For true semantic search with transformers, an external model server
// would be needed. This module provides useful similarity search with
// zero external dependencies.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A document (line or block of text) with its TF-IDF vector.
#[derive(Debug, Clone)]
pub struct ScoredDocument {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: usize,
    /// The text content.
    pub content: String,
    /// Cosine similarity score (0.0 to 1.0).
    pub score: f64,
}

/// TF-IDF index for a corpus of documents.
#[derive(Debug)]
pub struct TfIdfIndex {
    /// Term → document frequency (how many docs contain this term).
    doc_freq: HashMap<String, usize>,
    /// All documents with their term frequencies.
    documents: Vec<IndexedDoc>,
    /// Total number of documents.
    doc_count: usize,
    /// Stop words to exclude from indexing.
    stop_words: HashSet<String>,
}

#[derive(Debug, Clone)]
struct IndexedDoc {
    file: String,
    line: usize,
    content: String,
    term_freq: HashMap<String, f64>,
    magnitude: f64,
}

impl TfIdfIndex {
    /// Create a new empty TF-IDF index.
    pub fn new() -> Self {
        Self {
            doc_freq: HashMap::new(),
            documents: Vec::new(),
            doc_count: 0,
            stop_words: default_stop_words(),
        }
    }

    /// Add stop words to the index.
    pub fn with_stop_words(mut self, words: &[&str]) -> Self {
        for w in words {
            self.stop_words.insert(w.to_lowercase());
        }
        self
    }

    /// Tokenize text into normalized terms.
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .filter(|s| s.len() >= 2 && !self.stop_words.contains(s))
            .collect()
    }

    /// Compute term frequency vector for a token list.
    fn compute_tf(tokens: &[String]) -> HashMap<String, f64> {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for token in tokens {
            *freq.entry(token.clone()).or_insert(0) += 1;
        }
        let max_freq = freq.values().copied().max().unwrap_or(1) as f64;
        freq.into_iter()
            .map(|(term, count)| (term, count as f64 / max_freq))
            .collect()
    }

    /// Add a single document (line) to the index.
    pub fn add_document(&mut self, file: &str, line: usize, content: &str) {
        let tokens = self.tokenize(content);
        if tokens.is_empty() {
            return;
        }

        let tf = Self::compute_tf(&tokens);

        // Update document frequency
        let unique_terms: HashSet<_> = tokens.iter().cloned().collect();
        for term in &unique_terms {
            *self.doc_freq.entry(term.clone()).or_insert(0) += 1;
        }

        self.documents.push(IndexedDoc {
            file: file.to_string(),
            line,
            content: content.to_string(),
            term_freq: tf,
            magnitude: 0.0, // computed after all docs are added
        });
        self.doc_count += 1;
    }

    /// Index all lines from a file.
    pub fn index_file(&mut self, path: &Path) -> std::io::Result<usize> {
        let content = std::fs::read_to_string(path)?;
        let file_str = path.to_string_lossy().to_string();
        let mut count = 0;
        for (i, line) in content.lines().enumerate() {
            self.add_document(&file_str, i + 1, line);
            count += 1;
        }
        Ok(count)
    }

    /// Finalize the index — compute IDF weights and document magnitudes.
    /// Must be called after all documents are added and before querying.
    pub fn finalize(&mut self) {
        let n = self.doc_count as f64;
        if n == 0.0 {
            return;
        }

        for doc in &mut self.documents {
            let mut magnitude_sq = 0.0;
            for (term, tf) in &doc.term_freq {
                let df = self.doc_freq.get(term).copied().unwrap_or(1) as f64;
                let idf = (n / df).ln() + 1.0; // smoothed IDF
                let tfidf = tf * idf;
                magnitude_sq += tfidf * tfidf;
            }
            doc.magnitude = magnitude_sq.sqrt();
        }
    }

    /// Query the index for documents similar to the query string.
    ///
    /// Returns results sorted by descending similarity score.
    pub fn query(&self, query_text: &str, max_results: usize) -> Vec<ScoredDocument> {
        let tokens = self.tokenize(query_text);
        if tokens.is_empty() {
            return Vec::new();
        }

        let query_tf = Self::compute_tf(&tokens);
        let n = self.doc_count as f64;

        // Build query TF-IDF vector and compute its magnitude
        let mut query_tfidf: HashMap<&str, f64> = HashMap::new();
        let mut query_mag_sq = 0.0;
        for (term, tf) in &query_tf {
            let df = self.doc_freq.get(term).copied().unwrap_or(1) as f64;
            let idf = (n / df).ln() + 1.0;
            let tfidf = tf * idf;
            query_tfidf.insert(term, tfidf);
            query_mag_sq += tfidf * tfidf;
        }
        let query_mag = query_mag_sq.sqrt();

        if query_mag == 0.0 {
            return Vec::new();
        }

        // Compute cosine similarity for each document
        let mut results: Vec<ScoredDocument> = self
            .documents
            .iter()
            .filter_map(|doc| {
                if doc.magnitude == 0.0 {
                    return None;
                }

                let mut dot_product = 0.0;
                for (term, q_tfidf) in &query_tfidf {
                    if let Some(d_tf) = doc.term_freq.get(*term) {
                        let df = self.doc_freq.get(*term).copied().unwrap_or(1) as f64;
                        let idf = (n / df).ln() + 1.0;
                        let d_tfidf = d_tf * idf;
                        dot_product += q_tfidf * d_tfidf;
                    }
                }

                let score = dot_product / (query_mag * doc.magnitude);
                if score > 0.0 {
                    Some(ScoredDocument {
                        file: doc.file.clone(),
                        line: doc.line,
                        content: doc.content.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(max_results);
        results
    }

    /// Get the number of indexed documents.
    pub fn doc_count(&self) -> usize {
        self.doc_count
    }

    /// Get the number of unique terms.
    pub fn term_count(&self) -> usize {
        self.doc_freq.len()
    }
}

impl Default for TfIdfIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Default English stop words.
fn default_stop_words() -> HashSet<String> {
    [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can",
        "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by", "from",
        "as", "into", "through", "during", "before", "after", "above", "below", "between", "out",
        "off", "over", "under", "again", "further", "then", "once", "here", "there", "when",
        "where", "why", "how", "all", "both", "each", "few", "more", "most", "other", "some",
        "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "just",
        "because", "but", "and", "or", "if", "while", "about", "up", "it", "its", "he", "she",
        "we", "they", "this", "that", "these", "those", "am",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_index() {
        let index = TfIdfIndex::new();
        assert_eq!(index.doc_count(), 0);
        assert_eq!(index.term_count(), 0);
    }

    #[test]
    fn test_tokenize() {
        let index = TfIdfIndex::new();
        let tokens = index.tokenize("Hello, world! This is a test_func.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test_func".to_string()));
        // "a", "is", "this" are stop words and should be filtered
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn test_add_document() {
        let mut index = TfIdfIndex::new();
        index.add_document("test.rs", 1, "fn main() { println!(\"hello world\"); }");
        assert_eq!(index.doc_count(), 1);
        assert!(index.term_count() > 0);
    }

    #[test]
    fn test_empty_document_skipped() {
        let mut index = TfIdfIndex::new();
        index.add_document("test.rs", 1, ""); // empty
        index.add_document("test.rs", 2, "// "); // only stop words and short tokens
        assert_eq!(index.doc_count(), 0);
    }

    #[test]
    fn test_query_basic() {
        let mut index = TfIdfIndex::new();
        index.add_document("src/main.rs", 1, "fn main() { println!(\"hello world\"); }");
        index.add_document(
            "src/lib.rs",
            1,
            "pub fn search_files(pattern: &str) -> Vec<String>",
        );
        index.add_document(
            "src/lib.rs",
            2,
            "pub fn transform_text(input: &str) -> String",
        );
        index.add_document("src/utils.rs", 1, "fn parse_config(path: &Path) -> Config");
        index.add_document(
            "src/utils.rs",
            2,
            "fn validate_pattern(pattern: &str) -> bool",
        );
        index.finalize();

        let results = index.query("search pattern files", 10);
        assert!(!results.is_empty());
        // The line with "search_files" and "pattern" should score highest
        assert!(
            results[0].content.contains("search_files") || results[0].content.contains("pattern")
        );
    }

    #[test]
    fn test_query_no_match() {
        let mut index = TfIdfIndex::new();
        index.add_document("test.rs", 1, "fn hello_world()");
        index.finalize();

        let results = index.query("database connection pool", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_max_results() {
        let mut index = TfIdfIndex::new();
        for i in 0..100 {
            index.add_document(
                "test.rs",
                i + 1,
                &format!("function_{} handles error processing", i),
            );
        }
        index.finalize();

        let results = index.query("error processing function", 5);
        assert!(results.len() <= 5);
    }

    #[test]
    fn test_score_ordering() {
        let mut index = TfIdfIndex::new();
        index.add_document("a.rs", 1, "error handling and error recovery in rust");
        index.add_document("b.rs", 1, "database connection pooling");
        index.add_document("c.rs", 1, "error logging mechanism");
        index.finalize();

        let results = index.query("error handling", 10);
        assert!(results.len() >= 2);
        // First result should have higher score than second
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn test_cosine_similarity_range() {
        let mut index = TfIdfIndex::new();
        index.add_document("test.rs", 1, "pattern matching algorithm");
        index.add_document("test.rs", 2, "regular expression pattern");
        index.finalize();

        let results = index.query("pattern", 10);
        for r in &results {
            assert!(
                r.score >= 0.0 && r.score <= 1.0,
                "Score {} out of range",
                r.score
            );
        }
    }

    #[test]
    fn test_index_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line one\nline two\nline three\n").unwrap();

        let mut index = TfIdfIndex::new();
        let count = index.index_file(&file_path).unwrap();
        assert_eq!(count, 3);
        assert!(index.doc_count() > 0);
    }

    #[test]
    fn test_with_stop_words() {
        let index = TfIdfIndex::new().with_stop_words(&["rust", "cargo"]);
        let tokens = index.tokenize("rust cargo build release");
        assert!(!tokens.contains(&"rust".to_string()));
        assert!(!tokens.contains(&"cargo".to_string()));
        assert!(tokens.contains(&"build".to_string()));
        assert!(tokens.contains(&"release".to_string()));
    }

    #[test]
    fn test_default_impl() {
        let index = TfIdfIndex::default();
        assert_eq!(index.doc_count(), 0);
    }

    #[test]
    fn test_scored_document_fields() {
        let doc = ScoredDocument {
            file: "test.rs".into(),
            line: 42,
            content: "fn test()".into(),
            score: 0.85,
        };
        assert_eq!(doc.file, "test.rs");
        assert_eq!(doc.line, 42);
        assert_eq!(doc.score, 0.85);
    }
}
