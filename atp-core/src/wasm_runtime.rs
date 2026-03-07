//! WASM plugin runtime — sandboxed plugin execution via WebAssembly.
//!
//! Enables safe third-party plugin execution in a WebAssembly sandbox
//! with capability-based permissions. Plugins are compiled to `.wasm`
//! modules and loaded at runtime.
//!
//! # Architecture
//!
//! - [`WasmCapability`]: Permission flags for WASM plugins
//! - [`WasmPluginManifest`]: Metadata and capability declaration for a WASM plugin
//! - [`WasmModule`]: A loaded WASM module ready for execution
//! - [`WasmRuntime`]: The runtime that manages WASM plugin lifecycle
//! - [`WasmEngine`]: Trait for pluggable WASM execution backends (e.g. wasmtime, wasmer)
//!
//! # Security Model
//!
//! Plugins run in a strict sandbox:
//! - No filesystem access unless explicitly granted (`FileRead`, `FileWrite`)
//! - No network access unless granted (`Network`)
//! - No environment variable access unless granted (`Environment`)
//! - Memory and execution time are bounded
//! - All I/O goes through capability-checked host functions
//!
//! # Engine Backends
//!
//! The [`WasmEngine`] trait allows plugging in real WASM runtimes such as
//! wasmtime or wasmer. The default [`BuiltinWasmEngine`] provides bytecode-level
//! validation (magic bytes, version, section parsing) and simulated execution
//! suitable for testing and development.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Capability flags for WASM plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WasmCapability {
    /// Read files from the filesystem.
    FileRead,
    /// Write files to the filesystem.
    FileWrite,
    /// Make network requests.
    Network,
    /// Access environment variables.
    Environment,
    /// Spawn subprocesses.
    Process,
    /// Access stdin/stdout.
    Stdio,
}

impl WasmCapability {
    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            WasmCapability::FileRead => "Read files from the filesystem",
            WasmCapability::FileWrite => "Write files to the filesystem",
            WasmCapability::Network => "Make network requests",
            WasmCapability::Environment => "Access environment variables",
            WasmCapability::Process => "Spawn subprocesses",
            WasmCapability::Stdio => "Access stdin/stdout",
        }
    }

    /// Whether this capability is considered dangerous.
    pub fn is_dangerous(&self) -> bool {
        matches!(
            self,
            WasmCapability::FileWrite | WasmCapability::Network | WasmCapability::Process
        )
    }
}

/// Manifest for a WASM plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginManifest {
    /// Plugin name.
    pub name: String,
    /// Plugin version (semver).
    pub version: String,
    /// Description.
    pub description: String,
    /// Author(s).
    pub author: String,
    /// Required capabilities.
    pub capabilities: Vec<WasmCapability>,
    /// Exported function names.
    pub exports: Vec<String>,
    /// Maximum memory in pages (64KB each, default 256 = 16MB).
    pub max_memory_pages: u32,
    /// Maximum execution time in milliseconds (default 30_000).
    pub max_execution_ms: u64,
    /// WASM module path (relative to plugin directory).
    pub module_path: String,
}

impl Default for WasmPluginManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.1.0".into(),
            description: String::new(),
            author: String::new(),
            capabilities: Vec::new(),
            exports: Vec::new(),
            max_memory_pages: 256,
            max_execution_ms: 30_000,
            module_path: String::new(),
        }
    }
}

/// A loaded WASM module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModule {
    /// Plugin manifest.
    pub manifest: WasmPluginManifest,
    /// Path to the .wasm file.
    pub wasm_path: PathBuf,
    /// Whether the module has been validated.
    pub validated: bool,
    /// Module size in bytes.
    pub size_bytes: u64,
    /// SHA-256 hash of the module.
    pub hash: String,
}

/// Result of executing a WASM plugin function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmResult {
    /// Plugin name.
    pub plugin: String,
    /// Function that was called.
    pub function: String,
    /// Output (serialized as string).
    pub output: String,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
    /// Memory used in bytes.
    pub memory_bytes: u64,
    /// Whether execution succeeded.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Execution limits for WASM plugins.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WasmLimits {
    /// Maximum memory in bytes.
    pub max_memory: u64,
    /// Maximum execution time in milliseconds.
    pub max_time_ms: u64,
    /// Maximum output size in bytes.
    pub max_output: u64,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self {
            max_memory: 16 * 1024 * 1024, // 16 MB
            max_time_ms: 30_000,          // 30 seconds
            max_output: 10 * 1024 * 1024, // 10 MB
        }
    }
}

