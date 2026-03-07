// ---------------------------------------------------------------------------
// text_wrap.rs — Text wrapping & layout
// ---------------------------------------------------------------------------
//
// Word wrap, hard wrap, justify (left/right/center/full), indent/dedent,
// hanging indent, margin control, column formatting.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
    Full,
}

/// Wrap options.
#[derive(Debug, Clone)]
pub struct WrapOptions {
    /// Maximum line width in characters.
    pub width: usize,
    /// Alignment for each line.
    pub align: Align,
    /// String to prepend to every line (margin/indent).
    pub indent: String,
    /// String to prepend to first line only (overrides `indent` for line 0).
    pub first_indent: Option<String>,
    /// Whether to perform hard wrapping (break mid-word) when a word exceeds width.
    pub hard_wrap: bool,
}

impl Default for WrapOptions {
    fn default() -> Self {
        Self {
            width: 80,
            align: Align::Left,
            indent: String::new(),
            first_indent: None,
            hard_wrap: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Core wrapping
// ---------------------------------------------------------------------------

/// Word-wrap `text` to the given `width`.
///
/// Words are split on whitespace. If `hard_wrap` is true, long words are
/// broken at the width boundary.
pub fn word_wrap(text: &str, width: usize) -> Vec<String> {
    wrap(
        text,
        &WrapOptions {
            width,
            ..Default::default()
        },
    )
}

/// Hard-wrap `text` — break lines at exactly `width` characters.
pub fn hard_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
            continue;
        }
        for chunk in chars.chunks(width) {
            lines.push(chunk.iter().collect());
        }
    }
    lines
}

