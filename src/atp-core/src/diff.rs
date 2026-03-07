//! Structured diff engine for ATP outputs.
//!
//! Computes semantic diffs between two ATP outputs (search results, pipeline
//! results, config states, etc.). Supports JSON patch (RFC 6902), unified diff,
//! and side-by-side comparison. Useful for comparing two pipeline runs or
//! detecting regressions in query outputs.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single diff operation (RFC 6902 JSON Patch style).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffOp {
    /// Operation kind: "add", "remove", "replace"
    pub op: DiffOpKind,
    /// JSON pointer path to the changed value
    pub path: String,
    /// The old value (for "remove" and "replace")
    pub old_value: Option<Value>,
    /// The new value (for "add" and "replace")
    pub new_value: Option<Value>,
}

/// Kind of diff operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiffOpKind {
    Add,
    Remove,
    Replace,
}

/// Summary of differences between two values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    /// List of diff operations.
    pub ops: Vec<DiffOp>,
    /// Number of additions.
    pub additions: usize,
    /// Number of removals.
    pub removals: usize,
    /// Number of replacements.
    pub replacements: usize,
    /// Whether the two values are identical.
    pub identical: bool,
}

/// A line in a unified diff output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifiedLine {
    /// Line present in both.
    Context(String),
    /// Line only in the left (removed).
    Removed(String),
    /// Line only in the right (added).
    Added(String),
}

/// Compute a structured diff between two JSON values.
pub fn diff_json(left: &Value, right: &Value) -> DiffResult {
    let mut ops = Vec::new();
    diff_recursive(left, right, String::new(), &mut ops);

    let additions = ops.iter().filter(|o| o.op == DiffOpKind::Add).count();
    let removals = ops.iter().filter(|o| o.op == DiffOpKind::Remove).count();
    let replacements = ops.iter().filter(|o| o.op == DiffOpKind::Replace).count();
    let identical = ops.is_empty();

    DiffResult {
        ops,
        additions,
        removals,
        replacements,
        identical,
    }
}

fn diff_recursive(left: &Value, right: &Value, path: String, ops: &mut Vec<DiffOp>) {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            // Check removed and changed keys
            for (key, lv) in l {
                let child_path = format!("{}/{}", path, key);
                match r.get(key) {
                    Some(rv) => diff_recursive(lv, rv, child_path, ops),
                    None => ops.push(DiffOp {
                        op: DiffOpKind::Remove,
                        path: child_path,
                        old_value: Some(lv.clone()),
                        new_value: None,
                    }),
                }
            }
            // Check added keys
            for (key, rv) in r {
                if !l.contains_key(key) {
                    ops.push(DiffOp {
                        op: DiffOpKind::Add,
                        path: format!("{}/{}", path, key),
                        old_value: None,
                        new_value: Some(rv.clone()),
                    });
                }
            }
        }
        (Value::Array(l), Value::Array(r)) => {
            let max_len = l.len().max(r.len());
            for i in 0..max_len {
                let child_path = format!("{}/{}", path, i);
                match (l.get(i), r.get(i)) {
                    (Some(lv), Some(rv)) => diff_recursive(lv, rv, child_path, ops),
                    (Some(lv), None) => ops.push(DiffOp {
                        op: DiffOpKind::Remove,
                        path: child_path,
                        old_value: Some(lv.clone()),
                        new_value: None,
                    }),
                    (None, Some(rv)) => ops.push(DiffOp {
                        op: DiffOpKind::Add,
                        path: child_path,
                        old_value: None,
                        new_value: Some(rv.clone()),
                    }),
                    (None, None) => {}
                }
            }
        }
        _ => {
            if left != right {
                ops.push(DiffOp {
                    op: DiffOpKind::Replace,
                    path,
                    old_value: Some(left.clone()),
                    new_value: Some(right.clone()),
                });
            }
        }
    }
}