/// WASM plugin runtime — manages plugin lifecycle and execution.
///
/// This is the manager, not the actual WASM engine. The actual engine
/// (wasmtime, wasmer, etc.) would be plugged in via the `WasmEngine` trait.
/// This module provides the framework, security model, and API surface.
pub struct WasmRuntime {
    /// Loaded modules keyed by plugin name.
    modules: HashMap<String, WasmModule>,
    /// Granted capabilities per plugin.
    granted_capabilities: HashMap<String, Vec<WasmCapability>>,
    /// Default execution limits.
    limits: WasmLimits,
    /// Plugin directory.
    plugin_dir: PathBuf,
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmRuntime {
    /// Create a new WASM runtime.
    pub fn new() -> Self {
        let plugin_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("atp")
            .join("wasm-plugins");

        Self {
            modules: HashMap::new(),
            granted_capabilities: HashMap::new(),
            limits: WasmLimits::default(),
            plugin_dir,
        }
    }

    /// Create a runtime with a specific plugin directory.
    pub fn with_plugin_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            plugin_dir: dir.into(),
            ..Self::new()
        }
    }

    /// Set execution limits.
    pub fn set_limits(&mut self, limits: WasmLimits) {
        self.limits = limits;
    }

    /// Load a WASM plugin from its manifest file.
    pub fn load_plugin(&mut self, manifest_path: &Path) -> Result<&WasmModule> {
        let manifest_text = std::fs::read_to_string(manifest_path)
            .with_context(|| format!("reading manifest {}", manifest_path.display()))?;
        let manifest: WasmPluginManifest =
            toml::from_str(&manifest_text).with_context(|| "parsing WASM plugin manifest")?;

        let wasm_path = manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&manifest.module_path);

        if !wasm_path.exists() {
            bail!(
                "WASM module not found: {} (referenced from {})",
                wasm_path.display(),
                manifest_path.display()
            );
        }

        let wasm_bytes = std::fs::read(&wasm_path)?;
        let hash = compute_sha256(&wasm_bytes);
        let size_bytes = wasm_bytes.len() as u64;

        // Basic WASM validation: check magic number
        let validated = wasm_bytes.len() >= 8 && wasm_bytes[0..4] == [0x00, 0x61, 0x73, 0x6D]; // \0asm

        let module = WasmModule {
            manifest: manifest.clone(),
            wasm_path,
            validated,
            size_bytes,
            hash,
        };

        let name = manifest.name.clone();
        self.modules.insert(name.clone(), module);
        Ok(self.modules.get(&name).unwrap())
    }

    /// Grant capabilities to a plugin.
    pub fn grant_capabilities(
        &mut self,
        plugin: &str,
        capabilities: Vec<WasmCapability>,
    ) -> Result<()> {
        if !self.modules.contains_key(plugin) {
            bail!("Plugin '{}' is not loaded", plugin);
        }
        self.granted_capabilities
            .insert(plugin.to_string(), capabilities);
        Ok(())
    }

    /// Check if a plugin has a specific capability.
    pub fn has_capability(&self, plugin: &str, capability: WasmCapability) -> bool {
        self.granted_capabilities
            .get(plugin)
            .is_some_and(|caps| caps.contains(&capability))
    }

    /// Check if all requested capabilities are granted.
    pub fn check_capabilities(&self, plugin: &str) -> Result<()> {
        let module = self
            .modules
            .get(plugin)
            .with_context(|| format!("Plugin '{}' not loaded", plugin))?;

        for cap in &module.manifest.capabilities {
            if !self.has_capability(plugin, *cap) {
                bail!(
                    "Plugin '{}' requires capability {:?} ({}) which is not granted",
                    plugin,
                    cap,
                    cap.description()
                );
            }
        }

        Ok(())
    }

    /// Execute a function in a loaded WASM plugin.
    ///
    /// Note: This is a simulation. Real execution would use wasmtime/wasmer.
    /// The API surface is designed for future integration.
    pub fn execute(&self, plugin: &str, function: &str, input: &str) -> Result<WasmResult> {
        let module = self
            .modules
            .get(plugin)
            .with_context(|| format!("Plugin '{}' not loaded", plugin))?;

        // Check capabilities
        self.check_capabilities(plugin)?;

        // Check that the function is exported
        if !module.manifest.exports.contains(&function.to_string()) {
            bail!(
                "Plugin '{}' does not export function '{}'",
                plugin,
                function
            );
        }

        // Simulate execution
        let start = std::time::Instant::now();

        // In a real implementation, we would:
        // 1. Instantiate the WASM module with wasmtime
        // 2. Set up host imports with capability checks
        // 3. Call the exported function with input
        // 4. Collect output and enforce limits
        let output = format!(
            "[WASM] {}::{}({}) → simulated output",
            plugin,
            function,
            if input.len() > 50 {
                &input[..50]
            } else {
                input
            }
        );

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(WasmResult {
            plugin: plugin.to_string(),
            function: function.to_string(),
            output,
            duration_ms,
            memory_bytes: 0,
            success: true,
            error: None,
        })
    }

    /// List all loaded plugins.
    pub fn list_plugins(&self) -> Vec<&WasmModule> {
        self.modules.values().collect()
    }

    /// Get a loaded module by name.
    pub fn get_module(&self, name: &str) -> Option<&WasmModule> {
        self.modules.get(name)
    }

    /// Unload a plugin.
    pub fn unload_plugin(&mut self, name: &str) -> bool {
        self.granted_capabilities.remove(name);
        self.modules.remove(name).is_some()
    }

    /// Get plugin directory path.
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Get execution limits.
    pub fn limits(&self) -> WasmLimits {
        self.limits
    }

    /// Validate a WASM module's integrity.
    pub fn validate_module(&self, name: &str) -> Result<bool> {
        let module = self
            .modules
            .get(name)
            .with_context(|| format!("Plugin '{}' not loaded", name))?;

        if !module.wasm_path.exists() {
            return Ok(false);
        }

        let current_bytes = std::fs::read(&module.wasm_path)?;
        let current_hash = compute_sha256(&current_bytes);

        Ok(current_hash == module.hash)
    }

    /// Generate a security audit report for a plugin.
    pub fn security_audit(&self, name: &str) -> Result<String> {
        let module = self
            .modules
            .get(name)
            .with_context(|| format!("Plugin '{}' not loaded", name))?;

        let mut report = String::new();
        report.push_str(&format!("Security Audit: {}\n", module.manifest.name));
        report.push_str(&format!("Version: {}\n", module.manifest.version));
        report.push_str(&format!("Author: {}\n", module.manifest.author));
        report.push_str(&format!("Module Size: {} bytes\n", module.size_bytes));
        report.push_str(&format!("SHA-256: {}\n", module.hash));
        report.push_str(&format!("Validated: {}\n", module.validated));
        report.push_str("\nRequested Capabilities:\n");

        for cap in &module.manifest.capabilities {
            let granted = self.has_capability(name, *cap);
            let danger = if cap.is_dangerous() {
                " [DANGEROUS]"
            } else {
                ""
            };
            report.push_str(&format!(
                "  {:?}: {} (granted: {}){}\n",
                cap,
                cap.description(),
                granted,
                danger
            ));
        }

        Ok(report)
    }
}

