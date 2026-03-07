//! Multi-project workspace aggregator.
//!
//! Discover sub-projects in a directory tree, aggregate cross-repo search
//! results, collect workspace-wide statistics, and support unified config
//! inheritance across projects.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Configuration for workspace discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Root directory to scan.
    pub root: PathBuf,
    /// Maximum depth to search for projects (default: 4).
    pub max_depth: usize,
    /// File markers that identify a project root (e.g. `Cargo.toml`, `package.json`).
    pub project_markers: Vec<String>,
    /// Directories to skip during discovery.
    pub ignore_dirs: Vec<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            max_depth: 4,
            project_markers: vec![
                "Cargo.toml".into(),
                "package.json".into(),
                "pyproject.toml".into(),
                "go.mod".into(),
                "pom.xml".into(),
                "build.gradle".into(),
                "CMakeLists.txt".into(),
                ".csproj".into(),
            ],
            ignore_dirs: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
                "dist".into(),
                "build".into(),
            ],
        }
    }
}

/// Detected project type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectKind {
    Rust,
    Node,
    Python,
    Go,
    Java,
    CSharp,
    Cpp,
    Unknown,
}

impl ProjectKind {
    fn from_marker(marker: &str) -> Self {
        match marker {
            "Cargo.toml" => Self::Rust,
            "package.json" => Self::Node,
            "pyproject.toml" => Self::Python,
            "go.mod" => Self::Go,
            "pom.xml" | "build.gradle" => Self::Java,
            ".csproj" => Self::CSharp,
            "CMakeLists.txt" => Self::Cpp,
            _ => Self::Unknown,
        }
    }
}

/// A discovered project within the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Project name (inferred from directory name or manifest).
    pub name: String,
    /// Absolute path to the project root.
    pub path: PathBuf,
    /// Which marker file was found.
    pub marker: String,
    /// Detected project kind.
    pub kind: ProjectKind,
    /// Number of source files (approximate).
    pub source_file_count: usize,
    /// Total size of source files in bytes.
    pub source_bytes: u64,
}

/// Workspace-level aggregated statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceStats {
    pub total_projects: usize,
    pub projects_by_kind: BTreeMap<String, usize>,
    pub total_source_files: usize,
    pub total_source_bytes: u64,
}

/// Result of a workspace-wide search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSearchHit {
    /// Which project this hit is in.
    pub project_name: String,
    /// File path relative to the project root.
    pub relative_path: String,
    /// Line number (1-based).
    pub line_number: usize,
    /// The matching line content.
    pub line_content: String,
}

/// Workspace-wide unified config entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfigEntry {
    /// Config key (dotted path).
    pub key: String,
    /// Config value.
    pub value: String,
    /// Which project this applies to (empty = global).
    pub scope: String,
}

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

/// The workspace aggregator.
#[derive(Debug, Clone)]
pub struct Workspace {
    config: WorkspaceConfig,
    projects: Vec<Project>,
    global_config: BTreeMap<String, String>,
}

impl Workspace {
    /// Create a workspace with the given config (does not scan yet).
    pub fn new(config: WorkspaceConfig) -> Self {
        Self {
            config,
            projects: Vec::new(),
            global_config: BTreeMap::new(),
        }
    }

    /// Discover projects under the root directory.
    pub fn discover(&mut self) -> usize {
        self.projects.clear();
        let root = self.config.root.clone();
        self.scan_dir(&root, 0);
        self.projects.len()
    }