/// Compute a unified text diff between two string slices.
pub fn diff_text(left: &str, right: &str) -> Vec<UnifiedLine> {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    // Simple LCS-based diff
    let lcs = compute_lcs(&left_lines, &right_lines);
    let mut result = Vec::new();
    let mut li = 0;
    let mut ri = 0;

    for &(ll, rl) in &lcs {
        // Emit removed lines before this LCS match
        while li < ll {
            result.push(UnifiedLine::Removed(left_lines[li].to_string()));
            li += 1;
        }
        // Emit added lines before this LCS match
        while ri < rl {
            result.push(UnifiedLine::Added(right_lines[ri].to_string()));
            ri += 1;
        }
        // Emit context (matching) line
        result.push(UnifiedLine::Context(left_lines[li].to_string()));
        li += 1;
        ri += 1;
    }

    // Remaining lines
    while li < left_lines.len() {
        result.push(UnifiedLine::Removed(left_lines[li].to_string()));
        li += 1;
    }
    while ri < right_lines.len() {
        result.push(UnifiedLine::Added(right_lines[ri].to_string()));
        ri += 1;
    }

    result
}

/// Compute Longest Common Subsequence indices.
fn compute_lcs<'a>(left: &[&'a str], right: &[&'a str]) -> Vec<(usize, usize)> {
    let m = left.len();
    let n = right.len();
    let mut dp = vec![vec![0u32; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if left[i - 1] == right[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find LCS indices
    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if left[i - 1] == right[j - 1] {
            result.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

/// Format unified diff lines as a string.
pub fn format_unified(lines: &[UnifiedLine]) -> String {
    let mut out = String::new();
    for line in lines {
        match line {
            UnifiedLine::Context(s) => {
                out.push_str("  ");
                out.push_str(s);
                out.push('\n');
            }
            UnifiedLine::Removed(s) => {
                out.push_str("- ");
                out.push_str(s);
                out.push('\n');
            }
            UnifiedLine::Added(s) => {
                out.push_str("+ ");
                out.push_str(s);
                out.push('\n');
            }
        }
    }
    out
}

/// Apply a JSON diff (patch) to a value, producing the patched result.
pub fn apply_patch(value: &Value, ops: &[DiffOp]) -> Value {
    let mut result = value.clone();
    for op in ops {
        match &op.op {
            DiffOpKind::Add | DiffOpKind::Replace => {
                if let Some(new_val) = &op.new_value {
                    set_at_path(&mut result, &op.path, new_val.clone());
                }
            }
            DiffOpKind::Remove => {
                remove_at_path(&mut result, &op.path);
            }
        }
    }
    result
}

fn set_at_path(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        *root = value;
        return;
    }

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last segment — set the value
            match current {
                Value::Object(map) => {
                    map.insert(part.to_string(), value);
                    return;
                }
                Value::Array(arr) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        if idx < arr.len() {
                            arr[idx] = value;
                        } else {
                            arr.push(value);
                        }
                    }
                    return;
                }
                _ => return,
            }
        } else {
            current = match current {
                Value::Object(map) => map
                    .entry(part.to_string())
                    .or_insert_with(|| Value::Object(serde_json::Map::new())),
                Value::Array(arr) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        if idx < arr.len() {
                            &mut arr[idx]
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                }
                _ => return,
            };
        }
    }
}

