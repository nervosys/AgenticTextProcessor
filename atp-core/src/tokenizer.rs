//! # Tokenizer — Configurable Text Tokenisation
//!
//! Word, sentence, and paragraph boundary detection, regex-based
//! splitting, and BPE-like subword tokenisation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Tokenisation granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Granularity {
    /// Unicode word boundaries (alphanumeric sequences).
    Word,
    /// Sentence boundaries (`.`, `!`, `?` followed by whitespace or end).
    Sentence,
    /// Paragraph boundaries (blank lines).
    Paragraph,
    /// Fixed byte-length chunks.
    FixedLen,
    /// Regex-based split.
    Regex,
}

/// A single token with provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// The token text.
    pub text: String,
    /// Byte offset of start in original input.
    pub start: usize,
    /// Byte offset of end (exclusive) in original input.
    pub end: usize,
    /// Token index (0-based).
    pub index: usize,
}

/// Options for tokenisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerOptions {
    /// Granularity.
    pub granularity: Granularity,
    /// Regex pattern (only used when granularity = Regex).
    pub pattern: String,
    /// Fixed chunk length (only used when granularity = FixedLen).
    pub chunk_len: usize,
    /// Whether to lowercase all tokens.
    pub lowercase: bool,
    /// Whether to strip punctuation from word tokens.
    pub strip_punct: bool,
    /// Minimum token length to keep (0 = keep all).
    pub min_len: usize,
}

impl Default for TokenizerOptions {
    fn default() -> Self {
        Self {
            granularity: Granularity::Word,
            pattern: String::new(),
            chunk_len: 64,
            lowercase: false,
            strip_punct: false,
            min_len: 0,
        }
    }
}

/// Vocabulary for BPE-like subword tokenisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vocabulary {
    pub tokens: Vec<String>,
    pub merges: Vec<(String, String)>,
}

/// Summary statistics for a tokenisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStats {
    pub total_tokens: usize,
    pub unique_tokens: usize,
    pub avg_len: f64,
    pub max_len: usize,
    pub min_len: usize,
}

// ---------------------------------------------------------------------------
// Core tokenisation
// ---------------------------------------------------------------------------

/// Tokenise text with the given options.
pub fn tokenize(input: &str, opts: &TokenizerOptions) -> Vec<Token> {
    let raw = match opts.granularity {
        Granularity::Word => tokenize_words(input),
        Granularity::Sentence => tokenize_sentences(input),
        Granularity::Paragraph => tokenize_paragraphs(input),
        Granularity::FixedLen => tokenize_fixed(input, opts.chunk_len),
        Granularity::Regex => tokenize_regex(input, &opts.pattern),
    };

    let mut result = Vec::new();
    for (idx, mut tok) in raw.into_iter().enumerate() {
        if opts.strip_punct && opts.granularity == Granularity::Word {
            tok.text = tok
                .text
                .chars()
                .filter(|c| !c.is_ascii_punctuation())
                .collect();
            if tok.text.is_empty() {
                continue;
            }
        }
        if opts.lowercase {
            tok.text = tok.text.to_lowercase();
        }
        if opts.min_len > 0 && tok.text.len() < opts.min_len {
            continue;
        }
        tok.index = idx;
        result.push(tok);
    }
    // Re-index after filtering
    for (i, tok) in result.iter_mut().enumerate() {
        tok.index = i;
    }
    result
}

pub fn tokenize_words(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (i, ch) in input.char_indices() {
        let is_word = ch.is_alphanumeric() || ch == '_' || ch == '\'';
        if is_word {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start {
            let end = i;
            tokens.push(Token {
                text: input[s..end].to_string(),
                start: s,
                end,
                index: 0,
            });
            start = None;
        }
    }
    if let Some(s) = start {
        tokens.push(Token {
            text: input[s..].to_string(),
            start: s,
            end: input.len(),
            index: 0,
        });
    }
    tokens
}

pub fn tokenize_sentences(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = input.chars().collect();
    let byte_offsets: Vec<usize> = input.char_indices().map(|(i, _)| i).collect();

    let mut i = 0;
    while i < chars.len() {
        if (chars[i] == '.' || chars[i] == '!' || chars[i] == '?')
            && (i + 1 >= chars.len() || chars[i + 1].is_whitespace())
        {
            let end_byte = if i + 1 < byte_offsets.len() {
                byte_offsets[i + 1]
            } else {
                input.len()
            };
            let text = input[start..end_byte].trim().to_string();
            if !text.is_empty() {
                tokens.push(Token {
                    text,
                    start,
                    end: end_byte,
                    index: 0,
                });
            }
            // Skip whitespace after sentence terminator
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            start = if i < byte_offsets.len() {
                byte_offsets[i]
            } else {
                input.len()
            };
            continue;
        }
        i += 1;
    }
    // Remaining text
    let rest = input[start..].trim();
    if !rest.is_empty() {
        tokens.push(Token {
            text: rest.to_string(),
            start,
            end: input.len(),
            index: 0,
        });
    }
    tokens
}

pub fn tokenize_paragraphs(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut para_start: Option<usize> = None;
    let mut offset = 0;

    for line in input.split('\n') {
        let line_end = offset + line.len();
        if line.trim().is_empty() {
            if let Some(ps) = para_start {
                let text = input[ps..offset.saturating_sub(1).max(ps)]
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    tokens.push(Token {
                        text,
                        start: ps,
                        end: offset,
                        index: 0,
                    });
                }
                para_start = None;
            }
        } else if para_start.is_none() {
            para_start = Some(offset);
        }
        offset = line_end + 1; // +1 for the \n
    }
    if let Some(ps) = para_start {
        let text = input[ps..].trim().to_string();
        if !text.is_empty() {
            tokens.push(Token {
                text,
                start: ps,
                end: input.len(),
                index: 0,
            });
        }
    }
    tokens
}

