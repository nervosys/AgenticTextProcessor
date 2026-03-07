# ATP — Roadmap & Implementation Status

**Project**: Agentic Text Processor (ATP)
**Started**: March 2026
**Version**: 2.0.0
**Architecture**: Rust workspace — 6 crates (`atp-core`, `atp-cli`, `atp-tui`, `atp-gui`, `atp-wasm`, `atp-lsp`)

---

## Milestone 1: Core Engines ✅

**Status**: Complete

| Feature                                                               | File                              | Status |
| --------------------------------------------------------------------- | --------------------------------- | ------ |
| Grep engine (14 config options, multi-pattern, context, byte-offsets) | `src/atp-core/src/engine/grep.rs`     | ✅ Done |
| Sed engine (5 command types, address-range, in-place, backup)         | `src/atp-core/src/engine/sed.rs`      | ✅ Done |
| Awk engine (12 expression types, 9 conditions, 8 aggregations)        | `src/atp-core/src/engine/awk.rs`      | ✅ Done |
| Pipeline engine (9 stage types, DSL parser, YAML input)               | `src/atp-core/src/engine/pipeline.rs` | ✅ Done |
| AQL engine (tokenizer, parser, 15 stage types, evaluator)             | `src/atp-core/src/engine/aql.rs`      | ✅ Done |
| File traversal with gitignore, globs, depth limits                    | `src/atp-core/src/traversal.rs`       | ✅ Done |
| Context extraction (function, block, indent, lines, file)             | `src/atp-core/src/context.rs`         | ✅ Done |
| Output formatting (JSON, JSON-pretty, YAML, CSV, Human)               | `src/atp-core/src/output.rs`          | ✅ Done |
| Self-describing ontology (capabilities, commands, types, errors)      | `src/atp-core/src/ontology.rs`        | ✅ Done |

---

## Milestone 2: CLI Commands ✅

**Status**: Complete

| Command     | Aliases                          | File                                | Status |
| ----------- | -------------------------------- | ----------------------------------- | ------ |
| `search`    | `s`, `grep`, `find`              | `src/atp-cli/src/commands/search.rs`    | ✅ Done |
| `transform` | `t`, `sed`, `replace`            | `src/atp-cli/src/commands/transform.rs` | ✅ Done |
| `analyze`   | `a`, `awk`, `fields`             | `src/atp-cli/src/commands/analyze.rs`   | ✅ Done |
| `pipeline`  | `pipe`, `chain`                  | `src/atp-cli/src/commands/pipeline.rs`  | ✅ Done |
| `query`     | `q`, `aql`, `run`                | `src/atp-cli/src/commands/query.rs`     | ✅ Done |
| `ontology`  | `onto`, `capabilities`, `schema` | `src/atp-cli/src/commands/ontology.rs`  | ✅ Done |
| `explain`   | `x`, `preview`                   | `src/atp-cli/src/commands/explain.rs`   | ✅ Done |
| `scope`     | `ls`, `files`                    | `src/atp-cli/src/commands/scope.rs`     | ✅ Done |
| `validate`  | `check`                          | `src/atp-cli/src/commands/validate.rs`  | ✅ Done |
| `context`   | `ctx`                            | `src/atp-cli/src/commands/context.rs`   | ✅ Done |

---

## Milestone 3: AQL — ATP Query Language ✅

**Status**: Complete

| Feature                                                    | Status |
| ---------------------------------------------------------- | ------ |
| Keyword-based syntax design (15 stage types, 11 modifiers) | ✅ Done |
| Tokenizer (strings, regex, keywords, operators)            | ✅ Done |
| Recursive-descent parser                                   | ✅ Done |
| Full evaluator with all modifiers                          | ✅ Done |
| CLI `query` command with `--explain` and `--validate`      | ✅ Done |
| Stdin piping (`cat file \| atp query ...`)                 | ✅ Done |
| Shell pipe chaining (`atp query ... \| atp query ...`)     | ✅ Done |
| SYNTAX.md formal reference                                 | ✅ Done |

---

## Milestone 4: Completeness & Polish ✅

**Status**: Complete

| Task                                                             | File(s)                            | Status |
| ---------------------------------------------------------------- | ---------------------------------- | ------ |
| Add AQL validation to `validate` command                         | `src/atp-cli/src/commands/validate.rs` | ✅ Done |
| Add `query`, `validate`, `context` to ontology                   | `src/atp-core/src/ontology.rs`         | ✅ Done |
| Improve `explain` command (query/scope/validate/context support) | `src/atp-cli/src/commands/explain.rs`  | ✅ Done |
| Fix JSONL output format (true line-delimited)                    | `src/atp-core/src/output.rs`           | ✅ Done |
| TUI: implement Transform tab execution                           | `src/atp-tui/src/app.rs`               | ✅ Done |
| TUI: implement Analyze tab execution                             | `src/atp-tui/src/app.rs`               | ✅ Done |
| TUI: implement Pipeline tab execution                            | `src/atp-tui/src/app.rs`               | ✅ Done |
| GUI: implement Analyze tab                                       | `src/atp-gui/src/app.rs`               | ✅ Done |
| GUI: implement Pipeline tab                                      | `src/atp-gui/src/app.rs`               | ✅ Done |
| Unit tests for awk engine                                        | `src/atp-core/src/engine/awk.rs`       | ✅ Done |
| Unit tests for pipeline engine                                   | `src/atp-core/src/engine/pipeline.rs`  | ✅ Done |
| Unit tests for traversal                                         | `src/atp-core/src/traversal.rs`        | ✅ Done |
| Unit tests for context extraction                                | `src/atp-core/src/context.rs`          | ✅ Done |

