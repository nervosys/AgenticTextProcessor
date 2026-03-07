//! Pipeline checkpointing and resume.
//!
//! Serialize intermediate pipeline state mid-execution, allowing pipelines
//! to be resumed from the last good checkpoint after a failure. Supports
//! idempotent replay with content-addressed stage snapshots.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique identifier for a checkpoint.
pub type CheckpointId = String;

/// Configuration for checkpointing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Whether checkpointing is enabled (default: true).
    pub enabled: bool,
    /// Directory for checkpoint files (default: `.atp/checkpoints`).
    pub checkpoint_dir: Option<PathBuf>,
    /// Maximum number of checkpoints to retain (default: 10).
    pub max_checkpoints: usize,
    /// Whether to remove checkpoints on successful pipeline completion.
    pub cleanup_on_success: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            checkpoint_dir: None,
            max_checkpoints: 10,
            cleanup_on_success: true,
        }
    }
}

/// Data captured at a single stage boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageCheckpoint {
    /// Stage index (0-based).
    pub stage_index: usize,
    /// Stage label / type.
    pub stage_label: String,
    /// SHA-256 of the output at this stage.
    pub content_hash: String,
    /// The intermediate output lines.
    pub output_lines: Vec<String>,
    /// Timestamp when this checkpoint was taken.
    pub timestamp: u64,
}

/// A full pipeline checkpoint = pipeline id + ordered stage snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique ID for this checkpoint (SHA-256 of pipeline def + input).
    pub id: CheckpointId,
    /// Human-readable pipeline name.
    pub pipeline_name: String,
    /// Pipeline definition hash.
    pub pipeline_hash: String,
    /// Input content hash.
    pub input_hash: String,
    /// Total number of stages.
    pub total_stages: usize,
    /// Stage checkpoints captured so far (index → checkpoint).
    pub stages: BTreeMap<usize, StageCheckpoint>,
    /// When this checkpoint was first created.
    pub created_at: u64,
    /// When this checkpoint was last updated.
    pub updated_at: u64,
}

/// Status of attempting to resume from a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResumeStatus {
    /// No checkpoint found — start from scratch.
    Fresh,
    /// Checkpoint found; resume from the given stage index.
    ResumeFrom {
        stage_index: usize,
        initial_lines: Vec<String>,
    },
    /// Checkpoint found but input changed — discard and restart.
    InputChanged,
    /// Checkpoint found but pipeline definition changed — discard and restart.
    PipelineChanged,
}

/// Checkpoint manager statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckpointStats {
    pub total_checkpoints: usize,
    pub total_size_bytes: u64,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sha256(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn default_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".atp")
        .join("checkpoints")
}

// ---------------------------------------------------------------------------
// CheckpointManager
// ---------------------------------------------------------------------------

/// Manages checkpoint lifecycle: create, update, resume, prune.
#[derive(Debug, Clone)]
pub struct CheckpointManager {
    config: CheckpointConfig,
    /// In-memory store of checkpoints (id → checkpoint).
    store: BTreeMap<CheckpointId, Checkpoint>,
    /// Index: pipeline_name → most recent CheckpointId.
    name_index: BTreeMap<String, CheckpointId>,
}

impl CheckpointManager {
    /// Create a new manager with the given config.
    pub fn new(config: CheckpointConfig) -> Self {
        Self {
            config,
            store: BTreeMap::new(),
            name_index: BTreeMap::new(),
        }
    }

    /// Get the checkpoint directory.
    pub fn checkpoint_dir(&self) -> PathBuf {
        self.config
            .checkpoint_dir
            .clone()
            .unwrap_or_else(default_dir)
    }

    /// Compute the checkpoint ID from pipeline def + input.
    pub fn checkpoint_id(pipeline_def: &str, input: &str) -> CheckpointId {
        sha256(&format!("{pipeline_def}|{input}"))
    }

