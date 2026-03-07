// ---------------------------------------------------------------------------
// emoji.rs — Emoji processing
// ---------------------------------------------------------------------------
//
// Detection & counting, shortcode conversion (:smile: <-> 😄),
// Unicode name lookup, skin-tone stripping, sentiment hints.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An emoji occurrence in text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiMatch {
    /// The emoji characters.
    pub emoji: String,
    /// Byte offset in source text.
    pub offset: usize,
    /// Short name if known.
    pub name: Option<String>,
}

/// Emoji sentiment hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sentiment {
    Positive,
    Negative,
    Neutral,
}

// ---------------------------------------------------------------------------
// Emoji database (subset — most common)
// ---------------------------------------------------------------------------

/// (emoji, shortcode, name, sentiment)
const EMOJI_DB: &[(&str, &str, &str, Sentiment)] = &[
    ("😀", ":grinning:", "grinning face", Sentiment::Positive),
    (
        "😃",
        ":smiley:",
        "grinning face with big eyes",
        Sentiment::Positive,
    ),
    (
        "😄",
        ":smile:",
        "grinning face with smiling eyes",
        Sentiment::Positive,
    ),
    (
        "😁",
        ":grin:",
        "beaming face with smiling eyes",
        Sentiment::Positive,
    ),
    ("😂", ":joy:", "face with tears of joy", Sentiment::Positive),
    (
        "🤣",
        ":rofl:",
        "rolling on the floor laughing",
        Sentiment::Positive,
    ),
    (
        "😊",
        ":blush:",
        "smiling face with smiling eyes",
        Sentiment::Positive,
    ),
    (
        "😇",
        ":innocent:",
        "smiling face with halo",
        Sentiment::Positive,
    ),
    (
        "🙂",
        ":slightly_smiling_face:",
        "slightly smiling face",
        Sentiment::Positive,
    ),
    ("😉", ":wink:", "winking face", Sentiment::Positive),
    (
        "😍",
        ":heart_eyes:",
        "smiling face with heart-eyes",
        Sentiment::Positive,
    ),
    (
        "🥰",
        ":smiling_face_with_hearts:",
        "smiling face with hearts",
        Sentiment::Positive,
    ),
    (
        "😘",
        ":kissing_heart:",
        "face blowing a kiss",
        Sentiment::Positive,
    ),
    (
        "😎",
        ":sunglasses:",
        "smiling face with sunglasses",
        Sentiment::Positive,
    ),
    ("🤩", ":star_struck:", "star-struck", Sentiment::Positive),
    ("😏", ":smirk:", "smirking face", Sentiment::Neutral),
    ("😐", ":neutral_face:", "neutral face", Sentiment::Neutral),
    (
        "😑",
        ":expressionless:",
        "expressionless face",
        Sentiment::Neutral,
    ),
    ("🤔", ":thinking:", "thinking face", Sentiment::Neutral),
    ("🤗", ":hugs:", "hugging face", Sentiment::Positive),
    ("😢", ":cry:", "crying face", Sentiment::Negative),
    ("😭", ":sob:", "loudly crying face", Sentiment::Negative),
    (
        "😤",
        ":triumph:",
        "face with steam from nose",
        Sentiment::Negative,
    ),
    ("😡", ":rage:", "pouting face", Sentiment::Negative),
    ("😠", ":angry:", "angry face", Sentiment::Negative),
    (
        "🤬",
        ":cursing_face:",
        "face with symbols on mouth",
        Sentiment::Negative,
    ),
    (
        "😱",
        ":scream:",
        "face screaming in fear",
        Sentiment::Negative,
    ),
    ("😨", ":fearful:", "fearful face", Sentiment::Negative),
    (
        "😰",
        ":cold_sweat:",
        "anxious face with sweat",
        Sentiment::Negative,
    ),
    (
        "😥",
        ":disappointed_relieved:",
        "sad but relieved face",
        Sentiment::Negative,
    ),
    ("💀", ":skull:", "skull", Sentiment::Neutral),
    ("👍", ":thumbsup:", "thumbs up", Sentiment::Positive),
    ("👎", ":thumbsdown:", "thumbs down", Sentiment::Negative),
    ("❤️", ":heart:", "red heart", Sentiment::Positive),
    ("💔", ":broken_heart:", "broken heart", Sentiment::Negative),
    ("🔥", ":fire:", "fire", Sentiment::Positive),
    ("⭐", ":star:", "star", Sentiment::Positive),
    (
        "✅",
        ":white_check_mark:",
        "check mark button",
        Sentiment::Positive,
    ),
    ("❌", ":x:", "cross mark", Sentiment::Negative),
    ("⚠️", ":warning:", "warning", Sentiment::Neutral),
    ("🎉", ":tada:", "party popper", Sentiment::Positive),
    (
        "🎊",
        ":confetti_ball:",
        "confetti ball",
        Sentiment::Positive,
    ),
    ("💯", ":100:", "hundred points", Sentiment::Positive),
    ("🚀", ":rocket:", "rocket", Sentiment::Positive),
    ("💡", ":bulb:", "light bulb", Sentiment::Neutral),
    ("📝", ":memo:", "memo", Sentiment::Neutral),
    ("🐛", ":bug:", "bug", Sentiment::Neutral),
    ("🤖", ":robot:", "robot", Sentiment::Neutral),
    ("👻", ":ghost:", "ghost", Sentiment::Neutral),
    ("🌟", ":star2:", "glowing star", Sentiment::Positive),
];