---

## Milestone 5: Regulatory Compliance ✅

**Status**: Complete

| Task                                                      | File(s)                              | Status |
| --------------------------------------------------------- | ------------------------------------ | ------ |
| Audit current codebase for security posture               | —                                    | ✅ Done |
| FIPS 180-4 SHA-256 hashing (NIST test vectors verified)   | `src/atp-core/src/compliance.rs`         | ✅ Done |
| Structured audit logging (NIST SP 800-53 AU-3/AU-8)       | `src/atp-core/src/compliance.rs`         | ✅ Done |
| CUI data classification markings (NIST SP 800-171)        | `src/atp-core/src/compliance.rs`         | ✅ Done |
| CMMC 2.0 control mapping (12 controls across 7 domains)   | `src/atp-core/src/compliance.rs`         | ✅ Done |
| Compliance report generation                              | `src/atp-core/src/compliance.rs`         | ✅ Done |
| DataMarking field on AtpEnvelope                          | `src/atp-core/src/output.rs`             | ✅ Done |
| Provenance input_hash activation (with_input_hash)        | `src/atp-core/src/output.rs`             | ✅ Done |
| CLI `compliance` command (report/controls/integrity)      | `src/atp-cli/src/commands/compliance.rs` | ✅ Done |
| Ontology: regulatory_compliance capability + command spec | `src/atp-core/src/ontology.rs`           | ✅ Done |
| COMPLIANCE.md documentation                               | `COMPLIANCE.md`                      | ✅ Done |
| Unit tests for compliance module (11 tests)               | `src/atp-core/src/compliance.rs`         | ✅ Done |

---

## Milestone 6: Backwards Compatibility ✅

**Status**: Complete

| Task                                                    | File(s)                       | Status |
| ------------------------------------------------------- | ----------------------------- | ------ |
| POSIX grep arg parser → `GrepConfig` translation        | `src/atp-core/src/compat.rs`      | ✅ Done |
| POSIX sed arg parser → `SedConfig` translation          | `src/atp-core/src/compat.rs`      | ✅ Done |
| POSIX awk arg/program parser → `AwkConfig` translation  | `src/atp-core/src/compat.rs`      | ✅ Done |
| `atp-grep` binary (POSIX flags + typed output)          | `src/atp-cli/src/bin/atp_grep.rs` | ✅ Done |
| `atp-sed` binary (POSIX flags + typed output)           | `src/atp-cli/src/bin/atp_sed.rs`  | ✅ Done |
| `atp-awk` binary (POSIX flags + typed output)           | `src/atp-cli/src/bin/atp_awk.rs`  | ✅ Done |
| Stdin piping support for all compat binaries            | `src/atp-cli/src/bin/atp_*.rs`    | ✅ Done |
| Auto-format detection (JSON when piped, human when TTY) | `src/atp-cli/src/bin/atp_*.rs`    | ✅ Done |
| Unit tests for compat parsers (22 tests)                | `src/atp-core/src/compat.rs`      | ✅ Done |

---

## Milestone 7: Future Enhancements ✅

**Status**: Complete

| Feature                                                   | Priority | Status                                                         |
| --------------------------------------------------------- | -------- | -------------------------------------------------------------- |
| Streaming execution for large files                       | Medium   | ✅ Done — `search_reader`, `transform_reader`, `process_reader` |
| AQL REPL mode                                             | Medium   | ✅ Done — interactive shell, 10 dot-commands, history           |
| Watch mode for live file monitoring                       | Medium   | ✅ Done — `notify` crate, debounce, `--clear`                   |
| GUI: Apply button for transforms (in-place editing)       | Medium   | ✅ Done — confirmation dialog, backup support                   |
| Improved function detection heuristics                    | Low      | ✅ Done — multi-language, decorators, attributes                |
| Plugin/extension system                                   | Low      | ✅ Done — `AtpPlugin` trait, TOML manifests, registry           |
| Semantic/embedding-based search (`PatternType::Semantic`) | Medium   | ✅ Done — TF-IDF scoring, document ranking                      |
| WASM compilation for browser-based use                    | Low      | ✅ Done — `atp-wasm` crate, wasm-bindgen bindings               |
| LSP integration                                           | Low      | ✅ Done — `atp-lsp` crate, completion/hover/diagnostics         |

---

## Milestone 8: Production Hardening ✅

**Status**: Complete

