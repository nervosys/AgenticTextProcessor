# Architecture

## Workspace Structure

```
AgenticTextProcessor/
├── src/atp-core/           # Core library
│   ├── src/
│   │   ├── engine/     # Processing engines
│   │   │   ├── aql.rs  # AQL parser & interpreter
│   │   │   ├── awk.rs  # Awk engine
│   │   │   ├── grep.rs # Grep engine
│   │   │   ├── sed.rs  # Sed engine
│   │   │   └── pipeline.rs  # Multi-stage pipelines
│   │   ├── compat.rs   # Backward-compatible CLI parsing
│   │   ├── compliance.rs # FIPS/CMMC compliance
│   │   ├── context.rs  # Function/block context detection
│   │   ├── ontology.rs # Machine-readable capability schema
│   │   ├── output.rs   # Output types and formatters
│   │   ├── plugin.rs   # Plugin system
│   │   ├── semantic.rs # TF-IDF semantic search
│   │   ├── telemetry.rs # Performance & usage telemetry
│   │   └── traversal.rs # File/directory traversal
│   ├── benches/        # Criterion benchmarks
│   └── tests/          # Integration tests
├── src/atp-cli/            # CLI binary
│   ├── src/
│   │   ├── main.rs     # Entry point & subcommand dispatch
│   │   └── commands/   # Per-command handlers
├── src/atp-tui/            # Terminal UI (ratatui)
├── src/atp-gui/            # Desktop GUI (egui/eframe)
├── src/atp-wasm/           # WebAssembly bindings
├── src/atp-lsp/            # LSP server for AQL
├── docs/               # mdBook documentation
└── .github/workflows/  # CI/CD pipelines
```

## Data Flow

```
Input (files/stdin)
    │
    ▼
┌─────────────┐
│  Traversal   │  File discovery, glob filtering, depth limits
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Engine     │  grep/sed/awk/AQL processing
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Output      │  Type-safe result structs
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Formatter   │  JSON/YAML/CSV/Human/JSONL
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Envelope    │  Schema, provenance, metadata
└─────────────┘
```

## Key Design Decisions

1. **Dry-run by default**: Transform operations preview changes without modifying files
2. **Typed output**: All results are strongly typed Rust structs, serialized via serde
3. **Engine separation**: Each engine (grep/sed/awk) is independent and composable
4. **AQL as unifier**: AQL provides a single syntax that dispatches to appropriate engines
5. **No async runtime**: Core library uses only `std::thread` to stay lightweight
6. **Minimal dependencies**: HMAC-SHA256 implemented using only `sha2` (no `hmac` crate)

## Test Architecture

- **Unit tests**: In each module (173 tests in atp-core)
- **Integration tests**: Cross-module end-to-end tests (21 tests)
- **WASM tests**: Browser-compatible tests (15 tests)
- **LSP tests**: Protocol-level tests (15 tests)
- **Benchmarks**: Criterion benchmarks for all engines
- **Doc tests**: Inline documentation examples (1 test)

Total: **225 tests**