/// Skin tone modifiers (Fitzpatrick scale).
const SKIN_TONES: &[char] = &[
    '\u{1F3FB}', // light
    '\u{1F3FC}', // medium-light
    '\u{1F3FD}', // medium
    '\u{1F3FE}', // medium-dark
    '\u{1F3FF}', // dark
];

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Check if a character is likely an emoji.
pub fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    // Common emoji ranges
    matches!(cp,
        0x1F600..=0x1F64F   // Emoticons
        | 0x1F300..=0x1F5FF // Misc Symbols & Pictographs
        | 0x1F680..=0x1F6FF // Transport & Map
        | 0x1F700..=0x1F77F // Alchemical
        | 0x1F780..=0x1F7FF // Geometric Shapes Ext
        | 0x1F800..=0x1F8FF // Supplemental Arrows-C
        | 0x1F900..=0x1F9FF // Supplemental Symbols
        | 0x1FA00..=0x1FA6F // Chess Symbols
        | 0x1FA70..=0x1FAFF // Symbols and Pictographs Ext-A
        | 0x2600..=0x26FF   // Misc Symbols
        | 0x2700..=0x27BF   // Dingbats
        | 0x231A..=0x231B   // Watch, Hourglass
        | 0x23E9..=0x23F3   // Various
        | 0x23F8..=0x23FA   // Various
        | 0x25AA..=0x25AB   // Squares
        | 0x25B6 | 0x25C0   // Play buttons
        | 0x25FB..=0x25FE   // Squares
        | 0xFE00..=0xFE0F   // Variation selectors
        | 0x200D            // Zero Width Joiner
        | 0x20E3            // Combining Enclosing Keycap
        | 0x1F1E0..=0x1F1FF // Regional indicators
        | 0xE0020..=0xE007F // Tags
    )
}

