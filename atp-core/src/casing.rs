// ---------------------------------------------------------------------------
// casing.rs — Case conversion utilities
// ---------------------------------------------------------------------------
//
// camelCase, PascalCase, snake_case, kebab-case, SCREAMING_SNAKE,
// Title Case, Sentence case, dot.case, path/case, auto-detection.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Recognized case styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStyle {
    CamelCase,
    PascalCase,
    SnakeCase,
    ScreamingSnake,
    KebabCase,
    TitleCase,
    SentenceCase,
    DotCase,
    PathCase,
    FlatCase,
}

// ---------------------------------------------------------------------------
// Word splitting
// ---------------------------------------------------------------------------

/// Split a string into its constituent words regardless of current casing.
///
/// Handles camelCase boundaries, underscores, hyphens, dots, slashes, and spaces.
pub fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let c = chars[i];
        if c == '_' || c == '-' || c == '.' || c == '/' || c == ' ' || c == '\t' {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            i += 1;
            continue;
        }
        // camelCase / PascalCase boundary
        if c.is_uppercase() && !current.is_empty() {
            // Check for acronym: consecutive uppercase followed by lowercase
            let prev_upper = current.chars().last().is_some_and(|p| p.is_uppercase());
            if prev_upper && i + 1 < len && chars[i + 1].is_lowercase() {
                // e.g. "XMLParser" → "XML" + "Parser": split before this char
                words.push(current.clone());
                current.clear();
            } else if !prev_upper {
                // Normal camelCase boundary
                words.push(current.clone());
                current.clear();
            }
        }
        current.push(c);
        i += 1;
    }
    if !current.is_empty() {
        words.push(current);
    }
    // Lowercase everything for consistent downstream processing
    words.iter().map(|w| w.to_lowercase()).collect()
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

/// Convert to `camelCase`.
pub fn to_camel(s: &str) -> String {
    let ws = split_words(s);
    if ws.is_empty() {
        return String::new();
    }
    let mut out = ws[0].clone();
    for w in &ws[1..] {
        let mut chars = w.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_uppercase().next().unwrap_or(first));
            out.extend(chars);
        }
    }
    out
}

/// Convert to `PascalCase`.
pub fn to_pascal(s: &str) -> String {
    let ws = split_words(s);
    ws.iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    let mut r = first.to_uppercase().to_string();
                    r.extend(chars);
                    r
                }
                None => String::new(),
            }
        })
        .collect()
}

/// Convert to `snake_case`.
pub fn to_snake(s: &str) -> String {
    split_words(s).join("_")
}

