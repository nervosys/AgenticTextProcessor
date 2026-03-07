# ATP Compliance — NIST FIPS, CMMC 2.0, and DoD Regulatory Posture

This document describes the Agentic Text Processor's (ATP) compliance posture with respect to NIST Federal Information Processing Standards (FIPS), the Cybersecurity Maturity Model Certification (CMMC) 2.0, and other Department of Defense (DoD) relevant regulations.

---

## Table of Contents

- [Overview](#overview)
- [Regulatory Frameworks](#regulatory-frameworks)
- [Cryptographic Compliance (FIPS 180-4)](#cryptographic-compliance-fips-180-4)
- [CMMC 2.0 Control Mapping](#cmmc-20-control-mapping)
- [CUI Data Marking](#cui-data-marking)
- [Audit Logging](#audit-logging)
- [Data Handling Policy](#data-handling-policy)
- [Deployment Guidance](#deployment-guidance)
- [CLI Usage](#cli-usage)
- [Programmatic API](#programmatic-api)

---

## Overview

ATP is a local, offline text processing tool that operates entirely in-memory with no network connections, no persistent storage, and no temporary files. This architecture inherently minimizes attack surface and simplifies compliance posture.

ATP provides **mechanisms** for regulatory compliance — cryptographic hashing, audit logging, CUI marking, and integrity verification. **Organizational policy** determines how these mechanisms are deployed and configured within a regulated environment.

## Regulatory Frameworks

| Framework        | Version | Coverage                                                    |
| ---------------- | ------- | ----------------------------------------------------------- |
| NIST FIPS 180-4  | 2015    | SHA-256 Secure Hash Standard — integrity verification       |
| NIST SP 800-53   | Rev 5   | Security controls — AU (Audit), SI (Integrity), CM (Config) |
| NIST SP 800-171  | Rev 2   | Protecting CUI — data marking, access control, audit        |
| CMMC 2.0         | 2.0     | DoD contractor cybersecurity requirements (Levels 1-3)      |
| DoD CUI Registry | Current | Controlled Unclassified Information categories and markings |

## Cryptographic Compliance (FIPS 180-4)

### Algorithm

ATP uses **SHA-256** (Secure Hash Algorithm 256-bit) as specified by FIPS 180-4 for all integrity verification operations.

### Implementation

- **Library**: RustCrypto `sha2` crate (v0.10.x)
- **Language**: Pure Rust with hardware acceleration via SHA-NI on supported CPUs
- **Algorithm correctness**: Verified against NIST test vectors (see unit tests)
- **Determinism**: SHA-256 is fully deterministic — identical inputs always produce identical outputs

### FIPS 140-3 Validation Status

> **Important**: The RustCrypto `sha2` crate implements the FIPS 180-4 *algorithm* correctly but is **not itself** a FIPS 140-3 *validated* cryptographic module.

For environments requiring FIPS 140-3 compliance:

1. Deploy ATP on a platform with **OS-level FIPS mode** enabled:
   - **Windows**: Enable FIPS mode via Group Policy (`Computer Configuration > Windows Settings > Security Settings > Local Policies > Security Options > "System cryptography: Use FIPS compliant algorithms"`)
   - **RHEL/CentOS**: `fips-mode-setup --enable`
   - **Ubuntu**: Install `ubuntu-fips` package
2. The OS-level FIPS mode ensures all cryptographic operations route through validated modules
3. ATP's SHA-256 implementation produces identical output to FIPS-validated implementations (algorithm compliance)

### Integrity Verification Use Cases

- **Input provenance**: SHA-256 hash of input files attached to every output envelope
- **File integrity manifests**: Multi-file hash manifests for baseline verification
- **Chain-of-custody**: Provenance records include input hashes for reproducibility

## CMMC 2.0 Control Mapping

ATP maps to the following CMMC 2.0 practices (based on NIST SP 800-171 Rev 2):

### Audit & Accountability (AU)

| Practice ID | NIST Ref | Title                  | Status        |
| ----------- | -------- | ---------------------- | ------------- |
| AU.L2-3.3.1 | 3.3.1    | System Auditing        | ✅ Implemented |
| AU.L2-3.3.2 | 3.3.2    | Audit Record Content   | ✅ Implemented |
| AU.L2-3.3.4 | 3.3.4    | Audit Failure Alerting | ⚠️ Partial     |

### Configuration Management (CM)

| Practice ID | NIST Ref | Title                    | Status        |
| ----------- | -------- | ------------------------ | ------------- |
| CM.L2-3.4.1 | 3.4.1    | System Baselining        | ✅ Implemented |
| CM.L2-3.4.3 | 3.4.3    | System Change Management | ✅ Implemented |

### Identification & Authentication (IA)

| Practice ID | NIST Ref | Title                       | Status |
| ----------- | -------- | --------------------------- | ------ |
| IA.L2-3.5.3 | 3.5.3    | Multi-factor Authentication | N/A    |

### Media Protection (MP)

| Practice ID | NIST Ref | Title            | Status             |
| ----------- | -------- | ---------------- | ------------------ |
| MP.L2-3.8.1 | 3.8.1    | Media Protection | 📋 Policy-Dependent |

### System & Communications Protection (SC)

| Practice ID   | NIST Ref | Title          | Status    |
| ------------- | -------- | -------------- | --------- |
| SC.L2-3.13.11 | 3.13.11  | CUI Encryption | ⚠️ Partial |

### System & Information Integrity (SI)

| Practice ID  | NIST Ref | Title                        | Status        |
| ------------ | -------- | ---------------------------- | ------------- |
| SI.L1-3.14.1 | 3.14.1   | Flaw Remediation             | ✅ Implemented |
| SI.L2-3.14.6 | 3.14.6   | Security Alerts & Advisories | ✅ Implemented |
| SI.L2-3.14.7 | 3.14.7   | Software & Input Integrity   | ✅ Implemented |

### Access Control (AC)

| Practice ID  | NIST Ref | Title                | Status        |
| ------------ | -------- | -------------------- | ------------- |
| AC.L2-3.1.20 | 3.1.20   | External Connections | ✅ Implemented |

### Status Legend

- ✅ **Implemented** — Fully implemented in ATP code
- ⚠️ **Partial** — Some aspects require external configuration (e.g., SIEM for alerting)
- 📋 **Policy-Dependent** — ATP provides the mechanism; organizational policy provides the management
- **N/A** — Not applicable to a local text processing tool

## CUI Data Marking

ATP supports Controlled Unclassified Information (CUI) markings per NIST SP 800-171 §3.1.20 and the DoD CUI Registry.

### Classification Levels

| Level     | Banner               | Description                                         |
| --------- | -------------------- | --------------------------------------------------- |
| `Public`  | `PUBLIC`             | No handling restrictions                            |
| `FOUO`    | `FOUO`               | For Official Use Only — federal employees           |
| `CUI`     | `CUI`                | Controlled Unclassified Information — CMMC Level 2+ |
| `CUI//SP` | `CUI//SP-{category}` | CUI with specified dissemination controls           |

### Dissemination Controls

| Control          | Description                         |
| ---------------- | ----------------------------------- |
| `NOFORN`         | No foreign dissemination            |
| `FED_ONLY`       | Federal employees only              |
| `FED_CON`        | Federal and contractor use          |
| `NO_RESTRICTION` | No restrictions beyond CUI baseline |

### Usage

Data markings can be attached to any ATP output envelope:

```rust
use atp_core::compliance::DataMarking;

// Mark output as CUI with Cyber Threat Intelligence category
let marking = DataMarking::cui(Some("CTI"));

// Attach to envelope
let envelope = AtpEnvelope::new("search", results, metadata)
    .with_data_marking(marking);
```

## Audit Logging

ATP generates structured audit events conforming to NIST SP 800-53 AU-3 (Content of Audit Records) and AU-8 (Time Stamps).

### Audit Event Fields

Per AU-3, every event contains:

| Field                | Description                                           | Requirement       |
| -------------------- | ----------------------------------------------------- | ----------------- |
| `event_id`           | UUIDv4 unique identifier                              | AU-3 (source)     |
| `timestamp`          | ISO 8601 UTC timestamp                                | AU-8              |
| `severity`           | INFO, WARNING, ERROR, CRITICAL                        | AU-3 (what type)  |
| `category`           | Event category (file_access, command_execution, etc.) | AU-3 (what type)  |
| `action`             | Human-readable description                            | AU-3 (what)       |
| `source_component`   | Originating component (e.g., "atp-core")              | AU-3 (where)      |
| `outcome`            | success, failure, denied, unknown                     | AU-3 (outcome)    |
| `details`            | Structured key-value data                             | AU-3 (additional) |
| `control_references` | CMMC/NIST controls satisfied                          | Traceability      |

### Auditable Event Categories

| Category              | Description                    | Controls                   |
| --------------------- | ------------------------------ | -------------------------- |
| `file_access`         | File read operations           | AU.L2-3.3.1, AU.L2-3.3.2   |
| `file_modification`   | File write/modify operations   | AU.L2-3.3.1, CM.L2-3.4.3   |
| `command_execution`   | CLI command invocations        | AU.L2-3.3.1, SI.L1-3.14.1  |
| `data_transformation` | Text transformation operations | AU.L2-3.3.1                |
| `query_execution`     | AQL query processing           | AU.L2-3.3.1                |
| `integrity_check`     | SHA-256 hash verification      | SI.L1-3.14.1, SI.L2-3.14.6 |
| `error_condition`     | Error/failure events           | AU.L2-3.3.4                |

### SIEM Integration

Audit logs export as **JSONL** (one JSON object per line) for SIEM ingestion:

```bash
# Export audit log in JSONL format
atp compliance --controls --format jsonl
```

Each line is a self-contained JSON object parseable by Splunk, Elastic, Azure Sentinel, and other SIEM platforms.

## Data Handling Policy

| Policy                 | Value    | Notes                                                  |
| ---------------------- | -------- | ------------------------------------------------------ |
| Network Access         | **None** | ATP operates entirely offline                          |
| Persistent Storage     | **None** | No databases, caches, or state files                   |
| Temporary Files        | **None** | All processing is in-memory                            |
| Memory-Only Processing | **Yes**  | Data exists only during execution                      |
| CUI Marking Support    | **Yes**  | Output envelopes support classification markings       |
| Audit Logging          | **Yes**  | Structured events for all security-relevant operations |
| Default Dry-Run        | **Yes**  | File modifications require explicit `--in-place`       |

### Security Implications

1. **No data at rest**: ATP never persists sensitive data beyond the user's existing file system
2. **No data in transit**: No network connections means no transmission security concerns
3. **Minimal attack surface**: No listening ports, no IPC, no shared memory
4. **Deterministic execution**: Same inputs always produce same outputs (reproducibility)
5. **Explicit destructive operations**: File modifications require explicit opt-in via `--in-place`

## Deployment Guidance

### For CMMC Level 1 (Basic Safeguarding of FCI)

ATP meets Level 1 requirements out of the box:
- SI.L1-3.14.1: Input validation on all patterns, expressions, and queries
- No network exposure, no persistent storage

### For CMMC Level 2 (Protection of CUI)

1. Enable OS-level FIPS mode on the deployment platform
2. Use CUI data markings on output containing controlled information
3. Direct audit log output to an organizational SIEM
4. Apply organizational access control policies to ATP binary and processed files
5. Include ATP in system baseline documentation (use `atp compliance --report`)

### For FIPS 140-3 Environments

1. **Enable OS FIPS mode** (see [Cryptographic Compliance](#cryptographic-compliance-fips-180-4))
2. **Verify SHA-256 correctness** using `atp compliance --integrity` against known test vectors
3. **Document** that ATP uses the FIPS 180-4 algorithm via a software implementation that produces correct output; FIPS 140-3 module validation is provided by the platform

## CLI Usage

```bash
# Full compliance report (JSON)
atp compliance

# Full compliance report (human-readable)
atp compliance --format human

# List all CMMC 2.0 controls
atp compliance --controls

# List only implemented controls
atp compliance --controls -s implemented

# List Audit & Accountability controls
atp compliance --controls -d AU

# SHA-256 file integrity hash
atp compliance -i src/main.rs

# Multi-file integrity manifest
atp compliance -i src/main.rs src/lib.rs Cargo.toml

# Aliases: cmmc, fips
atp cmmc --report
atp fips -i Cargo.lock
```

## Programmatic API

The `atp_core::compliance` module exposes all compliance types and functions:

```rust
use atp_core::compliance::{
    // FIPS 180-4 hashing
    sha256_hash, sha256_hash_file, sha256_hash_files,
    // Audit logging
    AuditLog, AuditEvent, AuditSeverity, AuditCategory, AuditOutcome,
    // CUI marking
    DataMarking, DataClassification, DisseminationControl,
    // CMMC 2.0 controls
    cmmc_control_mapping, CmmcControl, ControlStatus,
    // Compliance reporting
    generate_compliance_report, ComplianceReport,
};

// Hash a file (FIPS 180-4 SHA-256)
let hash = sha256_hash_file(Path::new("data.txt"))?;

// Create an audit log
let mut audit = AuditLog::new();
audit.record_file_access("data.txt", "read", AuditOutcome::Success);
audit.record_command("search", &["pattern".into()], AuditOutcome::Success);

// Export for SIEM
let jsonl = audit.to_jsonl();

// Mark output as CUI
let marking = DataMarking::cui(Some("CTI"));

// Generate compliance report
let report = generate_compliance_report();
```

---

*This document is generated from ATP's compliance module. Run `atp compliance --format human` for the live report.*
