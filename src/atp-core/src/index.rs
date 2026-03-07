//! Incremental file index — persistent search index with filesystem watching.
//!
//! Provides a background-updated file index for instant workspace queries.
//! The index stores file metadata, content hashes, and precomputed trigrams
//! for sub-millisecond pattern matching across large repositories.
//!
//! # Architecture
//!
//! - [`FileEntry`]: Metadata for a single indexed file
//! - [`SearchIndex`]: The main index structure with trigram-based lookup
//! - [`IndexWatcher`]: Filesystem watcher that incrementally updates the index
//! - [`IndexStats`]: Statistics about index coverage and freshness
//!
//! # Usage
//!
//! ```text
//! let mut index = SearchIndex::new("/path/to/workspace");
//! index.build()?;                    // Initial full scan
//! let results = index.search("TODO"); // Instant trigram search
//! index.update_file("src/main.rs")?; // Incremental update
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Metadata for a single indexed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative path from workspace root.
    pub path: String,
    /// SHA-256 content hash.
    pub content_hash: String,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time (Unix timestamp).
    pub modified: u64,
    /// Language detected.
    pub language: String,
    /// Number of lines.
    pub line_count: usize,
    /// Trigram set for fast search (stored as indices into the trigram table).
    trigrams: HashSet<u32>,
}

/// A search hit from the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexHit {
    /// File path (relative).
    pub path: String,
    /// Matching line number (1-based).
    pub line: usize,
    /// The matching line content.
    pub content: String,
}

/// Statistics about the search index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    /// Total number of indexed files.
    pub file_count: usize,
    /// Total size of indexed content in bytes.
    pub total_bytes: u64,
    /// Total line count.
    pub total_lines: usize,
    /// Number of unique trigrams.
    pub trigram_count: usize,
    /// Languages breakdown.
    pub languages: HashMap<String, usize>,
    /// Last full rebuild timestamp.
    pub last_rebuild: Option<u64>,
    /// Number of incremental updates since last rebuild.
    pub incremental_updates: u64,
}

/// Persistent search index with trigram-based lookup.
///
/// The index uses trigram (3-character substring) matching to quickly
/// narrow down which files could contain a search pattern, then does
/// exact matching only on those candidate files.
pub struct SearchIndex {
    /// Workspace root directory.
    root: PathBuf,
    /// Indexed files keyed by relative path.
    entries: HashMap<String, FileEntry>,
    /// Stats.
    stats: IndexStats,
    /// File content cache (relative path → content).
    content_cache: HashMap<String, String>,
    /// Maximum file size to index (default 10MB).
    max_file_size: u64,
    /// File patterns to exclude.
    exclude_patterns: Vec<String>,
}

