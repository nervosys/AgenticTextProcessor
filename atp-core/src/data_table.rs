//! # In-Memory Columnar Data Table
//!
//! Typed columns (String, Int, Float, Bool), with filter, sort, group-by
//! aggregation, join, and pivot operations.

use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Column types
// ---------------------------------------------------------------------------

/// A single cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Column data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Int,
    Float,
    Bool,
}

/// A named column definition.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub col_type: ColumnType,
}

// ---------------------------------------------------------------------------
// Row & Table
// ---------------------------------------------------------------------------

/// A row is a vector of values ordered by column index.
pub type Row = Vec<Value>;

/// The data table.
#[derive(Debug, Clone)]
pub struct DataTable {
    columns: Vec<Column>,
    rows: Vec<Row>,
}

impl DataTable {
    /// Create an empty table with the given column definitions.
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    /// Create a table from column names (all String type).
    pub fn from_names(names: &[&str]) -> Self {
        let columns = names
            .iter()
            .map(|n| Column {
                name: n.to_string(),
                col_type: ColumnType::String,
            })
            .collect();
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    /// Number of columns.
    pub fn width(&self) -> usize {
        self.columns.len()
    }

    /// Number of rows.
    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// Column definitions.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// All rows.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Column index by name.
    pub fn col_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// Add a row. The row must have the same length as columns.
    pub fn add_row(&mut self, row: Row) -> Result<(), String> {
        if row.len() != self.columns.len() {
            return Err(format!(
                "Row has {} values but table has {} columns",
                row.len(),
                self.columns.len()
            ));
        }
        self.rows.push(row);
        Ok(())
    }

    /// Get a cell value.
    pub fn get(&self, row: usize, col: usize) -> Option<&Value> {
        self.rows.get(row).and_then(|r| r.get(col))
    }

    /// Get a cell by column name.
    pub fn get_by_name(&self, row: usize, col_name: &str) -> Option<&Value> {
        let idx = self.col_index(col_name)?;
        self.get(row, idx)
    }

    // ----- Filter -----

    /// Filter rows where `predicate(row)` returns true.
    pub fn filter<F: Fn(&Row) -> bool>(&self, predicate: F) -> DataTable {
        let rows: Vec<Row> = self.rows.iter().filter(|r| predicate(r)).cloned().collect();
        DataTable {
            columns: self.columns.clone(),
            rows,
        }
    }

    /// Filter by column value equality.
    pub fn filter_eq(&self, col_name: &str, value: &Value) -> DataTable {
        if let Some(idx) = self.col_index(col_name) {
            self.filter(|row| row.get(idx) == Some(value))
        } else {
            self.clone()
        }
    }

    // ----- Sort -----

    /// Sort by column (ascending). Nulls sort first.
    pub fn sort_by(&self, col_name: &str) -> DataTable {
        self.sort_by_dir(col_name, true)
    }

    /// Sort by column with direction.
    pub fn sort_by_dir(&self, col_name: &str, ascending: bool) -> DataTable {
        let idx = match self.col_index(col_name) {
            Some(i) => i,
            None => return self.clone(),
        };
        let mut rows = self.rows.clone();
        rows.sort_by(|a, b| {
            let cmp = cmp_values(
                a.get(idx).unwrap_or(&Value::Null),
                b.get(idx).unwrap_or(&Value::Null),
            );
            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
        DataTable {
            columns: self.columns.clone(),
            rows,
        }
    }

    // ----- Group-By -----

    /// Group rows by a column, computing aggregations.
    pub fn group_by(&self, key_col: &str, agg_col: &str, agg: Aggregation) -> DataTable {
        let key_idx = match self.col_index(key_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };
        let agg_idx = match self.col_index(agg_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };

        let mut groups: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for row in &self.rows {
            let key = row.get(key_idx).map(|v| v.to_string()).unwrap_or_default();
            if let Some(val) = row.get(agg_idx).and_then(|v| v.as_float()) {
                groups.entry(key).or_default().push(val);
            }
        }

        let result_cols = vec![
            Column {
                name: key_col.to_string(),
                col_type: ColumnType::String,
            },
            Column {
                name: format!("{}_{}", agg.as_str(), agg_col),
                col_type: ColumnType::Float,
            },
        ];
        let mut result_rows = Vec::new();
        for (key, vals) in &groups {
            let agg_val = compute_agg(&agg, vals);
            result_rows.push(vec![Value::Str(key.clone()), Value::Float(agg_val)]);
        }

        DataTable {
            columns: result_cols,
            rows: result_rows,
        }
    }

    // ----- Join -----

    /// Inner join with another table on matching column names.
    pub fn inner_join(&self, other: &DataTable, left_col: &str, right_col: &str) -> DataTable {
        let left_idx = match self.col_index(left_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };
        let right_idx = match other.col_index(right_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };

        // Build result columns: all left + right (excluding the join column)
        let mut cols = self.columns.clone();
        for (i, c) in other.columns.iter().enumerate() {
            if i != right_idx {
                cols.push(Column {
                    name: format!("{}_{}", "r", c.name),
                    col_type: c.col_type,
                });
            }
        }

        let mut rows = Vec::new();
        for left_row in &self.rows {
            let left_key = left_row.get(left_idx).unwrap_or(&Value::Null).to_string();
            for right_row in &other.rows {
                let right_key = right_row.get(right_idx).unwrap_or(&Value::Null).to_string();
                if left_key == right_key {
                    let mut combined = left_row.clone();
                    for (i, val) in right_row.iter().enumerate() {
                        if i != right_idx {
                            combined.push(val.clone());
                        }
                    }
                    rows.push(combined);
                }
            }
        }

        DataTable {
            columns: cols,
            rows,
        }
    }

    // ----- Pivot -----

    /// Pivot: rows become columns. `row_col` = row key, `pivot_col` = new column
    /// names, `value_col` = cell values.
    pub fn pivot(&self, row_col: &str, pivot_col: &str, value_col: &str) -> DataTable {
        let row_idx = match self.col_index(row_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };
        let piv_idx = match self.col_index(pivot_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };
        let val_idx = match self.col_index(value_col) {
            Some(i) => i,
            None => return DataTable::new(vec![]),
        };

        // Collect unique pivot values (for column headers)
        let mut pivot_values: Vec<String> = Vec::new();
        for row in &self.rows {
            let pv = row.get(piv_idx).map(|v| v.to_string()).unwrap_or_default();
            if !pivot_values.contains(&pv) {
                pivot_values.push(pv);
            }
        }

        // Build result columns
        let mut cols = vec![Column {
            name: row_col.to_string(),
            col_type: ColumnType::String,
        }];
        for pv in &pivot_values {
            cols.push(Column {
                name: pv.clone(),
                col_type: ColumnType::String,
            });
        }

        // Group data
        let mut data: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        for row in &self.rows {
            let rk = row.get(row_idx).map(|v| v.to_string()).unwrap_or_default();
            let pk = row.get(piv_idx).map(|v| v.to_string()).unwrap_or_default();
            let val = row.get(val_idx).cloned().unwrap_or(Value::Null);
            data.entry(rk).or_default().insert(pk, val);
        }

        let mut result_rows = Vec::new();
        for (rk, values) in &data {
            let mut r = vec![Value::Str(rk.clone())];
            for pv in &pivot_values {
                r.push(values.get(pv).cloned().unwrap_or(Value::Null));
            }
            result_rows.push(r);
        }

        DataTable {
            columns: cols,
            rows: result_rows,
        }
    }

    // ----- Utilities -----

    /// Select specific columns.
    pub fn select(&self, col_names: &[&str]) -> DataTable {
        let indices: Vec<usize> = col_names.iter().filter_map(|n| self.col_index(n)).collect();
        let columns: Vec<Column> = indices.iter().map(|&i| self.columns[i].clone()).collect();
        let rows: Vec<Row> = self
            .rows
            .iter()
            .map(|row| {
                indices
                    .iter()
                    .map(|&i| row.get(i).cloned().unwrap_or(Value::Null))
                    .collect()
            })
            .collect();
        DataTable { columns, rows }
    }

    /// Render as CSV text.
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        // Header
        let headers: Vec<&str> = self.columns.iter().map(|c| c.name.as_str()).collect();
        out.push_str(&headers.join(","));
        out.push('\n');
        // Rows
        for row in &self.rows {
            let vals: Vec<String> = row.iter().map(|v| v.to_string()).collect();
            out.push_str(&vals.join(","));
            out.push('\n');
        }
        out
    }

    /// Parse from CSV text (all columns are String type).
    pub fn from_csv(csv: &str) -> Result<DataTable, String> {
        let mut lines = csv.lines();
        let header = lines.next().ok_or("Empty CSV")?;
        let col_names: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
        let columns: Vec<Column> = col_names
            .iter()
            .map(|n| Column {
                name: n.to_string(),
                col_type: ColumnType::String,
            })
            .collect();
        let mut rows = Vec::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let vals: Vec<Value> = line
                .split(',')
                .map(|s| Value::Str(s.trim().to_string()))
                .collect();
            if vals.len() != columns.len() {
                return Err(format!(
                    "Row has {} values, expected {}",
                    vals.len(),
                    columns.len()
                ));
            }
            rows.push(vals);
        }
        Ok(DataTable { columns, rows })
    }
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Aggregation function for group-by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

impl Aggregation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Aggregation::Sum => "sum",
            Aggregation::Avg => "avg",
            Aggregation::Min => "min",
            Aggregation::Max => "max",
            Aggregation::Count => "count",
        }
    }
}

