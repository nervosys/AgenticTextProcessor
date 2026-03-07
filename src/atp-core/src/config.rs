//! User configuration system for ATP.
//!
//! Loads defaults from `~/.atp/config.toml` (global), `.atprc` (per-project),
//! and `.atp.toml` (project-level, v2).
//!
//! Merge order (later wins):
//! 1. Global `~/.atp/config.toml`
//! 2. Per-project `.atprc` (walks up from cwd)
//! 3. Project-level `.atp.toml` (walks up from cwd)
//! 4. CLI flags (handled by caller)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AtpConfig {
    /// Default output format ("human", "json", "csv", "yaml", "jsonl").
    #[serde(default)]
    pub format: Option<String>,

    /// Disable colour output globally.
    #[serde(default)]
    pub no_color: bool,

    /// Verbose / debug output.
    #[serde(default)]
    pub verbose: bool,

    /// Default field separator for AQL / awk.
    #[serde(default)]
    pub separator: Option<String>,

    /// Search settings.
    #[serde(default)]
    pub search: SearchConfig,

    /// Compliance settings.
    #[serde(default)]
    pub compliance: ComplianceConfig,

    /// Telemetry settings.
    #[serde(default)]
    pub telemetry: TelemetryConfig,

    /// Plugin settings.
    #[serde(default)]
    pub plugin: PluginConfig,

    /// Pipeline defaults (v2).
    #[serde(default)]
    pub pipeline: PipelineDefaults,

    /// Scope / file-selection defaults (v2).
    #[serde(default)]
    pub scope: ScopeDefaults,
}

/// Search-related defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SearchConfig {
    /// Search case-insensitively by default.
    #[serde(default)]
    pub case_insensitive: bool,

    /// Number of context lines before a match.
    #[serde(default)]
    pub context_before: Option<usize>,

    /// Number of context lines after a match.
    #[serde(default)]
    pub context_after: Option<usize>,

    /// Maximum number of matches.
    #[serde(default)]
    pub max_matches: Option<usize>,

    /// Respect `.gitignore` by default.
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,

    /// Maximum traversal depth.
    #[serde(default)]
    pub max_depth: Option<usize>,

    /// Glob patterns to exclude.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Glob patterns to include (if non-empty, only these are searched).
    #[serde(default)]
    pub include: Vec<String>,
}

/// Compliance defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ComplianceConfig {
    /// Default classification marking (e.g. "CUI", "FOUO", "PUBLIC").
    #[serde(default)]
    pub default_marking: Option<String>,

    /// Organization name for compliance reports.
    #[serde(default)]
    pub organization: Option<String>,
}

/// Telemetry defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TelemetryConfig {
    /// Enable usage telemetry.
    #[serde(default)]
    pub enabled: bool,
}

/// Plugin defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PluginConfig {
    /// Extra directories to search for plugins.
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
}

/// Pipeline configuration (v2: from `.atp.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PipelineDefaults {
    /// Default streaming channel capacity.
    #[serde(default)]
    pub channel_capacity: Option<usize>,
    /// Default streaming batch size.
    #[serde(default)]
    pub batch_size: Option<usize>,
    /// Named saved pipelines (name → DSL string).
    #[serde(default)]
    pub saved: std::collections::HashMap<String, String>,
}

/// Scope / file-selection defaults (v2: from `.atp.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ScopeDefaults {
    /// Root directories to search (relative to project root).
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    /// File extensions to include (e.g. `["rs", "py", "ts"]`).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Whether to follow symlinks.
    #[serde(default)]
    pub follow_symlinks: bool,
}

fn default_true() -> bool {
    true
}

impl AtpConfig {
    /// Load configuration by merging (in order of precedence):
    /// 1. Global `~/.atp/config.toml`
    /// 2. Per-project `.atprc` (walks up from `cwd`)
    ///
    /// CLI flags override both (handled by the caller).
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // 1. Global config
        if let Some(global) = global_config_path() {
            if global.exists() {
                let content = std::fs::read_to_string(&global)
                    .with_context(|| format!("reading {}", global.display()))?;
                let parsed: AtpConfig = toml::from_str(&content)
                    .with_context(|| format!("parsing {}", global.display()))?;
                config = config.merge(parsed);
            }
        }

