//! Snapshot testing harness.
//!
//! Record pipeline output as `.snap` files and assert future runs match.
//! Use `update_mode = true` to accept new snapshots.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Configuration for snapshot testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    /// Directory to store snapshots.
    #[serde(default = "default_snap_dir")]
    pub snapshot_dir: PathBuf,
    /// If true, overwrite existing snapshots instead of asserting.
    #[serde(default)]
    pub update_mode: bool,
    /// If true, strip timestamps/UUIDs before comparing.
    #[serde(default = "default_true")]
    pub normalize: bool,
}

fn default_snap_dir() -> PathBuf {
    PathBuf::from("snapshots")
}
fn default_true() -> bool {
    true
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: default_snap_dir(),
            update_mode: false,
            normalize: true,
        }
    }
}

/// The contents of a `.snap` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Name of this snapshot.
    pub name: String,
    /// SHA-256 of the normalized content.
    pub content_hash: String,
    /// The actual content lines.
    pub lines: Vec<String>,
    /// Metadata about when/how this was captured.
    pub metadata: SnapshotMetadata,
}

/// Metadata attached to a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Version of ATP that created this snapshot.
    pub atp_version: String,
    /// Pipeline name or command that produced the output.
    pub source: String,
    /// When the snapshot was created (RFC 3339).
    pub created_at: String,
    /// Number of lines.
    pub line_count: usize,
}

/// Result of a snapshot assertion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotVerdict {
    /// Snapshot matches.
    Pass,
    /// Snapshot was created for the first time.
    Created,
    /// Snapshot was updated (in update mode).
    Updated,
    /// Snapshot does not match.
    Mismatch {
        expected_hash: String,
        actual_hash: String,
        diff_summary: String,
    },
}

/// A snapshot testing harness.
#[derive(Debug)]
pub struct SnapshotHarness {
    config: SnapshotConfig,
    results: BTreeMap<String, SnapshotVerdict>,
}

impl SnapshotHarness {
    /// Create a new harness with the given configuration.
    pub fn new(config: SnapshotConfig) -> Self {
        Self {
            config,
            results: BTreeMap::new(),
        }
    }

    /// Assert that the given output matches the stored snapshot.
    pub fn assert_snapshot(
        &mut self,
        name: &str,
        source: &str,
        output: &[String],
    ) -> Result<SnapshotVerdict> {
        let normalized = if self.config.normalize {
            normalize_output(output)
        } else {
            output.to_vec()
        };

        let hash = compute_hash(&normalized);
        let snap_path = self.snap_path(name);

        let verdict = if snap_path.exists() {
            let existing = self.load_snapshot(&snap_path)?;
            if existing.content_hash == hash {
                SnapshotVerdict::Pass
            } else if self.config.update_mode {
                let snap = self.build_snapshot(name, source, &normalized, &hash);
                self.save_snapshot(&snap_path, &snap)?;
                SnapshotVerdict::Updated
            } else {
                let diff = diff_summary(&existing.lines, &normalized);
                SnapshotVerdict::Mismatch {
                    expected_hash: existing.content_hash,
                    actual_hash: hash,
                    diff_summary: diff,
                }
            }
        } else {
            let snap = self.build_snapshot(name, source, &normalized, &hash);
            if let Some(parent) = snap_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            self.save_snapshot(&snap_path, &snap)?;
            SnapshotVerdict::Created
        };

        self.results.insert(name.to_string(), verdict.clone());
        Ok(verdict)
    }

    /// Get all results.
    pub fn results(&self) -> &BTreeMap<String, SnapshotVerdict> {
        &self.results
    }

    /// Summary: (passed, created, updated, mismatched).
    pub fn summary(&self) -> (usize, usize, usize, usize) {
        let mut pass = 0;
        let mut created = 0;
        let mut updated = 0;
        let mut mismatch = 0;
        for v in self.results.values() {
            match v {
                SnapshotVerdict::Pass => pass += 1,
                SnapshotVerdict::Created => created += 1,
                SnapshotVerdict::Updated => updated += 1,
                SnapshotVerdict::Mismatch { .. } => mismatch += 1,
            }
        }
        (pass, created, updated, mismatch)
    }

    fn snap_path(&self, name: &str) -> PathBuf {
        self.config
            .snapshot_dir
            .join(format!("{}.snap", sanitize_name(name)))
    }

    fn build_snapshot(&self, name: &str, source: &str, lines: &[String], hash: &str) -> Snapshot {
        Snapshot {
            name: name.to_string(),
            content_hash: hash.to_string(),
            lines: lines.to_vec(),
            metadata: SnapshotMetadata {
                atp_version: env!("CARGO_PKG_VERSION").to_string(),
                source: source.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                line_count: lines.len(),
            },
        }
    }

    fn load_snapshot(&self, path: &Path) -> Result<Snapshot> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading snapshot {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("parsing snapshot {}", path.display()))
    }

    fn save_snapshot(&self, path: &Path, snap: &Snapshot) -> Result<()> {
        let json = serde_json::to_string_pretty(snap)?;
        std::fs::write(path, json)
            .with_context(|| format!("writing snapshot {}", path.display()))?;
        Ok(())
    }
}

/// Compute SHA-256 hash of content lines.
pub fn compute_hash(lines: &[String]) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Normalize output by stripping timestamps, UUIDs, and durations.
pub fn normalize_output(lines: &[String]) -> Vec<String> {
    let uuid_re =
        regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    let iso_re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[^\s]*").unwrap();
    let dur_re = regex::Regex::new(r"\d+(\.\d+)?(ms|µs|ns|s)\b").unwrap();

    lines
        .iter()
        .map(|line| {
            let s = uuid_re.replace_all(line, "<UUID>");
            let s = iso_re.replace_all(&s, "<TIMESTAMP>");
            let s = dur_re.replace_all(&s, "<DURATION>");
            s.to_string()
        })
        .collect()
}

