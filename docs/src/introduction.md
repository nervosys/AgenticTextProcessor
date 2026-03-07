# ATP — Agentic Text Processor

**An agentic-first successor to `grep`, `sed`, and `awk`, combined into one program.**

ATP is designed from the ground up for AI agent operation while remaining fully usable by humans through CLI, TUI, and GUI interfaces.

## Why ATP?

Traditional Unix text tools (`grep`, `sed`, `awk`) — born at Bell Labs in the 1970s — remain the most powerful text processing trinity on Unix systems. But they were designed for human terminals and shell pipes, producing unstructured text that AI agents cannot reliably parse.

ATP inherits the battle-tested semantics of grep/sed/awk and adds **strongly typed, deterministic, schema-validated output** that AI agents can reliably consume and reason over.

| Feature          | grep/sed/awk        | **ATP**                                       |
| ---------------- | ------------------- | --------------------------------------------- |
| Output format    | Unstructured text   | **Strongly typed JSON/YAML/CSV**              |
| Self-description | Man pages           | **Machine-readable ontology**                 |
| Error handling   | Exit codes + stderr | **Typed error taxonomy with suggestions**     |
| Multi-file       | Per-file invocation | **Atomic multi-file/directory operations**    |
| Composition      | Shell pipes (text)  | **Typed pipeline stages**                     |
| Reproducibility  | None                | **Full provenance tracking**                  |
| Safety           | No preview          | **Dry-run, explain, validate**                |
| Context          | Line-based          | **Function/block/indent-aware**               |
| Discoverability  | `--help` text       | **Complete ontology with schemas & examples** |

## Workspace Crates

| Crate      | Description                                      |
| ---------- | ------------------------------------------------ |
| `atp-core` | Library: engines, AQL, telemetry, compliance     |
| `atp-cli`  | Binary: `atp` + `atp-grep`, `atp-sed`, `atp-awk` |
| `atp-tui`  | Terminal UI (ratatui)                            |
| `atp-gui`  | Desktop GUI (egui/eframe)                        |
| `atp-wasm` | WebAssembly bindings (wasm-bindgen)              |
| `atp-lsp`  | Language Server Protocol for AQL files           |

## Quick Links

- [Getting Started](./getting-started.md)
- [CLI Reference](./cli-reference.md)
- [AQL Language](./aql.md)
- [Plugin System](./plugins.md)
- [Architecture](./architecture.md)
