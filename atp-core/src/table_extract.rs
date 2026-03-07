// ---------------------------------------------------------------------------
// table_extract.rs — Table extraction from text
// ---------------------------------------------------------------------------
//
// Detect & parse ASCII tables, Markdown tables, and fixed-width column data
// into structured output.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single row of cell values.
pub type Row = Vec<String>;

/// A parsed table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Column headers, if detected.
    pub headers: Vec<String>,
    /// Data rows.
    pub rows: Vec<Row>,
}

impl Table {
    /// Number of columns (from headers, or first row).
    pub fn num_columns(&self) -> usize {
        if !self.headers.is_empty() {
            self.headers.len()
        } else {
            self.rows.first().map_or(0, |r| r.len())
        }
    }

    /// Number of data rows.
    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }

    /// Get a column by header name.
    pub fn column(&self, name: &str) -> Vec<String> {
        if let Some(idx) = self.headers.iter().position(|h| h == name) {
            self.rows
                .iter()
                .filter_map(|r| r.get(idx).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Render back as a Markdown table.
    pub fn to_markdown(&self) -> String {
        let ncols = self.num_columns();
        if ncols == 0 {
            return String::new();
        }
        let mut lines = Vec::new();
        if !self.headers.is_empty() {
            lines.push(format!("| {} |", self.headers.join(" | ")));
            lines.push(format!("| {} |", vec!["---"; ncols].join(" | ")));
        }
        for row in &self.rows {
            let mut cells: Vec<String> = row.clone();
            cells.resize(ncols, String::new());
            lines.push(format!("| {} |", cells.join(" | ")));
        }
        lines.join("\n")
    }
}

/// Which kind of table format was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableFormat {
    Markdown,
    Ascii,
    FixedWidth,
}

/// Result of table detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedTable {
    pub table: Table,
    pub format: TableFormat,
    /// Line number (0-based) where the table starts.
    pub start_line: usize,
    /// Line number (0-based) where the table ends.
    pub end_line: usize,
}

// ---------------------------------------------------------------------------
// Markdown table parsing
// ---------------------------------------------------------------------------

/// Parse a Markdown table.
pub fn parse_markdown(text: &str) -> Option<Table> {
    let lines: Vec<&str> = text.lines().collect();
    // Find first pipe-delimited line
    let first_pipe = lines.iter().position(|l| l.contains('|'))?;
    let header_line = lines[first_pipe];
    // Check next line for separator (---+)
    let sep_idx = first_pipe + 1;
    if sep_idx >= lines.len() {
        return None;
    }
    let sep_line = lines[sep_idx];
    if !is_markdown_separator(sep_line) {
        return None;
    }
    let headers = split_pipe_row(header_line);
    let mut rows = Vec::new();
    for line in &lines[sep_idx + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('|') {
            break;
        }
        rows.push(split_pipe_row(line));
    }
    Some(Table { headers, rows })
}

fn is_markdown_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }
    let stripped = trimmed.replace(['|', '-', ':', ' '], "");
    stripped.is_empty()
}

fn split_pipe_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let stripped = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let stripped = stripped.strip_suffix('|').unwrap_or(stripped);
    stripped.split('|').map(|s| s.trim().to_string()).collect()
}

// ---------------------------------------------------------------------------
// ASCII box table parsing
// ---------------------------------------------------------------------------

/// Parse an ASCII box-drawing table (+---------+).
pub fn parse_ascii(text: &str) -> Option<Table> {
    let lines: Vec<&str> = text.lines().collect();
    // Find horizontal rule lines
    let rule_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| if is_ascii_rule(l) { Some(i) } else { None })
        .collect();
    if rule_indices.len() < 2 {
        return None;
    }
    // Content lines between rules
    let mut sections: Vec<Vec<Vec<String>>> = Vec::new();
    for window in rule_indices.windows(2) {
        let start = window[0] + 1;
        let end = window[1];
        let mut section_rows = Vec::new();
        for &line in &lines[start..end] {
            let cells = split_ascii_row(line);
            if !cells.is_empty() {
                section_rows.push(cells);
            }
        }
        if !section_rows.is_empty() {
            sections.push(section_rows);
        }
    }
    if sections.is_empty() {
        return None;
    }
    let (headers, rows) = if sections.len() >= 2 {
        let h = sections[0].first().cloned().unwrap_or_default();
        let r: Vec<Row> = sections[1..].iter().flat_map(|s| s.clone()).collect();
        (h, r)
    } else {
        (Vec::new(), sections[0].clone())
    };
    Some(Table { headers, rows })
}

