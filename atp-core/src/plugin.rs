//! Plugin system — extensible pipeline stages and output formats.
//!
//! ATP supports a plugin architecture that allows users to define custom:
//! - **Pipeline stages**: Transform or filter data in custom ways
//! - **Output formats**: Render results in custom formats
//!
//! Plugins are loaded from a configurable directory (default: `~/.atp/plugins/`)
//! and are specified as TOML configuration files that describe the plugin's
//! behavior using built-in primitives.
//!
//! # Plugin Architecture
//!
//! Plugins use a trait-based design:
//! - [`AtpPlugin`] — Core trait all plugins implement
//! - [`StagePlugin`] — Plugin that adds a custom pipeline stage
//! - [`FormatPlugin`] — Plugin that adds a custom output format
//! - [`PluginRegistry`] — Manages plugin discovery and lifecycle
//!
//! # Example Plugin Configuration
//!
//! ```toml
//! [plugin]
//! name = "csv-summary"
//! version = "1.0.0"
//! description = "Summarize CSV files with column statistics"
//! kind = "stage"
//!
//! [stage]
//! input = "records"
//! output = "records"
//!
//! [stage.transform]
//! type = "aggregate"
//! fields = ["sum:2", "avg:3", "count"]
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Core trait for all ATP plugins.
pub trait AtpPlugin: Send + Sync {
    /// Unique name of the plugin.
    fn name(&self) -> &str;
    /// Version string.
    fn version(&self) -> &str;
    /// Human-readable description.
    fn description(&self) -> &str;
    /// Plugin kind: "stage" or "format".
    fn kind(&self) -> PluginKind;
}

/// Plugin that adds a custom pipeline stage.
pub trait StagePlugin: AtpPlugin {
    /// Execute the stage on input data, returning transformed data.
    fn execute(&self, input: Vec<PluginRecord>) -> Result<Vec<PluginRecord>>;
}

/// Plugin that adds a custom output format.
pub trait FormatPlugin: AtpPlugin {
    /// Format the given data into a string.
    fn format(&self, data: &serde_json::Value) -> Result<String>;
    /// The format name (used with `--format <name>`).
    fn format_name(&self) -> &str;
}

/// Plugin kind discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    Stage,
    Format,
}

/// A record passed between plugin stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRecord {
    /// Source file (if applicable).
    pub file: String,
    /// Source line number (if applicable).
    pub line: usize,
    /// Content text.
    pub content: String,
    /// Key-value metadata.
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Plugin manifest — the TOML configuration file for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginInfo,
    pub stage: Option<StageConfig>,
    pub format: Option<FormatConfig>,
}

/// Basic plugin information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: PluginKind,
    /// Optional author information.
    pub author: Option<String>,
    /// Optional license.
    pub license: Option<String>,
}

/// Configuration for a stage plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageConfig {
    /// What the stage expects as input: "lines", "records", "text".
    pub input: String,
    /// What the stage produces as output: "lines", "records", "text".
    pub output: String,
    /// The transformation to apply.
    pub transform: StageTransform,
}

/// Transformation type for a stage plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StageTransform {
    /// Apply a regex substitution.
    Replace {
        pattern: String,
        replacement: String,
        #[serde(default)]
        global: bool,
    },
    /// Filter lines matching a pattern.
    Filter { pattern: String },
    /// Sort records.
    Sort {
        #[serde(default)]
        field: Option<usize>,
        #[serde(default)]
        descending: bool,
    },
    /// Aggregate records.
    Aggregate { fields: Vec<String> },
    /// Custom shell command (each line piped through).
    Shell { command: String },
}

/// Configuration for a format plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatConfig {
    /// The format name (used with `--format <name>`).
    pub name: String,
    /// Template string for formatting each record.
    pub template: String,
    /// Optional header template.
    pub header: Option<String>,
    /// Optional footer template.
    pub footer: Option<String>,
    /// Field separator for template interpolation.
    #[serde(default = "default_separator")]
    pub separator: String,
}

fn default_separator() -> String {
    "\t".to_string()
}

/// Plugin registry — manages plugin discovery, loading, and lookup.
pub struct PluginRegistry {
    manifests: Vec<PluginManifest>,
    plugin_dir: PathBuf,
}

