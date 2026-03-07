//! # Transactional Batch Operations
//!
//! Group file operations (rename, move, copy, transform) into a
//! transaction with dry-run preview, journaled execution, and
//! rollback on failure.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single operation in a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchOp {
    /// Copy file from source to destination.
    Copy { src: PathBuf, dst: PathBuf },
    /// Move / rename file.
    Move { src: PathBuf, dst: PathBuf },
    /// Delete a file.
    Delete { path: PathBuf },
    /// Create a file with content.
    Create { path: PathBuf, content: String },
    /// Apply a text transform (regex replace) to a file.
    Transform {
        path: PathBuf,
        pattern: String,
        replacement: String,
    },
    /// Append text to a file.
    Append { path: PathBuf, text: String },
}

impl BatchOp {
    /// Human-readable description.
    pub fn describe(&self) -> String {
        match self {
            Self::Copy { src, dst } => format!("COPY {} -> {}", src.display(), dst.display()),
            Self::Move { src, dst } => format!("MOVE {} -> {}", src.display(), dst.display()),
            Self::Delete { path } => format!("DELETE {}", path.display()),
            Self::Create { path, .. } => format!("CREATE {}", path.display()),
            Self::Transform {
                path,
                pattern,
                replacement,
            } => {
                format!(
                    "TRANSFORM {} s/{}/{}/",
                    path.display(),
                    pattern,
                    replacement
                )
            }
            Self::Append { path, .. } => format!("APPEND {}", path.display()),
        }
    }

    /// Files that will be read by this operation.
    pub fn source_paths(&self) -> Vec<&Path> {
        match self {
            Self::Copy { src, .. } => vec![src],
            Self::Move { src, .. } => vec![src],
            Self::Delete { path } => vec![path],
            Self::Create { .. } => vec![],
            Self::Transform { path, .. } => vec![path],
            Self::Append { path, .. } => vec![path],
        }
    }

    /// Files that will be written/modified.
    pub fn target_paths(&self) -> Vec<&Path> {
        match self {
            Self::Copy { dst, .. } => vec![dst],
            Self::Move { dst, .. } => vec![dst],
            Self::Delete { .. } => vec![],
            Self::Create { path, .. } => vec![path],
            Self::Transform { path, .. } => vec![path],
            Self::Append { path, .. } => vec![path],
        }
    }
}

/// Batch execution plan status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpStatus {
    Pending,
    Success,
    Failed,
    Skipped,
    RolledBack,
}

/// Outcome record for a single operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpRecord {
    pub index: usize,
    pub op: String,
    pub status: OpStatus,
    pub error: Option<String>,
    /// Undo information for rollback.
    pub undo: Option<UndoEntry>,
}

/// Undo information for rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoEntry {
    /// Delete a file that was created or copied.
    DeleteFile(PathBuf),
    /// Restore file content.
    RestoreContent { path: PathBuf, content: String },
    /// Move file back.
    MoveBack { from: PathBuf, to: PathBuf },
    /// Recreate a deleted file.
    Recreate { path: PathBuf, content: String },
}

/// Options for batch execution.
#[derive(Debug, Clone)]
pub struct BatchOptions {
    /// If true, only preview — don't execute.
    pub dry_run: bool,
    /// If true, stop and rollback on first failure.
    pub stop_on_error: bool,
    /// If true, create parent directories as needed.
    pub create_dirs: bool,
    /// Maximum number of operations (0 = unlimited).
    pub max_ops: usize,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            stop_on_error: true,
            create_dirs: true,
            max_ops: 0,
        }
    }
}

/// A batch of operations.
#[derive(Debug, Clone)]
pub struct Batch {
    pub ops: Vec<BatchOp>,
    pub options: BatchOptions,
}

/// Batch execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub records: Vec<OpRecord>,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub rolled_back: usize,
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Dry-run preview
// ---------------------------------------------------------------------------

/// Preview a batch (dry-run).
pub fn preview(ops: &[BatchOp]) -> Vec<String> {
    ops.iter()
        .enumerate()
        .map(|(i, op)| format!("[{}] {}", i + 1, op.describe()))
        .collect()
}