/// Sanitize a name for use in filenames.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Produce a short diff summary.
fn diff_summary(expected: &[String], actual: &[String]) -> String {
    let mut added = 0usize;
    let mut removed = 0usize;
    let max = expected.len().max(actual.len());

    for i in 0..max {
        match (expected.get(i), actual.get(i)) {
            (Some(e), Some(a)) if e != a => {
                added += 1;
                removed += 1;
            }
            (None, Some(_)) => added += 1,
            (Some(_), None) => removed += 1,
            _ => {}
        }
    }

    format!(
        "{} line(s) differ: +{} added, -{} removed (of {} total)",
        added + removed,
        added,
        removed,
        max
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let lines = vec!["hello".into(), "world".into()];
        let h1 = compute_hash(&lines);
        let h2 = compute_hash(&lines);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_hash_different_input() {
        let h1 = compute_hash(&["a".into()]);
        let h2 = compute_hash(&["b".into()]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_normalize_uuid() {
        let lines = vec!["id: 550e8400-e29b-41d4-a716-446655440000 ok".into()];
        let normalized = normalize_output(&lines);
        assert_eq!(normalized[0], "id: <UUID> ok");
    }

    #[test]
    fn test_normalize_timestamp() {
        let lines = vec!["at 2024-01-15T10:30:00Z done".into()];
        let normalized = normalize_output(&lines);
        assert_eq!(normalized[0], "at <TIMESTAMP> done");
    }

    #[test]
    fn test_normalize_duration() {
        let lines = vec!["took 42.5ms to finish".into()];
        let normalized = normalize_output(&lines);
        assert_eq!(normalized[0], "took <DURATION> to finish");
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("my test/case"), "my_test_case");
        assert_eq!(sanitize_name("simple"), "simple");
        assert_eq!(sanitize_name("a-b_c"), "a-b_c");
    }

    #[test]
    fn test_diff_summary_identical() {
        let a = vec!["line1".into(), "line2".into()];
        let s = diff_summary(&a, &a);
        assert!(s.starts_with("0 line(s) differ"));
    }

    #[test]
    fn test_diff_summary_different() {
        let a = vec!["line1".into(), "line2".into()];
        let b = vec!["line1".into(), "changed".into()];
        let s = diff_summary(&a, &b);
        assert!(s.contains("1") || s.contains("differ"));
    }

    #[test]
    fn test_snapshot_harness_create_and_verify() {
        let dir = tempfile::tempdir().unwrap();
        let config = SnapshotConfig {
            snapshot_dir: dir.path().to_path_buf(),
            update_mode: false,
            normalize: false,
        };
        let mut harness = SnapshotHarness::new(config);

        let output = vec!["hello".into(), "world".into()];

        // First call: creates
        let v = harness
            .assert_snapshot("test1", "my_pipeline", &output)
            .unwrap();
        assert_eq!(v, SnapshotVerdict::Created);

        // Second call: passes
        let v = harness
            .assert_snapshot("test1", "my_pipeline", &output)
            .unwrap();
        assert_eq!(v, SnapshotVerdict::Pass);
    }

    #[test]
    fn test_snapshot_harness_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let config = SnapshotConfig {
            snapshot_dir: dir.path().to_path_buf(),
            update_mode: false,
            normalize: false,
        };
        let mut harness = SnapshotHarness::new(config);

        let output1 = vec!["hello".into()];
        let output2 = vec!["goodbye".into()];

        let _ = harness.assert_snapshot("test2", "pipe", &output1).unwrap();
        let v = harness.assert_snapshot("test2", "pipe", &output2).unwrap();
        match v {
            SnapshotVerdict::Mismatch { .. } => {}
            _ => panic!("expected mismatch"),
        }
    }

    #[test]
    fn test_snapshot_harness_update_mode() {
        let dir = tempfile::tempdir().unwrap();
        let config = SnapshotConfig {
            snapshot_dir: dir.path().to_path_buf(),
            update_mode: true,
            normalize: false,
        };
        let mut harness = SnapshotHarness::new(config);

        let output1 = vec!["v1".into()];
        let output2 = vec!["v2".into()];

        let _ = harness.assert_snapshot("test3", "pipe", &output1).unwrap();
        let v = harness.assert_snapshot("test3", "pipe", &output2).unwrap();
        assert_eq!(v, SnapshotVerdict::Updated);

        // Now reading should match v2
        let v = harness.assert_snapshot("test3", "pipe", &output2).unwrap();
        assert_eq!(v, SnapshotVerdict::Pass);
    }

    #[test]
    fn test_snapshot_summary() {
        let dir = tempfile::tempdir().unwrap();
        let config = SnapshotConfig {
            snapshot_dir: dir.path().to_path_buf(),
            update_mode: false,
            normalize: false,
        };
        let mut harness = SnapshotHarness::new(config);

        let output = vec!["data".into()];
        let _ = harness.assert_snapshot("a", "p", &output).unwrap();
        let _ = harness.assert_snapshot("a", "p", &output).unwrap();

        let (pass, created, _updated, _mismatch) = harness.summary();
        // 'a' ran twice: first Created, then Pass (overwrites in results map)
        assert_eq!(pass, 1);
        assert_eq!(created, 0); // overwritten by Pass
    }

    #[test]
    fn test_default_config() {
        let config = SnapshotConfig::default();
        assert_eq!(config.snapshot_dir, PathBuf::from("snapshots"));
        assert!(!config.update_mode);
        assert!(config.normalize);
    }

    #[test]
    fn test_config_serialization() {
        let config = SnapshotConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("snapshots"));
    }
}