impl PluginRegistry {
    /// Create a new empty registry with the given plugin directory.
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self {
            manifests: Vec::new(),
            plugin_dir,
        }
    }

    /// Create a registry using the default plugin directory (`~/.atp/plugins/`).
    pub fn default_dir() -> Self {
        let dir = default_plugin_dir();
        Self::new(dir)
    }

    /// Discover and load all plugin manifests from the plugin directory.
    pub fn discover(&mut self) -> Result<usize> {
        self.manifests.clear();

        if !self.plugin_dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(&self.plugin_dir).with_context(|| {
            format!(
                "Failed to read plugin directory: {}",
                self.plugin_dir.display()
            )
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Load .toml files as plugin manifests
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                match self.load_manifest(&path) {
                    Ok(manifest) => self.manifests.push(manifest),
                    Err(e) => {
                        eprintln!("Warning: Failed to load plugin {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(self.manifests.len())
    }

    /// Load a single plugin manifest from a TOML file.
    pub fn load_manifest(&self, path: &Path) -> Result<PluginManifest> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read plugin file: {}", path.display()))?;
        let manifest: PluginManifest = toml::from_str(&content)
            .with_context(|| format!("Failed to parse plugin manifest: {}", path.display()))?;
        Ok(manifest)
    }

    /// Get all loaded plugin manifests.
    pub fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }

    /// Find a stage plugin by name.
    pub fn find_stage(&self, name: &str) -> Option<&PluginManifest> {
        self.manifests
            .iter()
            .find(|m| m.plugin.kind == PluginKind::Stage && m.plugin.name == name)
    }

    /// Find a format plugin by format name.
    pub fn find_format(&self, format_name: &str) -> Option<&PluginManifest> {
        self.manifests.iter().find(|m| {
            m.plugin.kind == PluginKind::Format
                && m.format
                    .as_ref()
                    .map(|f| f.name == format_name)
                    .unwrap_or(false)
        })
    }

    /// List all available plugins with their metadata.
    pub fn list_plugins(&self) -> Vec<PluginSummary> {
        self.manifests
            .iter()
            .map(|m| PluginSummary {
                name: m.plugin.name.clone(),
                version: m.plugin.version.clone(),
                description: m.plugin.description.clone(),
                kind: m.plugin.kind.clone(),
            })
            .collect()
    }

    /// Execute a stage plugin by name on the given records.
    pub fn execute_stage(
        &self,
        name: &str,
        records: Vec<PluginRecord>,
    ) -> Result<Vec<PluginRecord>> {
        let manifest = self
            .find_stage(name)
            .ok_or_else(|| anyhow::anyhow!("Stage plugin not found: {name}"))?;

        let stage = manifest
            .stage
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin {name} has no stage configuration"))?;

        match &stage.transform {
            StageTransform::Replace {
                pattern,
                replacement,
                global,
            } => {
                let re = regex::Regex::new(pattern)
                    .with_context(|| format!("Invalid plugin pattern: {pattern}"))?;
                Ok(records
                    .into_iter()
                    .map(|mut r| {
                        r.content = if *global {
                            re.replace_all(&r.content, replacement.as_str()).to_string()
                        } else {
                            re.replace(&r.content, replacement.as_str()).to_string()
                        };
                        r
                    })
                    .collect())
            }
            StageTransform::Filter { pattern } => {
                let re = regex::Regex::new(pattern)
                    .with_context(|| format!("Invalid plugin filter pattern: {pattern}"))?;
                Ok(records
                    .into_iter()
                    .filter(|r| re.is_match(&r.content))
                    .collect())
            }
            StageTransform::Sort { field, descending } => {
                let mut sorted = records;
                match field {
                    Some(idx) => {
                        sorted.sort_by(|a, b| {
                            let a_val = a
                                .metadata
                                .get(&format!("field_{idx}"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&a.content);
                            let b_val = b
                                .metadata
                                .get(&format!("field_{idx}"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&b.content);
                            a_val.cmp(b_val)
                        });
                    }
                    None => sorted.sort_by(|a, b| a.content.cmp(&b.content)),
                }
                if *descending {
                    sorted.reverse();
                }
                Ok(sorted)
            }
            StageTransform::Aggregate { fields } => {
                let count = records.len();
                let mut result = PluginRecord {
                    file: String::new(),
                    line: 0,
                    content: format!("{count} records"),
                    metadata: BTreeMap::new(),
                };
                result
                    .metadata
                    .insert("count".to_string(), serde_json::Value::Number(count.into()));
                for spec in fields {
                    result
                        .metadata
                        .insert(spec.clone(), serde_json::Value::String(spec.clone()));
                }
                Ok(vec![result])
            }
            StageTransform::Shell { command } => {
                // Execute each record through a shell command
                let mut output = Vec::new();
                for record in records {
                    let result =
                        std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                            .args(if cfg!(windows) {
                                vec!["/C", command]
                            } else {
                                vec!["-c", command]
                            })
                            .stdin(std::process::Stdio::piped())
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .output();

                    match result {
                        Ok(out) => {
                            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                            for (i, line) in stdout.lines().enumerate() {
                                output.push(PluginRecord {
                                    file: record.file.clone(),
                                    line: i + 1,
                                    content: line.to_string(),
                                    metadata: BTreeMap::new(),
                                });
                            }
                        }
                        Err(e) => {
                            anyhow::bail!("Shell plugin command failed: {e}");
                        }
                    }
                }
                Ok(output)
            }
        }
    }

    /// Get the plugin directory path.
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

/// Summary information about a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: PluginKind,
}

/// Get the default plugin directory.
fn default_plugin_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            PathBuf::from(profile).join(".atp").join("plugins")
        } else {
            PathBuf::from(".atp").join("plugins")
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".atp").join("plugins")
        } else {
            PathBuf::from(".atp").join("plugins")
        }
    }
}

