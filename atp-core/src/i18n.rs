//! # Internationalisation & Unicode Utilities
//!
//! NFC/NFD/NFKC/NFKD normalisation (pure-Rust approximation for ASCII-Latin),
//! transliteration, case folding, grapheme segmentation, script detection,
//! and locale-aware collation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Normalisation forms
// ---------------------------------------------------------------------------

/// Unicode normalisation form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NormForm {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

/// Decomposition table for common Latin composed characters.
/// Maps a composed char → base + combining mark (simplified).
const DECOMPOSE: &[(char, &str)] = &[
    ('\u{00C0}', "A\u{0300}"), // À
    ('\u{00C1}', "A\u{0301}"), // Á
    ('\u{00C2}', "A\u{0302}"), // Â
    ('\u{00C3}', "A\u{0303}"), // Ã
    ('\u{00C4}', "A\u{0308}"), // Ä
    ('\u{00C5}', "A\u{030A}"), // Å
    ('\u{00C7}', "C\u{0327}"), // Ç
    ('\u{00C8}', "E\u{0300}"), // È
    ('\u{00C9}', "E\u{0301}"), // É
    ('\u{00CA}', "E\u{0302}"), // Ê
    ('\u{00CB}', "E\u{0308}"), // Ë
    ('\u{00CC}', "I\u{0300}"), // Ì
    ('\u{00CD}', "I\u{0301}"), // Í
    ('\u{00CE}', "I\u{0302}"), // Î
    ('\u{00CF}', "I\u{0308}"), // Ï
    ('\u{00D1}', "N\u{0303}"), // Ñ
    ('\u{00D2}', "O\u{0300}"), // Ò
    ('\u{00D3}', "O\u{0301}"), // Ó
    ('\u{00D4}', "O\u{0302}"), // Ô
    ('\u{00D5}', "O\u{0303}"), // Õ
    ('\u{00D6}', "O\u{0308}"), // Ö
    ('\u{00D9}', "U\u{0300}"), // Ù
    ('\u{00DA}', "U\u{0301}"), // Ú
    ('\u{00DB}', "U\u{0302}"), // Û
    ('\u{00DC}', "U\u{0308}"), // Ü
    ('\u{00DD}', "Y\u{0301}"), // Ý
    ('\u{00E0}', "a\u{0300}"), // à
    ('\u{00E1}', "a\u{0301}"), // á
    ('\u{00E2}', "a\u{0302}"), // â
    ('\u{00E3}', "a\u{0303}"), // ã
    ('\u{00E4}', "a\u{0308}"), // ä
    ('\u{00E5}', "a\u{030A}"), // å
    ('\u{00E7}', "c\u{0327}"), // ç
    ('\u{00E8}', "e\u{0300}"), // è
    ('\u{00E9}', "e\u{0301}"), // é
    ('\u{00EA}', "e\u{0302}"), // ê
    ('\u{00EB}', "e\u{0308}"), // ë
    ('\u{00EC}', "i\u{0300}"), // ì
    ('\u{00ED}', "i\u{0301}"), // í
    ('\u{00EE}', "i\u{0302}"), // î
    ('\u{00EF}', "i\u{0308}"), // ï
    ('\u{00F1}', "n\u{0303}"), // ñ
    ('\u{00F2}', "o\u{0300}"), // ò
    ('\u{00F3}', "o\u{0301}"), // ó
    ('\u{00F4}', "o\u{0302}"), // ô
    ('\u{00F5}', "o\u{0303}"), // õ
    ('\u{00F6}', "o\u{0308}"), // ö
    ('\u{00F9}', "u\u{0300}"), // ù
    ('\u{00FA}', "u\u{0301}"), // ú
    ('\u{00FB}', "u\u{0302}"), // û
    ('\u{00FC}', "u\u{0308}"), // ü
    ('\u{00FD}', "y\u{0301}"), // ý
    ('\u{00FF}', "y\u{0308}"), // ÿ
];

/// Build a decomposition lookup.
fn decompose_map() -> BTreeMap<char, &'static str> {
    DECOMPOSE.iter().copied().collect()
}

/// Build a composition lookup (reverse of decompose).
fn compose_map() -> BTreeMap<String, char> {
    DECOMPOSE.iter().map(|&(c, s)| (s.to_string(), c)).collect()
}