fn compute_sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// WasmEngine trait — pluggable WASM execution backend
// ---------------------------------------------------------------------------

/// Trait for pluggable WASM execution backends (wasmtime, wasmer, etc.).
///
/// Implement this trait to provide a real WASM engine backend.
/// The [`BuiltinWasmEngine`] provides bytecode validation + simulated execution.
pub trait WasmEngine: Send + Sync {
    /// Validate WASM bytecode and return a list of exported function names.
    fn validate(&self, wasm_bytes: &[u8]) -> Result<Vec<String>>;

    /// Execute a function in a loaded WASM module with the given input.
    fn execute(
        &self,
        wasm_bytes: &[u8],
        function: &str,
        input: &str,
        limits: &WasmLimits,
    ) -> Result<WasmResult>;
}

/// Built-in WASM engine with bytecode-level validation.
///
/// Performs real WASM binary format parsing:
/// - Magic number verification (`\0asm`)
/// - Version check (1)
/// - Section header enumeration
/// - Export section parsing to discover exported function names
///
/// Execution is simulated (returns canned output). For real execution,
/// use a `wasmtime` or `wasmer` backend implementing [`WasmEngine`].
pub struct BuiltinWasmEngine;

impl BuiltinWasmEngine {
    pub fn new() -> Self {
        Self
    }