/// Registry index entry — metadata for a published plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: PluginKind,
    pub author: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub download_url: Option<String>,
    pub checksum: Option<String>,
    pub tags: Vec<String>,
    pub downloads: u64,
}

/// Plugin SDK — helpers for creating and scaffolding plugins.
pub struct PluginSdk;

impl PluginSdk {
    /// Generate a scaffold plugin manifest TOML.
    pub fn scaffold_stage(name: &str, description: &str) -> String {
        format!(
            r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "{description}"
kind = "stage"
author = ""
license = "MIT"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "filter"
pattern = "TODO"
"#,
        )
    }

    /// Generate a scaffold format plugin manifest TOML.
    pub fn scaffold_format(name: &str, format_name: &str, description: &str) -> String {
        format!(
            "[plugin]\n\
             name = \"{name}\"\n\
             version = \"0.1.0\"\n\
             description = \"{description}\"\n\
             kind = \"format\"\n\
             author = \"\"\n\
             license = \"MIT\"\n\
             \n\
             [format]\n\
             name = \"{format_name}\"\n\
             template = \"{{{{file}}}}:{{{{line}}}}: {{{{content}}}}\"\n\
             header = \"Results\"\n\
             separator = \"\\t\"\n"
        )
    }

    /// Validate a plugin manifest for correctness.
    pub fn validate_manifest(manifest: &PluginManifest) -> Vec<String> {
        let mut errors = Vec::new();
        if manifest.plugin.name.is_empty() {
            errors.push("Plugin name is required".to_string());
        }
        if manifest.plugin.version.is_empty() {
            errors.push("Plugin version is required".to_string());
        }
        if manifest.plugin.description.is_empty() {
            errors.push("Plugin description is required".to_string());
        }
        match manifest.plugin.kind {
            PluginKind::Stage => {
                if manifest.stage.is_none() {
                    errors.push("Stage plugin requires [stage] configuration".to_string());
                }
            }
            PluginKind::Format => {
                if manifest.format.is_none() {
                    errors.push("Format plugin requires [format] configuration".to_string());
                }
            }
        }
        errors
    }

    /// Package a plugin for distribution — creates a .tar.gz-ready bundle description.
    pub fn package_info(manifest: &PluginManifest) -> PluginPackage {
        PluginPackage {
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            files: vec![format!("{}.toml", manifest.plugin.name)],
        }
    }
}

/// Information about a packaged plugin for distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPackage {
    pub name: String,
    pub version: String,
    pub files: Vec<String>,
}

impl PluginRegistry {
    /// Install a plugin from a local TOML file path.
    pub fn install_local(&mut self, source: &Path) -> Result<String> {
        let manifest = self.load_manifest(source)?;
        let errors = PluginSdk::validate_manifest(&manifest);
        if !errors.is_empty() {
            anyhow::bail!("Invalid plugin manifest:\n{}", errors.join("\n"));
        }
        let name = manifest.plugin.name.clone();
        let dest = self.plugin_dir.join(format!("{name}.toml"));
        std::fs::create_dir_all(&self.plugin_dir)?;
        std::fs::copy(source, &dest)?;
        self.manifests.push(manifest);
        Ok(name)
    }

