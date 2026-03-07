//! # Spellcheck — Spell Checking & Suggestions
//!
//! Levenshtein / Damerau-Levenshtein edit distance, dictionary lookup,
//! phonetic matching (Soundex), and ranked correction suggestions.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A spelling diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellIssue {
    /// 0-based word index.
    pub index: usize,
    /// The misspelt word.
    pub word: String,
    /// Ranked suggestions (best first).
    pub suggestions: Vec<Suggestion>,
    /// Byte offset in original text.
    pub offset: usize,
}

/// A correction suggestion with its score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub word: String,
    pub distance: usize,
    pub phonetic_match: bool,
}

/// Spell-check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellResult {
    pub total_words: usize,
    pub errors: usize,
    pub issues: Vec<SpellIssue>,
}

/// Options for spell checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellOptions {
    /// Max edit distance for suggestions.
    pub max_distance: usize,
    /// Max suggestions per word.
    pub max_suggestions: usize,
    /// Include phonetic matching.
    pub phonetic: bool,
    /// Custom extra words to accept.
    pub extra_words: Vec<String>,
}

impl Default for SpellOptions {
    fn default() -> Self {
        Self {
            max_distance: 2,
            max_suggestions: 5,
            phonetic: true,
            extra_words: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in dictionary (common English words)
// ---------------------------------------------------------------------------

fn builtin_dict() -> HashSet<&'static str> {
    let words = [
        "a",
        "about",
        "above",
        "after",
        "again",
        "against",
        "all",
        "am",
        "an",
        "and",
        "any",
        "are",
        "as",
        "at",
        "be",
        "because",
        "been",
        "before",
        "being",
        "below",
        "between",
        "both",
        "but",
        "by",
        "can",
        "could",
        "did",
        "do",
        "does",
        "doing",
        "down",
        "during",
        "each",
        "few",
        "for",
        "from",
        "further",
        "get",
        "got",
        "had",
        "has",
        "have",
        "having",
        "he",
        "her",
        "here",
        "hers",
        "herself",
        "him",
        "himself",
        "his",
        "how",
        "i",
        "if",
        "in",
        "into",
        "is",
        "it",
        "its",
        "itself",
        "just",
        "know",
        "let",
        "like",
        "make",
        "me",
        "might",
        "more",
        "most",
        "must",
        "my",
        "myself",
        "no",
        "nor",
        "not",
        "now",
        "of",
        "off",
        "on",
        "once",
        "only",
        "or",
        "other",
        "our",
        "ours",
        "ourselves",
        "out",
        "over",
        "own",
        "same",
        "she",
        "should",
        "so",
        "some",
        "such",
        "than",
        "that",
        "the",
        "their",
        "theirs",
        "them",
        "themselves",
        "then",
        "there",
        "these",
        "they",
        "this",
        "those",
        "through",
        "to",
        "too",
        "under",
        "until",
        "up",
        "very",
        "was",
        "we",
        "were",
        "what",
        "when",
        "where",
        "which",
        "while",
        "who",
        "whom",
        "why",
        "will",
        "with",
        "would",
        "you",
        "your",
        "yours",
        "yourself",
        "yourselves",
        // Common extra words
        "hello",
        "world",
        "test",
        "code",
        "data",
        "file",
        "name",
        "text",
        "line",
        "word",
        "error",
        "check",
        "result",
        "input",
        "output",
        "value",
        "type",
        "function",
        "program",
        "system",
        "time",
        "day",
        "year",
        "way",
        "part",
        "place",
        "case",
        "number",
        "group",
        "good",
        "great",
        "new",
        "old",
        "first",
        "last",
        "long",
        "little",
        "big",
        "small",
        "right",
        "left",
        "high",
        "low",
        "next",
        "early",
        "young",
        "important",
        "public",
        "bad",
        "different",
        "another",
        "able",
        "also",
        "back",
        "much",
        "still",
        "even",
        "many",
        "well",
        "thing",
        "man",
        "woman",
        "child",
        "hand",
        "work",
        "life",
        "country",
        "area",
        "water",
        "point",
        "home",
        "school",
        "play",
        "keep",
        "need",
        "start",
        "try",
        "help",
        "show",
        "hear",
        "turn",
        "ask",
        "use",
        "find",
        "give",
        "tell",
        "say",
        "take",
        "come",
        "see",
        "look",
        "want",
        "think",
        "go",
        "run",
        "move",
        "live",
        "believe",
        "feel",
        "read",
        "write",
        "learn",
        "change",
        "follow",
        "stop",
        "open",
        "close",
        "begin",
        "seem",
        "talk",
        "love",
        "walk",
        "grow",
        "wait",
        "plan",
        "eat",
        "sit",
        "stand",
        "sleep",
        "remember",
        "buy",
        "pay",
        "meet",
        "include",
        "continue",
        "set",
        "end",
        "call",
        "head",
        "car",
        "city",
        "tree",
        "red",
        "green",
        "blue",
        "white",
        "black",
        "yellow",
        "spell",
        "check",
        "correct",
        "wrong",
        "right",
        "fix",
        "change",
    ];
    words.iter().copied().collect()
}

// ---------------------------------------------------------------------------
// Edit distance
// ---------------------------------------------------------------------------

/// Levenshtein edit distance.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

/// Damerau-Levenshtein edit distance (includes transpositions).
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                dp[i][j] = dp[i][j].min(dp[i - 2][j - 2] + cost);
            }
        }
    }
    dp[m][n]
}

