//! Incremental pipeline compilation cache.
//!
//! Content-addressed cache for pipeline stage results. Each stage execution is
//! keyed by SHA-256(stage_definition + input_content), allowing unchanged stages
//! to be skipped on re-execution. Cache entries are stored under `~/.atp/cache/`
//! with LRU eviction when the directory exceeds a configurable size limit.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Configuration for the pipeline cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled (default: true).
    pub enabled: bool,
    /// Maximum cache size in bytes (default: 256 MiB).
    pub max_size_bytes: u64,
    /// Cache directory (default: `~/.atp/cache`).
    pub cache_dir: Option<PathBuf>,
    /// Time-to-live in seconds (default: 86400 = 24h). 0 = no expiry.
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_bytes: 256 * 1024 * 1024,
            cache_dir: None,
            ttl_seconds: 86400,
        }
    }
}

/// A single cache entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// SHA-256 key for this entry.
    pub key: String,
    /// The cached stage output lines.
    pub output_lines: Vec<String>,
    /// When this entry was created.
    pub created_at: u64,
    /// Size in bytes of the stored output.
    pub size_bytes: u64,
    /// Stage index and type that produced this entry.
    pub stage_info: String,
}

/// Cache statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of entries in the cache.
    pub entries: usize,
    /// Total size in bytes.
    pub total_size_bytes: u64,
    /// Number of cache hits since last reset.
    pub hits: u64,
    /// Number of cache misses since last reset.
    pub misses: u64,
    /// Hit rate as a percentage.
    pub hit_rate_percent: f64,
    /// Maximum configured size.
    pub max_size_bytes: u64,
}

/// Content-addressed pipeline stage cache.
pub struct PipelineCache {
    config: CacheConfig,
    /// In-memory index: key → entry metadata.
    index: BTreeMap<String, CacheEntry>,
    /// Hit/miss counters.
    hits: u64,
    misses: u64,
    /// Cache directory path (resolved).
    cache_dir: PathBuf,
}