        // 2. Per-project .atprc
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(rc) = find_rc_file(&cwd) {
                let content = std::fs::read_to_string(&rc)
                    .with_context(|| format!("reading {}", rc.display()))?;
                let parsed: AtpConfig = toml::from_str(&content)
                    .with_context(|| format!("parsing {}", rc.display()))?;
                config = config.merge(parsed);
            }
        }

        // 3. Project-level .atp.toml (v2)
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(project) = find_project_config(&cwd) {
                let content = std::fs::read_to_string(&project)
                    .with_context(|| format!("reading {}", project.display()))?;
                let parsed: AtpConfig = toml::from_str(&content)
                    .with_context(|| format!("parsing {}", project.display()))?;
                config = config.merge(parsed);
            }
        }

        Ok(config)
    }

    /// Merge `other` into `self`, preferring values from `other` when set.
    pub fn merge(mut self, other: Self) -> Self {
        if other.format.is_some() {
            self.format = other.format;
        }
        if other.no_color {
            self.no_color = true;
        }
        if other.verbose {
            self.verbose = true;
        }
        if other.separator.is_some() {
            self.separator = other.separator;
        }

        // Search
        if other.search.case_insensitive {
            self.search.case_insensitive = true;
        }
        if other.search.context_before.is_some() {
            self.search.context_before = other.search.context_before;
        }
        if other.search.context_after.is_some() {
            self.search.context_after = other.search.context_after;
        }
        if other.search.max_matches.is_some() {
            self.search.max_matches = other.search.max_matches;
        }
        if other.search.max_depth.is_some() {
            self.search.max_depth = other.search.max_depth;
        }
        if !other.search.exclude.is_empty() {
            self.search.exclude = other.search.exclude;
        }
        if !other.search.include.is_empty() {
            self.search.include = other.search.include;
        }

        // Compliance
        if other.compliance.default_marking.is_some() {
            self.compliance.default_marking = other.compliance.default_marking;
        }
        if other.compliance.organization.is_some() {
            self.compliance.organization = other.compliance.organization;
        }

        // Telemetry
        if other.telemetry.enabled {
            self.telemetry.enabled = true;
        }

        // Plugin
        if !other.plugin.dirs.is_empty() {
            self.plugin.dirs.extend(other.plugin.dirs);
        }

        // Pipeline
        if other.pipeline.channel_capacity.is_some() {
            self.pipeline.channel_capacity = other.pipeline.channel_capacity;
        }
        if other.pipeline.batch_size.is_some() {
            self.pipeline.batch_size = other.pipeline.batch_size;
        }
        if !other.pipeline.saved.is_empty() {
            self.pipeline.saved.extend(other.pipeline.saved);
        }

        // Scope
        if !other.scope.roots.is_empty() {
            self.scope.roots = other.scope.roots;
        }
        if !other.scope.extensions.is_empty() {
            self.scope.extensions = other.scope.extensions;
        }
        if other.scope.follow_symlinks {
            self.scope.follow_symlinks = true;
        }

        self
    }

    /// Write a default config to `~/.atp/config.toml` (creates directory if needed).
    pub fn write_default() -> Result<PathBuf> {
        let path = global_config_path()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let default = Self::default();
        let content = toml::to_string_pretty(&default)?;
        std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    /// Serialize to TOML string.
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

/// Return `~/.atp/config.toml`.
pub fn global_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".atp").join("config.toml"))
}