pub fn tokenize_fixed(input: &str, chunk_len: usize) -> Vec<Token> {
    if chunk_len == 0 {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let end = (i + chunk_len).min(input.len());
        // Avoid splitting in the middle of a multi-byte char
        let end = truncate_to_char_boundary(input, end);
        if end <= i {
            break;
        }
        tokens.push(Token {
            text: input[i..end].to_string(),
            start: i,
            end,
            index: 0,
        });
        i = end;
    }
    tokens
}

fn truncate_to_char_boundary(s: &str, mut max: usize) -> usize {
    while max > 0 && !s.is_char_boundary(max) {
        max -= 1;
    }
    max
}

pub fn tokenize_regex(input: &str, pattern: &str) -> Vec<Token> {
    if pattern.is_empty() {
        return tokenize_words(input);
    }
    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut tokens = Vec::new();
    let mut last = 0;
    for mat in re.find_iter(input) {
        if mat.start() > last {
            let text = input[last..mat.start()].to_string();
            if !text.trim().is_empty() {
                tokens.push(Token {
                    text,
                    start: last,
                    end: mat.start(),
                    index: 0,
                });
            }
        }
        last = mat.end();
    }
    if last < input.len() {
        let text = input[last..].to_string();
        if !text.trim().is_empty() {
            tokens.push(Token {
                text,
                start: last,
                end: input.len(),
                index: 0,
            });
        }
    }
    tokens
}

// ---------------------------------------------------------------------------
// BPE-like subword tokenisation
// ---------------------------------------------------------------------------

/// Learn a BPE vocabulary from a corpus by performing `num_merges` merges.
pub fn learn_bpe(corpus: &str, num_merges: usize) -> Vocabulary {
    let words: Vec<Vec<String>> = corpus
        .split_whitespace()
        .map(|w| w.chars().map(|c| c.to_string()).collect())
        .collect();

    let mut current: Vec<Vec<String>> = words;
    let mut merges = Vec::new();

    for _ in 0..num_merges {
        let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();
        for word in &current {
            for pair in word.windows(2) {
                *pair_counts
                    .entry((pair[0].clone(), pair[1].clone()))
                    .or_insert(0) += 1;
            }
        }
        if pair_counts.is_empty() {
            break;
        }
        let best = pair_counts
            .iter()
            .max_by_key(|(_, &v)| v)
            .map(|(k, _)| k.clone());
        let Some((a, b)) = best else { break };
        let merged = format!("{a}{b}");
        merges.push((a.clone(), b.clone()));
        // Apply merge
        current = current
            .into_iter()
            .map(|word| apply_merge(&word, &a, &b, &merged))
            .collect();
    }

    let mut vocab_set: HashMap<String, ()> = HashMap::new();
    for word in &current {
        for tok in word {
            vocab_set.entry(tok.clone()).or_insert(());
        }
    }
    let mut tokens: Vec<String> = vocab_set.into_keys().collect();
    tokens.sort();

    Vocabulary { tokens, merges }
}

fn apply_merge(word: &[String], a: &str, b: &str, merged: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < word.len() {
        if i + 1 < word.len() && word[i] == a && word[i + 1] == b {
            result.push(merged.to_string());
            i += 2;
        } else {
            result.push(word[i].clone());
            i += 1;
        }
    }
    result
}