    /// Remove a plugin by name.
    pub fn uninstall(&mut self, name: &str) -> Result<()> {
        let dest = self.plugin_dir.join(format!("{name}.toml"));
        if dest.exists() {
            std::fs::remove_file(&dest)?;
        }
        self.manifests.retain(|m| m.plugin.name != name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry_empty() {
        let registry = PluginRegistry::new(PathBuf::from("/nonexistent"));
        assert!(registry.manifests().is_empty());
        assert!(registry.find_stage("foo").is_none());
        assert!(registry.find_format("bar").is_none());
    }

    #[test]
    fn test_plugin_manifest_parse() {
        let toml_str = r#"
[plugin]
name = "test-filter"
version = "1.0.0"
description = "Filter lines matching a pattern"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "filter"
pattern = "TODO"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "test-filter");
        assert_eq!(manifest.plugin.kind, PluginKind::Stage);
        assert!(manifest.stage.is_some());
        match &manifest.stage.unwrap().transform {
            StageTransform::Filter { pattern } => assert_eq!(pattern, "TODO"),
            _ => panic!("Expected Filter transform"),
        }
    }

    #[test]
    fn test_plugin_manifest_format() {
        let toml_str = r#"
[plugin]
name = "markdown-table"
version = "0.1.0"
description = "Output results as Markdown table"
kind = "format"

[format]
name = "md-table"
template = "| {file} | {line} | {content} |"
header = "| File | Line | Content |\n|------|------|---------|"
separator = " | "
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.kind, PluginKind::Format);
        assert!(manifest.format.is_some());
        assert_eq!(manifest.format.unwrap().name, "md-table");
    }

    #[test]
    fn test_execute_filter_stage() {
        let toml_str = r#"
[plugin]
name = "todo-filter"
version = "1.0.0"
description = "Keep only TODO lines"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "filter"
pattern = "TODO"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let mut registry = PluginRegistry::new(PathBuf::from("."));
        registry.manifests.push(manifest);

        let records = vec![
            PluginRecord {
                file: "test.rs".into(),
                line: 1,
                content: "// TODO: fix this".into(),
                metadata: BTreeMap::new(),
            },
            PluginRecord {
                file: "test.rs".into(),
                line: 2,
                content: "let x = 1;".into(),
                metadata: BTreeMap::new(),
            },
            PluginRecord {
                file: "test.rs".into(),
                line: 3,
                content: "// TODO: and this".into(),
                metadata: BTreeMap::new(),
            },
        ];

        let result = registry.execute_stage("todo-filter", records).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].content.contains("TODO: fix"));
        assert!(result[1].content.contains("TODO: and"));
    }

    #[test]
    fn test_execute_replace_stage() {
        let toml_str = r#"
[plugin]
name = "redact"
version = "1.0.0"
description = "Redact email addresses"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "replace"
pattern = "[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}"
replacement = "[REDACTED]"
global = true
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let mut registry = PluginRegistry::new(PathBuf::from("."));
        registry.manifests.push(manifest);

        let records = vec![PluginRecord {
            file: "data.txt".into(),
            line: 1,
            content: "Contact user@example.com or admin@test.org".into(),
            metadata: BTreeMap::new(),
        }];

        let result = registry.execute_stage("redact", records).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Contact [REDACTED] or [REDACTED]");
    }

    #[test]
    fn test_execute_sort_stage() {
        let toml_str = r#"
[plugin]
name = "sorter"
version = "1.0.0"
description = "Sort records"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "sort"
descending = true
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let mut registry = PluginRegistry::new(PathBuf::from("."));
        registry.manifests.push(manifest);

        let records = vec![
            PluginRecord {
                file: "".into(),
                line: 1,
                content: "banana".into(),
                metadata: BTreeMap::new(),
            },
            PluginRecord {
                file: "".into(),
                line: 2,
                content: "apple".into(),
                metadata: BTreeMap::new(),
            },
            PluginRecord {
                file: "".into(),
                line: 3,
                content: "cherry".into(),
                metadata: BTreeMap::new(),
            },
        ];

        let result = registry.execute_stage("sorter", records).unwrap();
        assert_eq!(result[0].content, "cherry");
        assert_eq!(result[1].content, "banana");
        assert_eq!(result[2].content, "apple");
    }

    #[test]
    fn test_discover_no_dir() {
        let mut registry = PluginRegistry::new(PathBuf::from("/nonexistent_dir_xyz"));
        let count = registry.discover().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_list_plugins() {
        let mut registry = PluginRegistry::new(PathBuf::from("."));
        let toml_str = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "filter"
pattern = "test"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        registry.manifests.push(manifest);

        let list = registry.list_plugins();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test-plugin");
        assert_eq!(list[0].kind, PluginKind::Stage);
    }
}
