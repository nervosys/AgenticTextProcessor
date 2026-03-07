//! Compliance command — NIST FIPS, CMMC 2.0, and DoD regulatory reporting.
//!
//! Provides structured compliance information for regulatory requirements:
//! - Full compliance report with control mapping
//! - CMMC 2.0 control listing and status
//! - File integrity verification (SHA-256 / FIPS 180-4)
//! - Audit log export for SIEM ingestion

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use atp_core::compliance::{
    cmmc_control_mapping, generate_compliance_report, sha256_hash_file, sha256_hash_files,
    ControlStatus,
};
use atp_core::output::*;

#[derive(Args)]
pub struct ComplianceArgs {
    /// Show the full compliance posture report
    #[arg(long, short = 'r')]
    pub report: bool,

    /// List all CMMC 2.0 / NIST SP 800-171 controls and their status
    #[arg(long, short = 'c')]
    pub controls: bool,

    /// Verify file integrity using SHA-256 (FIPS 180-4)
    #[arg(long, short = 'i')]
    pub integrity: bool,

    /// Filter controls by status: implemented, partial, na, policy
    #[arg(long, short = 's')]
    pub status_filter: Option<String>,

    /// Filter controls by domain (e.g., "AU", "SI", "CM")
    #[arg(long, short = 'd')]
    pub domain_filter: Option<String>,

    /// Files to hash for integrity verification
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,
}

pub fn execute(args: ComplianceArgs, format: Format, raw: bool) -> Result<()> {
    // Default to --report if no specific sub-action is given
    let show_report = args.report || (!args.controls && !args.integrity);

    if args.integrity {
        return execute_integrity(args, format, raw);
    }

    if args.controls {
        return execute_controls(args, format, raw);
    }

    if show_report {
        return execute_report(format, raw);
    }

    Ok(())
}

/// Generate and display the full compliance report.
fn execute_report(format: Format, raw: bool) -> Result<()> {
    let start = std::time::Instant::now();
    let report = generate_compliance_report();
    let duration = start.elapsed().as_millis() as u64;

    if raw {
        match format {
            Format::Human => print_report_human(&report),
            Format::Yaml => println!("{}", serde_yaml::to_string(&report)?),
            _ => println!("{}", serde_json::to_string_pretty(&report)?),
        }
    } else {
        let provenance = Provenance::new("compliance", vec!["--report".into()]);
        let metadata = ExecutionMetadata {
            files_scanned: 0,
            files_matched: 0,
            duration_ms: duration,
            provenance,
        };
        let envelope = AtpEnvelope::new("compliance", report, metadata);
        println!("{}", OutputFormatter::format(&envelope, format)?);
    }

    Ok(())
}

/// List CMMC 2.0 controls with optional filtering.
fn execute_controls(args: ComplianceArgs, format: Format, raw: bool) -> Result<()> {
    let start = std::time::Instant::now();
    let mut controls = cmmc_control_mapping();

    // Filter by status
    if let Some(ref filter) = args.status_filter {
        let target = match filter.to_lowercase().as_str() {
            "implemented" | "impl" => ControlStatus::Implemented,
            "partial" => ControlStatus::Partial,
            "na" | "not-applicable" => ControlStatus::NotApplicable,
            "policy" | "policy-dependent" => ControlStatus::PolicyDependent,
            _ => anyhow::bail!(
                "Unknown status filter: {filter}. Use: implemented, partial, na, policy"
            ),
        };
        controls.retain(|c| c.status == target);
    }

    // Filter by domain
    if let Some(ref domain) = args.domain_filter {
        let domain_upper = domain.to_uppercase();
        controls.retain(|c| c.practice_id.starts_with(&domain_upper));
    }

    let duration = start.elapsed().as_millis() as u64;

    if raw {
        match format {
            Format::Human => {
                for c in &controls {
                    println!(
                        "[{}] {} — {} (Level {})",
                        c.status, c.practice_id, c.title, c.level
                    );
                    println!("    {}", c.implementation);
                    println!();
                }
            }
            Format::Yaml => println!("{}", serde_yaml::to_string(&controls)?),
            _ => println!("{}", serde_json::to_string_pretty(&controls)?),
        }
    } else {
        let provenance = Provenance::new("compliance", vec!["--controls".into()]);
        let metadata = ExecutionMetadata {
            files_scanned: 0,
            files_matched: controls.len(),
            duration_ms: duration,
            provenance,
        };
        let envelope = AtpEnvelope::new("compliance", controls, metadata);
        println!("{}", OutputFormatter::format(&envelope, format)?);
    }

    Ok(())
}