/// Find all emoji occurrences in text.
pub fn find_emojis(text: &str) -> Vec<EmojiMatch> {
    let db = emoji_to_name_map();
    let mut results = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((offset, c)) = chars.next() {
        if is_emoji(c) && !is_skin_tone(c) && c != '\u{FE0F}' && c != '\u{200D}' {
            // Build up the full emoji sequence
            let mut emoji = String::new();
            emoji.push(c);
            // Consume following variation selectors, ZWJ sequences, skin tones
            while let Some(&(_, next)) = chars.peek() {
                if is_skin_tone(next) || next == '\u{FE0F}' {
                    emoji.push(next);
                    chars.next();
                } else if next == '\u{200D}' {
                    emoji.push(next);
                    chars.next();
                    if let Some(&(_, after_zwj)) = chars.peek() {
                        if is_emoji(after_zwj) {
                            emoji.push(after_zwj);
                            chars.next();
                        }
                    }
                } else {
                    break;
                }
            }
            let name = db.get(emoji.as_str()).map(|s| s.to_string());
            results.push(EmojiMatch {
                emoji,
                offset,
                name,
            });
        }
    }
    results
}

/// Count emojis in text.
pub fn count(text: &str) -> usize {
    find_emojis(text).len()
}

/// Check if text contains any emoji.
pub fn contains_emoji(text: &str) -> bool {
    text.chars()
        .any(|c| is_emoji(c) && !is_skin_tone(c) && c != '\u{FE0F}' && c != '\u{200D}')
}

/// Extract just the emoji characters from text.
pub fn extract(text: &str) -> Vec<String> {
    find_emojis(text).into_iter().map(|m| m.emoji).collect()
}

// ---------------------------------------------------------------------------
// Shortcodes
// ---------------------------------------------------------------------------