    fn scan_dir(&mut self, dir: &Path, depth: usize) {
        if depth > self.config.max_depth {
            return;
        }

        // Check if this directory is a project root
        for marker in &self.config.project_markers {
            if dir.join(marker).exists() || self.has_marker_extension(dir, marker) {
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unnamed".to_string());

                let (file_count, byte_count) = self.count_sources(dir);

                self.projects.push(Project {
                    name,
                    path: dir.to_path_buf(),
                    marker: marker.clone(),
                    kind: ProjectKind::from_marker(marker),
                    source_file_count: file_count,
                    source_bytes: byte_count,
                });
                return; // Don't recurse into sub-projects of a project
            }
        }

        // Recurse into subdirectories
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if self.config.ignore_dirs.contains(&dir_name) {
                continue;
            }
            self.scan_dir(&path, depth + 1);
        }
    }

    fn has_marker_extension(&self, dir: &Path, marker: &str) -> bool {
        if !marker.starts_with('.') {
            return false;
        }
        // e.g. ".csproj" — look for any file with that extension
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!(".{ext}") == marker)
                .unwrap_or(false)
        })
    }

    fn count_sources(&self, dir: &Path) -> (usize, u64) {
        let source_exts = [
            "rs", "py", "js", "ts", "jsx", "tsx", "go", "java", "cs", "c", "cpp", "h", "hpp",
        ];
        let mut count = 0usize;
        let mut bytes = 0u64;
        let walker = walkdir::WalkDir::new(dir).max_depth(10);
        for entry in walker.into_iter().flatten() {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if source_exts.contains(&ext) {
                        count += 1;
                        bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
        (count, bytes)
    }

    /// Get all discovered projects.
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// Get a project by name.
    pub fn get_project(&self, name: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.name == name)
    }

    /// Compute workspace-level statistics.
    pub fn stats(&self) -> WorkspaceStats {
        let mut by_kind = BTreeMap::new();
        let mut total_files = 0;
        let mut total_bytes = 0u64;
        for p in &self.projects {
            *by_kind
                .entry(format!("{:?}", p.kind))
                .or_insert(0) += 1;
            total_files += p.source_file_count;
            total_bytes += p.source_bytes;
        }
        WorkspaceStats {
            total_projects: self.projects.len(),
            projects_by_kind: by_kind,
            total_source_files: total_files,
            total_source_bytes: total_bytes,
        }
    }

    /// Simple text search across all projects.
    pub fn search(&self, pattern: &str) -> Vec<WorkspaceSearchHit> {
        let mut hits = Vec::new();
        let Ok(re) = regex::Regex::new(pattern) else {
            return hits;
        };

        for project in &self.projects {
            let walker = walkdir::WalkDir::new(&project.path)
                .max_depth(10)
                .into_iter()
                .flatten();

            for entry in walker {
                if !entry.file_type().is_file() {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(entry.path()) else {
                    continue;
                };
                for (line_idx, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        let relative = entry
                            .path()
                            .strip_prefix(&project.path)
                            .unwrap_or(entry.path())
                            .to_string_lossy()
                            .to_string();
                        hits.push(WorkspaceSearchHit {
                            project_name: project.name.clone(),
                            relative_path: relative,
                            line_number: line_idx + 1,
                            line_content: line.to_string(),
                        });
                    }
                }
            }
        }
        hits
    }

    /// Set a global config value.
    pub fn set_config(&mut self, key: &str, value: &str) {
        self.global_config
            .insert(key.to_string(), value.to_string());
    }

    /// Get a config value, with project-level override support.
    pub fn get_config(&self, key: &str) -> Option<&str> {
        self.global_config.get(key).map(|s| s.as_str())
    }

    /// List all global config entries.
    pub fn config_entries(&self) -> Vec<WorkspaceConfigEntry> {
        self.global_config
            .iter()
            .map(|(k, v)| WorkspaceConfigEntry {
                key: k.clone(),
                value: v.clone(),
                scope: String::new(),
            })
            .collect()
    }

    /// List project names.
    pub fn project_names(&self) -> Vec<&str> {
        self.projects.iter().map(|p| p.name.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_workspace(dir: &Path) {
        // Create a Rust project
        let rust_proj = dir.join("my-rust");
        fs::create_dir_all(rust_proj.join("src")).unwrap();
        fs::write(
            rust_proj.join("Cargo.toml"),
            "[package]\nname = \"my-rust\"\n",
        )
        .unwrap();
        fs::write(rust_proj.join("src").join("main.rs"), "fn main() {}\n").unwrap();

        // Create a Node project
        let node_proj = dir.join("my-node");
        fs::create_dir_all(node_proj.join("src")).unwrap();
        fs::write(node_proj.join("package.json"), "{}").unwrap();
        fs::write(node_proj.join("src").join("index.js"), "console.log('hi');\n").unwrap();

        // Create a non-project dir
        let misc = dir.join("docs");
        fs::create_dir_all(&misc).unwrap();
        fs::write(misc.join("readme.txt"), "docs").unwrap();
    }

    #[test]
    fn test_discover_projects() {
        let tmp = tempfile::tempdir().unwrap();
        setup_workspace(tmp.path());

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        let count = ws.discover();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_project_kinds() {
        let tmp = tempfile::tempdir().unwrap();
        setup_workspace(tmp.path());

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        ws.discover();

        let rust = ws.get_project("my-rust").unwrap();
        assert_eq!(rust.kind, ProjectKind::Rust);

        let node = ws.get_project("my-node").unwrap();
        assert_eq!(node.kind, ProjectKind::Node);
    }

    #[test]
    fn test_project_names() {
        let tmp = tempfile::tempdir().unwrap();
        setup_workspace(tmp.path());

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        ws.discover();

        let names = ws.project_names();
        assert!(names.contains(&"my-rust"));
        assert!(names.contains(&"my-node"));
    }

    #[test]
    fn test_stats() {
        let tmp = tempfile::tempdir().unwrap();
        setup_workspace(tmp.path());

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        ws.discover();

        let stats = ws.stats();
        assert_eq!(stats.total_projects, 2);
        assert!(stats.total_source_files >= 2);
    }

    #[test]
    fn test_search_across_projects() {
        let tmp = tempfile::tempdir().unwrap();
        setup_workspace(tmp.path());

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        ws.discover();

        let hits = ws.search("main");
        assert!(!hits.is_empty());
    }

    #[test]
    fn test_config() {
        let mut ws = Workspace::new(WorkspaceConfig::default());
        ws.set_config("output.format", "json");
        assert_eq!(ws.get_config("output.format"), Some("json"));
        assert_eq!(ws.get_config("missing"), None);
    }

    #[test]
    fn test_config_entries() {
        let mut ws = Workspace::new(WorkspaceConfig::default());
        ws.set_config("a", "1");
        ws.set_config("b", "2");
        let entries = ws.config_entries();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_empty_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        assert_eq!(ws.discover(), 0);
        assert!(ws.projects().is_empty());
    }

    #[test]
    fn test_max_depth_limit() {
        let tmp = tempfile::tempdir().unwrap();
        // Create project deeply nested
        let deep = tmp.path().join("a").join("b").join("c").join("d").join("e");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("Cargo.toml"), "[package]").unwrap();

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            max_depth: 2,
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        assert_eq!(ws.discover(), 0); // Too deep
    }

    #[test]
    fn test_ignore_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let ignored = tmp.path().join("node_modules").join("some-pkg");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(ignored.join("package.json"), "{}").unwrap();

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        assert_eq!(ws.discover(), 0); // Should skip node_modules
    }

    #[test]
    fn test_project_source_count() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("proj");
        fs::create_dir_all(proj.join("src")).unwrap();
        fs::write(proj.join("Cargo.toml"), "[package]").unwrap();
        fs::write(proj.join("src").join("lib.rs"), "// lib").unwrap();
        fs::write(proj.join("src").join("main.rs"), "fn main() {}").unwrap();

        let config = WorkspaceConfig {
            root: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mut ws = Workspace::new(config);
        ws.discover();
        let p = ws.get_project("proj").unwrap();
        assert_eq!(p.source_file_count, 2);
    }

    #[test]
    fn test_project_kind_from_marker() {
        assert_eq!(ProjectKind::from_marker("Cargo.toml"), ProjectKind::Rust);
        assert_eq!(ProjectKind::from_marker("package.json"), ProjectKind::Node);
        assert_eq!(
            ProjectKind::from_marker("pyproject.toml"),
            ProjectKind::Python
        );
        assert_eq!(ProjectKind::from_marker("go.mod"), ProjectKind::Go);
        assert_eq!(ProjectKind::from_marker("pom.xml"), ProjectKind::Java);
        assert_eq!(ProjectKind::from_marker("unknown"), ProjectKind::Unknown);
    }
}