/// Validate a batch for obvious problems.
pub fn validate(ops: &[BatchOp]) -> Vec<String> {
    let mut issues = Vec::new();
    let mut seen_targets: BTreeMap<String, usize> = BTreeMap::new();

    for (i, op) in ops.iter().enumerate() {
        // Check for duplicate targets
        for t in op.target_paths() {
            let key = t.display().to_string();
            if let Some(prev) = seen_targets.get(&key) {
                issues.push(format!(
                    "Operation {} and {} both write to {}",
                    prev + 1,
                    i + 1,
                    key
                ));
            }
            seen_targets.insert(key, i);
        }

        // Check transform pattern validity
        if let BatchOp::Transform { pattern, .. } = op {
            if regex::Regex::new(pattern).is_err() {
                issues.push(format!("Operation {}: invalid regex '{}'", i + 1, pattern));
            }
        }
    }
    issues
}

// ---------------------------------------------------------------------------
// In-memory execution (for testing / sandboxed use)
// ---------------------------------------------------------------------------

/// In-memory filesystem for sandboxed batch execution.
#[derive(Debug, Clone, Default)]
pub struct MemFs {
    pub files: BTreeMap<String, String>,
}

impl MemFs {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, path: &str, content: &str) {
        self.files.insert(path.into(), content.into());
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(|s| s.as_str())
    }

    pub fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn remove(&mut self, path: &str) -> Option<String> {
        self.files.remove(path)
    }

    /// Execute a batch against the in-memory filesystem.
    pub fn execute(&mut self, batch: &Batch) -> BatchResult {
        let mut records = Vec::new();
        let mut succeeded = 0;
        let mut failed = 0;
        let mut skipped = 0;

        for (i, op) in batch.ops.iter().enumerate() {
            if batch.options.max_ops > 0 && i >= batch.options.max_ops {
                break;
            }

            if batch.options.dry_run {
                records.push(OpRecord {
                    index: i,
                    op: op.describe(),
                    status: OpStatus::Skipped,
                    error: None,
                    undo: None,
                });
                skipped += 1;
                continue;
            }

            let result = self.execute_op(op);
            match result {
                Ok(undo) => {
                    records.push(OpRecord {
                        index: i,
                        op: op.describe(),
                        status: OpStatus::Success,
                        error: None,
                        undo,
                    });
                    succeeded += 1;
                }
                Err(e) => {
                    records.push(OpRecord {
                        index: i,
                        op: op.describe(),
                        status: OpStatus::Failed,
                        error: Some(e),
                        undo: None,
                    });
                    failed += 1;
                    if batch.options.stop_on_error {
                        // Rollback previous successful ops
                        let rolled_back = self.rollback(&records);
                        return BatchResult {
                            total: batch.ops.len(),
                            succeeded: 0,
                            failed,
                            skipped: batch.ops.len() - i - 1,
                            rolled_back,
                            records,
                            dry_run: false,
                        };
                    }
                }
            }
        }

        BatchResult {
            total: batch.ops.len(),
            succeeded,
            failed,
            skipped,
            rolled_back: 0,
            records,
            dry_run: batch.options.dry_run,
        }
    }

    fn execute_op(&mut self, op: &BatchOp) -> Result<Option<UndoEntry>, String> {
        match op {
            BatchOp::Copy { src, dst } => {
                let src_key = src.display().to_string();
                let dst_key = dst.display().to_string();
                let content = self
                    .files
                    .get(&src_key)
                    .cloned()
                    .ok_or_else(|| format!("source not found: {}", src_key))?;
                let undo = if self.files.contains_key(&dst_key) {
                    Some(UndoEntry::RestoreContent {
                        path: dst.clone(),
                        content: self.files[&dst_key].clone(),
                    })
                } else {
                    Some(UndoEntry::DeleteFile(dst.clone()))
                };
                self.files.insert(dst_key, content);
                Ok(undo)
            }
            BatchOp::Move { src, dst } => {
                let src_key = src.display().to_string();
                let dst_key = dst.display().to_string();
                let content = self
                    .files
                    .remove(&src_key)
                    .ok_or_else(|| format!("source not found: {}", src_key))?;
                self.files.insert(dst_key, content);
                Ok(Some(UndoEntry::MoveBack {
                    from: dst.clone(),
                    to: src.clone(),
                }))
            }
            BatchOp::Delete { path } => {
                let key = path.display().to_string();
                let content = self
                    .files
                    .remove(&key)
                    .ok_or_else(|| format!("file not found: {}", key))?;
                Ok(Some(UndoEntry::Recreate {
                    path: path.clone(),
                    content,
                }))
            }
            BatchOp::Create { path, content } => {
                let key = path.display().to_string();
                let undo = if self.files.contains_key(&key) {
                    Some(UndoEntry::RestoreContent {
                        path: path.clone(),
                        content: self.files[&key].clone(),
                    })
                } else {
                    Some(UndoEntry::DeleteFile(path.clone()))
                };
                self.files.insert(key, content.clone());
                Ok(undo)
            }
            BatchOp::Transform {
                path,
                pattern,
                replacement,
            } => {
                let key = path.display().to_string();
                let content = self
                    .files
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| format!("file not found: {}", key))?;
                let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {}", e))?;
                let new_content = re.replace_all(&content, replacement.as_str()).to_string();
                let undo = Some(UndoEntry::RestoreContent {
                    path: path.clone(),
                    content,
                });
                self.files.insert(key, new_content);
                Ok(undo)
            }
            BatchOp::Append { path, text } => {
                let key = path.display().to_string();
                let content = self
                    .files
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| format!("file not found: {}", key))?;
                let undo = Some(UndoEntry::RestoreContent {
                    path: path.clone(),
                    content: content.clone(),
                });
                self.files.insert(key, format!("{}{}", content, text));
                Ok(undo)
            }
        }
    }

    fn rollback(&mut self, records: &[OpRecord]) -> usize {
        let mut count = 0;
        for rec in records.iter().rev() {
            if rec.status == OpStatus::Success {
                if let Some(undo) = &rec.undo {
                    match undo {
                        UndoEntry::DeleteFile(p) => {
                            self.files.remove(&p.display().to_string());
                        }
                        UndoEntry::RestoreContent { path, content } => {
                            self.files
                                .insert(path.display().to_string(), content.clone());
                        }
                        UndoEntry::MoveBack { from, to } => {
                            if let Some(content) = self.files.remove(&from.display().to_string()) {
                                self.files.insert(to.display().to_string(), content);
                            }
                        }
                        UndoEntry::Recreate { path, content } => {
                            self.files
                                .insert(path.display().to_string(), content.clone());
                        }
                    }
                    count += 1;
                }
            }
        }
        count
    }
}