    /// Parse the WASM binary export section to find exported function names.
    fn parse_exports(wasm_bytes: &[u8]) -> Result<Vec<String>> {
        if wasm_bytes.len() < 8 {
            bail!("WASM binary too short");
        }
        if wasm_bytes[0..4] != [0x00, 0x61, 0x73, 0x6D] {
            bail!("Invalid WASM magic number");
        }
        let version =
            u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
        if version != 1 {
            bail!("Unsupported WASM version: {version}");
        }

        let mut exports = Vec::new();
        let mut offset = 8;

        // Walk sections
        while offset < wasm_bytes.len() {
            if offset >= wasm_bytes.len() {
                break;
            }
            let section_id = wasm_bytes[offset];
            offset += 1;

            // Decode LEB128 section length
            let (section_len, bytes_read) = decode_leb128(&wasm_bytes[offset..])?;
            offset += bytes_read;

            if section_id == 7 {
                // Export section
                let section_end = offset + section_len as usize;
                if section_end > wasm_bytes.len() {
                    break;
                }
                let (count, cr) = decode_leb128(&wasm_bytes[offset..])?;
                let mut pos = offset + cr;
                for _ in 0..count {
                    if pos >= section_end {
                        break;
                    }
                    // Name length
                    let (name_len, nr) = decode_leb128(&wasm_bytes[pos..])?;
                    pos += nr;
                    let name_end = pos + name_len as usize;
                    if name_end > section_end {
                        break;
                    }
                    let name = std::str::from_utf8(&wasm_bytes[pos..name_end])
                        .unwrap_or("<invalid>")
                        .to_string();
                    pos = name_end;
                    // Export kind (1 byte) + index (LEB128)
                    if pos < section_end {
                        let kind = wasm_bytes[pos];
                        pos += 1;
                        let (_, ir) = decode_leb128(&wasm_bytes[pos..])?;
                        pos += ir;
                        // kind 0 = function export
                        if kind == 0 {
                            exports.push(name);
                        }
                    }
                }
                offset = section_end;
            } else {
                // Skip section
                offset += section_len as usize;
            }
        }

        Ok(exports)
    }
}

impl Default for BuiltinWasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmEngine for BuiltinWasmEngine {
    fn validate(&self, wasm_bytes: &[u8]) -> Result<Vec<String>> {
        Self::parse_exports(wasm_bytes)
    }

    fn execute(
        &self,
        wasm_bytes: &[u8],
        function: &str,
        input: &str,
        _limits: &WasmLimits,
    ) -> Result<WasmResult> {
        // Validate first
        let exports = self.validate(wasm_bytes)?;
        if !exports.contains(&function.to_string()) {
            bail!(
                "Function '{}' not found in WASM exports: {:?}",
                function,
                exports
            );
        }

        let start = std::time::Instant::now();
        let output = format!(
            "[WASM:builtin] {}({}) → simulated output ({} bytes module)",
            function,
            if input.len() > 50 {
                &input[..50]
            } else {
                input
            },
            wasm_bytes.len(),
        );
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(WasmResult {
            plugin: "builtin".into(),
            function: function.into(),
            output,
            duration_ms,
            memory_bytes: wasm_bytes.len() as u64,
            success: true,
            error: None,
        })
    }
}

/// Decode an unsigned LEB128 value from a byte slice.
fn decode_leb128(bytes: &[u8]) -> Result<(u32, usize)> {
    let mut result: u32 = 0;
    let mut shift = 0u32;
    for (i, &byte) in bytes.iter().enumerate() {
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
        if shift > 28 {
            bail!("LEB128 overflow");
        }
    }
    bail!("Unexpected end of LEB128 data")
}

