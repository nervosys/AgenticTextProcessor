//! # Advanced Text Formatter
//!
//! Provides table formatting (ASCII, Unicode box-drawing, Markdown), column
//! alignment, text wrapping/truncation/padding, and indent normalization.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Table border style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    /// No borders.
    None,
    /// ASCII: `+`, `-`, `|`.
    #[default]
    Ascii,
    /// Unicode box-drawing characters.
    Unicode,
    /// Markdown-style `|` and `---`.
    Markdown,
}

/// Column alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// How to handle text that exceeds column width.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Overflow {
    /// Truncate with `…`.
    #[default]
    Truncate,
    /// Wrap to next line within the cell.
    Wrap,
    /// Allow the column to expand.
    Expand,
}

/// Column definition.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column header text.
    pub header: String,
    /// Minimum width in characters.
    pub min_width: usize,
    /// Maximum width (0 = unlimited).
    pub max_width: usize,
    /// Alignment.
    pub alignment: Alignment,
    /// Overflow handling.
    pub overflow: Overflow,
}

impl ColumnDef {
    pub fn new(header: &str) -> Self {
        Self {
            header: header.to_string(),
            min_width: 0,
            max_width: 0,
            alignment: Alignment::Left,
            overflow: Overflow::Truncate,
        }
    }

    pub fn with_width(mut self, min: usize, max: usize) -> Self {
        self.min_width = min;
        self.max_width = max;
        self
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn with_overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }
}

/// A formatted table.
#[derive(Debug, Clone)]
pub struct Table {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Vec<String>>,
    pub border_style: BorderStyle,
    pub header_separator: bool,
}

impl Table {
    /// Create a table from header strings with default column defs.
    pub fn new(headers: &[&str]) -> Self {
        Self {
            columns: headers.iter().map(|h| ColumnDef::new(h)).collect(),
            rows: Vec::new(),
            border_style: BorderStyle::Ascii,
            header_separator: true,
        }
    }