fn is_ascii_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    trimmed.starts_with('+')
        && trimmed.ends_with('+')
        && trimmed
            .chars()
            .all(|c| c == '+' || c == '-' || c == '=' || c == ' ')
}

fn split_ascii_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return Vec::new();
    }
    let stripped = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let stripped = stripped.strip_suffix('|').unwrap_or(stripped);
    stripped.split('|').map(|s| s.trim().to_string()).collect()
}

// ---------------------------------------------------------------------------
// Fixed-width column parsing
// ---------------------------------------------------------------------------

/// Parse fixed-width columnar text using column boundary positions.
pub fn parse_fixed_width(text: &str, boundaries: &[usize]) -> Table {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() || boundaries.is_empty() {
        return Table {
            headers: Vec::new(),
            rows: Vec::new(),
        };
    }
    let extract_cells = |line: &str| -> Vec<String> {
        let mut cells = Vec::new();
        let mut prev = 0;
        for &b in boundaries {
            let start = prev.min(line.len());
            let end = b.min(line.len());
            cells.push(line.get(start..end).unwrap_or("").trim().to_string());
            prev = b;
        }
        // Last column
        if prev < line.len() {
            cells.push(line[prev..].trim().to_string());
        }
        cells
    };
    let headers = extract_cells(lines[0]);
    let rows: Vec<Row> = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| extract_cells(l))
        .collect();
    Table { headers, rows }
}