| Task                                                      | File(s)                                     | Status |
| --------------------------------------------------------- | ------------------------------------------- | ------ |
| Remote telemetry system (CloudWatch-style)                | `src/atp-core/src/telemetry.rs`                 | ✅ Done |
| Semantic search engine (TF-IDF, BM25 scoring)             | `src/atp-core/src/semantic.rs`                  | ✅ Done |
| WASM bindings (grep/sed/awk/aql/semantic/ontology)        | `src/atp-wasm/src/lib.rs`                       | ✅ Done |
| LSP server (completion, hover, diagnostics, formatting)   | `src/atp-lsp/src/server.rs`                     | ✅ Done |
| CI/CD pipeline (8 jobs: lint, test, MSRV, miri, coverage) | `.github/workflows/ci.yml`                  | ✅ Done |
| Release workflows (7 targets, binary artifacts)           | `.github/workflows/release.yml`             | ✅ Done |
| Cross-compilation (5 targets)                             | `Cross.toml`, `.github/workflows/cross.yml` | ✅ Done |
| Criterion benchmarks (grep, sed, awk, aql, semantic)      | `src/atp-core/benches/engines.rs`               | ✅ Done |
| Shell completions (bash, zsh, fish, PowerShell, elvish)   | `src/atp-cli/src/commands/completions.rs`       | ✅ Done |
| Man page generation                                       | `src/atp-cli/src/commands/manpage.rs`           | ✅ Done |
| Integration test suite (21 tests)                         | `src/atp-core/tests/integration.rs`             | ✅ Done |
| Documentation site (mdBook, 27 pages)                     | `docs/`                                     | ✅ Done |
| crates.io publication metadata                            | `Cargo.toml` (all crates)                   | ✅ Done |
| AGPL-3.0-only license                                    | `LICENSE`                                   | ✅ Done |

---

## Milestone 9: v1.0 Release ✅

**Status**: Complete

| Task                                       | Status |
| ------------------------------------------ | ------ |
| Fix all clippy warnings (0 warnings)       | ✅ Done |
| Fix flaky telemetry test (race condition)  | ✅ Done |
| CHANGELOG.md (full release history)        | ✅ Done |
| Update ROADMAP.md for v1.0                 | ✅ Done |
| Update README.md for v1.0                  | ✅ Done |
| Bump version to 1.0.0                      | ✅ Done |
| Final validation (tests, clippy, fmt, doc) | ✅ Done |

---

## Milestone 10: v1.1 — Advanced Features ✅

**Status**: Complete

| Task                                                    | File(s)                          | Status |
| ------------------------------------------------------- | -------------------------------- | ------ |
| AQL v2: `let` variables                                 | `src/atp-core/src/engine/aql.rs`     | ✅ Done |
| AQL v2: `if`/`else`/`end` conditionals                  | `src/atp-core/src/engine/aql.rs`     | ✅ Done |
| AQL v2: `group by` with aggregations                    | `src/atp-core/src/engine/aql.rs`     | ✅ Done |
| AQL v2: `def`/`call` user-defined functions             | `src/atp-core/src/engine/aql.rs`     | ✅ Done |
| AQL v2 unit tests (14 new tests)                        | `src/atp-core/src/engine/aql.rs`     | ✅ Done |
| Parallel file search (rayon)                            | `src/atp-core/src/engine/grep.rs`    | ✅ Done |
| Memory-mapped I/O (memmap2)                             | `src/atp-core/src/engine/grep.rs`    | ✅ Done |
| Benchmark regression CI gate (120% threshold)           | `.github/workflows/ci.yml`       | ✅ Done |
| Configuration system (`~/.atp/config.toml`, `.atprc`)   | `src/atp-core/src/config.rs`         | ✅ Done |
| CLI `config` command (`--show`, `--init`, `--path`)     | `src/atp-cli/src/commands/config.rs` | ✅ Done |
| MCP server mode (4 tools, JSON-RPC 2.0 over stdio)      | `src/atp-cli/src/commands/mcp.rs`    | ✅ Done |
| VS Code extension (AQL grammar, LSP client)             | `editors/vscode/`                | ✅ Done |
| Property-based tests (11 proptest tests)                | `src/atp-core/tests/property.rs`     | ✅ Done |
| Fuzz harness (3 cargo-fuzz targets + corpus)            | `fuzz/`                          | ✅ Done |
| Homebrew formula                                        | `packaging/homebrew/atp.rb`      | ✅ Done |
| Scoop manifest (Windows)                                | `packaging/scoop/atp.json`       | ✅ Done |
| Debian packaging (control, rules, changelog, copyright) | `packaging/debian/`              | ✅ Done |
| RPM spec                                                | `packaging/rpm/atp.spec`         | ✅ Done |

---

## Milestone 11: v1.2 — Async, AI, Code Intel, Remote, Plugin SDK ✅

**Status**: Complete

| Task                                                  | File(s)                           | Status |
| ----------------------------------------------------- | --------------------------------- | ------ |
| Async I/O & streaming (tokio, URL sources, channels)  | `src/atp-core/src/async_io.rs`        | ✅ Done |
| AI/LLM integration (Ollama, OpenAI, NL-to-AQL)        | `src/atp-core/src/ai.rs`              | ✅ Done |
| Code intelligence (14 languages, symbol extraction)   | `src/atp-core/src/code_intel.rs`      | ✅ Done |
| Remote execution (SSH, parallel, host files)          | `src/atp-core/src/remote.rs`          | ✅ Done |
| CLI `ai` command (NL-to-AQL, explain, suggest)        | `src/atp-cli/src/commands/ai.rs`      | ✅ Done |
| CLI `remote` command (SSH distributed execution)      | `src/atp-cli/src/commands/remote.rs`  | ✅ Done |
| CLI `symbols` command (code intelligence queries)     | `src/atp-cli/src/commands/symbols.rs` | ✅ Done |
| CLI `plugin` command (install, scaffold, validate)    | `src/atp-cli/src/commands/plugin.rs`  | ✅ Done |
| TUI overhaul (file browser, AQL tab, Symbols tab)     | `src/atp-tui/src/`                    | ✅ Done |
| Plugin SDK (scaffold, validate, package, marketplace) | `src/atp-core/src/plugin.rs`          | ✅ Done |
| Crates.io publication prep (version deps, dry-run)    | `Cargo.toml` (all crates)         | ✅ Done |

