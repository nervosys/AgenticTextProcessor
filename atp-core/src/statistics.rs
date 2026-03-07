//! # Text Statistics Engine
//!
//! Character, word, sentence and paragraph counts, word-frequency analysis,
//! n-gram extraction, Shannon entropy, Flesch-Kincaid readability,
//! and Zipf distribution analysis.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Complete statistics for a body of text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStats {
    pub chars: usize,
    pub chars_no_spaces: usize,
    pub words: usize,
    pub unique_words: usize,
    pub sentences: usize,
    pub paragraphs: usize,
    pub lines: usize,
    pub avg_word_length: f64,
    pub avg_sentence_length: f64,
    pub entropy: f64,
    pub readability: Readability,
}

/// Readability scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Readability {
    pub flesch_reading_ease: f64,
    pub flesch_kincaid_grade: f64,
    pub automated_readability: f64,
}

/// A word-frequency entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordFreq {
    pub word: String,
    pub count: usize,
    pub frequency: f64,
}

/// An n-gram entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ngram {
    pub tokens: Vec<String>,
    pub count: usize,
}

/// Zipf analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipfAnalysis {
    pub entries: Vec<ZipfEntry>,
    pub r_squared: f64,
}

/// Single entry in Zipf table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipfEntry {
    pub rank: usize,
    pub word: String,
    pub count: usize,
    pub expected_freq: f64,
    pub actual_freq: f64,
}

/// Character class distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharDistribution {
    pub alphabetic: usize,
    pub numeric: usize,
    pub whitespace: usize,
    pub punctuation: usize,
    pub other: usize,
}

// ---------------------------------------------------------------------------
// Core analysis
// ---------------------------------------------------------------------------

/// Compute full statistics for `text`.
pub fn analyze(text: &str) -> TextStats {
    let chars = text.chars().count();
    let chars_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
    let words_vec = extract_words(text);
    let words = words_vec.len();
    let unique_words = {
        let mut uniq: Vec<String> = words_vec.iter().map(|w| w.to_lowercase()).collect();
        uniq.sort();
        uniq.dedup();
        uniq.len()
    };
    let sentences = count_sentences(text);
    let paragraphs = count_paragraphs(text);
    let lines = if text.is_empty() {
        0
    } else {
        text.lines().count()
    };

    let avg_word_length = if words > 0 {
        words_vec.iter().map(|w| w.len()).sum::<usize>() as f64 / words as f64
    } else {
        0.0
    };
    let avg_sentence_length = if sentences > 0 {
        words as f64 / sentences as f64
    } else {
        0.0
    };

    let entropy = shannon_entropy(text);
    let readability = compute_readability(&words_vec, sentences);

    TextStats {
        chars,
        chars_no_spaces,
        words,
        unique_words,
        sentences,
        paragraphs,
        lines,
        avg_word_length,
        avg_sentence_length,
        entropy,
        readability,
    }
}

/// Extract words (alphabetic sequences).
pub fn extract_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' || ch == '-' {
            current.push(ch);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Count sentences (terminated by `.`, `!`, `?`).
pub fn count_sentences(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut prev_end = false;
    for ch in text.chars() {
        if ch == '.' || ch == '!' || ch == '?' {
            if !prev_end {
                count += 1;
            }
            prev_end = true;
        } else if !ch.is_whitespace() {
            prev_end = false;
        }
    }
    // If there's text but no sentence-ender, count as one sentence
    if count == 0 && !text.trim().is_empty() {
        count = 1;
    }
    count
}

/// Count paragraphs (separated by blank lines).
pub fn count_paragraphs(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut in_para = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            if in_para {
                in_para = false;
            }
        } else if !in_para {
            in_para = true;
            count += 1;
        }
    }
    count
}