/// Build a minimal valid WASM module (for testing).
///
/// Creates a WASM binary with:
/// - Magic number + version
/// - A type section (function type `() -> ()`)
/// - A function section (one function using type 0)
/// - An export section exporting the given function names
/// - A code section (each function body = `end`)
pub fn build_test_wasm_module(exports: &[&str]) -> Vec<u8> {
    let mut wasm = Vec::new();
    // Magic + version
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // \0asm
    wasm.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version 1

    let func_count = exports.len();

    // Section 1: Type section — one type: () -> ()
    {
        let mut section = Vec::new();
        encode_leb128(&mut section, 1); // 1 type
        section.push(0x60); // func type
        encode_leb128(&mut section, 0); // 0 params
        encode_leb128(&mut section, 0); // 0 results
        wasm.push(1); // section id = Type
        encode_leb128(&mut wasm, section.len() as u32);
        wasm.extend_from_slice(&section);
    }

    // Section 3: Function section — N functions, all using type 0
    {
        let mut section = Vec::new();
        encode_leb128(&mut section, func_count as u32);
        for _ in 0..func_count {
            encode_leb128(&mut section, 0); // type index 0
        }
        wasm.push(3); // section id = Function
        encode_leb128(&mut wasm, section.len() as u32);
        wasm.extend_from_slice(&section);
    }

    // Section 7: Export section
    {
        let mut section = Vec::new();
        encode_leb128(&mut section, func_count as u32);
        for (i, name) in exports.iter().enumerate() {
            encode_leb128(&mut section, name.len() as u32);
            section.extend_from_slice(name.as_bytes());
            section.push(0x00); // export kind = function
            encode_leb128(&mut section, i as u32); // function index
        }
        wasm.push(7); // section id = Export
        encode_leb128(&mut wasm, section.len() as u32);
        wasm.extend_from_slice(&section);
    }

    // Section 10: Code section — N function bodies, each = [size=2, local_count=0, end]
    {
        let mut section = Vec::new();
        encode_leb128(&mut section, func_count as u32);
        for _ in 0..func_count {
            // Function body: [local_decl_count=0, end]
            let body = vec![0x00, 0x0B]; // 0 locals, end
            encode_leb128(&mut section, body.len() as u32);
            section.extend_from_slice(&body);
        }
        wasm.push(10); // section id = Code
        encode_leb128(&mut wasm, section.len() as u32);
        wasm.extend_from_slice(&section);
    }

    wasm
}