---

## Milestone 12: v1.3 — Index, DAP, WASM, Notebooks, Distributed ✅

**Status**: Complete

| Task                                                       | File(s)                               | Status |
| ---------------------------------------------------------- | ------------------------------------- | ------ |
| Fix flaky telemetry test (USAGE_TEST_MUTEX serialization)  | `src/atp-core/src/telemetry.rs`           | ✅ Done |
| Tree-sitter code intelligence (scope trees, call graphs)   | `src/atp-core/src/code_intel.rs`          | ✅ Done |
| Incremental file index (trigram search, persistence)       | `src/atp-core/src/index.rs`               | ✅ Done |
| DAP integration (AQL debugger, breakpoints, snapshots)     | `src/atp-core/src/dap.rs`                 | ✅ Done |
| WASM plugin runtime (sandbox, capability security)         | `src/atp-core/src/wasm_runtime.rs`        | ✅ Done |
| Notebook / literate mode (markdown + AQL, execute, render) | `src/atp-core/src/notebook.rs`            | ✅ Done |
| Distributed pipelines (scatter/gather, partition, merge)   | `src/atp-core/src/distributed.rs`         | ✅ Done |
| CLI `index` command (build, search, status, update, files) | `src/atp-cli/src/commands/index.rs`       | ✅ Done |
| CLI `debug` command (step, run, info modes)                | `src/atp-cli/src/commands/debug.rs`       | ✅ Done |
| CLI `notebook` command (run, render, info actions)         | `src/atp-cli/src/commands/notebook.rs`    | ✅ Done |
| CLI `distributed` command (scatter/gather execution)       | `src/atp-cli/src/commands/distributed.rs` | ✅ Done |
| 67 new unit tests across 5 new modules                     | —                                     | ✅ Done |

---

## Milestone 13: v1.4 — Streaming, Tracing, WASM Engine, DAP Wire, LSP v2 ✅

**Status**: Complete

| Task                                                         | File(s)                           | Status |
| ------------------------------------------------------------ | --------------------------------- | ------ |
| Aho-Corasick multi-pattern grep                              | `src/atp-core/src/engine/grep.rs`     | ✅ Done |
| OpenTelemetry-style tracing (TracingConfig, TracedOperation) | `src/atp-core/src/telemetry.rs`       | ✅ Done |
| Streaming pipelines (async mpsc, StreamingConfig)            | `src/atp-core/src/engine/pipeline.rs` | ✅ Done |
| LSP v2 (context-aware completion, workspace symbols)         | `src/atp-lsp/src/server.rs`           | ✅ Done |
| LSP notebook diagnostics for `.atp.md` files                 | `src/atp-lsp/src/server.rs`           | ✅ Done |
| TUI v2 (10 tabs: +Index, +Debug, +Notebook, +Distributed)    | `src/atp-tui/src/app.rs`              | ✅ Done |
| Real WASM engine (WasmEngine trait, binary parser)           | `src/atp-core/src/wasm_runtime.rs`    | ✅ Done |
| Project `.atp.toml` (PipelineDefaults, ScopeDefaults)        | `src/atp-core/src/config.rs`          | ✅ Done |
| DAP wire protocol (Content-Length framed JSON encode/decode) | `src/atp-core/src/dap.rs`             | ✅ Done |
| End-to-end integration tests (10 new, 30 total)              | `src/atp-core/tests/integration.rs`   | ✅ Done |
| README.md refresh (badges, diagram, commands, tests)         | `README.md`                       | ✅ Done |
| ~55 new unit tests across all enhanced modules               | —                                 | ✅ Done |

---

## Milestone 14: v1.5 — Cache, Diff, Profile, Schema, Optimizer, Rate Limit, Git, Snapshot ✅

**Status**: Complete

| Task                                                     | File(s)                      | Status |
| -------------------------------------------------------- | ---------------------------- | ------ |
| Incremental compilation cache (SHA-256, LRU, TTL, disk)  | `src/atp-core/src/cache.rs`      | ✅ Done |
| Structured diff engine (JSON diff, text diff, patch)     | `src/atp-core/src/diff.rs`       | ✅ Done |
| Profile-guided optimization hints (4 analysis rules)     | `src/atp-core/src/profile.rs`    | ✅ Done |
| Schema registry (10 JSON Schema 2020-12 definitions)     | `src/atp-core/src/schema.rs`     | ✅ Done |
| Query plan optimizer (pushdown, dead-stage, dedup)       | `src/atp-core/src/optimizer.rs`  | ✅ Done |
| Rate-limited pipelines (token bucket, circuit breaker)   | `src/atp-core/src/rate_limit.rs` | ✅ Done |
| Git-aware search (changed files, blame, line ranges)     | `src/atp-core/src/git_search.rs` | ✅ Done |
| Snapshot testing harness (create, verify, update, norm.) | `src/atp-core/src/snapshot.rs`   | ✅ Done |
| 4 new fuzz targets (pipeline, config, diff, optimizer)   | `fuzz/fuzz_targets/`         | ✅ Done |
| ~90 new unit tests across 8 new modules                  | —                            | ✅ Done |