/// Convert shortcodes like `:smile:` to emoji characters.
pub fn shortcode_to_emoji(text: &str) -> String {
    let map = shortcode_map();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b':' {
            // Look for closing colon
            if let Some(end) = text[i + 1..].find(':') {
                let end_abs = i + 1 + end;
                let code = &text[i..=end_abs];
                if let Some(emoji) = map.get(code) {
                    result.push_str(emoji);
                    i = end_abs + 1;
                    continue;
                }
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Convert emoji characters to shortcodes.
pub fn emoji_to_shortcode(text: &str) -> String {
    let map = emoji_to_shortcode_map();
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if is_emoji(c) && !is_skin_tone(c) && c != '\u{FE0F}' && c != '\u{200D}' {
            let mut emoji = String::new();
            emoji.push(c);
            while let Some(&next) = chars.peek() {
                if is_skin_tone(next) || next == '\u{FE0F}' {
                    emoji.push(next);
                    chars.next();
                } else if next == '\u{200D}' {
                    emoji.push(next);
                    chars.next();
                    if let Some(&after_zwj) = chars.peek() {
                        if is_emoji(after_zwj) {
                            emoji.push(after_zwj);
                            chars.next();
                        }
                    }
                } else {
                    break;
                }
            }
            if let Some(code) = map.get(emoji.as_str()) {
                result.push_str(code);
            } else {
                result.push_str(&emoji);
            }
        } else {
            result.push(c);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Skin tone
// ---------------------------------------------------------------------------

fn is_skin_tone(c: char) -> bool {
    SKIN_TONES.contains(&c)
}

/// Strip skin tone modifiers from emoji text.
pub fn strip_skin_tones(text: &str) -> String {
    text.chars().filter(|c| !is_skin_tone(*c)).collect()
}

// ---------------------------------------------------------------------------
// Sentiment
// ---------------------------------------------------------------------------

/// Get the sentiment of emojis in text.
pub fn sentiment(text: &str) -> Sentiment {
    let map = emoji_sentiment_map();
    let emojis = find_emojis(text);
    if emojis.is_empty() {
        return Sentiment::Neutral;
    }
    let mut score: i32 = 0;
    let mut counted = 0;
    for em in &emojis {
        if let Some(s) = map.get(em.emoji.as_str()) {
            match s {
                Sentiment::Positive => score += 1,
                Sentiment::Negative => score -= 1,
                Sentiment::Neutral => {}
            }
            counted += 1;
        }
    }
    if counted == 0 {
        return Sentiment::Neutral;
    }
    if score > 0 {
        Sentiment::Positive
    } else if score < 0 {
        Sentiment::Negative
    } else {
        Sentiment::Neutral
    }
}

// ---------------------------------------------------------------------------
// Name lookup
// ---------------------------------------------------------------------------

/// Look up the Unicode name for an emoji.
pub fn name(emoji: &str) -> Option<String> {
    let map = emoji_to_name_map();
    map.get(emoji).map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Internal maps
// ---------------------------------------------------------------------------

fn shortcode_map() -> HashMap<&'static str, &'static str> {
    EMOJI_DB.iter().map(|(e, sc, _, _)| (*sc, *e)).collect()
}

fn emoji_to_shortcode_map() -> HashMap<&'static str, &'static str> {
    EMOJI_DB.iter().map(|(e, sc, _, _)| (*e, *sc)).collect()
}

fn emoji_to_name_map() -> HashMap<&'static str, &'static str> {
    EMOJI_DB.iter().map(|(e, _, n, _)| (*e, *n)).collect()
}

fn emoji_sentiment_map() -> HashMap<&'static str, Sentiment> {
    EMOJI_DB.iter().map(|(e, _, _, s)| (*e, *s)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_emoji() {
        assert!(is_emoji('😀'));
        assert!(is_emoji('🚀'));
        assert!(!is_emoji('A'));
        assert!(!is_emoji(' '));
    }

    #[test]
    fn test_find_emojis() {
        let matches = find_emojis("Hello 😀 World 🚀!");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].emoji, "😀");
        assert_eq!(matches[1].emoji, "🚀");
    }

    #[test]
    fn test_count() {
        assert_eq!(count("No emoji here"), 0);
        assert_eq!(count("😀😂🚀"), 3);
    }

    #[test]
    fn test_contains_emoji() {
        assert!(contains_emoji("Hi 😀"));
        assert!(!contains_emoji("plain text"));
    }

    #[test]
    fn test_extract() {
        let e = extract("a 😀 b 🚀 c");
        assert_eq!(e, vec!["😀", "🚀"]);
    }

    #[test]
    fn test_shortcode_to_emoji() {
        let result = shortcode_to_emoji("Hello :smile: world :rocket:");
        assert!(result.contains("😄"));
        assert!(result.contains("🚀"));
    }

    #[test]
    fn test_emoji_to_shortcode() {
        let result = emoji_to_shortcode("Hello 😄 world");
        assert!(result.contains(":smile:"));
    }

    #[test]
    fn test_shortcode_unknown() {
        let result = shortcode_to_emoji("Hello :unknown_code: world");
        assert_eq!(result, "Hello :unknown_code: world");
    }

    #[test]
    fn test_strip_skin_tones() {
        let stripped = strip_skin_tones("👍\u{1F3FD}");
        assert!(!stripped.contains('\u{1F3FD}'));
        assert!(stripped.contains('👍'));
    }

    #[test]
    fn test_sentiment_positive() {
        assert_eq!(sentiment("Great job 😀👍🎉"), Sentiment::Positive);
    }

    #[test]
    fn test_sentiment_negative() {
        assert_eq!(sentiment("So sad 😢😭"), Sentiment::Negative);
    }

    #[test]
    fn test_sentiment_neutral() {
        assert_eq!(sentiment("No emojis here"), Sentiment::Neutral);
    }

    #[test]
    fn test_name_lookup() {
        assert_eq!(name("😀"), Some("grinning face".to_string()));
        assert_eq!(name("🚀"), Some("rocket".to_string()));
    }

    #[test]
    fn test_name_unknown() {
        assert_eq!(name("X"), None);
    }

    #[test]
    fn test_no_emojis_empty() {
        assert_eq!(find_emojis("").len(), 0);
    }

    #[test]
    fn test_emoji_match_has_name() {
        let matches = find_emojis("😀");
        assert_eq!(matches[0].name, Some("grinning face".to_string()));
    }
}