impl PipelineCache {
    /// Create a new cache with the given configuration.
    pub fn new(config: CacheConfig) -> Result<Self> {
        let cache_dir = match &config.cache_dir {
            Some(dir) => dir.clone(),
            None => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".atp").join("cache")
            }
        };
        Ok(Self {
            config,
            index: BTreeMap::new(),
            hits: 0,
            misses: 0,
            cache_dir,
        })
    }

    /// Create a cache that operates purely in-memory (for testing).
    pub fn in_memory() -> Self {
        Self {
            config: CacheConfig {
                enabled: true,
                max_size_bytes: 1024 * 1024,
                cache_dir: None,
                ttl_seconds: 0,
            },
            index: BTreeMap::new(),
            hits: 0,
            misses: 0,
            cache_dir: PathBuf::from("/tmp/atp-cache-test"),
        }
    }

    /// Compute a cache key from stage definition + input data.
    pub fn compute_key(stage_definition: &str, input_data: &[String]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(stage_definition.as_bytes());
        hasher.update(b"|");
        for line in input_data {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        format!("{:x}", hasher.finalize())
    }

    /// Look up a cached result by key.
    pub fn get(&mut self, key: &str) -> Option<&CacheEntry> {
        if !self.config.enabled {
            self.misses += 1;
            return None;
        }

        // Check TTL
        if self.config.ttl_seconds > 0 {
            if let Some(entry) = self.index.get(key) {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now - entry.created_at > self.config.ttl_seconds {
                    self.index.remove(key);
                    self.misses += 1;
                    return None;
                }
            }
        }

        if self.index.contains_key(key) {
            self.hits += 1;
            self.index.get(key)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Store a result in the cache.
    pub fn put(&mut self, key: String, output_lines: Vec<String>, stage_info: String) {
        if !self.config.enabled {
            return;
        }

        let size_bytes: u64 = output_lines.iter().map(|l| l.len() as u64 + 1).sum();

        // Evict if necessary
        self.evict_if_needed(size_bytes);

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntry {
            key: key.clone(),
            output_lines,
            created_at: now,
            size_bytes,
            stage_info,
        };

        self.index.insert(key, entry);
    }

    /// Evict oldest entries until we have room for `needed_bytes`.
    fn evict_if_needed(&mut self, needed_bytes: u64) {
        let total: u64 = self.index.values().map(|e| e.size_bytes).sum();
        if total + needed_bytes <= self.config.max_size_bytes {
            return;
        }

        // Collect entries sorted by creation time (oldest first)
        let mut entries: Vec<(String, u64, u64)> = self
            .index
            .iter()
            .map(|(k, e)| (k.clone(), e.created_at, e.size_bytes))
            .collect();
        entries.sort_by_key(|(_, ts, _)| *ts);

        let mut freed = 0u64;
        let target = (total + needed_bytes).saturating_sub(self.config.max_size_bytes);
        for (key, _, size) in entries {
            if freed >= target {
                break;
            }
            self.index.remove(&key);
            freed += size;
        }
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.index.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let total_size_bytes: u64 = self.index.values().map(|e| e.size_bytes).sum();
        let total = self.hits + self.misses;
        let hit_rate_percent = if total > 0 {
            (self.hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        CacheStats {
            entries: self.index.len(),
            total_size_bytes,
            hits: self.hits,
            misses: self.misses,
            hit_rate_percent,
            max_size_bytes: self.config.max_size_bytes,
        }
    }

    /// Save the cache index to disk.
    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("Cannot create cache dir: {:?}", self.cache_dir))?;
        let index_path = self.cache_dir.join("index.json");
        let json =
            serde_json::to_string_pretty(&self.index).context("Failed to serialize cache index")?;
        std::fs::write(&index_path, json).context("Failed to write cache index")?;
        Ok(())
    }

    /// Load the cache index from disk.
    pub fn load(&mut self) -> Result<()> {
        let index_path = self.cache_dir.join("index.json");
        if !index_path.exists() {
            return Ok(());
        }
        let json = std::fs::read_to_string(&index_path).context("Failed to read cache index")?;
        self.index = serde_json::from_str(&json).context("Failed to deserialize cache index")?;
        Ok(())
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_key_deterministic() {
        let k1 = PipelineCache::compute_key("search:TODO", &["line1".into(), "line2".into()]);
        let k2 = PipelineCache::compute_key("search:TODO", &["line1".into(), "line2".into()]);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_compute_key_different_inputs() {
        let k1 = PipelineCache::compute_key("search:TODO", &["a".into()]);
        let k2 = PipelineCache::compute_key("search:TODO", &["b".into()]);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_compute_key_different_stages() {
        let k1 = PipelineCache::compute_key("search:TODO", &["a".into()]);
        let k2 = PipelineCache::compute_key("search:FIXME", &["a".into()]);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_put_and_get() {
        let mut cache = PipelineCache::in_memory();
        let key = PipelineCache::compute_key("search:X", &["input".into()]);
        cache.put(
            key.clone(),
            vec!["result1".into(), "result2".into()],
            "Search".into(),
        );

        let entry = cache.get(&key).unwrap();
        assert_eq!(entry.output_lines, vec!["result1", "result2"]);
        assert_eq!(entry.stage_info, "Search");
    }

    #[test]
    fn test_get_miss() {
        let mut cache = PipelineCache::in_memory();
        assert!(cache.get("nonexistent").is_none());
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = PipelineCache::in_memory();
        let key = PipelineCache::compute_key("s", &["x".into()]);
        cache.put(key.clone(), vec!["out".into()], "Search".into());
        cache.get(&key); // hit
        cache.get("nope"); // miss

        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate_percent - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = PipelineCache::in_memory();
        cache.put("k1".into(), vec!["v".into()], "s".into());
        cache.put("k2".into(), vec!["v".into()], "s".into());
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = PipelineCache {
            config: CacheConfig {
                enabled: true,
                max_size_bytes: 50,
                cache_dir: None,
                ttl_seconds: 0,
            },
            index: BTreeMap::new(),
            hits: 0,
            misses: 0,
            cache_dir: PathBuf::from("/tmp/test"),
        };

        // Insert entries that exceed max_size_bytes
        cache.put(
            "k1".into(),
            vec!["abcdefghijklmnopqrstuvwxyz".into()],
            "s".into(),
        ); // ~27 bytes
        cache.put(
            "k2".into(),
            vec!["abcdefghijklmnopqrstuvwxyz".into()],
            "s".into(),
        ); // ~27 more → exceeds 50 → k1 evicted

        // k1 should have been evicted to make room
        assert!(cache.get("k1").is_none());
        assert!(cache.get("k2").is_some());
    }

    #[test]
    fn test_disabled_cache() {
        let mut cache = PipelineCache {
            config: CacheConfig {
                enabled: false,
                ..CacheConfig::default()
            },
            index: BTreeMap::new(),
            hits: 0,
            misses: 0,
            cache_dir: PathBuf::from("/tmp/test"),
        };

        cache.put("k".into(), vec!["v".into()], "s".into());
        assert!(cache.is_empty());
        assert!(cache.get("k").is_none());
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = PipelineCache::new(CacheConfig {
            enabled: true,
            max_size_bytes: 1024 * 1024,
            cache_dir: Some(dir.path().to_path_buf()),
            ttl_seconds: 0,
        })
        .unwrap();

        cache.put("key1".into(), vec!["hello".into()], "Search".into());
        cache.put("key2".into(), vec!["world".into()], "Filter".into());
        cache.save().unwrap();

        let mut cache2 = PipelineCache::new(CacheConfig {
            enabled: true,
            max_size_bytes: 1024 * 1024,
            cache_dir: Some(dir.path().to_path_buf()),
            ttl_seconds: 0,
        })
        .unwrap();
        cache2.load().unwrap();
        assert_eq!(cache2.len(), 2);
        assert_eq!(
            cache2.get("key1").unwrap().output_lines,
            vec!["hello".to_string()]
        );
    }
}
