//! Git-aware search — limit search scope to changed files/lines.
//!
//! Provides `--git-diff` to search only files changed since a given ref,
//! and `--git-blame` to annotate matches with last-change info.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Configuration for git-aware search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSearchConfig {
    /// Base ref to diff against (e.g. "HEAD", "main", "HEAD~3").
    #[serde(default = "default_base_ref")]
    pub base_ref: String,
    /// Only search changed files (not all files).
    #[serde(default)]
    pub changed_only: bool,
    /// Include blame annotations.
    #[serde(default)]
    pub include_blame: bool,
    /// Include staged changes.
    #[serde(default = "default_true")]
    pub include_staged: bool,
    /// Include unstaged changes.
    #[serde(default = "default_true")]
    pub include_unstaged: bool,
}

fn default_base_ref() -> String {
    "HEAD".into()
}
fn default_true() -> bool {
    true
}

impl Default for GitSearchConfig {
    fn default() -> Self {
        Self {
            base_ref: default_base_ref(),
            changed_only: false,
            include_blame: false,
            include_staged: true,
            include_unstaged: true,
        }
    }
}

/// A file changed in git.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub status: ChangeStatus,
    pub additions: usize,
    pub deletions: usize,
}

/// Git change status for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
}

impl ChangeStatus {
    fn from_code(code: &str) -> Self {
        match code.chars().next().unwrap_or('M') {
            'A' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            '?' => Self::Untracked,
            _ => Self::Modified,
        }
    }
}

/// Blame annotation for a line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameInfo {
    pub commit: String,
    pub author: String,
    pub date: String,
    pub line_number: usize,
}

/// Result of a git-aware search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSearchResult {
    /// Changed files found.
    pub changed_files: Vec<ChangedFile>,
    /// Changed line ranges indexed by file path.
    pub changed_lines: BTreeMap<String, Vec<LineRange>>,
    /// Base ref used.
    pub base_ref: String,
    /// Total additions.
    pub total_additions: usize,
    /// Total deletions.
    pub total_deletions: usize,
}

/// A range of lines.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

/// Check if a directory is inside a git repository.
pub fn is_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the list of changed files relative to a base ref.
pub fn changed_files(
    repo_dir: &Path,
    config: &GitSearchConfig,
) -> anyhow::Result<Vec<ChangedFile>> {
    let mut files = BTreeMap::new();

    // Staged changes
    if config.include_staged {
        let output = Command::new("git")
            .args(["diff", "--cached", "--name-status", &config.base_ref])
            .current_dir(repo_dir)
            .output()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some((status, path)) = parse_name_status_line(line) {
                    files.insert(
                        path.clone(),
                        ChangedFile {
                            path: PathBuf::from(&path),
                            status,
                            additions: 0,
                            deletions: 0,
                        },
                    );
                }
            }
        }
    }

    // Unstaged changes
    if config.include_unstaged {
        let output = Command::new("git")
            .args(["diff", "--name-status"])
            .current_dir(repo_dir)
            .output()?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some((status, path)) = parse_name_status_line(line) {
                    files.entry(path.clone()).or_insert(ChangedFile {
                        path: PathBuf::from(&path),
                        status,
                        additions: 0,
                        deletions: 0,
                    });
                }
            }
        }
    }

    // Get diff stats for additions/deletions
    let output = Command::new("git")
        .args(["diff", "--numstat", &config.base_ref])
        .current_dir(repo_dir)
        .output()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let additions = parts[0].parse::<usize>().unwrap_or(0);
                let deletions = parts[1].parse::<usize>().unwrap_or(0);
                let path = parts[2].to_string();
                if let Some(f) = files.get_mut(&path) {
                    f.additions = additions;
                    f.deletions = deletions;
                }
            }
        }
    }

    Ok(files.into_values().collect())
}