/// Verify file integrity using SHA-256.
fn execute_integrity(args: ComplianceArgs, format: Format, raw: bool) -> Result<()> {
    let start = std::time::Instant::now();

    if args.files.is_empty() {
        anyhow::bail!("Integrity check requires one or more file paths. Usage: atp compliance --integrity <FILE>...");
    }

    let result = if args.files.len() == 1 {
        // Single file — return hash directly
        let hash = sha256_hash_file(&args.files[0])?;
        let size = std::fs::metadata(&args.files[0])
            .map(|m| m.len())
            .unwrap_or(0);
        serde_json::json!({
            "algorithm": "SHA-256",
            "fips_standard": "FIPS 180-4",
            "file": args.files[0].display().to_string(),
            "sha256": hash,
            "size_bytes": size,
        })
    } else {
        // Multiple files — return manifest
        let paths: Vec<&std::path::Path> = args.files.iter().map(|p| p.as_path()).collect();
        let manifest = sha256_hash_files(&paths)?;
        serde_json::to_value(&manifest)?
    };

    let duration = start.elapsed().as_millis() as u64;

    if raw {
        match format {
            Format::Human => {
                if let Some(file) = result.get("file") {
                    println!(
                        "SHA-256 (FIPS 180-4): {} {}",
                        result["sha256"].as_str().unwrap_or(""),
                        file.as_str().unwrap_or("")
                    );
                } else if let Some(files) = result.get("files") {
                    for f in files.as_array().unwrap_or(&vec![]) {
                        println!(
                            "{}  {}",
                            f["sha256"].as_str().unwrap_or(""),
                            f["file"].as_str().unwrap_or("")
                        );
                    }
                    println!(
                        "\nManifest: {}",
                        result["manifest_hash"].as_str().unwrap_or("")
                    );
                }
            }
            Format::Yaml => println!("{}", serde_yaml::to_string(&result)?),
            _ => println!("{}", serde_json::to_string_pretty(&result)?),
        }
    } else {
        let provenance = Provenance::new(
            "compliance",
            args.files.iter().map(|p| p.display().to_string()).collect(),
        );
        let metadata = ExecutionMetadata {
            files_scanned: args.files.len(),
            files_matched: args.files.len(),
            duration_ms: duration,
            provenance,
        };
        let envelope = AtpEnvelope::new("compliance", result, metadata);
        println!("{}", OutputFormatter::format(&envelope, format)?);
    }

    Ok(())
}

/// Human-readable compliance report.
fn print_report_human(report: &atp_core::compliance::ComplianceReport) {
    println!("═══════════════════════════════════════════════════════════════");
    println!(
        "  ATP Compliance Report — {} v{}",
        report.tool, report.version
    );
    println!("  Generated: {}", report.report_timestamp);
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("REGULATORY FRAMEWORKS:");
    for fw in &report.frameworks {
        println!("  ▸ {} (v{})", fw.name, fw.version);
        println!("    {}", fw.description);
        println!(
            "    Implemented: {} | Partial: {} | N/A: {} | Policy: {}",
            fw.implemented_controls,
            fw.partial_controls,
            fw.not_applicable_controls,
            fw.policy_dependent_controls
        );
        println!();
    }

    println!("CRYPTOGRAPHIC MODULES:");
    for cm in &report.cryptographic_modules {
        println!("  ▸ {} ({})", cm.algorithm, cm.standard);
        println!("    Implementation: {}", cm.implementation);
        println!("    Crate: {} v{}", cm.crate_name, cm.crate_version);
        println!(
            "    FIPS 140-3 Validated: {}",
            if cm.fips_validated { "YES" } else { "NO" }
        );
        println!("    Note: {}", cm.fips_note);
        println!();
    }

    println!("DATA HANDLING POLICY:");
    let dh = &report.data_handling;
    println!("  Network Access:       {}", dh.network_access);
    println!("  Persistent Storage:   {}", dh.persistent_storage);
    println!("  Memory-Only:          {}", dh.memory_only_processing);
    println!("  CUI Marking Support:  {}", dh.cui_marking_support);
    println!("  Audit Logging:        {}", dh.audit_logging);
    println!("  Default Dry-Run:      {}", dh.default_dry_run);
    println!("  {}\n", dh.description);

    println!("CMMC 2.0 / NIST SP 800-171 CONTROLS:");
    for c in &report.controls {
        println!(
            "  [{}] {} — {} (Level {})",
            c.status, c.practice_id, c.title, c.level
        );
        println!("    Domain: {}", c.domain);
        println!("    NIST Ref: {}", c.nist_reference);
        println!("    {}", c.implementation);
        println!();
    }
}