    pub fn with_border(mut self, style: BorderStyle) -> Self {
        self.border_style = style;
        self
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    /// Render the table to a string.
    pub fn render(&self) -> String {
        let col_count = self.columns.len();
        if col_count == 0 {
            return String::new();
        }

        // Compute effective widths
        let mut widths = vec![0usize; col_count];
        for (i, col) in self.columns.iter().enumerate() {
            widths[i] = col.header.len().max(col.min_width);
        }
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    let max_w = self.columns[i].max_width;
                    let cell_w = cell.len();
                    if max_w > 0 {
                        widths[i] = widths[i].max(cell_w.min(max_w));
                    } else {
                        widths[i] = widths[i].max(cell_w);
                    }
                    widths[i] = widths[i].max(self.columns[i].min_width);
                }
            }
        }

        match self.border_style {
            BorderStyle::None => self.render_no_border(&widths),
            BorderStyle::Ascii => self.render_ascii(&widths),
            BorderStyle::Unicode => self.render_unicode(&widths),
            BorderStyle::Markdown => self.render_markdown(&widths),
        }
    }

    fn format_cell(text: &str, width: usize, alignment: Alignment, overflow: Overflow) -> String {
        let text = if overflow == Overflow::Truncate && text.len() > width && width > 1 {
            format!("{}…", &text[..width - 1])
        } else {
            text.to_string()
        };

        let text_len = text.len();
        if text_len >= width {
            return text;
        }

        let padding = width - text_len;
        match alignment {
            Alignment::Left => format!("{text}{}", " ".repeat(padding)),
            Alignment::Right => format!("{}{text}", " ".repeat(padding)),
            Alignment::Center => {
                let left = padding / 2;
                let right = padding - left;
                format!("{}{text}{}", " ".repeat(left), " ".repeat(right))
            }
        }
    }

    fn render_row(&self, row: &[String], widths: &[usize], sep: &str) -> String {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if i < self.columns.len() {
                    Self::format_cell(
                        cell,
                        widths[i],
                        self.columns[i].alignment,
                        self.columns[i].overflow,
                    )
                } else {
                    cell.clone()
                }
            })
            .collect();
        format!("{sep} {} {sep}", cells.join(&format!(" {sep} ")))
    }

    fn render_no_border(&self, widths: &[usize]) -> String {
        let mut out = String::new();
        let header_cells: Vec<String> = self.columns.iter().map(|c| c.header.clone()).collect();
        let header_line = self.render_row(&header_cells, widths, "");
        out.push_str(header_line.trim());
        out.push('\n');
        for row in &self.rows {
            let line = self.render_row(row, widths, "");
            out.push_str(line.trim());
            out.push('\n');
        }
        out
    }

    fn make_separator_ascii(widths: &[usize]) -> String {
        let dashes: Vec<String> = widths.iter().map(|w| "-".repeat(*w + 2)).collect();
        format!("+{}+", dashes.join("+"))
    }

    fn render_ascii(&self, widths: &[usize]) -> String {
        let mut out = String::new();
        let sep = Self::make_separator_ascii(widths);
        out.push_str(&sep);
        out.push('\n');
        let header_cells: Vec<String> = self.columns.iter().map(|c| c.header.clone()).collect();
        out.push_str(&self.render_row(&header_cells, widths, "|"));
        out.push('\n');
        if self.header_separator {
            out.push_str(&sep);
            out.push('\n');
        }
        for row in &self.rows {
            out.push_str(&self.render_row(row, widths, "|"));
            out.push('\n');
        }
        out.push_str(&sep);
        out.push('\n');
        out
    }

    fn render_unicode(&self, widths: &[usize]) -> String {
        let top: Vec<String> = widths.iter().map(|w| "─".repeat(*w + 2)).collect();
        let mid: Vec<String> = widths.iter().map(|w| "─".repeat(*w + 2)).collect();
        let bot: Vec<String> = widths.iter().map(|w| "─".repeat(*w + 2)).collect();
        let top_line = format!("┌{}┐", top.join("┬"));
        let mid_line = format!("├{}┤", mid.join("┼"));
        let bot_line = format!("└{}┘", bot.join("┴"));

        let mut out = String::new();
        out.push_str(&top_line);
        out.push('\n');
        let header_cells: Vec<String> = self.columns.iter().map(|c| c.header.clone()).collect();
        out.push_str(&self.render_row(&header_cells, widths, "│"));
        out.push('\n');
        if self.header_separator {
            out.push_str(&mid_line);
            out.push('\n');
        }
        for row in &self.rows {
            out.push_str(&self.render_row(row, widths, "│"));
            out.push('\n');
        }
        out.push_str(&bot_line);
        out.push('\n');
        out
    }

    fn render_markdown(&self, widths: &[usize]) -> String {
        let mut out = String::new();
        let header_cells: Vec<String> = self.columns.iter().map(|c| c.header.clone()).collect();
        out.push_str(&self.render_row(&header_cells, widths, "|"));
        out.push('\n');

        // Separator
        let sep_cells: Vec<String> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, col)| {
                let w = widths[i];
                match col.alignment {
                    Alignment::Left => format!(":{}", "-".repeat(w.saturating_sub(1).max(2))),
                    Alignment::Right => format!("{}:", "-".repeat(w.saturating_sub(1).max(2))),
                    Alignment::Center => format!(":{}:", "-".repeat(w.saturating_sub(2).max(1))),
                }
            })
            .collect();
        out.push_str(&format!("| {} |", sep_cells.join(" | ")));
        out.push('\n');

        for row in &self.rows {
            out.push_str(&self.render_row(row, widths, "|"));
            out.push('\n');
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Text utilities
// ---------------------------------------------------------------------------

/// Wrap text at the given width, breaking at word boundaries.
pub fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for input_line in text.lines() {
        let words: Vec<&str> = input_line.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in words {
            if current.is_empty() {
                current = word.to_string();
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Truncate text to `max_len` characters, adding `…` if truncated.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        format!("{}…", &text[..max_len - 1])
    }
}

/// Pad text to a fixed width with the given alignment.
pub fn pad(text: &str, width: usize, alignment: Alignment) -> String {
    if text.len() >= width {
        return text.to_string();
    }
    let padding = width - text.len();
    match alignment {
        Alignment::Left => format!("{text}{}", " ".repeat(padding)),
        Alignment::Right => format!("{}{text}", " ".repeat(padding)),
        Alignment::Center => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{text}{}", " ".repeat(left), " ".repeat(right))
        }
    }
}

/// Normalize indentation: convert tabs to spaces, optionally re-indent to a
/// fixed level. `tab_width` controls tab-to-space conversion. If `target_indent`
/// is `Some(n)`, all lines are re-indented to `n` spaces relative to the
/// minimum indentation found.
pub fn normalize_indent(text: &str, tab_width: usize, target_indent: Option<usize>) -> String {
    // Convert tabs to spaces
    let tab_spaces = " ".repeat(tab_width);
    let lines_expanded: Vec<String> = text.lines().map(|l| l.replace('\t', &tab_spaces)).collect();

    if let Some(target) = target_indent {
        // Find minimum indent (ignoring blank lines)
        let min_indent = lines_expanded
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        let indent_str = " ".repeat(target);
        lines_expanded
            .iter()
            .map(|l| {
                if l.trim().is_empty() {
                    String::new()
                } else {
                    let stripped = &l[min_indent.min(l.len())..];
                    format!(
                        "{indent_str}{}",
                        stripped
                            .trim_start_matches(' ')
                            .to_string()
                            .chars()
                            .collect::<String>()
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        lines_expanded.join("\n")
    }
}

/// Render a horizontal bar of a given character and width.
pub fn horizontal_rule(ch: char, width: usize) -> String {
    std::iter::repeat(ch).take(width).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_ascii() {
        let mut table = Table::new(&["Name", "Age"]);
        table.add_row(vec!["Alice".into(), "30".into()]);
        table.add_row(vec!["Bob".into(), "25".into()]);
        let out = table.render();
        assert!(out.contains("+"));
        assert!(out.contains("| Alice"));
        assert!(out.contains("| Bob"));
    }

    #[test]
    fn test_table_unicode() {
        let mut table = Table::new(&["X", "Y"]).with_border(BorderStyle::Unicode);
        table.add_row(vec!["1".into(), "2".into()]);
        let out = table.render();
        assert!(out.contains("┌"));
        assert!(out.contains("│"));
        assert!(out.contains("└"));
    }

    #[test]
    fn test_table_markdown() {
        let mut table = Table::new(&["Col1", "Col2"]).with_border(BorderStyle::Markdown);
        table.add_row(vec!["a".into(), "b".into()]);
        let out = table.render();
        assert!(out.contains("|"));
        assert!(out.contains("---"));
    }

    #[test]
    fn test_table_no_border() {
        let mut table = Table::new(&["H1", "H2"]).with_border(BorderStyle::None);
        table.add_row(vec!["x".into(), "y".into()]);
        let out = table.render();
        assert!(out.contains("H1"));
        assert!(!out.contains("+"));
    }

    #[test]
    fn test_alignment() {
        let col = ColumnDef::new("Num")
            .with_alignment(Alignment::Right)
            .with_width(6, 6);
        let mut table = Table {
            columns: vec![col],
            rows: Vec::new(),
            border_style: BorderStyle::Ascii,
            header_separator: true,
        };
        table.add_row(vec!["42".into()]);
        let out = table.render();
        assert!(out.contains("    42"));
    }

    #[test]
    fn test_center_alignment() {
        let s = pad("hi", 10, Alignment::Center);
        assert_eq!(s.len(), 10);
        assert!(s.starts_with("    ") || s.starts_with("   "));
    }

    #[test]
    fn test_truncation() {
        assert_eq!(truncate("hello world", 5), "hell…");
        assert_eq!(truncate("hi", 5), "hi");
        assert_eq!(truncate("long", 1), "…");
    }

    #[test]
    fn test_word_wrap() {
        let lines = word_wrap("the quick brown fox jumps over the lazy dog", 15);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(l.len() <= 15);
        }
    }

    #[test]
    fn test_word_wrap_empty() {
        let lines = word_wrap("", 10);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn test_pad_left() {
        assert_eq!(pad("hi", 6, Alignment::Left), "hi    ");
    }

    #[test]
    fn test_pad_right() {
        assert_eq!(pad("hi", 6, Alignment::Right), "    hi");
    }

    #[test]
    fn test_normalize_indent_tabs() {
        let input = "\tfoo\n\t\tbar";
        let out = normalize_indent(input, 4, None);
        assert!(out.contains("    foo"));
        assert!(out.contains("        bar"));
    }

    #[test]
    fn test_normalize_indent_target() {
        let input = "    line1\n        line2";
        let out = normalize_indent(input, 4, Some(2));
        for line in out.lines() {
            if !line.trim().is_empty() {
                assert!(line.starts_with("  "));
            }
        }
    }

    #[test]
    fn test_horizontal_rule() {
        assert_eq!(horizontal_rule('-', 5), "-----");
        assert_eq!(horizontal_rule('=', 3), "===");
    }

    #[test]
    fn test_table_column_overflow_truncate() {
        let col = ColumnDef::new("Name")
            .with_width(3, 6)
            .with_overflow(Overflow::Truncate);
        let mut table = Table {
            columns: vec![col],
            rows: Vec::new(),
            border_style: BorderStyle::Ascii,
            header_separator: true,
        };
        table.add_row(vec!["VeryLongName".into()]);
        let out = table.render();
        assert!(out.contains("VeryL…"));
    }

    #[test]
    fn test_empty_table() {
        let table = Table::new(&[]);
        assert_eq!(table.render(), "");
    }
}