    /// Begin or retrieve a checkpoint for a pipeline run.
    pub fn begin(
        &mut self,
        pipeline_name: &str,
        pipeline_def: &str,
        input: &str,
        total_stages: usize,
    ) -> ResumeStatus {
        if !self.config.enabled {
            return ResumeStatus::Fresh;
        }

        let pipeline_hash = sha256(pipeline_def);
        let input_hash = sha256(input);
        let id = Self::checkpoint_id(pipeline_def, input);

        // First check by name index: same pipeline_name with different content
        if let Some(prev_id) = self.name_index.get(pipeline_name).cloned() {
            if prev_id != id {
                if let Some(existing) = self.store.get(&prev_id) {
                    let is_pipeline_changed = existing.pipeline_hash != pipeline_hash;
                    let is_input_changed = existing.input_hash != input_hash;

                    // Remove old checkpoint
                    self.store.remove(&prev_id);
                    self.name_index.remove(pipeline_name);

                    // Create new checkpoint
                    let cp = Checkpoint {
                        id: id.clone(),
                        pipeline_name: pipeline_name.to_string(),
                        pipeline_hash: pipeline_hash.clone(),
                        input_hash: input_hash.clone(),
                        total_stages,
                        stages: BTreeMap::new(),
                        created_at: now_epoch(),
                        updated_at: now_epoch(),
                    };
                    self.store.insert(id.clone(), cp);
                    self.name_index
                        .insert(pipeline_name.to_string(), id.clone());
                    self.prune();

                    if is_pipeline_changed {
                        return ResumeStatus::PipelineChanged;
                    }
                    if is_input_changed {
                        return ResumeStatus::InputChanged;
                    }
                }
            }
        }

        if let Some(existing) = self.store.get(&id) {
            // Check pipeline def changed
            if existing.pipeline_hash != pipeline_hash {
                self.store.remove(&id);
                return ResumeStatus::PipelineChanged;
            }
            // Check input changed
            if existing.input_hash != input_hash {
                self.store.remove(&id);
                return ResumeStatus::InputChanged;
            }
            // Find the latest completed stage
            if let Some((&max_stage, stage_cp)) = existing.stages.iter().next_back() {
                if max_stage + 1 < total_stages {
                    return ResumeStatus::ResumeFrom {
                        stage_index: max_stage + 1,
                        initial_lines: stage_cp.output_lines.clone(),
                    };
                }
                // All stages done — nothing to resume
                return ResumeStatus::Fresh;
            }
            return ResumeStatus::Fresh;
        }

        // Create new checkpoint
        let cp = Checkpoint {
            id: id.clone(),
            pipeline_name: pipeline_name.to_string(),
            pipeline_hash,
            input_hash,
            total_stages,
            stages: BTreeMap::new(),
            created_at: now_epoch(),
            updated_at: now_epoch(),
        };
        self.store.insert(id.clone(), cp);
        self.name_index.insert(pipeline_name.to_string(), id);
        self.prune();
        ResumeStatus::Fresh
    }

    /// Record a stage checkpoint.
    pub fn record_stage(
        &mut self,
        checkpoint_id: &str,
        stage_index: usize,
        stage_label: &str,
        output_lines: &[String],
    ) -> Result<()> {
        let cp = self
            .store
            .get_mut(checkpoint_id)
            .context("checkpoint not found")?;

        let content = output_lines.join("\n");
        let content_hash = sha256(&content);

        cp.stages.insert(
            stage_index,
            StageCheckpoint {
                stage_index,
                stage_label: stage_label.to_string(),
                content_hash,
                output_lines: output_lines.to_vec(),
                timestamp: now_epoch(),
            },
        );
        cp.updated_at = now_epoch();
        Ok(())
    }

    /// Mark a pipeline as complete; optionally cleanup the checkpoint.
    pub fn complete(&mut self, checkpoint_id: &str) {
        if self.config.cleanup_on_success {
            if let Some(cp) = self.store.remove(checkpoint_id) {
                self.name_index.remove(&cp.pipeline_name);
            }
        }
    }

    /// Get a checkpoint by ID.
    pub fn get(&self, id: &str) -> Option<&Checkpoint> {
        self.store.get(id)
    }

    /// List all checkpoint IDs.
    pub fn list(&self) -> Vec<&str> {
        self.store.keys().map(|s| s.as_str()).collect()
    }