/// Parse the changed line ranges from a diff.
pub fn changed_line_ranges(
    repo_dir: &Path,
    config: &GitSearchConfig,
) -> anyhow::Result<BTreeMap<String, Vec<LineRange>>> {
    let output = Command::new("git")
        .args(["diff", "-U0", &config.base_ref])
        .current_dir(repo_dir)
        .output()?;

    let mut result = BTreeMap::new();

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let mut current_file: Option<String> = None;

        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("+++ b/") {
                current_file = Some(rest.to_string());
            } else if line.starts_with("@@ ") {
                if let Some(ref file) = current_file {
                    if let Some(range) = parse_hunk_header(line) {
                        result
                            .entry(file.clone())
                            .or_insert_with(Vec::new)
                            .push(range);
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Get blame info for a specific file.
pub fn blame_file(repo_dir: &Path, file_path: &Path) -> anyhow::Result<Vec<BlameInfo>> {
    let output = Command::new("git")
        .args(["blame", "--porcelain", &file_path.to_string_lossy()])
        .current_dir(repo_dir)
        .output()?;

    let mut results = Vec::new();
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let mut current_commit = String::new();
        let mut current_author = String::new();
        let mut current_date = String::new();
        let mut current_line = 0usize;

        for line in text.lines() {
            if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    // Emit previous if we have one
                    if !current_commit.is_empty() {
                        results.push(BlameInfo {
                            commit: current_commit.clone(),
                            author: current_author.clone(),
                            date: current_date.clone(),
                            line_number: current_line,
                        });
                    }
                    current_commit = parts[0].to_string();
                    current_line = parts[2].parse().unwrap_or(0);
                }
            } else if let Some(author) = line.strip_prefix("author ") {
                current_author = author.to_string();
            } else if let Some(date) = line.strip_prefix("author-time ") {
                current_date = date.to_string();
            }
        }

        // Emit last
        if !current_commit.is_empty() {
            results.push(BlameInfo {
                commit: current_commit,
                author: current_author,
                date: current_date,
                line_number: current_line,
            });
        }
    }

    Ok(results)
}

/// Build a full git search result.
pub fn git_search(repo_dir: &Path, config: &GitSearchConfig) -> anyhow::Result<GitSearchResult> {
    let files = changed_files(repo_dir, config)?;
    let lines = changed_line_ranges(repo_dir, config)?;

    let total_additions: usize = files.iter().map(|f| f.additions).sum();
    let total_deletions: usize = files.iter().map(|f| f.deletions).sum();

    Ok(GitSearchResult {
        changed_files: files,
        changed_lines: lines,
        base_ref: config.base_ref.clone(),
        total_additions,
        total_deletions,
    })
}

/// Filter a set of file paths to only those that are in the changed set.
pub fn filter_to_changed(all_files: &[PathBuf], changed: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    all_files
        .iter()
        .filter(|p| changed.contains(*p))
        .cloned()
        .collect()
}

/// Check if a specific line is within a changed range.
pub fn is_line_changed(
    file: &str,
    line: usize,
    changed_lines: &BTreeMap<String, Vec<LineRange>>,
) -> bool {
    changed_lines
        .get(file)
        .map(|ranges| ranges.iter().any(|r| line >= r.start && line <= r.end))
        .unwrap_or(false)
}

// ── helpers ──────────────────────────────────────────────────────────

fn parse_name_status_line(line: &str) -> Option<(ChangeStatus, String)> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() >= 2 {
        let status = ChangeStatus::from_code(parts[0]);
        let path = parts[1].to_string();
        Some((status, path))
    } else {
        None
    }
}