fn encode_leb128(buf: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_capability_description() {
        assert!(!WasmCapability::FileRead.description().is_empty());
        assert!(WasmCapability::FileWrite.is_dangerous());
        assert!(!WasmCapability::FileRead.is_dangerous());
        assert!(WasmCapability::Network.is_dangerous());
        assert!(WasmCapability::Process.is_dangerous());
    }

    #[test]
    fn test_wasm_manifest_default() {
        let m = WasmPluginManifest::default();
        assert!(m.name.is_empty());
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.max_memory_pages, 256);
        assert_eq!(m.max_execution_ms, 30_000);
    }

    #[test]
    fn test_wasm_limits_default() {
        let l = WasmLimits::default();
        assert_eq!(l.max_memory, 16 * 1024 * 1024);
        assert_eq!(l.max_time_ms, 30_000);
    }

    #[test]
    fn test_wasm_runtime_new() {
        let rt = WasmRuntime::new();
        assert!(rt.list_plugins().is_empty());
        assert!(rt.plugin_dir().to_string_lossy().contains("wasm-plugins"));
    }

    #[test]
    fn test_wasm_runtime_with_plugin_dir() {
        let rt = WasmRuntime::with_plugin_dir("/tmp/plugins");
        assert_eq!(rt.plugin_dir(), Path::new("/tmp/plugins"));
    }

    #[test]
    fn test_wasm_runtime_capability_check() {
        let mut rt = WasmRuntime::new();
        // Insert a mock module
        rt.modules.insert(
            "test-plugin".into(),
            WasmModule {
                manifest: WasmPluginManifest {
                    name: "test-plugin".into(),
                    capabilities: vec![WasmCapability::FileRead],
                    exports: vec!["process".into()],
                    ..Default::default()
                },
                wasm_path: PathBuf::from("test.wasm"),
                validated: true,
                size_bytes: 100,
                hash: "abc123".into(),
            },
        );

        // Not granted yet
        assert!(!rt.has_capability("test-plugin", WasmCapability::FileRead));
        assert!(rt.check_capabilities("test-plugin").is_err());

        // Grant capability
        rt.grant_capabilities("test-plugin", vec![WasmCapability::FileRead])
            .unwrap();
        assert!(rt.has_capability("test-plugin", WasmCapability::FileRead));
        assert!(rt.check_capabilities("test-plugin").is_ok());
    }

    #[test]
    fn test_wasm_runtime_execute_simulated() {
        let mut rt = WasmRuntime::new();
        rt.modules.insert(
            "echo".into(),
            WasmModule {
                manifest: WasmPluginManifest {
                    name: "echo".into(),
                    capabilities: vec![],
                    exports: vec!["run".into()],
                    ..Default::default()
                },
                wasm_path: PathBuf::from("echo.wasm"),
                validated: true,
                size_bytes: 50,
                hash: "def456".into(),
            },
        );

        let result = rt.execute("echo", "run", "hello").unwrap();
        assert!(result.success);
        assert!(result.output.contains("[WASM]"));
        assert!(result.output.contains("echo::run"));
    }

    #[test]
    fn test_wasm_runtime_execute_missing_export() {
        let mut rt = WasmRuntime::new();
        rt.modules.insert(
            "test".into(),
            WasmModule {
                manifest: WasmPluginManifest {
                    name: "test".into(),
                    capabilities: vec![],
                    exports: vec!["run".into()],
                    ..Default::default()
                },
                wasm_path: PathBuf::from("test.wasm"),
                validated: true,
                size_bytes: 50,
                hash: "hash".into(),
            },
        );

        let result = rt.execute("test", "nonexistent", "input");
        assert!(result.is_err());
    }

    #[test]
    fn test_wasm_runtime_unload() {
        let mut rt = WasmRuntime::new();
        rt.modules.insert(
            "temp".into(),
            WasmModule {
                manifest: WasmPluginManifest {
                    name: "temp".into(),
                    ..Default::default()
                },
                wasm_path: PathBuf::from("temp.wasm"),
                validated: false,
                size_bytes: 0,
                hash: String::new(),
            },
        );

        assert!(rt.get_module("temp").is_some());
        assert!(rt.unload_plugin("temp"));
        assert!(rt.get_module("temp").is_none());
        assert!(!rt.unload_plugin("temp")); // already removed
    }

    #[test]
    fn test_wasm_security_audit() {
        let mut rt = WasmRuntime::new();
        rt.modules.insert(
            "audited".into(),
            WasmModule {
                manifest: WasmPluginManifest {
                    name: "audited".into(),
                    version: "1.0.0".into(),
                    author: "Test Author".into(),
                    capabilities: vec![WasmCapability::FileRead, WasmCapability::Network],
                    ..Default::default()
                },
                wasm_path: PathBuf::from("audited.wasm"),
                validated: true,
                size_bytes: 1024,
                hash: "abc".into(),
            },
        );

        let audit = rt.security_audit("audited").unwrap();
        assert!(audit.contains("audited"));
        assert!(audit.contains("1.0.0"));
        assert!(audit.contains("FileRead"));
        assert!(audit.contains("[DANGEROUS]"));
    }

    #[test]
    fn test_wasm_result_fields() {
        let r = WasmResult {
            plugin: "test".into(),
            function: "run".into(),
            output: "ok".into(),
            duration_ms: 42,
            memory_bytes: 1024,
            success: true,
            error: None,
        };
        assert!(r.success);
        assert_eq!(r.duration_ms, 42);
    }

    #[test]
    fn test_compute_sha256() {
        let h1 = compute_sha256(b"hello");
        let h2 = compute_sha256(b"hello");
        assert_eq!(h1, h2);
        let h3 = compute_sha256(b"world");
        assert_ne!(h1, h3);
    }

    // ── WasmEngine / bytecode tests ───────────────────────────

    #[test]
    fn test_build_test_wasm_module() {
        let wasm = build_test_wasm_module(&["run", "process"]);
        // Must start with WASM magic
        assert_eq!(&wasm[0..4], &[0x00, 0x61, 0x73, 0x6D]);
        // Version 1
        assert_eq!(&wasm[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_builtin_engine_validate() {
        let wasm = build_test_wasm_module(&["run", "transform"]);
        let engine = BuiltinWasmEngine::new();
        let exports = engine.validate(&wasm).unwrap();
        assert_eq!(exports, vec!["run", "transform"]);
    }

    #[test]
    fn test_builtin_engine_validate_bad_magic() {
        let engine = BuiltinWasmEngine::new();
        assert!(engine.validate(b"not wasm bytes").is_err());
    }

    #[test]
    fn test_builtin_engine_validate_too_short() {
        let engine = BuiltinWasmEngine::new();
        assert!(engine.validate(b"\x00asm").is_err());
    }

    #[test]
    fn test_builtin_engine_execute() {
        let wasm = build_test_wasm_module(&["process"]);
        let engine = BuiltinWasmEngine::new();
        let result = engine
            .execute(&wasm, "process", "hello", &WasmLimits::default())
            .unwrap();
        assert!(result.success);
        assert!(result.output.contains("process"));
        assert!(result.output.contains("hello"));
    }

    #[test]
    fn test_builtin_engine_execute_missing_fn() {
        let wasm = build_test_wasm_module(&["run"]);
        let engine = BuiltinWasmEngine::new();
        let result = engine.execute(&wasm, "nonexistent", "x", &WasmLimits::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_leb128_roundtrip() {
        let mut buf = Vec::new();
        encode_leb128(&mut buf, 624485);
        let (val, _) = decode_leb128(&buf).unwrap();
        assert_eq!(val, 624485);
    }
}