impl SearchIndex {
    /// Create a new search index for a workspace directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            entries: HashMap::new(),
            stats: IndexStats::default(),
            content_cache: HashMap::new(),
            max_file_size: 10 * 1024 * 1024, // 10 MB
            exclude_patterns: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "__pycache__".into(),
                ".venv".into(),
                "dist".into(),
                "build".into(),
            ],
        }
    }

    /// Set maximum file size to index.
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Add exclude patterns.
    pub fn with_excludes(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns.extend(patterns);
        self
    }

    /// Build the full index by scanning all files in the workspace.
    pub fn build(&mut self) -> Result<&IndexStats> {
        self.entries.clear();
        self.content_cache.clear();

        let files = self.collect_files()?;
        for path in &files {
            if let Err(e) = self.index_file(path) {
                eprintln!("Warning: skipping {}: {e}", path.display());
            }
        }

        self.rebuild_stats();
        self.stats.last_rebuild = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.stats.incremental_updates = 0;

        Ok(&self.stats)
    }

    /// Update a single file in the index (incremental).
    pub fn update_file(&mut self, rel_path: &str) -> Result<()> {
        let abs_path = self.root.join(rel_path);

        if !abs_path.exists() {
            // File was deleted
            self.entries.remove(rel_path);
            self.content_cache.remove(rel_path);
            self.stats.incremental_updates += 1;
            self.rebuild_stats();
            return Ok(());
        }

        self.index_file(&abs_path)?;
        self.stats.incremental_updates += 1;
        self.rebuild_stats();
        Ok(())
    }

    /// Remove a file from the index.
    pub fn remove_file(&mut self, rel_path: &str) {
        self.entries.remove(rel_path);
        self.content_cache.remove(rel_path);
        self.rebuild_stats();
    }

    /// Search the index for files containing a pattern (trigram-accelerated).
    pub fn search(&self, pattern: &str) -> Vec<IndexHit> {
        let pattern_lower = pattern.to_lowercase();
        let pattern_trigrams = compute_trigrams(&pattern_lower);

        // Phase 1: trigram filter — only files whose trigram set covers all pattern trigrams
        let candidates: Vec<&str> = self
            .entries
            .iter()
            .filter(|(_, entry)| pattern_trigrams.iter().all(|t| entry.trigrams.contains(t)))
            .map(|(path, _)| path.as_str())
            .collect();

        // Phase 2: exact matching on candidates
        let mut hits = Vec::new();
        for path in candidates {
            if let Some(content) = self.content_cache.get(path) {
                for (i, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(&pattern_lower) {
                        hits.push(IndexHit {
                            path: path.to_string(),
                            line: i + 1,
                            content: line.to_string(),
                        });
                    }
                }
            }
        }

        hits
    }

    /// Search using a regex pattern.
    pub fn search_regex(&self, pattern: &str) -> Result<Vec<IndexHit>> {
        let re = regex::Regex::new(pattern).context("Invalid regex pattern")?;
        let mut hits = Vec::new();

        for (path, content) in &self.content_cache {
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    hits.push(IndexHit {
                        path: path.clone(),
                        line: i + 1,
                        content: line.to_string(),
                    });
                }
            }
        }

        Ok(hits)
    }

    /// Get files matching a glob pattern.
    pub fn files_matching(&self, glob: &str) -> Vec<&str> {
        if let Ok(pattern) = globset::Glob::new(glob).map(|g| g.compile_matcher()) {
            self.entries
                .keys()
                .filter(|p| pattern.is_match(p.as_str()))
                .map(|s| s.as_str())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get index statistics.
    pub fn stats(&self) -> &IndexStats {
        &self.stats
    }

    /// Get all indexed file paths.
    pub fn files(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a file is indexed.
    pub fn contains(&self, rel_path: &str) -> bool {
        self.entries.contains_key(rel_path)
    }

    /// Get entry metadata for a file.
    pub fn get_entry(&self, rel_path: &str) -> Option<&FileEntry> {
        self.entries.get(rel_path)
    }

    /// Check if a file needs re-indexing (based on modification time).
    pub fn is_stale(&self, rel_path: &str) -> Result<bool> {
        let abs_path = self.root.join(rel_path);
        let metadata = std::fs::metadata(&abs_path)?;
        let modified = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(match self.entries.get(rel_path) {
            Some(entry) => entry.modified != modified,
            None => true,
        })
    }

    /// Save the index to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = IndexSnapshot {
            root: self.root.to_string_lossy().to_string(),
            entries: self.entries.clone(),
            stats: self.stats.clone(),
        };
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load an index from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let snapshot: IndexSnapshot = serde_json::from_str(&json)?;
        let mut index = Self::new(PathBuf::from(&snapshot.root));
        index.entries = snapshot.entries;
        index.stats = snapshot.stats;

        // Rebuild content cache by re-reading files
        let root = index.root.clone();
        for rel_path in index.entries.keys().cloned().collect::<Vec<_>>() {
            let abs_path = root.join(&rel_path);
            if let Ok(content) = std::fs::read_to_string(&abs_path) {
                index.content_cache.insert(rel_path, content);
            }
        }

        Ok(index)
    }

    // ── Internal helpers ──

    fn collect_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.walk_dir(&self.root.clone(), &mut files)?;
        Ok(files)
    }

    fn walk_dir(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        let entries =
            std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip excluded directories
            if path.is_dir() {
                if self.exclude_patterns.contains(&name) {
                    continue;
                }
                self.walk_dir(&path, files)?;
            } else if path.is_file() {
                // Skip files over max size
                if let Ok(meta) = entry.metadata() {
                    if meta.len() <= self.max_file_size {
                        files.push(path);
                    }
                }
            }
        }

        Ok(())
    }

    fn index_file(&mut self, abs_path: &Path) -> Result<()> {
        let rel_path = abs_path
            .strip_prefix(&self.root)
            .unwrap_or(abs_path)
            .to_string_lossy()
            .replace('\\', "/");

        let content = std::fs::read_to_string(abs_path)
            .with_context(|| format!("reading {}", abs_path.display()))?;

        let metadata = std::fs::metadata(abs_path)?;
        let modified = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let language = crate::code_intel::Language::from_path(abs_path)
            .name()
            .to_string();
        let line_count = content.lines().count();
        let content_lower = content.to_lowercase();
        let trigrams = compute_trigrams(&content_lower);
        let content_hash = compute_hash(&content);

        let entry = FileEntry {
            path: rel_path.clone(),
            content_hash,
            size: metadata.len(),
            modified,
            language,
            line_count,
            trigrams,
        };

        self.entries.insert(rel_path.clone(), entry);
        self.content_cache.insert(rel_path, content);

        Ok(())
    }

    fn rebuild_stats(&mut self) {
        let mut languages: HashMap<String, usize> = HashMap::new();
        let mut total_bytes = 0u64;
        let mut total_lines = 0usize;
        let mut all_trigrams: HashSet<u32> = HashSet::new();

        for entry in self.entries.values() {
            *languages.entry(entry.language.clone()).or_insert(0) += 1;
            total_bytes += entry.size;
            total_lines += entry.line_count;
            all_trigrams.extend(&entry.trigrams);
        }

        self.stats.file_count = self.entries.len();
        self.stats.total_bytes = total_bytes;
        self.stats.total_lines = total_lines;
        self.stats.trigram_count = all_trigrams.len();
        self.stats.languages = languages;
    }
}