fn compute_agg(agg: &Aggregation, vals: &[f64]) -> f64 {
    if vals.is_empty() {
        return 0.0;
    }
    match agg {
        Aggregation::Sum => vals.iter().sum(),
        Aggregation::Avg => vals.iter().sum::<f64>() / vals.len() as f64,
        Aggregation::Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
        Aggregation::Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Aggregation::Count => vals.len() as f64,
    }
}

fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Int(x), Value::Float(y)) => (*x as f64)
            .partial_cmp(y)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => x
            .partial_cmp(&(*y as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        _ => a.to_string().cmp(&b.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_table() -> DataTable {
        let mut t = DataTable::new(vec![
            Column {
                name: "name".into(),
                col_type: ColumnType::String,
            },
            Column {
                name: "dept".into(),
                col_type: ColumnType::String,
            },
            Column {
                name: "salary".into(),
                col_type: ColumnType::Int,
            },
        ]);
        t.add_row(vec![
            Value::Str("Alice".into()),
            Value::Str("Eng".into()),
            Value::Int(100),
        ])
        .unwrap();
        t.add_row(vec![
            Value::Str("Bob".into()),
            Value::Str("Sales".into()),
            Value::Int(80),
        ])
        .unwrap();
        t.add_row(vec![
            Value::Str("Carol".into()),
            Value::Str("Eng".into()),
            Value::Int(120),
        ])
        .unwrap();
        t.add_row(vec![
            Value::Str("Dave".into()),
            Value::Str("Sales".into()),
            Value::Int(90),
        ])
        .unwrap();
        t
    }

    #[test]
    fn test_dimensions() {
        let t = sample_table();
        assert_eq!(t.width(), 3);
        assert_eq!(t.height(), 4);
    }

    #[test]
    fn test_get_cell() {
        let t = sample_table();
        assert_eq!(t.get(0, 0), Some(&Value::Str("Alice".into())));
        assert_eq!(t.get_by_name(1, "salary"), Some(&Value::Int(80)));
    }

    #[test]
    fn test_filter_eq() {
        let t = sample_table();
        let eng = t.filter_eq("dept", &Value::Str("Eng".into()));
        assert_eq!(eng.height(), 2);
    }

    #[test]
    fn test_filter_closure() {
        let t = sample_table();
        let high = t.filter(|row| row[2].as_int().unwrap_or(0) > 90);
        assert_eq!(high.height(), 2);
    }

    #[test]
    fn test_sort_ascending() {
        let t = sample_table();
        let sorted = t.sort_by("salary");
        assert_eq!(sorted.get(0, 2), Some(&Value::Int(80)));
        assert_eq!(sorted.get(3, 2), Some(&Value::Int(120)));
    }

    #[test]
    fn test_sort_descending() {
        let t = sample_table();
        let sorted = t.sort_by_dir("salary", false);
        assert_eq!(sorted.get(0, 2), Some(&Value::Int(120)));
    }

    #[test]
    fn test_group_by_sum() {
        let t = sample_table();
        let result = t.group_by("dept", "salary", Aggregation::Sum);
        assert_eq!(result.height(), 2);
        // Eng = 100 + 120 = 220, Sales = 80 + 90 = 170
        let eng_row = result.filter_eq("dept", &Value::Str("Eng".into()));
        assert_eq!(eng_row.get(0, 1), Some(&Value::Float(220.0)));
    }

    #[test]
    fn test_group_by_avg() {
        let t = sample_table();
        let result = t.group_by("dept", "salary", Aggregation::Avg);
        let sales = result.filter_eq("dept", &Value::Str("Sales".into()));
        assert_eq!(sales.get(0, 1), Some(&Value::Float(85.0)));
    }

    #[test]
    fn test_inner_join() {
        let mut dept_info = DataTable::from_names(&["dept", "location"]);
        dept_info
            .add_row(vec![Value::Str("Eng".into()), Value::Str("SF".into())])
            .unwrap();
        dept_info
            .add_row(vec![Value::Str("Sales".into()), Value::Str("NYC".into())])
            .unwrap();

        let t = sample_table();
        let joined = t.inner_join(&dept_info, "dept", "dept");
        assert_eq!(joined.height(), 4);
        assert!(joined.width() > t.width());
    }

    #[test]
    fn test_pivot() {
        let mut t = DataTable::from_names(&["quarter", "product", "revenue"]);
        t.add_row(vec![
            Value::Str("Q1".into()),
            Value::Str("A".into()),
            Value::Str("100".into()),
        ])
        .unwrap();
        t.add_row(vec![
            Value::Str("Q1".into()),
            Value::Str("B".into()),
            Value::Str("200".into()),
        ])
        .unwrap();
        t.add_row(vec![
            Value::Str("Q2".into()),
            Value::Str("A".into()),
            Value::Str("150".into()),
        ])
        .unwrap();
        let pivoted = t.pivot("quarter", "product", "revenue");
        assert_eq!(pivoted.height(), 2); // Q1, Q2
        assert_eq!(pivoted.width(), 3); // quarter, A, B
    }

    #[test]
    fn test_select() {
        let t = sample_table();
        let selected = t.select(&["name", "salary"]);
        assert_eq!(selected.width(), 2);
        assert_eq!(selected.height(), 4);
    }

    #[test]
    fn test_csv_roundtrip() {
        let t = sample_table();
        let csv = t.to_csv();
        assert!(csv.contains("name,dept,salary"));
        let parsed = DataTable::from_csv(&csv).unwrap();
        assert_eq!(parsed.height(), 4);
        assert_eq!(parsed.width(), 3);
    }

    #[test]
    fn test_add_row_wrong_size() {
        let mut t = DataTable::from_names(&["a", "b"]);
        assert!(t.add_row(vec![Value::Str("x".into())]).is_err());
    }

    #[test]
    fn test_value_display() {
        assert_eq!(Value::Null.to_string(), "NULL");
        assert_eq!(Value::Int(42).to_string(), "42");
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Float(3.14).to_string(), "3.14");
    }
}