---

## Milestone 15: v1.6 — Task Queue, Patterns, Checkpoint, Log Sink, Redact, Dep Graph, Template, Workspace, Hooks, Metrics ✅

**Status**: Complete

| Task                                                     | File(s)                      | Status |
| -------------------------------------------------------- | ---------------------------- | ------ |
| Task queue with dependency DAG (Kahn's toposort, retry)  | `src/atp-core/src/task_queue.rs` | ✅ Done |
| Pattern registry (4 built-in sets, 20 patterns)          | `src/atp-core/src/patterns.rs`   | ✅ Done |
| Checkpoint / resume manager (SHA-256, name index, disk)  | `src/atp-core/src/checkpoint.rs` | ✅ Done |
| Structured log sink (5 format parsers, alerts)           | `src/atp-core/src/log_sink.rs`   | ✅ Done |
| Sensitive data redactor (8 rules, 4 mask strategies)     | `src/atp-core/src/redact.rs`     | ✅ Done |
| Dependency graph analyzer (cycles, toposort, DOT export) | `src/atp-core/src/dep_graph.rs`  | ✅ Done |
| Template engine (Mustache-style, partials, iteration)    | `src/atp-core/src/template.rs`   | ✅ Done |
| Workspace aggregator (multi-project discovery, 8 kinds)  | `src/atp-core/src/workspace.rs`  | ✅ Done |
| Event bus / hook system (priority dispatch, skip/abort)  | `src/atp-core/src/hooks.rs`      | ✅ Done |
| Metrics registry (Counter, Gauge, Histogram, Prometheus) | `src/atp-core/src/metrics.rs`    | ✅ Done |
| ~136 new unit tests across 10 new modules                | —                            | ✅ Done |

---

## Milestone 16: v1.7 — Rule Engine, Formatter, Scheduler, Encryption, Changelog, Linter, Data Table, Rewrite, Archive, Report ✅

**Status**: Complete

| Task                                                         | File(s)                       | Status |
| ------------------------------------------------------------ | ----------------------------- | ------ |
| Declarative if-then rule engine (11 conditions, 9 actions)   | `src/atp-core/src/rule_engine.rs` | ✅ Done |
| Table formatter (4 border styles, alignment, overflow)       | `src/atp-core/src/formatter.rs`   | ✅ Done |
| Cron-like scheduler (5-field cron, missed-run detection)     | `src/atp-core/src/scheduler.rs`   | ✅ Done |
| Encryption (PBKDF2-HMAC-SHA256, XOR stream, auth tags)       | `src/atp-core/src/encryption.rs`  | ✅ Done |
| Changelog generator (Conventional Commits, semver bumps)     | `src/atp-core/src/changelog.rs`   | ✅ Done |
| Text/code linter (7 rule types, autofix, file-type filter)   | `src/atp-core/src/linter.rs`      | ✅ Done |
| In-memory data table (filter, sort, group-by, join, pivot)   | `src/atp-core/src/data_table.rs`  | ✅ Done |
| Multi-pass rewrite engine (named rule sets, scope, dry-run)  | `src/atp-core/src/rewrite.rs`     | ✅ Done |
| Archive processor (tar, gzip, search, extract, auto-detect)  | `src/atp-core/src/archive.rs`     | ✅ Done |
| Report composer (Markdown, HTML, JSON, PlainText, templates) | `src/atp-core/src/report.rs`      | ✅ Done |
| ~148 new unit tests across 10 new modules                    | —                             | ✅ Done |

---

## Milestone 17: v1.8 — Converter, Statistics, State Machine, I18n, Batch, Annotation, Highlight, Timeline, Tree, Validator ✅

**Status**: Complete

| Task                                                          | File(s)                         | Status |
| ------------------------------------------------------------- | ------------------------------- | ------ |
| Multi-format converter (JSON, YAML, TOML, CSV, XML, INI)      | `src/atp-core/src/converter.rs`     | ✅ Done |
| Text statistics (readability, entropy, Zipf, n-grams)         | `src/atp-core/src/statistics.rs`    | ✅ Done |
| Programmable state machine (guards, actions, DOT export)      | `src/atp-core/src/state_machine.rs` | ✅ Done |
| Unicode / i18n (normalisation, transliteration, scripts)      | `src/atp-core/src/i18n.rs`          | ✅ Done |
| Transactional batch operations (MemFs, rollback, dry-run)     | `src/atp-core/src/batch.rs`         | ✅ Done |
| Annotation engine (spans, overlap resolution, kappa)          | `src/atp-core/src/annotation.rs`    | ✅ Done |
| Syntax highlighting (5 languages, ANSI/HTML/SVG, themes)      | `src/atp-core/src/highlight.rs`     | ✅ Done |
| Timeline / temporal events (bucketing, anomalies, sparklines) | `src/atp-core/src/timeline.rs`      | ✅ Done |
| Generic tree (DFS/BFS, diff, merge, prune, JSON)              | `src/atp-core/src/tree.rs`          | ✅ Done |
| Multi-format validator (email, URL, semver, UUID, IP, schema) | `src/atp-core/src/validator.rs`     | ✅ Done |
| ~158 new unit tests across 10 new modules                     | —                               | ✅ Done |

---


## Milestone 18: v1.9 — Codec, Fingerprint, Markdown, Sampler, Tokenizer, Spellcheck, Calendar, Compress, Color, Macro Engine ✅

**Status**: Complete

| Task                                                          | File(s)                          | Status |
| ------------------------------------------------------------- | -------------------------------- | ------ |
| Multi-codec encoder/decoder (Base64, Hex, URL, HTML, ROT13)  | `src/atp-core/src/codec.rs`          | ✅ Done |
| Document fingerprinting (SimHash, MinHash, shingling)         | `src/atp-core/src/fingerprint.rs`    | ✅ Done |
| Markdown parser and renderer (AST, HTML, TOC, links)          | `src/atp-core/src/markdown.rs`       | ✅ Done |
| Text sampling (Reservoir, Systematic, Stratified, Weighted)   | `src/atp-core/src/sampler.rs`        | ✅ Done |
| Multi-granularity tokenizer (Word, Sentence, BPE)             | `src/atp-core/src/tokenizer.rs`      | ✅ Done |
| Spell checker (Levenshtein, Soundex, suggestions)             | `src/atp-core/src/spellcheck.rs`     | ✅ Done |
| Date/time utilities (parsing, arithmetic, relative dates)     | `src/atp-core/src/calendar.rs`       | ✅ Done |
| Text compression (RLE, LZ77, Huffman coding)                  | `src/atp-core/src/compress.rs`       | ✅ Done |
| Color manipulation (RGB/HSL, WCAG accessibility, palettes)    | `src/atp-core/src/color.rs`          | ✅ Done |
| Macro engine (11 step types, variables, recorder, recursion)  | `src/atp-core/src/macro_engine.rs`   | ✅ Done |
| ~165 new unit tests across 10 new modules                     | —                                | ✅ Done |

---

## Milestone 19: v2.0 — Fuzzy, Summarizer, Casing, Text Wrap, MIME, Graph, Shell, URL, Table Extract, Emoji ✅

**Status**: Complete

| Task                                                          | File(s)                          | Status |
| ------------------------------------------------------------- | -------------------------------- | ------ |
| Fuzzy string matching (Jaro, Jaro-Winkler, trigrams)          | `src/atp-core/src/fuzzy.rs`          | ✅ Done |
| Text summarization (TF-IDF, TextRank, Lead strategies)        | `src/atp-core/src/summarizer.rs`     | ✅ Done |
| Case conversion and detection (10 styles)                     | `src/atp-core/src/casing.rs`         | ✅ Done |
| Text wrapping (word-wrap, hard-wrap, columns, dedent)         | `src/atp-core/src/text_wrap.rs`      | ✅ Done |
| MIME type detection (magic bytes, extensions, charset)         | `src/atp-core/src/mime.rs`           | ✅ Done |
| Text graph analytics (co-occurrence, PageRank, TextRank)      | `src/atp-core/src/graph.rs`          | ✅ Done |
| Shell command analysis (shebang, portability, escaping)        | `src/atp-core/src/shell.rs`          | ✅ Done |
| URL parsing and manipulation (normalize, resolve, encode)     | `src/atp-core/src/url.rs`            | ✅ Done |
| Table detection and extraction (Markdown, ASCII, fixed-width) | `src/atp-core/src/table_extract.rs`  | ✅ Done |
| Emoji detection and manipulation (find, sentiment, shortcodes)| `src/atp-core/src/emoji.rs`          | ✅ Done |
| ~175 new unit tests across 10 new modules                     | —                                | ✅ Done |

---
## CLI Commands (28 total)

| Command       | Aliases                          | File                                  | Milestone |
| ------------- | -------------------------------- | ------------------------------------- | --------- |
| `search`      | `s`, `grep`, `find`              | `src/atp-cli/src/commands/search.rs`      | 2         |
| `transform`   | `t`, `sed`, `replace`            | `src/atp-cli/src/commands/transform.rs`   | 2         |
| `analyze`     | `a`, `awk`, `fields`             | `src/atp-cli/src/commands/analyze.rs`     | 2         |
| `pipeline`    | `pipe`, `chain`                  | `src/atp-cli/src/commands/pipeline.rs`    | 2         |
| `query`       | `q`, `aql`, `run`                | `src/atp-cli/src/commands/query.rs`       | 2         |
| `ontology`    | `onto`, `capabilities`, `schema` | `src/atp-cli/src/commands/ontology.rs`    | 2         |
| `explain`     | `x`, `preview`                   | `src/atp-cli/src/commands/explain.rs`     | 2         |
| `scope`       | `ls`, `files`                    | `src/atp-cli/src/commands/scope.rs`       | 2         |
| `validate`    | `check`                          | `src/atp-cli/src/commands/validate.rs`    | 2         |
| `context`     | `ctx`                            | `src/atp-cli/src/commands/context.rs`     | 2         |
| `compliance`  | `audit`, `cmmc`                  | `src/atp-cli/src/commands/compliance.rs`  | 5         |
| `repl`        | `shell`, `interactive`           | `src/atp-cli/src/commands/repl.rs`        | 7         |
| `watch`       | `monitor`, `w`                   | `src/atp-cli/src/commands/watch.rs`       | 7         |
| `stream`      | —                                | `src/atp-cli/src/commands/stream.rs`      | 7         |
| `completions` | —                                | `src/atp-cli/src/commands/completions.rs` | 8         |
| `manpage`     | —                                | `src/atp-cli/src/commands/manpage.rs`     | 8         |
| `plugins`     | —                                | `src/atp-cli/src/commands/plugins.rs`     | 7         |
| `config`      | `cfg`                            | `src/atp-cli/src/commands/config.rs`      | 10        |
| `mcp`         | `serve`                          | `src/atp-cli/src/commands/mcp.rs`         | 10        |
| `ai`          | `llm`                            | `src/atp-cli/src/commands/ai.rs`          | 11        |
| `remote`      | `ssh`                            | `src/atp-cli/src/commands/remote.rs`      | 11        |
| `symbols`     | `sym`, `code`                    | `src/atp-cli/src/commands/symbols.rs`     | 11        |
| `plugin`      | `plug`                           | `src/atp-cli/src/commands/plugin.rs`      | 11        |
| `index`       | `idx`                            | `src/atp-cli/src/commands/index.rs`       | 12        |
| `debug`       | `dbg`                            | `src/atp-cli/src/commands/debug.rs`       | 12        |
| `notebook`    | `nb`, `literate`                 | `src/atp-cli/src/commands/notebook.rs`    | 12        |
| `distributed` | `dist`, `scatter`                | `src/atp-cli/src/commands/distributed.rs` | 12        |

---

## Test Coverage

| Module             | Tests                                                 | Status |
| ------------------ | ----------------------------------------------------- | ------ |
| `engine::grep`     | 17 unit tests                                         | ✅      |
| `engine::sed`      | 16 unit tests                                         | ✅      |
| `engine::awk`      | 10 unit tests                                         | ✅      |
| `engine::pipeline` | 15 unit tests                                         | ✅      |
| `engine::aql`      | 42 unit tests + 1 doc-test                            | ✅      |
| `traversal`        | 6 unit tests                                          | ✅      |
| `context`          | 14 unit tests                                         | ✅      |
| `compliance`       | 11 unit tests                                         | ✅      |
| `compat`           | 22 unit tests                                         | ✅      |
| `plugin`           | 8 unit tests                                          | ✅      |
| `telemetry`        | 46 unit tests                                         | ✅      |
| `semantic`         | 13 unit tests                                         | ✅      |
| `output`           | 14 unit tests                                         | ✅      |
| `config`           | 12 unit tests                                         | ✅      |
| Integration tests  | 30 tests                                              | ✅      |
| Property tests     | 11 proptest tests                                     | ✅      |
| atp-lsp            | 25 unit tests                                         | ✅      |
| `async_io`         | 7 unit tests                                          | ✅      |
| `ai`               | 5 unit tests                                          | ✅      |
| `code_intel`       | 17 unit tests                                         | ✅      |
| `remote`           | 12 unit tests                                         | ✅      |
| `index`            | 10 unit tests                                         | ✅      |
| `dap`              | 18 unit tests                                         | ✅      |
| `wasm_runtime`     | 19 unit tests                                         | ✅      |
| `notebook`         | 13 unit tests                                         | ✅      |
| `distributed`      | 13 unit tests                                         | ✅      |
| atp-wasm           | 15 unit tests                                         | ✅      |
| `cache`            | 10 unit tests                                         | ✅      |
| `diff`             | 14 unit tests                                         | ✅      |
| `profile`          | 9 unit tests                                          | ✅      |
| `schema`           | 8 unit tests                                          | ✅      |
| `optimizer`        | 10 unit tests                                         | ✅      |
| `rate_limit`       | 14 unit tests                                         | ✅      |
| `git_search`       | 13 unit tests                                         | ✅      |
| `snapshot`         | 14 unit tests                                         | ✅      |
| `task_queue`       | 13 unit tests                                         | ✅      |
| `patterns`         | 14 unit tests                                         | ✅      |
| `checkpoint`       | 13 unit tests                                         | ✅      |
| `log_sink`         | 14 unit tests                                         | ✅      |
| `redact`           | 14 unit tests                                         | ✅      |
| `dep_graph`        | 15 unit tests                                         | ✅      |
| `template`         | 14 unit tests                                         | ✅      |
| `workspace`        | 12 unit tests                                         | ✅      |
| `hooks`            | 13 unit tests                                         | ✅      |
| `metrics`          | 14 unit tests                                         | ✅      |
| `rule_engine`      | 18 unit tests                                         | ✅      |
| `formatter`        | 16 unit tests                                         | ✅      |
| `scheduler`        | 16 unit tests                                         | ✅      |
| `encryption`       | 14 unit tests                                         | ✅      |
| `changelog`        | 16 unit tests                                         | ✅      |
| `linter`           | 14 unit tests                                         | ✅      |
| `data_table`       | 15 unit tests                                         | ✅      |
| `rewrite`          | 14 unit tests                                         | ✅      |
| `archive`          | 13 unit tests                                         | ✅      |
| `report`           | 12 unit tests                                         | ✅      |
| `converter`        | 17 unit tests                                         | ✅      |
| `statistics`       | 15 unit tests                                         | ✅      |
| `state_machine`    | 16 unit tests                                         | ✅      |
| `i18n`             | 16 unit tests                                         | ✅      |
| `batch`            | 15 unit tests                                         | ✅      |
| `annotation`       | 16 unit tests                                         | ✅      |
| `highlight`        | 16 unit tests                                         | ✅      |
| `timeline`         | 15 unit tests                                         | ✅      |
| `tree`             | 16 unit tests                                         | ✅      |
| `validator`        | 16 unit tests                                         | ✅      |
| Fuzz targets       | 7 (aql, grep, sed, pipeline, config, diff, optimizer) | ✅      |
| `codec`            | 17 unit tests                                         | ✅      |
| `fingerprint`      | 17 unit tests                                         | ✅      |
| `markdown`         | 17 unit tests                                         | ✅      |
| `sampler`          | 15 unit tests                                         | ✅      |
| `tokenizer`        | 16 unit tests                                         | ✅      |
| `spellcheck`       | 16 unit tests                                         | ✅      |
| `calendar`         | 18 unit tests                                         | ✅      |
| `compress`         | 15 unit tests                                         | ✅      |
| `color`            | 18 unit tests                                         | ✅      |
| `macro_engine`     | 16 unit tests                                         | ✅      |
| `fuzzy`            | 19 unit tests                                         | ✅      |
| `summarizer`       | 16 unit tests                                         | ✅      |
| `casing`           | 23 unit tests                                         | ✅      |
| `text_wrap`        | 16 unit tests                                         | ✅      |
| `mime`             | 17 unit tests                                         | ✅      |
| `graph`            | 15 unit tests                                         | ✅      |
| `shell`            | 17 unit tests                                         | ✅      |
| `url`              | 18 unit tests                                         | ✅      |
| `table_extract`    | 17 unit tests                                         | ✅      |
| `emoji`            | 17 unit tests                                         | ✅      |
| **Total**          | **~1235 tests + 1 doc-test**                           | ✅      |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                       ATP Ecosystem                          │
├─────────┬─────────┬─────────┬──────────┬─────────┬──────────┤
│  atp    │ atp-tui │ atp-gui │ atp-wasm │ atp-lsp │  Agent   │
│  (CLI)  │  (TUI)  │  (GUI)  │  (WASM)  │  (LSP)  │via JSON  │
├─────────┴─────────┴─────────┴──────────┴─────────┴──────────┤
│            POSIX Compatibility Binaries                      │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐                  │
│  │ atp-grep  │ │ atp-sed   │ │ atp-awk   │                  │
│  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘                  │
│        └──────────────┼──────────────┘                        │
│                  compat.rs                                    │
├──────────────────────────────────────────────────────────────┤
│                       atp-core                                │
│  ┌───────────────────────────────────────────────────────┐   │
│  │              AQL Engine (Unified)                      │   │
│  │   Tokenizer → Parser → AST → Evaluator                │   │
│  └───────────────────────────────────────────────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  Grep    │ │  Sed     │ │  Awk     │ │ Semantic │        │
│  │  Engine  │ │  Engine  │ │  Engine  │ │  Search  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Pipeline │ │ Ontology │ │ Context  │ │Telemetry │        │
│  │  Engine  │ │  System  │ │ Extract  │ │  System  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │Traversal │ │  Output  │ │Compliance│ │  Plugin  │        │
│  │  Walker  │ │ Formatter│ │  Module  │ │  System  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Async IO │ │    AI    │ │  Code    │ │  Remote  │        │
│  │ Streaming│ │  Engine  │ │  Intel   │ │  Executor│        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  Index   │ │   DAP    │ │  WASM    │ │ Notebook │        │
│  │  Engine  │ │ Debugger │ │ Runtime  │ │ Literate │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │Distribute│ │  Cache   │ │   Diff   │ │ Profile  │        │
│  │ Pipeline │ │  Engine  │ │  Engine  │ │  Hints   │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  Schema  │ │Optimizer │ │RateLimit │ │   Git    │        │
│  │ Registry │ │  (AQL)   │ │ Executor │ │  Search  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Snapshot │ │TaskQueue │ │ Patterns │ │Checkpoint│        │
│  │  Harness │ │  (DAG)   │ │ Registry │ │  Resume  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Log Sink │ │  Redact  │ │ DepGraph │ │ Template │        │
│  │  Parser  │ │  Engine  │ │ Analyzer │ │  Engine  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐                     │
│  │Workspace │ │  Hooks   │ │ Metrics  │                     │
│  │Aggregator│ │EventBus  │ │ Registry │                     │
│  └──────────┘ └──────────┘ └──────────┘                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  Rule    │ │Formatter │ │Scheduler │ │Encryption│        │
│  │  Engine  │ │  Tables  │ │  Cron    │ │  Module  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │Changelog │ │  Linter  │ │DataTable │ │ Rewrite  │        │
│  │Generator │ │  Engine  │ │ Columnar │ │  Engine  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐                                   │
│  │ Archive  │ │  Report  │                                   │
│  │Processor │ │ Composer │                                   │
│  └──────────┘ └──────────┘                                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │Converter │ │Statistic │ │  State   │ │   I18n   │        │
│  │ Formats  │ │  Engine  │ │ Machine  │ │ Unicode  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │  Batch   │ │Annotator │ │Highlight │ │ Timeline │        │
│  │  Ops     │ │  Engine  │ │  Syntax  │ │  Events  │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│  ┌──────────┐ ┌──────────┐                                   │
│  │   Tree   │ │Validator │                                   │
│  │Structure │ │  Multi   │                                   │
│  └──────────┘ └──────────┘                                   │
└──────────────────────────────────────────────────────────────┘
```