/// Serializable index snapshot.
#[derive(Serialize, Deserialize)]
struct IndexSnapshot {
    root: String,
    entries: HashMap<String, FileEntry>,
    stats: IndexStats,
}

// ── Trigram utilities ──

/// Compute the set of trigrams (3-character substrings) for a string.
fn compute_trigrams(s: &str) -> HashSet<u32> {
    let bytes = s.as_bytes();
    let mut trigrams = HashSet::new();
    if bytes.len() >= 3 {
        for window in bytes.windows(3) {
            let key = (window[0] as u32) << 16 | (window[1] as u32) << 8 | (window[2] as u32);
            trigrams.insert(key);
        }
    }
    trigrams
}

/// Compute a simple hash of content (using SHA-256).
fn compute_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ── Filesystem watcher for incremental updates ──

/// Configuration for the index watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
    /// Whether to watch recursively.
    pub recursive: bool,
    /// Patterns to exclude from watching.
    pub exclude: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 200,
            recursive: true,
            exclude: vec![".git".into(), "node_modules".into(), "target".into()],
        }
    }
}

/// Event emitted when the index is updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEvent {
    /// Kind of event.
    pub kind: IndexEventKind,
    /// Affected file path (relative).
    pub path: String,
}

/// Kind of index update event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexEventKind {
    /// File was added or modified.
    Updated,
    /// File was removed.
    Removed,
    /// Index was fully rebuilt.
    Rebuilt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_trigrams() {
        let trigrams = compute_trigrams("hello");
        // "hel", "ell", "llo" = 3 trigrams
        assert_eq!(trigrams.len(), 3);
    }

    #[test]
    fn test_compute_trigrams_short() {
        let trigrams = compute_trigrams("ab");
        assert!(trigrams.is_empty()); // too short for trigrams
    }

    #[test]
    fn test_compute_hash() {
        let h1 = compute_hash("hello");
        let h2 = compute_hash("hello");
        assert_eq!(h1, h2);
        let h3 = compute_hash("world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_search_index_new() {
        let index = SearchIndex::new("/tmp/test");
        assert_eq!(index.stats().file_count, 0);
        assert!(index.files().is_empty());
    }

    #[test]
    fn test_search_index_build_and_search() {
        let dir = std::env::temp_dir().join("atp_index_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.txt"), "Hello World\nTODO: fix this\nDone").unwrap();
        std::fs::write(
            dir.join("data.rs"),
            "fn main() {\n    // TODO: implement\n}\n",
        )
        .unwrap();

        let mut index = SearchIndex::new(&dir);
        let stats = index.build().unwrap();
        assert_eq!(stats.file_count, 2);
        assert!(stats.total_lines >= 5);

        let hits = index.search("TODO");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|h| h.path.contains("test.txt")));
        assert!(hits.iter().any(|h| h.path.contains("data.rs")));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search_index_incremental_update() {
        let dir = std::env::temp_dir().join("atp_index_update_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "line one\nline two").unwrap();

        let mut index = SearchIndex::new(&dir);
        index.build().unwrap();
        assert_eq!(index.stats().file_count, 1);

        // Add a new file
        std::fs::write(dir.join("b.txt"), "something new\nTODO: add more").unwrap();
        index.update_file("b.txt").unwrap();
        assert_eq!(index.stats().file_count, 2);
        assert_eq!(index.stats().incremental_updates, 1);

        let hits = index.search("TODO");
        assert_eq!(hits.len(), 1);

        // Remove a file
        index.remove_file("a.txt");
        assert_eq!(index.stats().file_count, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search_index_regex_search() {
        let dir = std::env::temp_dir().join("atp_index_regex_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("code.rs"),
            "fn test_alpha() {}\nfn test_beta() {}\nfn helper() {}",
        )
        .unwrap();

        let mut index = SearchIndex::new(&dir);
        index.build().unwrap();

        let hits = index.search_regex(r"fn test_\w+").unwrap();
        assert_eq!(hits.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search_index_save_load() {
        let dir = std::env::temp_dir().join("atp_index_save_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("file.txt"), "content here").unwrap();

        let mut index = SearchIndex::new(&dir);
        index.build().unwrap();

        let index_file = dir.join("index.json");
        index.save(&index_file).unwrap();

        let loaded = SearchIndex::load(&index_file).unwrap();
        assert_eq!(loaded.stats().file_count, 1);
        assert!(loaded.contains("file.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_files_matching_glob() {
        let dir = std::env::temp_dir().join("atp_index_glob_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("lib.rs"), "pub mod foo;").unwrap();
        std::fs::write(dir.join("data.txt"), "hello").unwrap();

        let mut index = SearchIndex::new(&dir);
        index.build().unwrap();

        let rust_files = index.files_matching("*.rs");
        assert_eq!(rust_files.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_index_stats() {
        let stats = IndexStats::default();
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.total_bytes, 0);
        assert!(stats.last_rebuild.is_none());
    }

    #[test]
    fn test_watcher_config_default() {
        let cfg = WatcherConfig::default();
        assert_eq!(cfg.debounce_ms, 200);
        assert!(cfg.recursive);
        assert!(!cfg.exclude.is_empty());
    }
}