/// Apply Unicode normalisation.
pub fn normalize(text: &str, form: NormForm) -> String {
    match form {
        NormForm::Nfd | NormForm::Nfkd => {
            let dmap = decompose_map();
            let mut out = String::with_capacity(text.len());
            for ch in text.chars() {
                if let Some(decomposed) = dmap.get(&ch) {
                    out.push_str(decomposed);
                } else {
                    out.push(ch);
                }
            }
            out
        }
        NormForm::Nfc | NormForm::Nfkc => {
            // First decompose, then compose
            let decomposed = normalize(text, NormForm::Nfd);
            let cmap = compose_map();
            let chars: Vec<char> = decomposed.chars().collect();
            let mut out = String::with_capacity(text.len());
            let mut i = 0;
            while i < chars.len() {
                if i + 1 < chars.len() {
                    let pair = format!("{}{}", chars[i], chars[i + 1]);
                    if let Some(&composed) = cmap.get(&pair) {
                        out.push(composed);
                        i += 2;
                        continue;
                    }
                }
                out.push(chars[i]);
                i += 1;
            }
            out
        }
    }
}

// ---------------------------------------------------------------------------
// Transliteration
// ---------------------------------------------------------------------------

/// ASCII transliteration table for Latin-extended characters.
const TRANSLIT: &[(char, &str)] = &[
    ('À', "A"),
    ('Á', "A"),
    ('Â', "A"),
    ('Ã', "A"),
    ('Ä', "Ae"),
    ('Å', "A"),
    ('Æ', "Ae"),
    ('Ç', "C"),
    ('È', "E"),
    ('É', "E"),
    ('Ê', "E"),
    ('Ë', "E"),
    ('Ì', "I"),
    ('Í', "I"),
    ('Î', "I"),
    ('Ï', "I"),
    ('Ð', "D"),
    ('Ñ', "N"),
    ('Ò', "O"),
    ('Ó', "O"),
    ('Ô', "O"),
    ('Õ', "O"),
    ('Ö', "Oe"),
    ('Ø', "O"),
    ('Ù', "U"),
    ('Ú', "U"),
    ('Û', "U"),
    ('Ü', "Ue"),
    ('Ý', "Y"),
    ('Þ', "Th"),
    ('ß', "ss"),
    ('à', "a"),
    ('á', "a"),
    ('â', "a"),
    ('ã', "a"),
    ('ä', "ae"),
    ('å', "a"),
    ('æ', "ae"),
    ('ç', "c"),
    ('è', "e"),
    ('é', "e"),
    ('ê', "e"),
    ('ë', "e"),
    ('ì', "i"),
    ('í', "i"),
    ('î', "i"),
    ('ï', "i"),
    ('ð', "d"),
    ('ñ', "n"),
    ('ò', "o"),
    ('ó', "o"),
    ('ô', "o"),
    ('õ', "o"),
    ('ö', "oe"),
    ('ø', "o"),
    ('ù', "u"),
    ('ú', "u"),
    ('û', "u"),
    ('ü', "ue"),
    ('ý', "y"),
    ('þ', "th"),
    ('ÿ', "y"),
    ('Ł', "L"),
    ('ł', "l"),
    ('Ő', "O"),
    ('ő', "o"),
    ('Ű', "U"),
    ('ű', "u"),
    ('Ş', "S"),
    ('ş', "s"),
    ('Ğ', "G"),
    ('ğ', "g"),
    ('İ', "I"),
    ('ı', "i"),
];

