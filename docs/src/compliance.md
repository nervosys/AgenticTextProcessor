# Compliance

ATP implements compliance features for NIST FIPS, CMMC 2.0, and DoD regulatory requirements.

## FIPS 180-4 SHA-256

All integrity hashing uses FIPS 180-4 compliant SHA-256 via the RustCrypto `sha2` crate.

```bash
# Generate compliance report
atp compliance --format json
```

## CMMC 2.0 Control Mapping

ATP maps its features to CMMC 2.0 controls across multiple domains:

- **AU (Audit & Accountability)**: Audit logging for all operations
- **CM (Configuration Management)**: Provenance tracking, dry-run by default
- **IA (Identification & Authentication)**: Not applicable (local tool)
- **MP (Media Protection)**: CUI data marking support
- **SI (System & Information Integrity)**: SHA-256 file integrity verification

## Data Classification

ATP supports CUI marking per NIST SP 800-171:

| Classification | Description                             |
| -------------- | --------------------------------------- |
| `Public`       | No restrictions                         |
| `Internal`     | Organization internal use               |
| `CUI`          | Controlled Unclassified Information     |
| `CUISpecified` | CUI with specific handling requirements |

## Audit Logging

All operations are auditable through the `AuditLog` system:

```rust
use atp_core::compliance::AuditLog;

let mut log = AuditLog::new();
log.record_file_access("src/main.rs", "read", true);
log.record_command("search", &["TODO", "src/"]);

// Export as JSONL
let jsonl = log.to_jsonl();
```

## Compliance Report

```bash
atp compliance --format json-pretty
```

Outputs framework summaries, control mappings, cryptographic module details, and data handling policies.