fn parse_hunk_header(line: &str) -> Option<LineRange> {
    // @@ -old,len +new,len @@ ...
    let after_at = line.strip_prefix("@@ ")?;
    let plus_part = after_at.split_whitespace().nth(1)?;
    let plus_part = plus_part.strip_prefix('+')?;
    let (start_str, len_str) = if let Some((s, l)) = plus_part.split_once(',') {
        (s, l)
    } else {
        (plus_part, "1")
    };
    let start: usize = start_str.parse().ok()?;
    let len: usize = len_str.parse().ok()?;
    if len == 0 {
        return None; // Deletion only, no new lines
    }
    Some(LineRange {
        start,
        end: start + len - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GitSearchConfig::default();
        assert_eq!(config.base_ref, "HEAD");
        assert!(!config.changed_only);
        assert!(!config.include_blame);
        assert!(config.include_staged);
        assert!(config.include_unstaged);
    }

    #[test]
    fn test_change_status_from_code() {
        assert_eq!(ChangeStatus::from_code("A"), ChangeStatus::Added);
        assert_eq!(ChangeStatus::from_code("M"), ChangeStatus::Modified);
        assert_eq!(ChangeStatus::from_code("D"), ChangeStatus::Deleted);
        assert_eq!(ChangeStatus::from_code("R100"), ChangeStatus::Renamed);
        assert_eq!(ChangeStatus::from_code("?"), ChangeStatus::Untracked);
    }

    #[test]
    fn test_parse_name_status_line() {
        let r = parse_name_status_line("M\tsrc/main.rs");
        assert!(r.is_some());
        let (status, path) = r.unwrap();
        assert_eq!(status, ChangeStatus::Modified);
        assert_eq!(path, "src/main.rs");
    }

    #[test]
    fn test_parse_name_status_add() {
        let r = parse_name_status_line("A\tnew_file.rs");
        assert!(r.is_some());
        let (status, _) = r.unwrap();
        assert_eq!(status, ChangeStatus::Added);
    }

    #[test]
    fn test_parse_hunk_header() {
        let r = parse_hunk_header("@@ -10,5 +20,3 @@ fn foo");
        assert!(r.is_some());
        let range = r.unwrap();
        assert_eq!(range.start, 20);
        assert_eq!(range.end, 22);
    }

    #[test]
    fn test_parse_hunk_header_single_line() {
        let r = parse_hunk_header("@@ -5 +10 @@ fn bar");
        assert!(r.is_some());
        let range = r.unwrap();
        assert_eq!(range.start, 10);
        assert_eq!(range.end, 10);
    }

    #[test]
    fn test_parse_hunk_header_deletion_only() {
        let r = parse_hunk_header("@@ -5,3 +5,0 @@ fn baz");
        assert!(r.is_none()); // No new lines
    }

    #[test]
    fn test_is_line_changed() {
        let mut changed = BTreeMap::new();
        changed.insert(
            "src/main.rs".to_string(),
            vec![
                LineRange { start: 10, end: 20 },
                LineRange { start: 50, end: 55 },
            ],
        );

        assert!(is_line_changed("src/main.rs", 15, &changed));
        assert!(is_line_changed("src/main.rs", 10, &changed));
        assert!(is_line_changed("src/main.rs", 20, &changed));
        assert!(!is_line_changed("src/main.rs", 9, &changed));
        assert!(!is_line_changed("src/main.rs", 21, &changed));
        assert!(is_line_changed("src/main.rs", 52, &changed));
        assert!(!is_line_changed("other.rs", 15, &changed));
    }

    #[test]
    fn test_filter_to_changed() {
        let all = vec![
            PathBuf::from("a.rs"),
            PathBuf::from("b.rs"),
            PathBuf::from("c.rs"),
        ];
        let mut changed = BTreeSet::new();
        changed.insert(PathBuf::from("b.rs"));
        changed.insert(PathBuf::from("c.rs"));

        let filtered = filter_to_changed(&all, &changed);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&PathBuf::from("b.rs")));
        assert!(filtered.contains(&PathBuf::from("c.rs")));
    }

    #[test]
    fn test_config_serialization() {
        let config = GitSearchConfig {
            base_ref: "main".into(),
            changed_only: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("main"));
        assert!(json.contains("changed_only"));
    }

    #[test]
    fn test_line_range_equality() {
        let a = LineRange { start: 1, end: 10 };
        let b = LineRange { start: 1, end: 10 };
        assert_eq!(a, b);
    }

    #[test]
    fn test_git_search_result_serialization() {
        let result = GitSearchResult {
            changed_files: vec![],
            changed_lines: BTreeMap::new(),
            base_ref: "HEAD".into(),
            total_additions: 5,
            total_deletions: 3,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("total_additions"));
        assert!(json.contains("HEAD"));
    }
}
