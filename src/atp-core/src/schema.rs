//! Schema registry — JSON Schema definitions for all ATP output types.
//!
//! Every ATP output type has an associated JSON Schema (draft 2020-12 compatible).
//! The registry provides lookup by type name and embeds `$schema` references in
//! output envelopes. Enables downstream validation of ATP outputs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A registered schema entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEntry {
    /// Type name (e.g., "SearchResults", "TransformResults")
    pub type_name: String,
    /// The JSON Schema definition.
    pub schema: Value,
    /// ATP version this schema was introduced in.
    pub since_version: String,
    /// Brief description.
    pub description: String,
}

/// Schema registry holding all ATP output type schemas.
pub struct SchemaRegistry {
    schemas: BTreeMap<String, SchemaEntry>,
    version: String,
}

impl SchemaRegistry {
    /// Create a new registry populated with all built-in schemas.
    pub fn new() -> Self {
        let version = crate::ATP_VERSION.to_string();
        let mut registry = Self {
            schemas: BTreeMap::new(),
            version: version.clone(),
        };

        // Register all built-in output types
        registry.register(SchemaEntry {
            type_name: "SearchResults".into(),
            schema: Self::search_results_schema(&version),
            since_version: "1.0.0".into(),
            description: "Results from a search/grep operation".into(),
        });

        registry.register(SchemaEntry {
            type_name: "TransformResults".into(),
            schema: Self::transform_results_schema(&version),
            since_version: "1.0.0".into(),
            description: "Results from a transform/sed operation".into(),
        });

        registry.register(SchemaEntry {
            type_name: "AnalysisResults".into(),
            schema: Self::analysis_results_schema(&version),
            since_version: "1.0.0".into(),
            description: "Results from an analyze/awk operation".into(),
        });

        registry.register(SchemaEntry {
            type_name: "PipelineResults".into(),
            schema: Self::pipeline_results_schema(&version),
            since_version: "1.0.0".into(),
            description: "Results from a multi-stage pipeline execution".into(),
        });

        registry.register(SchemaEntry {
            type_name: "ExplainResult".into(),
            schema: Self::explain_result_schema(&version),
            since_version: "1.0.0".into(),
            description: "Explanation of a command before execution".into(),
        });

        registry.register(SchemaEntry {
            type_name: "ComplianceReport".into(),
            schema: Self::compliance_report_schema(&version),
            since_version: "0.5.0".into(),
            description: "Regulatory compliance report (FIPS, CMMC 2.0)".into(),
        });

        registry.register(SchemaEntry {
            type_name: "AtpEnvelope".into(),
            schema: Self::envelope_schema(&version),
            since_version: "1.0.0".into(),
            description: "Universal output envelope wrapping all ATP results".into(),
        });

        registry.register(SchemaEntry {
            type_name: "DiffResult".into(),
            schema: Self::diff_result_schema(&version),
            since_version: "1.5.0".into(),
            description: "Structured diff between two ATP outputs".into(),
        });

        registry.register(SchemaEntry {
            type_name: "PipelineProfile".into(),
            schema: Self::profile_schema(&version),
            since_version: "1.5.0".into(),
            description: "Pipeline execution profile with optimization suggestions".into(),
        });

        registry.register(SchemaEntry {
            type_name: "CacheStats".into(),
            schema: Self::cache_stats_schema(&version),
            since_version: "1.5.0".into(),
            description: "Pipeline cache statistics".into(),
        });

        registry
    }

    /// Register a schema entry.
    pub fn register(&mut self, entry: SchemaEntry) {
        self.schemas.insert(entry.type_name.clone(), entry);
    }

    /// Look up a schema by type name.
    pub fn get(&self, type_name: &str) -> Option<&SchemaEntry> {
        self.schemas.get(type_name)
    }

    /// List all registered type names.
    pub fn list_types(&self) -> Vec<&str> {
        self.schemas.keys().map(|k| k.as_str()).collect()
    }

    /// Get the schema reference URL for a type.
    pub fn schema_ref(&self, type_name: &str) -> String {
        format!(
            "https://atp.nervosys.com/schemas/v{}/{}.json",
            self.version,
            type_name.to_lowercase()
        )
    }

