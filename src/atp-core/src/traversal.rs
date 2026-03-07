//! File and directory traversal with scope control.
//!
//! Provides gitignore-aware, glob-filtered, depth-limited directory walking
//! for multi-file and multi-directory operations.

use crate::output::FileScope;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Scope filter configuration (mirrors FileScope but adds runtime state).
#[derive(Debug, Clone)]
pub struct ScopeFilter {
    pub scope: FileScope,
    include_set: Option<GlobSet>,
    exclude_set: Option<GlobSet>,
}

impl ScopeFilter {
    pub fn new(scope: FileScope) -> Result<Self> {
        let include_set = if scope.include_globs.is_empty() {
            None
        } else {
            let mut builder = GlobSetBuilder::new();
            for glob in &scope.include_globs {
                builder
                    .add(Glob::new(glob).with_context(|| format!("Invalid include glob: {glob}"))?);
            }
            Some(builder.build()?)
        };

        let exclude_set = if scope.exclude_globs.is_empty() {
            None
        } else {
            let mut builder = GlobSetBuilder::new();
            for glob in &scope.exclude_globs {
                builder
                    .add(Glob::new(glob).with_context(|| format!("Invalid exclude glob: {glob}"))?);
            }
            Some(builder.build()?)
        };

        Ok(Self {
            scope,
            include_set,
            exclude_set,
        })
    }

    /// Check if a path should be included.
    pub fn matches(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check exclude first
        if let Some(ref exclude) = self.exclude_set {
            if exclude.is_match(path) || exclude.is_match(path_str.as_ref()) {
                return false;
            }
        }

        // Check include (if specified, only matching files are included)
        if let Some(ref include) = self.include_set {
            return include.is_match(path) || include.is_match(path_str.as_ref());
        }

        true
    }
}

/// Results from a scope listing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeResults {
    pub files: Vec<String>,
    pub total_files: usize,
    pub total_size_bytes: u64,
}

/// Directory walker that respects scope configuration.
pub struct Walker {
    filter: ScopeFilter,
}

impl Walker {
    pub fn new(scope: FileScope) -> Result<Self> {
        let filter = ScopeFilter::new(scope)?;
        Ok(Self { filter })
    }

    pub fn from_filter(filter: ScopeFilter) -> Self {
        Self { filter }
    }

    /// Collect all matching file paths.
    pub fn collect_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for root in &self.filter.scope.roots {
            if root.is_file() {
                if self.filter.matches(root) {
                    files.push(root.clone());
                }
                continue;
            }

            let mut builder = WalkBuilder::new(root);
            builder.hidden(false);
            builder.git_ignore(self.filter.scope.respect_gitignore);
            builder.follow_links(self.filter.scope.follow_symlinks);

            if let Some(max_depth) = self.filter.scope.max_depth {
                builder.max_depth(Some(max_depth));
            }

            for entry in builder.build() {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() && self.filter.matches(path) {
                    files.push(path.to_path_buf());
                }
            }
        }

        // Sort for deterministic output
        files.sort();
        files.dedup();

        Ok(files)
    }

    /// List files with metadata for the scope command.
    pub fn list_scope(&self) -> Result<ScopeResults> {
        let files = self.collect_files()?;
        let mut total_size = 0u64;

        let file_strings: Vec<String> = files
            .iter()
            .map(|p| {
                if let Ok(meta) = std::fs::metadata(p) {
                    total_size += meta.len();
                }
                p.display().to_string()
            })
            .collect();

        let total_files = file_strings.len();
        Ok(ScopeResults {
            files: file_strings,
            total_files,
            total_size_bytes: total_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_collect_files_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("b.txt"), "world").unwrap();
        let scope = FileScope {
            roots: vec![dir.path().to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };
        let walker = Walker::new(scope).unwrap();
        let files = walker.collect_files().unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_include_glob_filter() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.rs"), "code").unwrap();
        fs::write(dir.path().join("file.txt"), "text").unwrap();
        let scope = FileScope {
            roots: vec![dir.path().to_path_buf()],
            include_globs: vec!["*.rs".to_string()],
            respect_gitignore: false,
            ..Default::default()
        };
        let walker = Walker::new(scope).unwrap();
        let files = walker.collect_files().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().ends_with(".rs"));
    }

    #[test]
    fn test_exclude_glob_filter() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("keep.rs"), "code").unwrap();
        fs::write(dir.path().join("skip.log"), "log").unwrap();
        let scope = FileScope {
            roots: vec![dir.path().to_path_buf()],
            exclude_globs: vec!["*.log".to_string()],
            respect_gitignore: false,
            ..Default::default()
        };
        let walker = Walker::new(scope).unwrap();
        let files = walker.collect_files().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().ends_with(".rs"));
    }

    #[test]
    fn test_max_depth() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        let deep = sub.join("deep");
        fs::create_dir(&deep).unwrap();
        fs::write(dir.path().join("top.txt"), "x").unwrap();
        fs::write(sub.join("mid.txt"), "x").unwrap();
        fs::write(deep.join("bot.txt"), "x").unwrap();

        let scope = FileScope {
            roots: vec![dir.path().to_path_buf()],
            max_depth: Some(1),
            respect_gitignore: false,
            ..Default::default()
        };
        let walker = Walker::new(scope).unwrap();
        let files = walker.collect_files().unwrap();
        // depth=1 should only get root-level files
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_list_scope() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let scope = FileScope {
            roots: vec![dir.path().to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };
        let walker = Walker::new(scope).unwrap();
        let results = walker.list_scope().unwrap();
        assert_eq!(results.total_files, 1);
        assert_eq!(results.total_size_bytes, 5); // "hello" = 5 bytes
    }

    #[test]
    fn test_single_file_root() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let scope = FileScope {
            roots: vec![tmp.path().to_path_buf()],
            respect_gitignore: false,
            ..Default::default()
        };
        let walker = Walker::new(scope).unwrap();
        let files = walker.collect_files().unwrap();
        assert_eq!(files.len(), 1);
    }
}
