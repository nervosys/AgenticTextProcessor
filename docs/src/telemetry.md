# Telemetry

ATP includes a comprehensive telemetry system for performance monitoring and optional remote usage analytics.

## Performance Telemetry

Track bottlenecks and throughput across processing phases.

### TelemetrySession

```rust
use atp_core::telemetry::*;

let mut session = TelemetrySession::new("search");

// Record phase performance
session.record_phase("file_io", bytes_read, elapsed_secs);
session.record_phase("regex", bytes_processed, elapsed_secs);

// Record counts
session.record_counts(files, lines, matches);

// Detect bottleneck (lowest throughput phase)
let bottleneck = session.detect_bottleneck();

// Generate report
let report = session.finalize(true, None);
println!("Bottleneck: {}", report.bottleneck);
println!("Total: {} files, {} lines, {} matches",
    report.total_files, report.total_lines, report.total_matches);
```

### SharedTelemetry (Thread-Safe)

```rust
use atp_core::telemetry::SharedTelemetry;

let telemetry = SharedTelemetry::new("pipeline");

// Safe to clone and share across threads
let t = telemetry.clone();
std::thread::spawn(move || {
    t.record_phase("worker", bytes, elapsed);
});
```

### Bottleneck Phases

| Phase          | Description                     |
| -------------- | ------------------------------- |
| `FileIO`       | File reading is the bottleneck  |
| `RegexMatch`   | Pattern matching is slowest     |
| `Transform`    | Text transformation is slowest  |
| `OutputFormat` | Output serialization is slowest |
| `Pipeline`     | Pipeline orchestration overhead |
| `AqlEval`      | AQL evaluation is slowest       |
| `None`         | No bottleneck detected          |

### Telemetry Report

Reports include:
- Session ID and timing
- Per-phase throughput (bytes/sec)
- Rolling and overall throughput
- Bottleneck phase identification
- Event log (phase transitions, regex recompiles, etc.)
- Export/import as JSON

## Remote Usage Telemetry (Optional)

Remote telemetry is **disabled by default** and must be explicitly opted in.

```rust
use atp_core::telemetry::*;

usage_enable();   // Opt in
record_command("search", "find \"TODO\"");
record_error("PATTERN_INVALID", "unclosed group");
usage_disable();  // Opt out
```

Remote telemetry uses AWS CloudWatch with SigV4 authentication. All data is anonymous — no file contents or patterns are transmitted.