    /// Export all schemas as a single JSON document.
    pub fn export_all(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (name, entry) in &self.schemas {
            map.insert(
                name.clone(),
                serde_json::json!({
                    "description": entry.description,
                    "since": entry.since_version,
                    "schema": entry.schema,
                }),
            );
        }
        Value::Object(map)
    }

    /// Get the total number of registered schemas.
    pub fn len(&self) -> usize {
        self.schemas.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }

    // ─── Built-in Schema Definitions ────────────────────────────────────────

    fn envelope_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/atpenvelope.json", version),
            "title": "AtpEnvelope",
            "type": "object",
            "required": ["version", "command", "timestamp", "deterministic", "data", "metadata"],
            "properties": {
                "version": { "type": "string" },
                "command": { "type": "string" },
                "timestamp": { "type": "string", "format": "date-time" },
                "deterministic": { "type": "boolean" },
                "schema_ref": { "type": "string", "format": "uri" },
                "data": {},
                "metadata": { "$ref": "#/$defs/ExecutionMetadata" }
            },
            "$defs": {
                "ExecutionMetadata": {
                    "type": "object",
                    "required": ["files_scanned", "files_matched", "duration_ms", "provenance"],
                    "properties": {
                        "files_scanned": { "type": "integer" },
                        "files_matched": { "type": "integer" },
                        "duration_ms": { "type": "integer" },
                        "provenance": { "$ref": "#/$defs/Provenance" }
                    }
                },
                "Provenance": {
                    "type": "object",
                    "required": ["tool", "tool_version", "command", "args"],
                    "properties": {
                        "tool": { "type": "string" },
                        "tool_version": { "type": "string" },
                        "command": { "type": "string" },
                        "args": { "type": "array", "items": { "type": "string" } },
                        "working_directory": { "type": "string" },
                        "input_hash": { "type": ["string", "null"] }
                    }
                }
            }
        })
    }

    fn search_results_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/searchresults.json", version),
            "title": "SearchResults",
            "type": "object",
            "required": ["pattern", "pattern_type", "total_matches", "files_with_matches", "matches"],
            "properties": {
                "pattern": { "type": "string" },
                "pattern_type": { "type": "string", "enum": ["regex", "literal", "semantic"] },
                "total_matches": { "type": "integer", "minimum": 0 },
                "files_with_matches": { "type": "integer", "minimum": 0 },
                "matches": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/SearchMatch" }
                }
            },
            "$defs": {
                "SearchMatch": {
                    "type": "object",
                    "required": ["file", "line_number", "content"],
                    "properties": {
                        "file": { "type": "string" },
                        "line_number": { "type": "integer" },
                        "column": { "type": "integer" },
                        "content": { "type": "string" },
                        "byte_offset": { "type": "integer" }
                    }
                }
            }
        })
    }

    fn transform_results_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/transformresults.json", version),
            "title": "TransformResults",
            "type": "object",
            "required": ["expression", "files_modified", "total_changes"],
            "properties": {
                "expression": { "type": "string" },
                "files_modified": { "type": "integer" },
                "total_changes": { "type": "integer" },
                "changes": { "type": "array" }
            }
        })
    }

    fn analysis_results_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/analysisresults.json", version),
            "title": "AnalysisResults",
            "type": "object",
            "required": ["field_separator", "total_records", "records"],
            "properties": {
                "field_separator": { "type": "string" },
                "total_records": { "type": "integer" },
                "records": { "type": "array" }
            }
        })
    }

    fn pipeline_results_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/pipelineresults.json", version),
            "title": "PipelineResults",
            "type": "object",
            "required": ["stages", "final_output", "total_duration_ms"],
            "properties": {
                "stages": { "type": "array" },
                "final_output": {},
                "total_duration_ms": { "type": "integer" }
            }
        })
    }

    fn explain_result_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/explainresult.json", version),
            "title": "ExplainResult",
            "type": "object",
            "required": ["command", "description", "steps"],
            "properties": {
                "command": { "type": "string" },
                "description": { "type": "string" },
                "steps": { "type": "array" }
            }
        })
    }

    fn compliance_report_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/compliancereport.json", version),
            "title": "ComplianceReport",
            "type": "object",
            "required": ["controls"],
            "properties": {
                "controls": { "type": "array" },
                "timestamp": { "type": "string" }
            }
        })
    }

    fn diff_result_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/diffresult.json", version),
            "title": "DiffResult",
            "type": "object",
            "required": ["ops", "additions", "removals", "replacements", "identical"],
            "properties": {
                "ops": { "type": "array" },
                "additions": { "type": "integer" },
                "removals": { "type": "integer" },
                "replacements": { "type": "integer" },
                "identical": { "type": "boolean" }
            }
        })
    }

    fn profile_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/pipelineprofile.json", version),
            "title": "PipelineProfile",
            "type": "object",
            "required": ["stages", "total_duration", "total_records", "suggestions"],
            "properties": {
                "stages": { "type": "array" },
                "total_duration": { "type": "object" },
                "total_records": { "type": "integer" },
                "suggestions": { "type": "array" }
            }
        })
    }

    fn cache_stats_schema(version: &str) -> Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://atp.nervosys.com/schemas/v{}/cachestats.json", version),
            "title": "CacheStats",
            "type": "object",
            "required": ["entries", "total_size_bytes", "hits", "misses", "hit_rate_percent"],
            "properties": {
                "entries": { "type": "integer" },
                "total_size_bytes": { "type": "integer" },
                "hits": { "type": "integer" },
                "misses": { "type": "integer" },
                "hit_rate_percent": { "type": "number" },
                "max_size_bytes": { "type": "integer" }
            }
        })
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_all_types() {
        let reg = SchemaRegistry::new();
        assert!(reg.len() >= 10);
        assert!(reg.get("SearchResults").is_some());
        assert!(reg.get("TransformResults").is_some());
        assert!(reg.get("AnalysisResults").is_some());
        assert!(reg.get("PipelineResults").is_some());
        assert!(reg.get("AtpEnvelope").is_some());
        assert!(reg.get("DiffResult").is_some());
        assert!(reg.get("PipelineProfile").is_some());
        assert!(reg.get("CacheStats").is_some());
    }

    #[test]
    fn test_registry_lookup() {
        let reg = SchemaRegistry::new();
        let entry = reg.get("SearchResults").unwrap();
        assert_eq!(entry.type_name, "SearchResults");
        assert!(entry.schema.is_object());
        assert_eq!(entry.since_version, "1.0.0");
    }

    #[test]
    fn test_list_types() {
        let reg = SchemaRegistry::new();
        let types = reg.list_types();
        assert!(types.contains(&"SearchResults"));
        assert!(types.contains(&"AtpEnvelope"));
    }

    #[test]
    fn test_schema_ref() {
        let reg = SchemaRegistry::new();
        let url = reg.schema_ref("SearchResults");
        assert!(url.contains("searchresults.json"));
        assert!(url.starts_with("https://atp.nervosys.com/schemas/"));
    }

    #[test]
    fn test_export_all() {
        let reg = SchemaRegistry::new();
        let all = reg.export_all();
        assert!(all.is_object());
        assert!(all.get("SearchResults").is_some());
    }

    #[test]
    fn test_custom_registration() {
        let mut reg = SchemaRegistry::new();
        let initial = reg.len();
        reg.register(SchemaEntry {
            type_name: "CustomOutput".into(),
            schema: serde_json::json!({"type": "object"}),
            since_version: "1.5.0".into(),
            description: "A custom output type".into(),
        });
        assert_eq!(reg.len(), initial + 1);
        assert!(reg.get("CustomOutput").is_some());
    }

    #[test]
    fn test_search_schema_structure() {
        let reg = SchemaRegistry::new();
        let entry = reg.get("SearchResults").unwrap();
        let schema = &entry.schema;
        assert_eq!(schema["title"], "SearchResults");
        assert!(schema["required"].is_array());
        assert!(schema["properties"]["pattern"].is_object());
    }

    #[test]
    fn test_envelope_schema_structure() {
        let reg = SchemaRegistry::new();
        let entry = reg.get("AtpEnvelope").unwrap();
        let schema = &entry.schema;
        assert_eq!(schema["title"], "AtpEnvelope");
        assert!(schema["$defs"]["Provenance"].is_object());
    }
}