/// Shannon entropy (bits per character).
pub fn shannon_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let mut freq: BTreeMap<char, usize> = BTreeMap::new();
    let mut total = 0usize;
    for ch in text.chars() {
        *freq.entry(ch).or_insert(0) += 1;
        total += 1;
    }
    let mut entropy = 0.0;
    for &count in freq.values() {
        let p = count as f64 / total as f64;
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Count syllables in a word (English approximation).
fn count_syllables(word: &str) -> usize {
    let w = word.to_lowercase();
    if w.len() <= 3 {
        return 1;
    }
    let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
    let chars: Vec<char> = w.chars().collect();
    let mut count = 0;
    let mut prev_vowel = false;
    for (i, &ch) in chars.iter().enumerate() {
        let is_vowel = vowels.contains(&ch);
        if is_vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_vowel;
        // Silent e at end
        if i == chars.len() - 1 && ch == 'e' && count > 1 {
            count -= 1;
        }
    }
    if count == 0 {
        1
    } else {
        count
    }
}

fn compute_readability(words: &[String], sentences: usize) -> Readability {
    let word_count = words.len() as f64;
    let sentence_count = if sentences > 0 { sentences as f64 } else { 1.0 };
    let total_syllables: usize = words.iter().map(|w| count_syllables(w)).sum();
    let syllable_count = total_syllables as f64;

    let asl = word_count / sentence_count; // avg sentence length
    let asw = if word_count > 0.0 {
        syllable_count / word_count // avg syllables per word
    } else {
        0.0
    };

    let flesch_reading_ease = 206.835 - (1.015 * asl) - (84.6 * asw);
    let flesch_kincaid_grade = (0.39 * asl) + (11.8 * asw) - 15.59;

    let char_count: usize = words.iter().map(|w| w.len()).sum();
    let automated_readability = if word_count > 0.0 {
        (4.71 * (char_count as f64 / word_count)) + (0.5 * asl) - 21.43
    } else {
        0.0
    };

    Readability {
        flesch_reading_ease,
        flesch_kincaid_grade,
        automated_readability,
    }
}

// ---------------------------------------------------------------------------
// Word frequency
// ---------------------------------------------------------------------------

/// Top-N most frequent words (case-insensitive).
pub fn word_frequency(text: &str, top_n: usize) -> Vec<WordFreq> {
    let words = extract_words(text);
    let total = words.len();
    if total == 0 {
        return vec![];
    }
    let mut freq: BTreeMap<String, usize> = BTreeMap::new();
    for w in &words {
        *freq.entry(w.to_lowercase()).or_insert(0) += 1;
    }
    let mut entries: Vec<WordFreq> = freq
        .into_iter()
        .map(|(word, count)| WordFreq {
            word,
            count,
            frequency: count as f64 / total as f64,
        })
        .collect();
    entries.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));
    entries.truncate(top_n);
    entries
}

/// N-gram extraction.
pub fn ngrams(text: &str, n: usize) -> Vec<Ngram> {
    let words = extract_words(text);
    if words.len() < n || n == 0 {
        return vec![];
    }
    let mut freq: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    for window in words.windows(n) {
        let key: Vec<String> = window.iter().map(|w| w.to_lowercase()).collect();
        *freq.entry(key).or_insert(0) += 1;
    }
    let mut result: Vec<Ngram> = freq
        .into_iter()
        .map(|(tokens, count)| Ngram { tokens, count })
        .collect();
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result
}

// ---------------------------------------------------------------------------
// Zipf analysis
// ---------------------------------------------------------------------------

/// Zipf distribution analysis.
pub fn zipf_analysis(text: &str, top_n: usize) -> ZipfAnalysis {
    let freq = word_frequency(text, top_n);
    if freq.is_empty() {
        return ZipfAnalysis {
            entries: vec![],
            r_squared: 0.0,
        };
    }
    let max_count = freq[0].count as f64;
    let entries: Vec<ZipfEntry> = freq
        .iter()
        .enumerate()
        .map(|(i, wf)| {
            let rank = i + 1;
            ZipfEntry {
                rank,
                word: wf.word.clone(),
                count: wf.count,
                expected_freq: 1.0 / rank as f64,
                actual_freq: wf.count as f64 / max_count,
            }
        })
        .collect();

    // R² of log-log fit
    let n = entries.len() as f64;
    let mean_log_rank = entries.iter().map(|e| (e.rank as f64).ln()).sum::<f64>() / n;
    let mean_log_freq = entries.iter().map(|e| e.actual_freq.ln()).sum::<f64>() / n;
    let ss_tot: f64 = entries
        .iter()
        .map(|e| {
            let d = e.actual_freq.ln() - mean_log_freq;
            d * d
        })
        .sum();
    let ss_res: f64 = entries
        .iter()
        .map(|e| {
            let predicted = mean_log_freq - ((e.rank as f64).ln() - mean_log_rank); // Zipf: slope ≈ -1
            let d = e.actual_freq.ln() - predicted;
            d * d
        })
        .sum();
    let r_squared = if ss_tot > 0.0 {
        1.0 - (ss_res / ss_tot)
    } else {
        0.0
    };

    ZipfAnalysis { entries, r_squared }
}

// ---------------------------------------------------------------------------
// Character distribution
// ---------------------------------------------------------------------------

/// Character class distribution.
pub fn char_distribution(text: &str) -> CharDistribution {
    let mut dist = CharDistribution {
        alphabetic: 0,
        numeric: 0,
        whitespace: 0,
        punctuation: 0,
        other: 0,
    };
    for ch in text.chars() {
        if ch.is_alphabetic() {
            dist.alphabetic += 1;
        } else if ch.is_numeric() {
            dist.numeric += 1;
        } else if ch.is_whitespace() {
            dist.whitespace += 1;
        } else if ch.is_ascii_punctuation() {
            dist.punctuation += 1;
        } else {
            dist.other += 1;
        }
    }
    dist
}