/// Transliterate accented/special characters to ASCII.
pub fn transliterate(text: &str) -> String {
    let map: BTreeMap<char, &str> = TRANSLIT.iter().copied().collect();
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if let Some(repl) = map.get(&ch) {
            out.push_str(repl);
        } else if ch.is_ascii() {
            out.push(ch);
        } else {
            out.push(ch); // keep unknown non-ASCII as-is
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Case folding
// ---------------------------------------------------------------------------

/// Simple Unicode case fold (lowercase + special mappings).
pub fn case_fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            'ß' => out.push_str("ss"),
            'İ' => {
                out.push('i');
                out.push('\u{0307}');
            }
            _ => {
                for c in ch.to_lowercase() {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Case-insensitive equality using case folding.
pub fn case_fold_eq(a: &str, b: &str) -> bool {
    case_fold(a) == case_fold(b)
}

// ---------------------------------------------------------------------------
// Grapheme segmentation (simplified)
// ---------------------------------------------------------------------------

/// A grapheme cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grapheme {
    pub cluster: String,
    pub byte_offset: usize,
}

/// Combining character range check.
fn is_combining(ch: char) -> bool {
    let cp = ch as u32;
    (0x0300..=0x036F).contains(&cp) // Combining Diacritical Marks
        || (0x1AB0..=0x1AFF).contains(&cp)
        || (0x1DC0..=0x1DFF).contains(&cp)
        || (0x20D0..=0x20FF).contains(&cp)
        || (0xFE20..=0xFE2F).contains(&cp)
}

/// Segment text into grapheme clusters (simplified: base + combiners).
pub fn grapheme_clusters(text: &str) -> Vec<Grapheme> {
    let mut clusters = Vec::new();
    let mut current = String::new();
    let mut start = 0;

    for (i, ch) in text.char_indices() {
        if is_combining(ch) && !current.is_empty() {
            current.push(ch);
        } else {
            if !current.is_empty() {
                clusters.push(Grapheme {
                    cluster: std::mem::take(&mut current),
                    byte_offset: start,
                });
            }
            start = i;
            current.push(ch);
        }
    }
    if !current.is_empty() {
        clusters.push(Grapheme {
            cluster: current,
            byte_offset: start,
        });
    }
    clusters
}

/// Count grapheme clusters (visual character count).
pub fn grapheme_count(text: &str) -> usize {
    grapheme_clusters(text).len()
}

// ---------------------------------------------------------------------------
// Script detection
// ---------------------------------------------------------------------------

/// Detected script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Script {
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Han,
    Hiragana,
    Katakana,
    Hangul,
    Devanagari,
    Thai,
    Common,
    Unknown,
}

/// Detect script of a single character.
pub fn char_script(ch: char) -> Script {
    let cp = ch as u32;
    match cp {
        // ASCII letters/digits → Latin, everything else → Common
        0x0041..=0x005A | 0x0061..=0x007A | 0x0030..=0x0039 => Script::Latin,
        0x0000..=0x007F => Script::Common, // remaining Basic ASCII (punctuation, whitespace)
        0x0080..=0x024F => Script::Latin,
        0x0370..=0x03FF => Script::Greek,
        0x0400..=0x04FF => Script::Cyrillic,
        0x0590..=0x05FF => Script::Hebrew,
        0x0600..=0x06FF => Script::Arabic,
        0x0900..=0x097F => Script::Devanagari,
        0x0E00..=0x0E7F => Script::Thai,
        0x3040..=0x309F => Script::Hiragana,
        0x30A0..=0x30FF => Script::Katakana,
        0x4E00..=0x9FFF => Script::Han,
        0xAC00..=0xD7AF => Script::Hangul,
        _ if ch.is_alphanumeric() => Script::Unknown,
        _ => Script::Common,
    }
}

/// Detect the dominant script of a text.
pub fn detect_script(text: &str) -> Script {
    let mut counts: BTreeMap<Script, usize> = BTreeMap::new();
    for ch in text.chars() {
        let s = char_script(ch);
        if s != Script::Common {
            *counts.entry(s).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, c)| c)
        .map(|(s, _)| s)
        .unwrap_or(Script::Common)
}

/// Detect all scripts present in text.
pub fn detect_scripts(text: &str) -> Vec<Script> {
    let mut seen = std::collections::BTreeSet::new();
    for ch in text.chars() {
        let s = char_script(ch);
        if s != Script::Common {
            seen.insert(format!("{s:?}"));
        }
    }
    // Convert back — awkward but keeps ordering deterministic
    let mut result = Vec::new();
    for ch in text.chars() {
        let s = char_script(ch);
        if s != Script::Common && seen.remove(&format!("{s:?}")) {
            result.push(s);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Locale-aware collation (simplified)
// ---------------------------------------------------------------------------

/// Locale hint for collation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Locale {
    /// Root / default collation.
    Root,
    /// German (treat ä = ae, ö = oe, ü = ue, ß = ss).
    De,
    /// Swedish (å, ä, ö sort after z).
    Sv,
    /// Turkish (dotted/dotless i distinction).
    Tr,
}

/// Produce a collation key for sorting.
pub fn collation_key(text: &str, locale: Locale) -> String {
    match locale {
        Locale::Root => text.to_lowercase(),
        Locale::De => {
            let mut out = String::new();
            for ch in text.chars() {
                match ch {
                    'ä' | 'Ä' => out.push_str("ae"),
                    'ö' | 'Ö' => out.push_str("oe"),
                    'ü' | 'Ü' => out.push_str("ue"),
                    'ß' => out.push_str("ss"),
                    _ => {
                        for c in ch.to_lowercase() {
                            out.push(c);
                        }
                    }
                }
            }
            out
        }
        Locale::Sv => {
            // In Swedish, å ä ö sort after z
            let mut out = String::new();
            for ch in text.chars() {
                match ch {
                    'å' | 'Å' => out.push_str("zzz1"),
                    'ä' | 'Ä' => out.push_str("zzz2"),
                    'ö' | 'Ö' => out.push_str("zzz3"),
                    _ => {
                        for c in ch.to_lowercase() {
                            out.push(c);
                        }
                    }
                }
            }
            out
        }
        Locale::Tr => {
            let mut out = String::new();
            for ch in text.chars() {
                match ch {
                    'I' => out.push('ı'),
                    'İ' => out.push('i'),
                    _ => {
                        for c in ch.to_lowercase() {
                            out.push(c);
                        }
                    }
                }
            }
            out
        }
    }
}

/// Sort strings by locale-aware collation.
pub fn locale_sort(items: &mut [String], locale: Locale) {
    items.sort_by(|a, b| {
        let ka = collation_key(a, locale);
        let kb = collation_key(b, locale);
        ka.cmp(&kb)
    });
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Check if a string is pure ASCII.
pub fn is_ascii(text: &str) -> bool {
    text.is_ascii()
}

/// Count Unicode code points.
pub fn codepoint_count(text: &str) -> usize {
    text.chars().count()
}

/// Report byte length, char count, grapheme count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMeasure {
    pub bytes: usize,
    pub codepoints: usize,
    pub graphemes: usize,
}

/// Measure text dimensions.
pub fn measure(text: &str) -> TextMeasure {
    TextMeasure {
        bytes: text.len(),
        codepoints: text.chars().count(),
        graphemes: grapheme_count(text),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nfd_decompose() {
        let result = normalize("é", NormForm::Nfd);
        // é → e + combining acute
        assert_eq!(result.chars().count(), 2);
        assert_eq!(result.chars().next(), Some('e'));
    }

    #[test]
    fn test_nfc_compose() {
        // Decompose then recompose
        let decomposed = normalize("é", NormForm::Nfd);
        let recomposed = normalize(&decomposed, NormForm::Nfc);
        assert_eq!(recomposed, "é");
    }

    #[test]
    fn test_ascii_passthrough() {
        let text = "Hello World 123";
        assert_eq!(normalize(text, NormForm::Nfc), text);
        assert_eq!(normalize(text, NormForm::Nfd), text);
    }

    #[test]
    fn test_transliterate() {
        assert_eq!(transliterate("café"), "cafe");
        assert_eq!(transliterate("naïve"), "naive");
        assert_eq!(transliterate("Straße"), "Strasse");
        assert_eq!(transliterate("Ñoño"), "Nono");
    }

    #[test]
    fn test_case_fold() {
        assert_eq!(case_fold("Hello"), "hello");
        assert_eq!(case_fold("Straße"), "strasse");
    }

    #[test]
    fn test_case_fold_eq() {
        assert!(case_fold_eq("HELLO", "hello"));
        assert!(case_fold_eq("Straße", "strasse"));
        assert!(!case_fold_eq("abc", "def"));
    }

    #[test]
    fn test_grapheme_clusters() {
        let clusters = grapheme_clusters("e\u{0301}a"); // é a
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].cluster, "e\u{0301}");
        assert_eq!(clusters[1].cluster, "a");
    }

    #[test]
    fn test_grapheme_count_ascii() {
        assert_eq!(grapheme_count("hello"), 5);
    }

    #[test]
    fn test_detect_script_latin() {
        assert_eq!(detect_script("Hello World"), Script::Latin);
    }

    #[test]
    fn test_detect_script_cyrillic() {
        assert_eq!(detect_script("Привет"), Script::Cyrillic);
    }

    #[test]
    fn test_detect_script_cjk() {
        assert_eq!(detect_script("你好世界"), Script::Han);
    }

    #[test]
    fn test_locale_sort_german() {
        let mut items = vec!["Öl".into(), "Apfel".into(), "Über".into(), "Birne".into()];
        locale_sort(&mut items, Locale::De);
        assert_eq!(items[0], "Apfel");
        assert_eq!(items[1], "Birne");
    }

    #[test]
    fn test_locale_sort_swedish() {
        let mut items = vec!["ä".into(), "z".into(), "a".into(), "å".into()];
        locale_sort(&mut items, Locale::Sv);
        // a, z should come before å, ä
        assert_eq!(items[0], "a");
        assert_eq!(items[1], "z");
    }

    #[test]
    fn test_measure() {
        let m = measure("café");
        assert_eq!(m.codepoints, 4);
        assert!(m.bytes >= 4); // é is multi-byte
    }

    #[test]
    fn test_is_ascii() {
        assert!(is_ascii("hello"));
        assert!(!is_ascii("café"));
    }

    #[test]
    fn test_collation_key_turkish() {
        let k = collation_key("I", Locale::Tr);
        assert_eq!(k, "ı"); // Turkish: I → ı
    }
}