// ---------------------------------------------------------------------------
// Soundex (phonetic)
// ---------------------------------------------------------------------------

/// Compute American Soundex code for a word.
pub fn soundex(word: &str) -> String {
    let chars: Vec<char> = word
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if chars.is_empty() {
        return "0000".to_string();
    }
    let mut code = String::new();
    code.push(chars[0]);
    let digit = |c: char| -> char {
        match c {
            'B' | 'F' | 'P' | 'V' => '1',
            'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
            'D' | 'T' => '3',
            'L' => '4',
            'M' | 'N' => '5',
            'R' => '6',
            _ => '0', // A, E, I, O, U, H, W, Y
        }
    };
    let mut last = digit(chars[0]);
    for &ch in &chars[1..] {
        let d = digit(ch);
        if d != '0' && d != last {
            code.push(d);
            if code.len() == 4 {
                break;
            }
        }
        last = d;
    }
    while code.len() < 4 {
        code.push('0');
    }
    code
}

/// Check if two words sound alike (same Soundex code).
pub fn sounds_alike(a: &str, b: &str) -> bool {
    soundex(a) == soundex(b)
}

// ---------------------------------------------------------------------------
// Dictionary & suggestions
// ---------------------------------------------------------------------------

/// Build a dictionary from a word list.
pub fn build_dictionary(words: &[&str]) -> HashSet<String> {
    words.iter().map(|w| w.to_lowercase()).collect()
}

/// Check if a word is in the dictionary.
pub fn is_known(word: &str, dict: &HashSet<String>) -> bool {
    dict.contains(&word.to_lowercase())
}

/// Generate ranked suggestions for a misspelt word.
pub fn suggest(word: &str, dict: &HashSet<String>, opts: &SpellOptions) -> Vec<Suggestion> {
    let lower = word.to_lowercase();
    let word_soundex = soundex(&lower);
    let mut candidates: Vec<Suggestion> = Vec::new();

    for entry in dict {
        let dist = damerau_levenshtein(&lower, entry);
        if dist <= opts.max_distance {
            let phonetic = opts.phonetic && soundex(entry) == word_soundex;
            candidates.push(Suggestion {
                word: entry.clone(),
                distance: dist,
                phonetic_match: phonetic,
            });
        }
    }

    // Sort: phonetic matches first, then by distance, then alphabetically
    candidates.sort_by(|a, b| {
        b.phonetic_match
            .cmp(&a.phonetic_match)
            .then(a.distance.cmp(&b.distance))
            .then(a.word.cmp(&b.word))
    });
    candidates.truncate(opts.max_suggestions);
    candidates
}

// ---------------------------------------------------------------------------
// Spell checking
// ---------------------------------------------------------------------------

/// Spell-check text against the built-in dictionary + extras.
pub fn check(text: &str, opts: &SpellOptions) -> SpellResult {
    let mut dict = builtin_dict()
        .into_iter()
        .map(|w| w.to_string())
        .collect::<HashSet<String>>();
    for w in &opts.extra_words {
        dict.insert(w.to_lowercase());
    }
    check_with_dict(text, &dict, opts)
}

/// Spell-check text against a custom dictionary.
pub fn check_with_dict(text: &str, dict: &HashSet<String>, opts: &SpellOptions) -> SpellResult {
    let mut issues = Vec::new();
    let mut word_index = 0;
    let mut offset = 0;

    for word_str in text.split_whitespace() {
        let clean: String = word_str.chars().filter(|c| c.is_alphabetic()).collect();
        if clean.is_empty() || clean.len() == 1 {
            offset += word_str.len() + 1;
            word_index += 1;
            continue;
        }
        if !is_known(&clean, dict) {
            let suggestions = suggest(&clean, dict, opts);
            issues.push(SpellIssue {
                index: word_index,
                word: word_str.to_string(),
                suggestions,
                offset,
            });
        }
        offset += word_str.len() + 1;
        word_index += 1;
    }

    let total = text.split_whitespace().count();
    SpellResult {
        total_words: total,
        errors: issues.len(),
        issues,
    }
}