/// Render a simple text report.
pub fn format_report(stats: &TextStats) -> String {
    let mut out = String::new();
    out.push_str("=== Text Statistics ===\n");
    out.push_str(&format!("Characters:          {}\n", stats.chars));
    out.push_str(&format!("Characters (no ws):  {}\n", stats.chars_no_spaces));
    out.push_str(&format!("Words:               {}\n", stats.words));
    out.push_str(&format!("Unique words:        {}\n", stats.unique_words));
    out.push_str(&format!("Sentences:           {}\n", stats.sentences));
    out.push_str(&format!("Paragraphs:          {}\n", stats.paragraphs));
    out.push_str(&format!("Lines:               {}\n", stats.lines));
    out.push_str(&format!(
        "Avg word length:     {:.1}\n",
        stats.avg_word_length
    ));
    out.push_str(&format!(
        "Avg sentence length: {:.1} words\n",
        stats.avg_sentence_length
    ));
    out.push_str(&format!("Shannon entropy:     {:.2} bits\n", stats.entropy));
    out.push_str(&format!(
        "Flesch reading ease: {:.1}\n",
        stats.readability.flesch_reading_ease
    ));
    out.push_str(&format!(
        "Flesch-Kincaid grade: {:.1}\n",
        stats.readability.flesch_kincaid_grade
    ));
    out.push_str(&format!(
        "Automated readability: {:.1}\n",
        stats.readability.automated_readability
    ));
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "The quick brown fox jumps over the lazy dog. \
                           The dog barked loudly. The fox ran away!";

    #[test]
    fn test_analyze_basic() {
        let stats = analyze(SAMPLE);
        assert!(stats.words > 0);
        assert!(stats.sentences >= 3);
        assert!(stats.chars > 0);
        assert!(stats.paragraphs >= 1);
    }

    #[test]
    fn test_word_count() {
        let words = extract_words("hello world foo");
        assert_eq!(words.len(), 3);
    }

    #[test]
    fn test_sentence_count() {
        assert_eq!(count_sentences("Hello. World! Foo?"), 3);
        assert_eq!(count_sentences("No ending"), 1);
        assert_eq!(count_sentences(""), 0);
    }

    #[test]
    fn test_paragraph_count() {
        let text = "First para.\n\nSecond para.\n\nThird para.";
        assert_eq!(count_paragraphs(text), 3);
    }

    #[test]
    fn test_entropy() {
        assert_eq!(shannon_entropy(""), 0.0);
        let e = shannon_entropy("aaaa");
        assert!((e - 0.0).abs() < 0.001);
        let e2 = shannon_entropy("abcd");
        assert!(e2 > 1.5); // ~2.0 bits for 4 equally probable chars
    }

    #[test]
    fn test_word_frequency() {
        let freq = word_frequency("the cat and the dog and the fish", 3);
        assert_eq!(freq[0].word, "the");
        assert_eq!(freq[0].count, 3);
        assert_eq!(freq[1].word, "and");
        assert_eq!(freq[1].count, 2);
    }

    #[test]
    fn test_ngrams_bigrams() {
        let bgs = ngrams("a b c a b", 2);
        // "a b" appears twice
        assert!(bgs
            .iter()
            .any(|ng| ng.tokens == vec!["a", "b"] && ng.count == 2));
    }

    #[test]
    fn test_ngrams_trigrams() {
        let tgs = ngrams("one two three one two three", 3);
        assert!(tgs
            .iter()
            .any(|ng| ng.tokens == vec!["one", "two", "three"] && ng.count == 2));
    }

    #[test]
    fn test_zipf_analysis() {
        let text = "a a a a b b b c c d";
        let z = zipf_analysis(text, 4);
        assert_eq!(z.entries.len(), 4);
        assert_eq!(z.entries[0].word, "a");
        assert_eq!(z.entries[0].rank, 1);
    }

    #[test]
    fn test_char_distribution() {
        let dist = char_distribution("Hello 123!!");
        assert_eq!(dist.alphabetic, 5);
        assert_eq!(dist.numeric, 3);
        assert_eq!(dist.punctuation, 2);
        assert_eq!(dist.whitespace, 1);
    }

    #[test]
    fn test_readability() {
        let stats = analyze(SAMPLE);
        // Flesch reading ease: simple text should score > 50
        assert!(stats.readability.flesch_reading_ease > 0.0);
    }

    #[test]
    fn test_syllable_count() {
        assert_eq!(count_syllables("the"), 1);
        assert_eq!(count_syllables("beautiful"), 3);
        assert_eq!(count_syllables("I"), 1);
    }

    #[test]
    fn test_format_report() {
        let stats = analyze(SAMPLE);
        let report = format_report(&stats);
        assert!(report.contains("Words:"));
        assert!(report.contains("Shannon entropy:"));
    }

    #[test]
    fn test_empty_text() {
        let stats = analyze("");
        assert_eq!(stats.chars, 0);
        assert_eq!(stats.words, 0);
        assert_eq!(stats.sentences, 0);
        assert_eq!(stats.paragraphs, 0);
    }

    #[test]
    fn test_avg_word_length() {
        let stats = analyze("aa bbb cccc");
        // words: "aa"(2), "bbb"(3), "cccc"(4) → avg 3.0
        assert!((stats.avg_word_length - 3.0).abs() < 0.01);
    }
}