fn remove_at_path(root: &mut Value, path: &str) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            match current {
                Value::Object(map) => {
                    map.remove(*part);
                }
                Value::Array(arr) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        if idx < arr.len() {
                            arr.remove(idx);
                        }
                    }
                }
                _ => {}
            }
            return;
        } else {
            current = match current {
                Value::Object(map) => {
                    if let Some(v) = map.get_mut(*part) {
                        v
                    } else {
                        return;
                    }
                }
                Value::Array(arr) => {
                    if let Ok(idx) = part.parse::<usize>() {
                        if idx < arr.len() {
                            &mut arr[idx]
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                }
                _ => return,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diff_identical() {
        let v = json!({"a": 1, "b": [1, 2, 3]});
        let result = diff_json(&v, &v);
        assert!(result.identical);
        assert_eq!(result.ops.len(), 0);
    }

    #[test]
    fn test_diff_add_key() {
        let left = json!({"a": 1});
        let right = json!({"a": 1, "b": 2});
        let result = diff_json(&left, &right);
        assert!(!result.identical);
        assert_eq!(result.additions, 1);
        assert_eq!(result.ops[0].op, DiffOpKind::Add);
        assert_eq!(result.ops[0].path, "/b");
    }

    #[test]
    fn test_diff_remove_key() {
        let left = json!({"a": 1, "b": 2});
        let right = json!({"a": 1});
        let result = diff_json(&left, &right);
        assert_eq!(result.removals, 1);
        assert_eq!(result.ops[0].op, DiffOpKind::Remove);
    }

    #[test]
    fn test_diff_replace_value() {
        let left = json!({"a": 1});
        let right = json!({"a": 2});
        let result = diff_json(&left, &right);
        assert_eq!(result.replacements, 1);
        assert_eq!(result.ops[0].op, DiffOpKind::Replace);
        assert_eq!(result.ops[0].old_value, Some(json!(1)));
        assert_eq!(result.ops[0].new_value, Some(json!(2)));
    }

    #[test]
    fn test_diff_array() {
        let left = json!([1, 2, 3]);
        let right = json!([1, 2, 4]);
        let result = diff_json(&left, &right);
        assert_eq!(result.replacements, 1);
        assert_eq!(result.ops[0].path, "/2");
    }

    #[test]
    fn test_diff_array_length_change() {
        let left = json!([1, 2]);
        let right = json!([1, 2, 3]);
        let result = diff_json(&left, &right);
        assert_eq!(result.additions, 1);
    }

    #[test]
    fn test_diff_nested() {
        let left = json!({"a": {"b": 1}});
        let right = json!({"a": {"b": 2}});
        let result = diff_json(&left, &right);
        assert_eq!(result.ops[0].path, "/a/b");
    }

    #[test]
    fn test_text_diff_identical() {
        let lines = diff_text("hello\nworld", "hello\nworld");
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| matches!(l, UnifiedLine::Context(_))));
    }

    #[test]
    fn test_text_diff_addition() {
        let lines = diff_text("hello", "hello\nworld");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], UnifiedLine::Context("hello".into()));
        assert_eq!(lines[1], UnifiedLine::Added("world".into()));
    }

    #[test]
    fn test_text_diff_removal() {
        let lines = diff_text("hello\nworld", "hello");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], UnifiedLine::Context("hello".into()));
        assert_eq!(lines[1], UnifiedLine::Removed("world".into()));
    }

    #[test]
    fn test_text_diff_replacement() {
        let lines = diff_text("hello\nfoo", "hello\nbar");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], UnifiedLine::Context("hello".into()));
        assert_eq!(lines[1], UnifiedLine::Removed("foo".into()));
        assert_eq!(lines[2], UnifiedLine::Added("bar".into()));
    }

    #[test]
    fn test_format_unified() {
        let lines = vec![
            UnifiedLine::Context("hello".into()),
            UnifiedLine::Removed("old".into()),
            UnifiedLine::Added("new".into()),
        ];
        let formatted = format_unified(&lines);
        assert!(formatted.contains("  hello"));
        assert!(formatted.contains("- old"));
        assert!(formatted.contains("+ new"));
    }

    #[test]
    fn test_apply_patch() {
        let left = json!({"a": 1});
        let right = json!({"a": 2, "b": 3});
        let diff = diff_json(&left, &right);
        let patched = apply_patch(&left, &diff.ops);
        assert_eq!(patched, right);
    }

    #[test]
    fn test_apply_patch_remove() {
        let left = json!({"a": 1, "b": 2});
        let right = json!({"a": 1});
        let diff = diff_json(&left, &right);
        let patched = apply_patch(&left, &diff.ops);
        assert_eq!(patched, right);
    }
}