    /// Remove a specific checkpoint.
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(cp) = self.store.remove(id) {
            self.name_index.remove(&cp.pipeline_name);
            true
        } else {
            false
        }
    }

    /// Remove all checkpoints.
    pub fn clear(&mut self) {
        self.store.clear();
        self.name_index.clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> CheckpointStats {
        let mut s = CheckpointStats {
            total_checkpoints: self.store.len(),
            ..Default::default()
        };
        for cp in self.store.values() {
            let cp_size: u64 = cp
                .stages
                .values()
                .map(|sc| sc.output_lines.iter().map(|l| l.len() as u64).sum::<u64>())
                .sum();
            s.total_size_bytes += cp_size;
            let ts = cp.created_at;
            s.oldest_timestamp = Some(s.oldest_timestamp.map_or(ts, |o: u64| o.min(ts)));
            s.newest_timestamp = Some(s.newest_timestamp.map_or(ts, |o: u64| o.max(ts)));
        }
        s
    }

    /// Prune old checkpoints to stay within max_checkpoints limit.
    fn prune(&mut self) {
        while self.store.len() > self.config.max_checkpoints {
            // Remove the oldest checkpoint
            let oldest = self
                .store
                .iter()
                .min_by_key(|(_, cp)| cp.created_at)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.store.remove(&id);
            } else {
                break;
            }
        }
    }

    /// Save all checkpoints to disk as JSON.
    pub fn save_to_disk(&self) -> Result<PathBuf> {
        let dir = self.checkpoint_dir();
        std::fs::create_dir_all(&dir).context("create checkpoint dir")?;
        let path = dir.join("checkpoints.json");
        let json = serde_json::to_string_pretty(&self.store).context("serialize checkpoints")?;
        std::fs::write(&path, json).context("write checkpoint file")?;
        Ok(path)
    }

    /// Load checkpoints from disk.
    pub fn load_from_disk(&mut self) -> Result<usize> {
        let path = self.checkpoint_dir().join("checkpoints.json");
        if !path.exists() {
            return Ok(0);
        }
        let data = std::fs::read_to_string(&path).context("read checkpoint file")?;
        let loaded: BTreeMap<CheckpointId, Checkpoint> =
            serde_json::from_str(&data).context("parse checkpoint file")?;
        let count = loaded.len();
        // Rebuild name index
        self.name_index.clear();
        for (id, cp) in &loaded {
            self.name_index.insert(cp.pipeline_name.clone(), id.clone());
        }
        self.store = loaded;
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> CheckpointManager {
        CheckpointManager::new(CheckpointConfig {
            cleanup_on_success: false,
            ..Default::default()
        })
    }

    #[test]
    fn test_fresh_start() {
        let mut m = mgr();
        let status = m.begin("test-pipe", "grep foo | sort", "input data", 3);
        assert!(matches!(status, ResumeStatus::Fresh));
        assert_eq!(m.list().len(), 1);
    }

    #[test]
    fn test_record_and_resume() {
        let mut m = mgr();
        let pipe_def = "grep foo | sort";
        let input = "line1\nline2";
        m.begin("p1", pipe_def, input, 3);
        let id = CheckpointManager::checkpoint_id(pipe_def, input);

        // Record stage 0
        m.record_stage(&id, 0, "grep", &["foo".into()]).unwrap();

        // Re-begin should resume from stage 1
        let status = m.begin("p1", pipe_def, input, 3);
        match status {
            ResumeStatus::ResumeFrom {
                stage_index,
                initial_lines,
            } => {
                assert_eq!(stage_index, 1);
                assert_eq!(initial_lines, vec!["foo"]);
            }
            _ => panic!("expected ResumeFrom, got {:?}", status),
        }
    }

    #[test]
    fn test_input_changed() {
        let mut m = mgr();
        let pipe_def = "grep foo";
        m.begin("p1", pipe_def, "input_v1", 2);
        let id = CheckpointManager::checkpoint_id(pipe_def, "input_v1");
        m.record_stage(&id, 0, "grep", &["foo".into()]).unwrap();

        // Change input
        let status = m.begin("p1", pipe_def, "input_v2", 2);
        assert!(matches!(status, ResumeStatus::InputChanged));
    }

    #[test]
    fn test_pipeline_changed() {
        let mut m = mgr();
        let input = "data";
        m.begin("p1", "grep foo", input, 2);
        let id = CheckpointManager::checkpoint_id("grep foo", input);
        m.record_stage(&id, 0, "grep", &["foo".into()]).unwrap();

        // Same pipeline name, same input, but different pipeline def
        // The name_index detects this and returns PipelineChanged
        let status2 = m.begin("p1", "grep bar", input, 2);
        assert!(
            matches!(status2, ResumeStatus::PipelineChanged),
            "expected PipelineChanged, got {:?}",
            status2
        );
    }

    #[test]
    fn test_complete_with_cleanup() {
        let mut m = CheckpointManager::new(CheckpointConfig {
            cleanup_on_success: true,
            ..Default::default()
        });
        let pipe_def = "sort";
        let input = "z\na\nm";
        m.begin("p1", pipe_def, input, 1);
        let id = CheckpointManager::checkpoint_id(pipe_def, input);
        m.complete(&id);
        assert!(m.list().is_empty());
    }

    #[test]
    fn test_complete_without_cleanup() {
        let mut m = mgr();
        let pipe_def = "sort";
        let input = "data";
        m.begin("p1", pipe_def, input, 1);
        let id = CheckpointManager::checkpoint_id(pipe_def, input);
        m.complete(&id);
        assert_eq!(m.list().len(), 1);
    }

    #[test]
    fn test_prune() {
        let mut m = CheckpointManager::new(CheckpointConfig {
            max_checkpoints: 2,
            cleanup_on_success: false,
            ..Default::default()
        });
        m.begin("p1", "def1", "in1", 1);
        m.begin("p2", "def2", "in2", 1);
        m.begin("p3", "def3", "in3", 1);
        assert!(m.list().len() <= 2);
    }

    #[test]
    fn test_stats() {
        let mut m = mgr();
        m.begin("p1", "def1", "in1", 2);
        let id = CheckpointManager::checkpoint_id("def1", "in1");
        m.record_stage(&id, 0, "grep", &["hello".into(), "world".into()])
            .unwrap();
        let s = m.stats();
        assert_eq!(s.total_checkpoints, 1);
        assert!(s.total_size_bytes > 0);
    }

    #[test]
    fn test_remove() {
        let mut m = mgr();
        m.begin("p1", "def1", "in1", 1);
        let id = CheckpointManager::checkpoint_id("def1", "in1");
        assert!(m.remove(&id));
        assert!(m.list().is_empty());
        assert!(!m.remove("nonexistent"));
    }

    #[test]
    fn test_clear() {
        let mut m = mgr();
        m.begin("p1", "def1", "in1", 1);
        m.begin("p2", "def2", "in2", 1);
        m.clear();
        assert!(m.list().is_empty());
    }

    #[test]
    fn test_disk_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let config = CheckpointConfig {
            checkpoint_dir: Some(dir.path().to_path_buf()),
            cleanup_on_success: false,
            ..Default::default()
        };
        let mut m = CheckpointManager::new(config.clone());
        m.begin("p1", "def1", "in1", 2);
        let id = CheckpointManager::checkpoint_id("def1", "in1");
        m.record_stage(&id, 0, "grep", &["line".into()]).unwrap();
        m.save_to_disk().unwrap();

        let mut m2 = CheckpointManager::new(config);
        let loaded = m2.load_from_disk().unwrap();
        assert_eq!(loaded, 1);
        let cp = m2.get(&id).unwrap();
        assert_eq!(cp.stages.len(), 1);
    }

    #[test]
    fn test_content_hash_consistency() {
        let h1 = sha256("hello world");
        let h2 = sha256("hello world");
        let h3 = sha256("different");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_multiple_stages() {
        let mut m = mgr();
        let pipe = "grep | sort | uniq";
        let input = "data";
        m.begin("p1", pipe, input, 3);
        let id = CheckpointManager::checkpoint_id(pipe, input);
        m.record_stage(&id, 0, "grep", &["a".into(), "b".into()])
            .unwrap();
        m.record_stage(&id, 1, "sort", &["a".into(), "b".into()])
            .unwrap();

        let cp = m.get(&id).unwrap();
        assert_eq!(cp.stages.len(), 2);

        // Resume should be from stage 2
        let status = m.begin("p1", pipe, input, 3);
        match status {
            ResumeStatus::ResumeFrom { stage_index, .. } => assert_eq!(stage_index, 2),
            _ => panic!("expected ResumeFrom"),
        }
    }
}