/// Convert to `SCREAMING_SNAKE_CASE`.
pub fn to_screaming_snake(s: &str) -> String {
    split_words(s)
        .iter()
        .map(|w| w.to_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Convert to `kebab-case`.
pub fn to_kebab(s: &str) -> String {
    split_words(s).join("-")
}

/// Convert to `Title Case`.
pub fn to_title(s: &str) -> String {
    split_words(s)
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    let mut r = first.to_uppercase().to_string();
                    r.extend(chars);
                    r
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert to `Sentence case`.
pub fn to_sentence(s: &str) -> String {
    let ws = split_words(s);
    if ws.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::with_capacity(ws.len());
    for (i, w) in ws.iter().enumerate() {
        if i == 0 {
            let mut chars = w.chars();
            if let Some(first) = chars.next() {
                let mut r = first.to_uppercase().to_string();
                r.extend(chars);
                parts.push(r);
            }
        } else {
            parts.push(w.clone());
        }
    }
    parts.join(" ")
}

/// Convert to `dot.case`.
pub fn to_dot(s: &str) -> String {
    split_words(s).join(".")
}

/// Convert to `path/case`.
pub fn to_path(s: &str) -> String {
    split_words(s).join("/")
}

/// Convert to `flatcase` (all lowercase, no separators).
pub fn to_flat(s: &str) -> String {
    split_words(s).join("")
}

/// Convert to an arbitrary `CaseStyle`.
pub fn convert(s: &str, style: CaseStyle) -> String {
    match style {
        CaseStyle::CamelCase => to_camel(s),
        CaseStyle::PascalCase => to_pascal(s),
        CaseStyle::SnakeCase => to_snake(s),
        CaseStyle::ScreamingSnake => to_screaming_snake(s),
        CaseStyle::KebabCase => to_kebab(s),
        CaseStyle::TitleCase => to_title(s),
        CaseStyle::SentenceCase => to_sentence(s),
        CaseStyle::DotCase => to_dot(s),
        CaseStyle::PathCase => to_path(s),
        CaseStyle::FlatCase => to_flat(s),
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Attempt to detect the case style of `s`.
pub fn detect(s: &str) -> Option<CaseStyle> {
    if s.is_empty() {
        return None;
    }
    let has_upper = s.chars().any(|c| c.is_uppercase());
    let has_lower = s.chars().any(|c| c.is_lowercase());
    let has_underscore = s.contains('_');
    let has_hyphen = s.contains('-');
    let has_dot = s.contains('.');
    let has_slash = s.contains('/');
    let has_space = s.contains(' ');

    if has_underscore && !has_hyphen && !has_dot && !has_slash && !has_space {
        if has_upper && !has_lower {
            return Some(CaseStyle::ScreamingSnake);
        }
        return Some(CaseStyle::SnakeCase);
    }
    if has_hyphen && !has_underscore && !has_dot && !has_slash {
        return Some(CaseStyle::KebabCase);
    }
    if has_dot && !has_underscore && !has_hyphen && !has_slash && !has_space {
        return Some(CaseStyle::DotCase);
    }
    if has_slash && !has_underscore && !has_hyphen && !has_dot && !has_space {
        return Some(CaseStyle::PathCase);
    }
    if has_space {
        // Check if first word is capitalized and rest are lowercase
        let words: Vec<&str> = s.split_whitespace().collect();
        if words.len() > 1 {
            let all_capitalized = words
                .iter()
                .all(|w| w.chars().next().is_some_and(|c| c.is_uppercase()));
            if all_capitalized {
                return Some(CaseStyle::TitleCase);
            }
            let first_cap = words[0].chars().next().is_some_and(|c| c.is_uppercase());
            let rest_lower = words[1..]
                .iter()
                .all(|w| w.chars().next().map_or(true, |c| c.is_lowercase()));
            if first_cap && rest_lower {
                return Some(CaseStyle::SentenceCase);
            }
        }
        return None;
    }
    if has_upper && has_lower && !has_underscore && !has_hyphen {
        if s.chars().next().is_some_and(|c| c.is_uppercase()) {
            return Some(CaseStyle::PascalCase);
        }
        return Some(CaseStyle::CamelCase);
    }
    if !has_upper && has_lower && !has_underscore && !has_hyphen {
        return Some(CaseStyle::FlatCase);
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_words_snake() {
        assert_eq!(split_words("hello_world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_split_words_camel() {
        assert_eq!(split_words("helloWorld"), vec!["hello", "world"]);
    }

    #[test]
    fn test_split_words_pascal() {
        assert_eq!(split_words("HelloWorld"), vec!["hello", "world"]);
    }

    #[test]
    fn test_split_words_kebab() {
        assert_eq!(split_words("hello-world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_split_words_acronym() {
        assert_eq!(split_words("XMLParser"), vec!["xml", "parser"]);
    }

    #[test]
    fn test_to_camel() {
        assert_eq!(to_camel("hello_world"), "helloWorld");
    }

    #[test]
    fn test_to_pascal() {
        assert_eq!(to_pascal("hello_world"), "HelloWorld");
    }

    #[test]
    fn test_to_snake() {
        assert_eq!(to_snake("helloWorld"), "hello_world");
    }

    #[test]
    fn test_to_screaming_snake() {
        assert_eq!(to_screaming_snake("helloWorld"), "HELLO_WORLD");
    }

    #[test]
    fn test_to_kebab() {
        assert_eq!(to_kebab("HelloWorld"), "hello-world");
    }

    #[test]
    fn test_to_title() {
        assert_eq!(to_title("hello_world"), "Hello World");
    }

    #[test]
    fn test_to_sentence() {
        assert_eq!(to_sentence("hello_world"), "Hello world");
    }

    #[test]
    fn test_to_dot() {
        assert_eq!(to_dot("helloWorld"), "hello.world");
    }

    #[test]
    fn test_to_path() {
        assert_eq!(to_path("helloWorld"), "hello/world");
    }

    #[test]
    fn test_to_flat() {
        assert_eq!(to_flat("hello_world"), "helloworld");
    }

    #[test]
    fn test_detect_snake() {
        assert_eq!(detect("hello_world"), Some(CaseStyle::SnakeCase));
    }

    #[test]
    fn test_detect_camel() {
        assert_eq!(detect("helloWorld"), Some(CaseStyle::CamelCase));
    }

    #[test]
    fn test_detect_pascal() {
        assert_eq!(detect("HelloWorld"), Some(CaseStyle::PascalCase));
    }

    #[test]
    fn test_detect_screaming_snake() {
        assert_eq!(detect("HELLO_WORLD"), Some(CaseStyle::ScreamingSnake));
    }

    #[test]
    fn test_detect_kebab() {
        assert_eq!(detect("hello-world"), Some(CaseStyle::KebabCase));
    }

    #[test]
    fn test_convert_roundtrip() {
        let original = "helloWorld";
        let snake = convert(original, CaseStyle::SnakeCase);
        let back = convert(&snake, CaseStyle::CamelCase);
        assert_eq!(back, "helloWorld");
    }

    #[test]
    fn test_convert_all_styles() {
        let input = "hello_world";
        for style in [
            CaseStyle::CamelCase,
            CaseStyle::PascalCase,
            CaseStyle::SnakeCase,
            CaseStyle::ScreamingSnake,
            CaseStyle::KebabCase,
            CaseStyle::TitleCase,
            CaseStyle::SentenceCase,
            CaseStyle::DotCase,
            CaseStyle::PathCase,
            CaseStyle::FlatCase,
        ] {
            let result = convert(input, style);
            assert!(!result.is_empty(), "Empty result for {:?}", style);
        }
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(to_camel(""), "");
        assert_eq!(detect(""), None);
    }
}