/// Tokenise a string using a learned BPE vocabulary.
pub fn bpe_tokenize(input: &str, vocab: &Vocabulary) -> Vec<String> {
    let mut words: Vec<Vec<String>> = input
        .split_whitespace()
        .map(|w| w.chars().map(|c| c.to_string()).collect())
        .collect();

    for (a, b) in &vocab.merges {
        let merged = format!("{a}{b}");
        words = words
            .into_iter()
            .map(|w| apply_merge(&w, a, b, &merged))
            .collect();
    }

    words.into_iter().flatten().collect()
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Compute token statistics.
pub fn stats(tokens: &[Token]) -> TokenStats {
    if tokens.is_empty() {
        return TokenStats {
            total_tokens: 0,
            unique_tokens: 0,
            avg_len: 0.0,
            max_len: 0,
            min_len: 0,
        };
    }
    let mut unique: HashMap<&str, ()> = HashMap::new();
    let mut total_len = 0usize;
    let mut max_len = 0usize;
    let mut min_len = usize::MAX;
    for tok in tokens {
        unique.entry(&tok.text).or_insert(());
        total_len += tok.text.len();
        max_len = max_len.max(tok.text.len());
        min_len = min_len.min(tok.text.len());
    }
    TokenStats {
        total_tokens: tokens.len(),
        unique_tokens: unique.len(),
        avg_len: total_len as f64 / tokens.len() as f64,
        max_len,
        min_len,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_tokenize() {
        let opts = TokenizerOptions::default();
        let tokens = tokenize("Hello, world! This is a test.", &opts);
        let words: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        assert!(words.contains(&"Hello"));
        assert!(words.contains(&"world"));
        assert!(words.contains(&"test"));
    }

    #[test]
    fn test_word_lowercase() {
        let opts = TokenizerOptions {
            lowercase: true,
            ..Default::default()
        };
        let tokens = tokenize("Hello World", &opts);
        assert_eq!(tokens[0].text, "hello");
    }

    #[test]
    fn test_word_strip_punct() {
        let opts = TokenizerOptions {
            strip_punct: true,
            ..Default::default()
        };
        let tokens = tokenize("don't", &opts);
        assert_eq!(tokens[0].text, "dont");
    }

    #[test]
    fn test_word_min_len() {
        let opts = TokenizerOptions {
            min_len: 4,
            ..Default::default()
        };
        let tokens = tokenize("I am a test person", &opts);
        assert!(tokens.iter().all(|t| t.text.len() >= 4));
    }

    #[test]
    fn test_sentence_tokenize() {
        let opts = TokenizerOptions {
            granularity: Granularity::Sentence,
            ..Default::default()
        };
        let tokens = tokenize("Hello world. How are you? Fine!", &opts);
        assert_eq!(tokens.len(), 3);
        assert!(tokens[0].text.starts_with("Hello"));
        assert!(tokens[1].text.starts_with("How"));
    }

    #[test]
    fn test_paragraph_tokenize() {
        let opts = TokenizerOptions {
            granularity: Granularity::Paragraph,
            ..Default::default()
        };
        let tokens = tokenize("Para one.\n\nPara two.\n\nPara three.", &opts);
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn test_fixed_len_tokenize() {
        let opts = TokenizerOptions {
            granularity: Granularity::FixedLen,
            chunk_len: 5,
            ..Default::default()
        };
        let tokens = tokenize("abcdefghij", &opts);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "abcde");
        assert_eq!(tokens[1].text, "fghij");
    }

    #[test]
    fn test_regex_tokenize() {
        let opts = TokenizerOptions {
            granularity: Granularity::Regex,
            pattern: r",\s*".to_string(),
            ..Default::default()
        };
        let tokens = tokenize("a, b, c", &opts);
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn test_offsets() {
        let opts = TokenizerOptions::default();
        let tokens = tokenize("hello world", &opts);
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, 5);
        assert_eq!(tokens[1].start, 6);
    }

    #[test]
    fn test_bpe_learn() {
        let vocab = learn_bpe("low low low lowest newest", 5);
        assert!(!vocab.tokens.is_empty());
        assert!(!vocab.merges.is_empty());
    }

    #[test]
    fn test_bpe_tokenize() {
        let vocab = learn_bpe("ab ab ab cd cd", 3);
        let tokens = bpe_tokenize("ab cd", &vocab);
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_stats() {
        let opts = TokenizerOptions::default();
        let tokens = tokenize("hello world test", &opts);
        let s = stats(&tokens);
        assert_eq!(s.total_tokens, 3);
        assert_eq!(s.unique_tokens, 3);
        assert!(s.avg_len > 0.0);
    }

    #[test]
    fn test_empty_input() {
        let opts = TokenizerOptions::default();
        let tokens = tokenize("", &opts);
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_index_sequential() {
        let opts = TokenizerOptions::default();
        let tokens = tokenize("one two three four five", &opts);
        for (i, tok) in tokens.iter().enumerate() {
            assert_eq!(tok.index, i);
        }
    }

    #[test]
    fn test_stats_empty() {
        let s = stats(&[]);
        assert_eq!(s.total_tokens, 0);
    }
}