/// Wrap text with full options.
pub fn wrap(text: &str, opts: &WrapOptions) -> Vec<String> {
    let mut result = Vec::new();
    for paragraph in text.split('\n') {
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();
        for word in &words {
            let effective_width = if lines.is_empty() {
                let ind = opts.first_indent.as_deref().unwrap_or(&opts.indent);
                opts.width.saturating_sub(ind.len())
            } else {
                opts.width.saturating_sub(opts.indent.len())
            };
            if current_line.is_empty() {
                if word.len() > effective_width && opts.hard_wrap {
                    // Break the long word
                    let chars: Vec<char> = word.chars().collect();
                    for chunk in chars.chunks(effective_width) {
                        lines.push(chunk.iter().collect());
                    }
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word.len() <= effective_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                if word.len() > effective_width && opts.hard_wrap {
                    let chars: Vec<char> = word.chars().collect();
                    for chunk in chars.chunks(effective_width) {
                        lines.push(chunk.iter().collect());
                    }
                    current_line = String::new();
                } else {
                    current_line = word.to_string();
                }
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        result.extend(lines);
    }
    // Apply indent and alignment
    let mut final_lines = Vec::with_capacity(result.len());
    for (i, line) in result.iter().enumerate() {
        let ind = if i == 0 {
            opts.first_indent.as_deref().unwrap_or(&opts.indent)
        } else {
            &opts.indent
        };
        let effective_width = opts.width.saturating_sub(ind.len());
        let aligned = align_line(line, effective_width, opts.align);
        final_lines.push(format!("{ind}{aligned}"));
    }
    final_lines
}

/// Render wrapped lines back to a single string.
pub fn wrap_to_string(text: &str, opts: &WrapOptions) -> String {
    wrap(text, opts).join("\n")
}

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

fn align_line(line: &str, width: usize, align: Align) -> String {
    let len = line.chars().count();
    if len >= width {
        return line.to_string();
    }
    let pad = width - len;
    match align {
        Align::Left => line.to_string(),
        Align::Right => format!("{}{}", " ".repeat(pad), line),
        Align::Center => {
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", " ".repeat(left), line, " ".repeat(right))
        }
        Align::Full => justify_line(line, width),
    }
}

fn justify_line(line: &str, width: usize) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() <= 1 {
        return line.to_string();
    }
    let total_word_len: usize = words.iter().map(|w| w.len()).sum();
    let total_spaces = width.saturating_sub(total_word_len);
    let gaps = words.len() - 1;
    let base_space = total_spaces / gaps;
    let extra = total_spaces % gaps;
    let mut result = String::with_capacity(width);
    for (i, word) in words.iter().enumerate() {
        result.push_str(word);
        if i < gaps {
            let spaces = base_space + if i < extra { 1 } else { 0 };
            result.push_str(&" ".repeat(spaces));
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Indent / Dedent
// ---------------------------------------------------------------------------

/// Add `prefix` to the beginning of every line.
pub fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|l| format!("{prefix}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove common leading whitespace from all non-empty lines.
pub fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            if l.len() >= min_indent {
                &l[min_indent..]
            } else {
                l.trim()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Apply hanging indent: first line has no extra indent, subsequent lines get `prefix`.
pub fn hanging_indent(text: &str, width: usize, hang: &str) -> String {
    let lines = word_wrap(text, width);
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                l.to_string()
            } else {
                format!("{hang}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Column formatting
// ---------------------------------------------------------------------------

/// Format text into `num_cols` newspaper-style columns.
pub fn columns(text: &str, num_cols: usize, col_width: usize, gutter: usize) -> String {
    if num_cols == 0 {
        return text.to_string();
    }
    let lines = word_wrap(text, col_width);
    let rows_per_col = lines.len().div_ceil(num_cols);
    let mut col_data: Vec<Vec<&str>> = Vec::with_capacity(num_cols);
    for c in 0..num_cols {
        let start = c * rows_per_col;
        let end = (start + rows_per_col).min(lines.len());
        if start < lines.len() {
            col_data.push(lines[start..end].iter().map(|s| s.as_str()).collect());
        } else {
            col_data.push(Vec::new());
        }
    }
    let mut output = Vec::new();
    for row in 0..rows_per_col {
        let mut parts = Vec::new();
        for col in &col_data {
            let cell = col.get(row).unwrap_or(&"");
            let padded = format!("{:<width$}", cell, width = col_width);
            parts.push(padded);
        }
        output.push(parts.join(&" ".repeat(gutter)));
    }
    output.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_wrap_basic() {
        let lines = word_wrap("hello world foo bar", 10);
        assert_eq!(lines, vec!["hello", "world foo", "bar"]);
    }

    #[test]
    fn test_word_wrap_exact_fit() {
        let lines = word_wrap("hello world", 11);
        assert_eq!(lines, vec!["hello world"]);
    }

    #[test]
    fn test_hard_wrap() {
        let lines = hard_wrap("abcdefghij", 4);
        assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn test_hard_wrap_empty() {
        let lines = hard_wrap("", 4);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn test_align_right() {
        let opts = WrapOptions {
            width: 20,
            align: Align::Right,
            ..Default::default()
        };
        let lines = wrap("hello", &opts);
        assert_eq!(lines[0].len(), 20);
        assert!(lines[0].ends_with("hello"));
    }

    #[test]
    fn test_align_center() {
        let opts = WrapOptions {
            width: 20,
            align: Align::Center,
            ..Default::default()
        };
        let lines = wrap("hello", &opts);
        assert!(lines[0].starts_with("       "));
    }

    #[test]
    fn test_align_full() {
        let opts = WrapOptions {
            width: 20,
            align: Align::Full,
            ..Default::default()
        };
        let lines = wrap("hello world foo", &opts);
        // Justified lines should be padded to width (when multi-word)
        for line in &lines {
            if line.split_whitespace().count() > 1 {
                assert_eq!(line.len(), 20);
            }
        }
    }

    #[test]
    fn test_indent() {
        let r = indent("a\nb\nc", ">> ");
        assert_eq!(r, ">> a\n>> b\n>> c");
    }

    #[test]
    fn test_dedent() {
        let r = dedent("    a\n    b\n    c");
        assert_eq!(r, "a\nb\nc");
    }

    #[test]
    fn test_dedent_mixed() {
        let r = dedent("    a\n      b\n    c");
        assert_eq!(r, "a\n  b\nc");
    }

    #[test]
    fn test_hanging_indent() {
        let r = hanging_indent("a long text that wraps around", 15, "    ");
        let lines: Vec<&str> = r.lines().collect();
        assert!(lines.len() > 1);
        assert!(!lines[0].starts_with("    "));
        assert!(lines[1].starts_with("    "));
    }

    #[test]
    fn test_columns() {
        let r = columns("a b c d e f g h", 2, 10, 2);
        assert!(!r.is_empty());
    }

    #[test]
    fn test_wrap_with_indent() {
        let opts = WrapOptions {
            width: 20,
            indent: "  ".to_string(),
            ..Default::default()
        };
        let lines = wrap("hello world foo bar baz", &opts);
        for line in &lines {
            assert!(line.starts_with("  "));
        }
    }

    #[test]
    fn test_wrap_first_indent() {
        let opts = WrapOptions {
            width: 20,
            indent: "  ".to_string(),
            first_indent: Some("* ".to_string()),
            ..Default::default()
        };
        let lines = wrap("hello world foo bar baz qux", &opts);
        assert!(lines[0].starts_with("* "));
        if lines.len() > 1 {
            assert!(lines[1].starts_with("  "));
        }
    }

    #[test]
    fn test_wrap_to_string() {
        let opts = WrapOptions {
            width: 10,
            ..Default::default()
        };
        let s = wrap_to_string("hello world foo", &opts);
        assert!(s.contains('\n'));
    }

    #[test]
    fn test_empty_input() {
        let lines = word_wrap("", 80);
        assert!(lines.is_empty() || lines == vec![""]);
    }
}