/// Walk up from `start` looking for `.atprc` (TOML format).
pub fn find_rc_file(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".atprc");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Walk up from `start` looking for `.atp.toml` (project config, v2).
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".atp.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = AtpConfig::default();
        assert_eq!(config.format, None);
        assert!(!config.no_color);
        assert!(!config.verbose);
    }

    #[test]
    fn test_parse_toml() {
        let toml_str = r#"
format = "json"
no_color = true
separator = "\t"

[search]
case_insensitive = true
context_before = 3
context_after = 3
max_matches = 100
exclude = ["*.log", "target/"]

[compliance]
default_marking = "CUI"
organization = "Acme Corp"

[telemetry]
enabled = true

[plugin]
dirs = ["/usr/local/lib/atp/plugins"]
"#;
        let config: AtpConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.format.as_deref(), Some("json"));
        assert!(config.no_color);
        assert!(config.search.case_insensitive);
        assert_eq!(config.search.context_before, Some(3));
        assert_eq!(config.search.exclude, vec!["*.log", "target/"]);
        assert_eq!(config.compliance.default_marking.as_deref(), Some("CUI"));
        assert!(config.telemetry.enabled);
        assert_eq!(config.plugin.dirs.len(), 1);
    }

    #[test]
    fn test_merge_override() {
        let base = AtpConfig {
            format: Some("human".into()),
            ..Default::default()
        };
        let overlay = AtpConfig {
            format: Some("json".into()),
            verbose: true,
            ..Default::default()
        };
        let merged = base.merge(overlay);
        assert_eq!(merged.format.as_deref(), Some("json"));
        assert!(merged.verbose);
    }

    #[test]
    fn test_merge_preserves_base() {
        let base = AtpConfig {
            format: Some("csv".into()),
            no_color: true,
            ..Default::default()
        };
        let overlay = AtpConfig::default();
        let merged = base.merge(overlay);
        assert_eq!(merged.format.as_deref(), Some("csv"));
        assert!(merged.no_color);
    }

    #[test]
    fn test_roundtrip_toml() {
        let config = AtpConfig {
            format: Some("yaml".into()),
            separator: Some(",".into()),
            search: SearchConfig {
                case_insensitive: true,
                context_before: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let toml_str = config.to_toml().unwrap();
        let parsed: AtpConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_find_rc_file() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".atprc");
        let mut f = std::fs::File::create(&rc).unwrap();
        writeln!(f, "format = \"json\"").unwrap();

        let found = find_rc_file(dir.path());
        assert_eq!(found, Some(rc));
    }

    #[test]
    fn test_find_rc_file_parent() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".atprc");
        std::fs::write(&rc, "verbose = true").unwrap();
        let child = dir.path().join("subdir");
        std::fs::create_dir(&child).unwrap();

        let found = find_rc_file(&child);
        assert_eq!(found, Some(rc));
    }

    #[test]
    fn test_find_rc_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let found = find_rc_file(dir.path());
        // Might find one higher up, but won't find in tempdir
        // Just check it doesn't panic
        let _ = found;
    }

    // ── v2: .atp.toml project config ─────────────────────────

    #[test]
    fn test_pipeline_defaults_parse() {
        let toml_str = r#"
[pipeline]
channel_capacity = 2048
batch_size = 512

[pipeline.saved]
lint = 'find "TODO" | count'
clean = 'find "\\s+$" | delete'
"#;
        let config: AtpConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.pipeline.channel_capacity, Some(2048));
        assert_eq!(config.pipeline.batch_size, Some(512));
        assert_eq!(config.pipeline.saved.len(), 2);
        assert!(config.pipeline.saved.contains_key("lint"));
    }

    #[test]
    fn test_scope_defaults_parse() {
        let toml_str = r#"
[scope]
roots = ["src", "lib"]
extensions = ["rs", "toml"]
follow_symlinks = true
"#;
        let config: AtpConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.scope.roots.len(), 2);
        assert_eq!(config.scope.extensions, vec!["rs", "toml"]);
        assert!(config.scope.follow_symlinks);
    }

    #[test]
    fn test_find_project_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join(".atp.toml");
        std::fs::write(&cfg_path, "format = \"json\"\n").unwrap();

        let found = find_project_config(dir.path());
        assert_eq!(found, Some(cfg_path));
    }

    #[test]
    fn test_find_project_config_child() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join(".atp.toml");
        std::fs::write(&cfg_path, "verbose = true\n").unwrap();
        let child = dir.path().join("a").join("b");
        std::fs::create_dir_all(&child).unwrap();

        let found = find_project_config(&child);
        assert_eq!(found, Some(cfg_path));
    }

    #[test]
    fn test_merge_pipeline_saved() {
        let base = AtpConfig {
            pipeline: PipelineDefaults {
                saved: [("a".into(), "find x".into())].into_iter().collect(),
                ..Default::default()
            },
            ..Default::default()
        };
        let overlay = AtpConfig {
            pipeline: PipelineDefaults {
                saved: [("b".into(), "count".into())].into_iter().collect(),
                channel_capacity: Some(4096),
                ..Default::default()
            },
            ..Default::default()
        };
        let merged = base.merge(overlay);
        assert_eq!(merged.pipeline.saved.len(), 2);
        assert_eq!(merged.pipeline.channel_capacity, Some(4096));
    }
}