/// Detect column boundaries from fixed-width text by finding columns of spaces.
pub fn detect_boundaries(text: &str) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let max_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    if max_len == 0 {
        return Vec::new();
    }
    // A position is a boundary if it is a space in ALL lines
    let mut is_space = vec![true; max_len];
    for line in &lines {
        let bytes = line.as_bytes();
        for (i, flag) in is_space.iter_mut().enumerate() {
            if i < bytes.len() && bytes[i] != b' ' {
                    *flag = false;
                }
        }
    }
    // Find transitions from space to non-space
    let mut boundaries = Vec::new();
    let mut in_gap = false;
    for (i, &s) in is_space.iter().enumerate() {
        if s && !in_gap {
            in_gap = true;
        } else if !s && in_gap {
            boundaries.push(i);
            in_gap = false;
        }
    }
    boundaries
}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// Detect all tables in a text body.
pub fn detect_tables(text: &str) -> Vec<DetectedTable> {
    let mut results = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    // Try Markdown tables
    let mut i = 0;
    while i < lines.len() {
        if lines[i].contains('|') && i + 1 < lines.len() && is_markdown_separator(lines[i + 1]) {
            let start = i;
            let mut end = i + 2;
            while end < lines.len() && lines[end].contains('|') && !lines[end].trim().is_empty() {
                end += 1;
            }
            let block = lines[start..end].join("\n");
            if let Some(table) = parse_markdown(&block) {
                results.push(DetectedTable {
                    table,
                    format: TableFormat::Markdown,
                    start_line: start,
                    end_line: end - 1,
                });
            }
            i = end;
            continue;
        }
        i += 1;
    }
    // Try ASCII tables
    let mut j = 0;
    while j < lines.len() {
        if is_ascii_rule(lines[j]) {
            let start = j;
            let mut end = j + 1;
            while end < lines.len() {
                if is_ascii_rule(lines[end]) {
                    // Check if there's more after
                    let next = end + 1;
                    if next < lines.len()
                        && (lines[next].trim().starts_with('|') || is_ascii_rule(lines[next]))
                    {
                        end = next;
                    } else {
                        end += 1;
                        break;
                    }
                } else {
                    end += 1;
                }
            }
            let block = lines[start..end].join("\n");
            if let Some(table) = parse_ascii(&block) {
                results.push(DetectedTable {
                    table,
                    format: TableFormat::Ascii,
                    start_line: start,
                    end_line: end - 1,
                });
            }
            j = end;
            continue;
        }
        j += 1;
    }
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_basic() {
        let text = "| Name | Age |\n| --- | --- |\n| Alice | 30 |\n| Bob | 25 |";
        let t = parse_markdown(text).unwrap();
        assert_eq!(t.headers, vec!["Name", "Age"]);
        assert_eq!(t.num_rows(), 2);
    }

    #[test]
    fn test_markdown_column_access() {
        let text = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let t = parse_markdown(text).unwrap();
        assert_eq!(t.column("A"), vec!["1", "3"]);
        assert_eq!(t.column("B"), vec!["2", "4"]);
    }

    #[test]
    fn test_to_markdown() {
        let t = Table {
            headers: vec!["X".into(), "Y".into()],
            rows: vec![vec!["1".into(), "2".into()]],
        };
        let md = t.to_markdown();
        assert!(md.contains("| X | Y |"));
        assert!(md.contains("| 1 | 2 |"));
    }

    #[test]
    fn test_parse_ascii_basic() {
        let text = "+------+-----+\n| Name | Age |\n+------+-----+\n| Alice | 30 |\n+------+-----+";
        let t = parse_ascii(text).unwrap();
        assert_eq!(t.headers, vec!["Name", "Age"]);
        assert_eq!(t.num_rows(), 1);
    }

    #[test]
    fn test_parse_ascii_no_header() {
        let text = "+---+---+\n| a | b |\n| c | d |\n+---+---+";
        let t = parse_ascii(text).unwrap();
        assert_eq!(t.num_rows(), 2);
    }

    #[test]
    fn test_fixed_width() {
        let text = "Name    Age  City\nAlice   30   NYC\nBob     25   LA";
        let bounds = vec![8, 13];
        let t = parse_fixed_width(text, &bounds);
        assert_eq!(t.headers, vec!["Name", "Age", "City"]);
        assert_eq!(t.num_rows(), 2);
    }

    #[test]
    fn test_detect_boundaries() {
        let text = "Name    Age  City\nAlice   30   NYC\nBobby   25   LA";
        let b = detect_boundaries(text);
        assert!(!b.is_empty());
    }

    #[test]
    fn test_detect_markdown_table() {
        let text = "Some text\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nMore text";
        let tables = detect_tables(text);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].format, TableFormat::Markdown);
    }

    #[test]
    fn test_detect_ascii_table() {
        let text = "Before\n+---+---+\n| a | b |\n+---+---+\n| 1 | 2 |\n+---+---+\nAfter";
        let tables = detect_tables(text);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].format, TableFormat::Ascii);
    }

    #[test]
    fn test_num_columns() {
        let t = Table {
            headers: vec!["A".into(), "B".into(), "C".into()],
            rows: vec![vec!["1".into(), "2".into(), "3".into()]],
        };
        assert_eq!(t.num_columns(), 3);
    }

    #[test]
    fn test_num_columns_no_header() {
        let t = Table {
            headers: Vec::new(),
            rows: vec![vec!["1".into(), "2".into()]],
        };
        assert_eq!(t.num_columns(), 2);
    }

    #[test]
    fn test_empty_table() {
        let t = Table {
            headers: Vec::new(),
            rows: Vec::new(),
        };
        assert_eq!(t.num_columns(), 0);
        assert_eq!(t.num_rows(), 0);
        assert_eq!(t.to_markdown(), "");
    }

    #[test]
    fn test_split_pipe_row() {
        assert_eq!(split_pipe_row("| a | b | c |"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_column_missing() {
        let t = Table {
            headers: vec!["X".into()],
            rows: vec![vec!["1".into()]],
        };
        assert_eq!(t.column("Z"), Vec::<String>::new());
    }

    #[test]
    fn test_fixed_width_empty() {
        let t = parse_fixed_width("", &[5, 10]);
        assert_eq!(t.num_rows(), 0);
    }

    #[test]
    fn test_no_tables_detected() {
        let tables = detect_tables("Just plain text\nwith no tables.");
        assert!(tables.is_empty());
    }
}