/// Apply first suggestion to each misspelt word, returning corrected text.
pub fn auto_correct(text: &str, opts: &SpellOptions) -> String {
    let result = check(text, opts);
    let mut corrections: HashMap<usize, String> = HashMap::new();
    for issue in &result.issues {
        if let Some(first) = issue.suggestions.first() {
            corrections.insert(issue.index, first.word.clone());
        }
    }
    let mut out = Vec::new();
    for (i, word) in text.split_whitespace().enumerate() {
        if let Some(replacement) = corrections.get(&i) {
            out.push(replacement.as_str().to_string());
        } else {
            out.push(word.to_string());
        }
    }
    out.join(" ")
}

/// Summary report of spell checking.
pub fn report(result: &SpellResult) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Spell check: {} words, {} errors",
        result.total_words, result.errors
    ));
    for issue in &result.issues {
        let sugg: Vec<&str> = issue.suggestions.iter().map(|s| s.word.as_str()).collect();
        lines.push(format!(
            "  [{}] \"{}\" → [{}]",
            issue.index,
            issue.word,
            sugg.join(", ")
        ));
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
    fn test_levenshtein_same() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_basic() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_damerau_transposition() {
        // Levenshtein would say 2 (delete + insert), Damerau says 1
        assert_eq!(damerau_levenshtein("ab", "ba"), 1);
    }

    #[test]
    fn test_damerau_same_as_levenshtein_no_transposition() {
        assert_eq!(damerau_levenshtein("hello", "hello"), 0);
        assert_eq!(damerau_levenshtein("abc", "axc"), 1);
    }

    #[test]
    fn test_soundex_basic() {
        assert_eq!(soundex("Robert"), "R163");
        assert_eq!(soundex("Rupert"), "R163");
        assert!(sounds_alike("Robert", "Rupert"));
    }

    #[test]
    fn test_soundex_short() {
        assert_eq!(soundex("A"), "A000");
        assert_eq!(soundex(""), "0000");
    }

    #[test]
    fn test_is_known() {
        let dict = build_dictionary(&["hello", "world"]);
        assert!(is_known("hello", &dict));
        assert!(is_known("HELLO", &dict));
        assert!(!is_known("xyz", &dict));
    }

    #[test]
    fn test_suggest_basic() {
        let dict = build_dictionary(&["hello", "help", "held", "hero", "jelly"]);
        let opts = SpellOptions::default();
        let suggs = suggest("helo", &dict, &opts);
        assert!(!suggs.is_empty());
        assert_eq!(suggs[0].word, "hello"); // distance 1
    }

    #[test]
    fn test_check_clean_text() {
        let opts = SpellOptions::default();
        let result = check("the world is good", &opts);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_check_misspelled() {
        let opts = SpellOptions::default();
        let result = check("the wrold is goood", &opts);
        assert!(result.errors > 0);
        assert!(result.issues.iter().any(|i| i.word == "wrold"));
    }

    #[test]
    fn test_check_extra_words() {
        let opts = SpellOptions {
            extra_words: vec!["kubernetes".into(), "microservice".into()],
            ..Default::default()
        };
        let result = check("kubernetes is a microservice tool", &opts);
        // "kubernetes" and "microservice" should NOT be flagged
        assert!(
            !result.issues.iter().any(|i| i.word == "kubernetes"),
            "kubernetes should be accepted"
        );
    }

    #[test]
    fn test_auto_correct() {
        let opts = SpellOptions::default();
        let corrected = auto_correct("the wrold", &opts);
        assert_eq!(corrected, "the world");
    }

    #[test]
    fn test_report() {
        let opts = SpellOptions::default();
        let result = check("the wrold", &opts);
        let r = report(&result);
        assert!(r.contains("wrold"));
    }

    #[test]
    fn test_check_with_dict() {
        let dict = build_dictionary(&["cat", "hat", "bat"]);
        let opts = SpellOptions::default();
        let result = check_with_dict("cat hat xat", &dict, &opts);
        assert_eq!(result.errors, 1);
        assert_eq!(result.issues[0].word, "xat");
    }

    #[test]
    fn test_phonetic_in_suggestions() {
        let dict = build_dictionary(&["smith", "smyth", "smart"]);
        let opts = SpellOptions {
            phonetic: true,
            max_distance: 2,
            ..Default::default()
        };
        let suggs = suggest("smithe", &dict, &opts);
        // "smith" should appear with phonetic_match = true
        assert!(suggs.iter().any(|s| s.word == "smith" && s.phonetic_match));
    }

    #[test]
    fn test_empty_text() {
        let opts = SpellOptions::default();
        let result = check("", &opts);
        assert_eq!(result.total_words, 0);
        assert_eq!(result.errors, 0);
    }
}