/// Summary statistics.
pub fn summary(result: &BatchResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("Batch: {} operations\n", result.total));
    if result.dry_run {
        out.push_str("  Mode: DRY RUN\n");
    }
    out.push_str(&format!("  Succeeded: {}\n", result.succeeded));
    out.push_str(&format!("  Failed:    {}\n", result.failed));
    out.push_str(&format!("  Skipped:   {}\n", result.skipped));
    if result.rolled_back > 0 {
        out.push_str(&format!("  Rolled back: {}\n", result.rolled_back));
    }
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_fs() -> MemFs {
        let mut fs = MemFs::new();
        fs.add("a.txt", "hello world");
        fs.add("b.txt", "foo bar");
        fs.add("c.txt", "data 123");
        fs
    }

    #[test]
    fn test_copy() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Copy {
                src: "a.txt".into(),
                dst: "d.txt".into(),
            }],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 1);
        assert_eq!(fs.get("d.txt"), Some("hello world"));
        assert!(fs.exists("a.txt")); // source still exists
    }

    #[test]
    fn test_move() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Move {
                src: "a.txt".into(),
                dst: "moved.txt".into(),
            }],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 1);
        assert!(!fs.exists("a.txt"));
        assert_eq!(fs.get("moved.txt"), Some("hello world"));
    }

    #[test]
    fn test_delete() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Delete {
                path: "b.txt".into(),
            }],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 1);
        assert!(!fs.exists("b.txt"));
    }

    #[test]
    fn test_create() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Create {
                path: "new.txt".into(),
                content: "brand new".into(),
            }],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 1);
        assert_eq!(fs.get("new.txt"), Some("brand new"));
    }

    #[test]
    fn test_transform() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Transform {
                path: "a.txt".into(),
                pattern: "world".into(),
                replacement: "rust".into(),
            }],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 1);
        assert_eq!(fs.get("a.txt"), Some("hello rust"));
    }

    #[test]
    fn test_append() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Append {
                path: "a.txt".into(),
                text: "!!!".into(),
            }],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 1);
        assert_eq!(fs.get("a.txt"), Some("hello world!!!"));
    }

    #[test]
    fn test_rollback_on_failure() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![
                BatchOp::Transform {
                    path: "a.txt".into(),
                    pattern: "hello".into(),
                    replacement: "HI".into(),
                },
                BatchOp::Delete {
                    path: "nonexistent.txt".into(), // will fail
                },
            ],
            options: BatchOptions {
                stop_on_error: true,
                ..Default::default()
            },
        };
        let result = fs.execute(&batch);
        assert_eq!(result.failed, 1);
        assert!(result.rolled_back > 0);
        // The transform should have been rolled back
        assert_eq!(fs.get("a.txt"), Some("hello world"));
    }

    #[test]
    fn test_dry_run() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![BatchOp::Delete {
                path: "a.txt".into(),
            }],
            options: BatchOptions {
                dry_run: true,
                ..Default::default()
            },
        };
        let result = fs.execute(&batch);
        assert!(result.dry_run);
        assert_eq!(result.skipped, 1);
        assert!(fs.exists("a.txt")); // not deleted
    }

    #[test]
    fn test_preview() {
        let ops = vec![
            BatchOp::Copy {
                src: "a".into(),
                dst: "b".into(),
            },
            BatchOp::Delete { path: "c".into() },
        ];
        let lines = preview(&ops);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("COPY"));
        assert!(lines[1].contains("DELETE"));
    }

    #[test]
    fn test_validate_duplicate_targets() {
        let ops = vec![
            BatchOp::Create {
                path: "x.txt".into(),
                content: "a".into(),
            },
            BatchOp::Create {
                path: "x.txt".into(),
                content: "b".into(),
            },
        ];
        let issues = validate(&ops);
        assert!(!issues.is_empty());
        assert!(issues[0].contains("x.txt"));
    }

    #[test]
    fn test_validate_bad_regex() {
        let ops = vec![BatchOp::Transform {
            path: "a.txt".into(),
            pattern: "[invalid".into(),
            replacement: "x".into(),
        }];
        let issues = validate(&ops);
        assert!(!issues.is_empty());
        assert!(issues[0].contains("invalid regex"));
    }

    #[test]
    fn test_multi_op_batch() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![
                BatchOp::Copy {
                    src: "a.txt".into(),
                    dst: "d.txt".into(),
                },
                BatchOp::Transform {
                    path: "b.txt".into(),
                    pattern: "foo".into(),
                    replacement: "FOO".into(),
                },
                BatchOp::Append {
                    path: "c.txt".into(),
                    text: " appended".into(),
                },
            ],
            options: BatchOptions::default(),
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 3);
        assert_eq!(fs.get("d.txt"), Some("hello world"));
        assert_eq!(fs.get("b.txt"), Some("FOO bar"));
        assert_eq!(fs.get("c.txt"), Some("data 123 appended"));
    }

    #[test]
    fn test_summary() {
        let result = BatchResult {
            records: vec![],
            total: 5,
            succeeded: 3,
            failed: 1,
            skipped: 1,
            rolled_back: 0,
            dry_run: false,
        };
        let s = summary(&result);
        assert!(s.contains("Succeeded: 3"));
        assert!(s.contains("Failed:    1"));
    }

    #[test]
    fn test_op_describe() {
        let op = BatchOp::Copy {
            src: "a.txt".into(),
            dst: "b.txt".into(),
        };
        assert!(op.describe().contains("COPY"));
        let op2 = BatchOp::Transform {
            path: "x.txt".into(),
            pattern: "f".into(),
            replacement: "r".into(),
        };
        assert!(op2.describe().contains("TRANSFORM"));
    }

    #[test]
    fn test_max_ops() {
        let mut fs = setup_fs();
        let batch = Batch {
            ops: vec![
                BatchOp::Create {
                    path: "1.txt".into(),
                    content: "a".into(),
                },
                BatchOp::Create {
                    path: "2.txt".into(),
                    content: "b".into(),
                },
                BatchOp::Create {
                    path: "3.txt".into(),
                    content: "c".into(),
                },
            ],
            options: BatchOptions {
                max_ops: 2,
                ..Default::default()
            },
        };
        let result = fs.execute(&batch);
        assert_eq!(result.succeeded, 2);
        assert!(fs.exists("1.txt"));
        assert!(fs.exists("2.txt"));
        assert!(!fs.exists("3.txt"));
    }
}
