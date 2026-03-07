# Agentic Text Processor (ATP)

[![Version](https://img.shields.io/badge/version-2.0.0-blue)](CHANGELOG.md)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0--only-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/tests-1235%20passing-brightgreen)](#testing)

**An agentic-first single-program successor to the `grep`, `sed`, `awk` triad.**

ATP is designed from the ground up for AI agent operation while remaining fully usable by humans through CLI, TUI, GUI, WebAssembly, and Language Server Protocol interfaces.

### v2.0.0 Highlights

- **Fuzzy** — fuzzy string matching with Jaro, Jaro-Winkler, trigram similarity, bounded Levenshtein, hybrid scoring
- **Summarizer** — text summarization with TF-IDF, TextRank, and Lead strategies, keyword extraction
- **Casing** — case conversion and detection for 10 styles (camelCase, snake_case, kebab-case, PascalCase, etc.)
- **Text Wrap** — word-wrap, hard-wrap, columns, dedent, indent, hanging indent, alignment
- **MIME** — MIME type detection from magic bytes (16+ signatures), file extension mapping (~102 extensions), charset
- **Graph** — text graph analytics with co-occurrence graphs, PageRank, TextRank, centrality, DOT export
- **Shell** — shell command analysis with shebang parsing, bashism detection, portability checking, escaping
- **URL** — URL parsing, normalization, relative resolution, percent encoding/decoding, domain/TLD extraction
- **Table Extract** — table detection from Markdown, ASCII box-drawing, and fixed-width text formats
- **Emoji** — emoji detection, counting, sentiment analysis, shortcode conversion, skin tone handling

### v1.9.0 Highlights

- **Codec** — multi-format encoder/decoder with 7 codecs (Base64, Hex, URL, HTML, ROT13, QP, Percent) and auto-detection
- **Fingerprint** — document fingerprinting with SimHash, MinHash, shingle sets, rolling hashes, and duplicate detection
- **Markdown** — Markdown parser with block-level AST, HTML/plain-text rendering, TOC generation, link extraction, slugify
- **Sampler** — text sampling with 5 methods (random, reservoir, systematic, stratified, weighted) and deterministic PRNG
- **Tokenizer** — multi-granularity tokenizer (word, sentence, paragraph, fixed-length, regex) with BPE sub-word support
- **Spellcheck** — spell checker with Levenshtein/Damerau edit distance, Soundex phonetics, suggestions, auto-correct
- **Calendar** — date/time parsing (ISO-8601, US, EU, short), epoch conversion, arithmetic, relative dates, ISO weeks
- **Compress** — text compression with RLE, LZ77 sliding window, and Huffman coding with bit packing and analysis
- **Color** — hex/RGB/HSL parsing, WCAG 2.1 accessibility analysis, palette generation, lighten/darken/mix, ANSI output
- **Macro engine** — 11 step types, variable substitution, macro recorder, 3 built-in macros, recursion guard at depth 64

### v1.8.0 Highlights

- **Converter** — format conversion between JSON, YAML, TOML, CSV, XML, and INI with auto-detection
- **Statistics** — readability scores, Shannon entropy, Zipf analysis, word frequency, n-grams, char distribution
- **State machine** — programmable state machine with guards, actions, reachability analysis, and Graphviz export
- **I18n** — Unicode normalization, transliteration, script detection, locale-aware collation, grapheme clusters
- **Batch** — transactional batch operations with dry-run, rollback, undo log, and in-memory filesystem
- **Annotation** — text annotation engine with overlap strategies, standoff/inline/JSON export, Cohen's kappa
- **Highlight** — syntax highlighting for 5 languages with ANSI, HTML, SVG, and plain-text output
- **Timeline** — temporal event streams with bucketing, anomaly detection, sparklines, and duration histograms
- **Tree** — generic tree data structure with DFS/BFS iterators, diff, merge, and JSON serialization
- **Validator** — multi-format validation (email, URL, semver, UUID, IP) with JSON Schema and custom rule DSL

### v1.7.0 Highlights

- **Rule engine** — declarative if-then rules with 11 condition types, 9 actions, priorities, and conflict strategies
- **Formatter** — table formatting with 4 border styles (None, Ascii, Unicode, Markdown), alignment, overflow, word wrap
- **Scheduler** — cron-like 5-field expressions with one-shot/repeating schedules and missed-run detection
- **Encryption** — PBKDF2-HMAC-SHA256 key derivation, XOR stream cipher, HMAC authentication tags, JSON payloads
- **Changelog generator** — Conventional Commits parsing, semantic version bump suggestions, Markdown/JSON output
- **Linter** — configurable text/code linting with 7 built-in rule types, autofix support, file-type filtering
- **Data table** — in-memory columnar table with filter, sort, group-by, join, pivot, and CSV import/export
- **Rewrite engine** — multi-pass rewrite rules with named rule sets, scope constraints, dry-run preview
- **Archive processor** — tar/gzip creation and parsing, search inside archives, glob-based extraction
- **Report composer** — multi-section reports with Markdown, HTML, JSON, and PlainText output formats

### v1.6.0 Highlights

- **Task queue** — dependency DAG with Kahn's topological sort, priority scheduling, retry policies, and cancellation
- **Pattern registry** — 4 built-in pattern sets (log parsing, security, PII, code smells) with 20 named-capture patterns
- **Checkpoint / resume** — SHA-256 content-addressed checkpoints with name-indexed tracking and disk persistence
- **Structured log sink** — 5 format parsers (JSON, Syslog, CLF, Key-Value, Auto), severity filtering, and alert rules
- **Sensitive data redactor** — 8 detection rules (email, SSN, credit card, JWT, etc.) with 4 masking strategies
- **Dependency graph** — cycle detection, topological sort, transitive closure, depth analysis, and Graphviz DOT export
- **Template engine** — Mustache-style rendering with conditionals, iteration, partials, and HTML escaping
- **Workspace aggregator** — multi-project discovery across 8 project kinds with cross-project regex search
- **Event bus / hooks** — priority-ordered hook dispatch with skip/abort control, event logging, and statistics
- **Metrics registry** — thread-safe Counter, Gauge, and Histogram metrics with Prometheus text format export

---

## Why ATP?

Traditional Unix text tools (`grep`, `sed`, `awk`) — born at Bell Labs in the 1970s — remain the most powerful text processing trinity on Unix systems. Ken Thompson's grep (1973), Lee McMahon's sed (1977), and Aho, Weinberger & Kernighan's awk (1977) follow the Unix philosophy of small, composable tools. But they were designed for human terminals and shell pipes, producing unstructured text that AI agents cannot reliably parse.

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

---

## Installation

```bash
# Build from source
cargo build --release

# Install CLI
cargo install --path atp-cli

# Install TUI
cargo install --path atp-tui

# Install GUI
cargo install --path atp-gui

# Build WASM package (requires wasm-pack)
wasm-pack build atp-wasm --target web

# Generate shell completions (bash, zsh, fish, powershell, elvish)
atp completions bash > ~/.local/share/bash-completion/completions/atp
atp completions zsh > ~/.zfunc/_atp
atp completions fish > ~/.config/fish/completions/atp.fish
atp completions powershell > atp.ps1

# Generate man pages
atp manpage /usr/local/share/man/man1/
```

Binaries produced: `atp`, `atp-grep`, `atp-sed`, `atp-awk`, `atp-tui`, `atp-gui`

### Cross-Platform Support

Pre-built binaries are available for:

| Target                      | Platform              |
| --------------------------- | --------------------- |
| `x86_64-unknown-linux-gnu`  | Linux (x86_64)        |
| `x86_64-unknown-linux-musl` | Linux (static)        |
| `aarch64-unknown-linux-gnu` | Linux (ARM64)         |
| `x86_64-apple-darwin`       | macOS (Intel)         |
| `aarch64-apple-darwin`      | macOS (Apple Silicon) |
| `x86_64-pc-windows-msvc`    | Windows (x86_64)      |
| `aarch64-pc-windows-msvc`   | Windows (ARM64)       |
| `wasm32-unknown-unknown`    | WebAssembly           |

---

## Quick Start

### For AI Agents

```bash
# 1. Discover capabilities (always start here)
atp ontology --format json

# 2. Get schema for a specific command
atp ontology -c search --format json

# 3. Validate a pattern before use
atp validate -t pattern 'fn\s+\w+'

# 4. Search with structured output
atp search 'fn\s+\w+' src/ --include '*.rs' --format json

# 5. Preview a transformation (dry run)
atp transform -e 's/old_name/new_name/g' src/ --format json

# 6. Execute a pipeline
atp pipeline -e 'search:TODO | filter:FIXME | count' src/ --format json
```

### For Humans

```bash
# Search (like grep, but better)
atp search 'TODO|FIXME' src/ --context 2

# Transform (like sed, but safer — dry-run by default)
atp transform -e 's/foo/bar/g' src/

# Apply transformation
atp transform -e 's/foo/bar/g' src/ --in-place --backup bak

# Analyze (like awk)
atp analyze -F ',' -f '1,3' --aggregate 'count' data.csv

# Pipeline
atp pipeline -e 'search:import | sort | unique' src/

# Interactive TUI
atp-tui

# Desktop GUI
atp-gui

# Interactive REPL
atp repl

# Watch mode
atp watch 'find "error"' logs/
```

---

## POSIX Compatibility — Drop-In Replacements

ATP ships with `atp-grep`, `atp-sed`, and `atp-awk` — standalone binaries that accept **traditional POSIX flag syntax** but produce **typed structured output**. Use your existing muscle memory while gaining typed JSON/YAML/CSV output with provenance metadata.

```bash
# grep-compatible (typed output when piped, human-readable in TTY)
atp-grep -i -r 'TODO|FIXME' src/
atp-grep -c 'error' *.log
atp-grep -e 'foo' -e 'bar' --include='*.rs' src/
echo "hello world" | atp-grep 'hello'

# sed-compatible (dry-run by default, use -i for in-place)
atp-sed 's/old/new/g' file.txt
atp-sed -i.bak 's/debug/release/g' *.rs
atp-sed -e 's/a/b/' -e 's/c/d/' file.txt
echo "hello" | atp-sed 's/hello/world/'

# awk-compatible
atp-awk -F ',' '{print $1, $3}' data.csv
atp-awk '/error/ {print $2}' log.txt
atp-awk '{sum += $2} END {print sum}' numbers.txt
echo "a b c" | atp-awk '{print $2}'

# Force JSON output (for agents)
atp-grep --format json -r 'pattern' src/ | jq '.data.total_matches'
```

| Binary     | POSIX Equivalent | Output                     |
| ---------- | ---------------- | -------------------------- |
| `atp-grep` | `grep`           | `SearchResults` (typed)    |
| `atp-sed`  | `sed`            | `TransformResults` (typed) |
| `atp-awk`  | `awk`            | `AnalysisResults` (typed)  |

---

## AQL — ATP Query Language

AQL is a **unified, keyword-based query language** that replaces the fragmented regex-heavy syntaxes of `grep`, `sed`, and `awk` with a single composable language. AQL is jointly optimized for AI agents and humans.

```bash
# Instead of: grep -i 'error' *.log | awk -F'|' '{print $3}' | sort | uniq -c | sort -rn | head -10
# Write:
atp query 'find "error" ignore_case | set separator "|" | select fields 3 | sort | unique | take 10'
```

### Why AQL?

| Classical Syntax               | AQL Equivalent                                       | Improvement               |
| ------------------------------ | ---------------------------------------------------- | ------------------------- |
| `grep -i 'pattern'`            | `find "pattern" ignore_case`                         | Self-documenting modifier |
| `sed 's/old/new/g'`            | `replace "old" with "new" all`                       | No delimiter confusion    |
| `awk -F',' '{print $1,$3}'`    | `set separator "," \| select fields 1, 3`            | Readable keywords         |
| `sed '/start/,/end/s/a/b/g'`   | `replace "a" with "b" all between "start" and "end"` | English-readable          |
| `grep X \| sed ... \| awk ...` | `find "X" \| replace ... \| select ...`              | **One syntax, not three** |

### Quick Examples

```bash
# Literal search (strings default — no escaping needed)
atp query 'find "hello world"'

# Regex search (opt-in with /…/)
atp query 'find /fn\s+\w+/'

# Pipeline: find → sort → unique → take first 10
atp query 'find "TODO" | sort | unique | take 10' src/

# Validate syntax without executing
atp query --validate 'find "X" | sort desc | take 5'

# Explain what a query will do
atp query --explain 'find "error" | replace "error" with "warning" all | count'
```

**Key Design Principles:**
- **Literal by default** — `"hello.world"` matches the literal dot; use `/hello.world/` for regex
- **Keyword-based** — `ignore_case`, `whole_word`, `all` instead of cryptic flags
- **Composable** — pipe stages with `|`, each stage transforms typed data
- **Self-validating** — `--validate` checks syntax; `--explain` describes behavior
- **Agent-optimized** — deterministic parsing, zero ambiguity, precise error messages

See [SYNTAX.md](SYNTAX.md) for the complete grammar, stage reference, and comparison tables.

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
│  │ +Stream  │ │  System  │ │ Extract  │ │ +Tracing │        │
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
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │Workspace │ │  Hooks   │ │ Metrics  │ │  Config  │        │
│  │Aggregator│ │EventBus  │ │ Registry │ │.atp.toml │        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
└──────────────────────────────────────────────────────────────┘
```

### Crate Structure (6 Crates)

| Crate          | Binary                                  | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`atp-core`** | *(library)*                             | Core library: 79 modules — engines, AQL, ontology, output, compliance, plugins, telemetry, tracing, streaming, code intel, index, DAP, WASM, notebooks, distributed, cache, diff, profile, schema, optimizer, rate-limit, git-search, snapshot, task-queue, patterns, checkpoint, log-sink, redact, dep-graph, template, workspace, hooks, metrics, rule-engine, formatter, scheduler, encryption, changelog, linter, data-table, rewrite, archive, report, converter, statistics, state-machine, i18n, batch, annotation, highlight, timeline, tree, validator, codec, fingerprint, markdown, sampler, tokenizer, spellcheck, calendar, compress, color, macro-engine, fuzzy, summarizer, casing, text-wrap, mime, graph, shell, url, table-extract, emoji |
| **`atp-cli`**  | `atp`, `atp-grep`, `atp-sed`, `atp-awk` | CLI with 28 subcommands + POSIX-compatible binaries                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **`atp-tui`**  | `atp-tui`                               | Terminal UI with 10 tabs: Search, Transform, Analyze, Pipeline, AQL, Symbols, Index, Debug, Notebook, Distributed                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| **`atp-gui`**  | `atp-gui`                               | Desktop GUI with visual search, pipeline builder, and results viewer                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **`atp-wasm`** | *(npm package)*                         | WebAssembly bindings for browser and Node.js usage                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **`atp-lsp`**  | `atp-lsp`                               | Language Server Protocol v2: diagnostics, hover, completion, workspace symbols, notebook support                                                                                                                                                                                                                                                                                                                                                                                                                                                                |

---

## Agentic-First Features

### 1. Self-Describing Ontology

Every command, parameter, type, and capability is machine-readable:

```bash
atp ontology                          # Full ontology
atp ontology -c search                # Search command spec
atp ontology -s capabilities          # All capabilities
atp ontology -s types                 # All output types with schemas
atp ontology -s errors                # Error taxonomy
```

### 2. Strongly Typed Output Envelope

Every ATP output is wrapped in a universal envelope:

```json
{
  "version": "1.4.0",
  "command": "search",
  "timestamp": "2026-03-05T12:00:00Z",
  "deterministic": true,
  "schema_ref": "https://atp.nervosys.com/schemas/v1.4.0/search.json",
  "data": { ... },
  "metadata": {
    "files_scanned": 42,
    "files_matched": 7,
    "duration_ms": 15,
    "provenance": {
      "tool": "atp",
      "tool_version": "1.4.0",
      "command": "search",
      "args": ["TODO", "src/"],
      "working_directory": "/project",
      "input_hash": null
    }
  }
}
```

### 3. Deterministic Guarantees

Same inputs always produce the same outputs. The `deterministic` field in every envelope confirms this contract.

### 4. Explain Before Execute

Preview any command's behavior before running it:

```bash
atp explain "search 'pattern' src/"
atp explain "transform -e 's/old/new/g' --in-place src/"
```

### 5. Validate Without Executing

Check patterns, expressions, and pipelines for validity:

```bash
atp validate -t pattern 'fn\s+\w+'
atp validate -t expression 's/old/new/g'
atp validate -t pipeline 'search:TODO | filter:FIXME | count'
```

### 6. Smart Context Extraction

Go beyond line-based context to extract meaningful code regions:

```bash
atp context src/main.rs -L 42 -m function    # Enclosing function
atp context src/main.rs -L 42 -m block       # Enclosing block
atp context src/main.rs -L 42 -m indent      # Indentation scope
atp context src/main.rs -L 42 -m lines -n 10 # Fixed line range
```

### 7. Composable Pipelines

Chain operations with a simple DSL:

```bash
# Find, filter, deduplicate, sort, and take top results
atp pipeline -e 'search:import | unique | sort | head:20' src/

# Search, transform, and count
atp pipeline -e 'search:TODO | transform:s/TODO/DONE/g | count' src/
```

Pipeline stages: `search`, `filter`, `transform`, `analyze`, `sort`, `unique`, `head`, `tail`, `count`

### 8. Scope Control

Fine-grained control over which files are processed:

```bash
atp scope --include '*.rs' --exclude '*/test/*' --max-depth 5 src/
```

### 9. Provenance Tracking

Every output includes full provenance for reproducibility — tool version, exact command, arguments, working directory, and optional input content hash.

### 10. Typed Error Taxonomy

Errors are categorized and include recovery suggestions:

```json
{
  "code": "INVALID_PATTERN",
  "message": "Regex parse error: ...",
  "context": "Pattern: '[invalid'",
  "suggestion": "Check regex syntax. Use --literal for literal string matching."
}
```

### 11. Regulatory Compliance

Built-in support for FIPS 180-4, NIST SP 800-53/800-171, and CMMC 2.0:

```bash
# Generate compliance report
atp compliance report --format json

# List CMMC 2.0 controls
atp compliance controls --level 2

# Verify file integrity (FIPS SHA-256)
atp compliance integrity src/ --format json
```

See [COMPLIANCE.md](COMPLIANCE.md) for full regulatory compliance documentation.

### 12. Interactive REPL

An interactive AQL shell with readline editing and persistent history:

```bash
# Start the REPL
atp repl

# With a specific scope
atp repl --scope src/ --include '*.rs'

# Inside the REPL:
# atp> find "TODO" ignore_case | count
# atp> .scope src/
# atp> .format json-pretty
# atp> .help
# atp> .history
# atp> .quit
```

Dot-commands: `.help`, `.scope`, `.format`, `.include`, `.exclude`, `.depth`, `.history`, `.status`, `.clear`, `.quit`

### 13. Watch Mode

Monitor files for changes and automatically re-run queries:

```bash
# Watch for changes and re-run a search
atp watch 'find "TODO"' src/

# With custom debounce and screen clearing
atp watch 'find "error" ignore_case | count' logs/ --debounce 1000 --clear
```

### 14. Plugin System

Extend ATP with custom pipeline stages and output formats via TOML-based plugin manifests:

```bash
# Plugins are loaded from ~/.atp/plugins/*.toml
# Example plugin manifest:
```

```toml
[plugin]
name = "redact-emails"
version = "1.0.0"
description = "Redact email addresses from output"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "replace"
pattern = "[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}"
replacement = "[REDACTED]"
global = true
```

Plugin transform types: `replace`, `filter`, `sort`, `aggregate`, `shell`

### 15. Streaming Execution

All three engines support streaming execution for memory-efficient processing of large files:

```rust
// Process files line-by-line without loading into memory
let results = engine.search_file_streaming("path/to/huge.log")?;
let output = sed_engine.transform_file_streaming("path/to/huge.log")?;
let records = awk_engine.process_file_streaming("path/to/huge.csv")?;
```
```

---

## Output Formats

All commands support `--format`:

| Format      | Flag                   | Description                              |
| ----------- | ---------------------- | ---------------------------------------- |
| JSON        | `--format json`        | Compact JSON (default for piped output)  |
| JSON Pretty | `--format json-pretty` | Indented JSON                            |
| JSONL       | `--format jsonl`       | JSON Lines (one object per line)         |
| YAML        | `--format yaml`        | YAML output                              |
| CSV         | `--format csv`         | CSV output                               |
| Human       | `--format human`       | Colored terminal output (default in TTY) |

Auto-detection: ATP outputs JSON when piped to another program, human-readable text when in a terminal.

---

## Commands Reference

| Command       | Aliases                          | Description                                |
| ------------- | -------------------------------- | ------------------------------------------ |
| **`query`**   | **`q`, `aql`, `run`**            | **AQL unified query language (preferred)** |
| `search`      | `s`, `grep`, `find`              | Pattern search across files                |
| `transform`   | `t`, `sed`, `replace`            | Text transformation                        |
| `analyze`     | `a`, `awk`, `fields`             | Field-based processing                     |
| `pipeline`    | `pipe`, `chain`                  | Multi-stage pipelines (legacy DSL)         |
| `ontology`    | `onto`, `capabilities`, `schema` | Machine-readable self-description          |
| `explain`     | `x`, `preview`                   | Command explanation                        |
| `scope`       | `ls`, `files`                    | File scope listing                         |
| `validate`    | `check`                          | Input validation                           |
| `context`     | `ctx`                            | Smart context extraction                   |
| `compliance`  | `audit`, `cmmc`                  | Regulatory compliance (FIPS, CMMC 2.0)     |
| `repl`        | `shell`, `interactive`           | Interactive AQL shell with history         |
| `watch`       | `monitor`, `w`                   | File monitoring with auto re-execution     |
| `stream`      | —                                | Streaming stdin processing                 |
| `completions` | —                                | Generate shell completions                 |
| `manpage`     | `man`                            | Generate man pages                         |
| `plugins`     | —                                | List and manage plugins                    |
| `config`      | `cfg`                            | Configuration management                   |
| `mcp`         | `serve`                          | Model Context Protocol server              |
| `ai`          | `llm`                            | AI/LLM integration (NL-to-AQL)             |
| `remote`      | `ssh`                            | Remote SSH execution                       |
| `symbols`     | `sym`, `code`                    | Code intelligence / symbol extraction      |
| `plugin`      | `plug`                           | Plugin SDK (scaffold, validate, install)   |
| `index`       | `idx`                            | Incremental file index                     |
| `debug`       | `dbg`                            | AQL pipeline debugger (DAP)                |
| `notebook`    | `nb`, `literate`                 | Notebook / literate mode                   |
| `distributed` | `dist`, `scatter`                | Distributed scatter/gather pipelines       |

---

## grep/sed/awk Compatibility

ATP maps directly to the classic Unix text processing trinity. If you know grep/sed/awk, you already know ATP.

### grep → `atp search`

| grep flag              | ATP equivalent                     | Description                     |
| ---------------------- | ---------------------------------- | ------------------------------- |
| `grep -i`              | `atp search -i`                    | Case-insensitive                |
| `grep -v`              | `atp search -v`                    | Invert match                    |
| `grep -w`              | `atp search -w`                    | Whole word                      |
| `grep -c`              | `atp search -c`                    | Count matches per file          |
| `grep -l`              | `atp search --files-only`          | List filenames with matches     |
| `grep -L`              | `atp search --files-without-match` | List filenames without matches  |
| `grep -o`              | `atp search -o`                    | Only matching portion           |
| `grep -F`              | `atp search -F`                    | Fixed/literal string (no regex) |
| `grep -n`              | *(always on)*                      | Line numbers in every output    |
| `grep -r`              | *(always on)*                      | Recursive by default            |
| `grep -A/-B/-C`        | `atp search -A/-B/-C`              | Context lines after/before/both |
| `grep -m`              | `atp search -m`                    | Max matches                     |
| `grep -e pat1 -e pat2` | `atp search pat1 -e pat2`          | Multi-pattern (OR)              |
| `grep --include=*.rs`  | `atp search --include '*.rs'`      | File glob filter                |
| `grep --color`         | *(auto-detected)*                  | Colored output in TTY           |

### sed → `atp transform`

| sed command                  | ATP equivalent                                               | Description                   |
| ---------------------------- | ------------------------------------------------------------ | ----------------------------- |
| `sed 's/old/new/'`           | `atp transform -e 's/old/new/'`                              | First-occurrence substitution |
| `sed 's/old/new/g'`          | `atp transform -e 's/old/new/g'`                             | Global substitution           |
| `sed 's/old/new/gi'`         | `atp transform -e 's/old/new/gi'`                            | Case-insensitive              |
| `sed '/pat/d'`               | `atp transform --delete 'pat'`                               | Delete matching lines         |
| `sed -i`                     | `atp transform --in-place`                                   | In-place editing              |
| `sed -i.bak`                 | `atp transform --in-place --backup bak`                      | In-place with backup          |
| `sed -e cmd1 -e cmd2`        | `atp transform -e cmd1 -e cmd2`                              | Multiple commands             |
| `sed '10,20s/a/b/g'`         | `atp transform -e 's/a/b/g' --line-range 10,20`              | Line-range addressing         |
| `sed '/start/,/end/s/a/b/g'` | `atp transform -e 's/a/b/g' --address-range '/start/,/end/'` | Pattern-range addressing      |
| `sed 'y/abc/xyz/'`           | `atp transform` (transliterate)                              | Character transliteration     |
| *(preview only in head)*     | `atp transform` *(dry-run by default)*                       | **Safe by default**           |

### awk → `atp analyze`

| awk feature                       | ATP equivalent                     | Description                  |
| --------------------------------- | ---------------------------------- | ---------------------------- |
| `awk -F','`                       | `atp analyze -F ','`               | Field separator              |
| `awk '{print $1, $3}'`            | `atp analyze -f 1,3`               | Select fields                |
| `awk '/pat/{print}'`              | `atp analyze -p 'pat'`             | Pattern filtering            |
| `awk 'NR'`                        | NR in JSON output                  | Record number (automatic)    |
| `awk 'NF'`                        | NF in JSON output                  | Number of fields (automatic) |
| `awk '{sum+=$3}END{print sum}'`   | `atp analyze --aggregate 'sum:3'`  | Aggregation                  |
| `awk '{count++}END{print count}'` | `atp analyze --aggregate 'count'`  | Count                        |
| `awk 'sub(pat,repl,$1)'`          | Computed field: `Sub(1,pat,repl)`  | First substitution           |
| `awk 'gsub(pat,repl,$1)'`         | Computed field: `Gsub(1,pat,repl)` | Global substitution          |
| `awk 'match($1,pat)'`             | Computed field: `Match(1,pat)`     | Pattern matching             |
| `awk 'split($1,a,sep)'`           | Computed field: `Split(1,sep,n)`   | Field splitting              |

---

## Examples for Agent Workflows

### Example 1: Find and Rename a Symbol

```bash
# 1. Verify the pattern finds the right matches
atp search 'OldTypeName' src/ --format json | jq '.data.total_matches'

# 2. Preview the rename
atp transform -p 'OldTypeName' -r 'NewTypeName' -g src/ --format json

# 3. Apply with backup
atp transform -p 'OldTypeName' -r 'NewTypeName' -g --in-place --backup bak src/
```

### Example 2: Analyze Code Structure

```bash
# Find all function definitions with context
atp search 'fn\s+\w+' src/ --include '*.rs' --context 2 --format json

# Extract function-level context around a specific line
atp context src/lib.rs -L 42 -m function --format json

# Count lines per file
atp pipeline -e 'search:. | count' src/ --include '*.rs' --format json
```

### Example 3: Process Log Files

```bash
# Extract error lines and get frequency of error types
atp analyze -F '\|' -p 'ERROR' --aggregate 'freq:3' app.log --format json

# Find all unique IP addresses in access logs
atp pipeline -e 'search:\d+\.\d+\.\d+\.\d+ | unique' access.log
```

### Example 4: AQL Queries (Recommended)

```bash
# Find and count TODO comments across a project
atp query 'find "TODO" ignore_case | count' src/

# Replace a symbol name with validation
atp query --explain 'replace "oldFunc" with "newFunc" all ignore_case'
atp query 'replace "oldFunc" with "newFunc" all' src/ --include '*.rs'

# Analyze CSV data: filter rows, sort, take top results
atp query 'set separator "," | filter field 3 > 100 | sort by field 3 desc numeric | take 10' data.csv

# Multi-stage log analysis in a single query
atp query 'find "ERROR" ignore_case | set separator "|" | select fields 1, 3 | sort | unique' app.log
```

---

## WebAssembly (WASM)

Use ATP directly in the browser or Node.js via the `atp-wasm` package:

```javascript
import init, { search, transform, analyze, pipeline, query } from 'atp-wasm';

await init();

// Search
const results = search('TODO', 'file contents here', '{"case_insensitive": true}');
console.log(JSON.parse(results));

// Transform
const transformed = transform('s/old/new/g', 'old text here', '{}');

// Run AQL query
const output = query('find "TODO" ignore_case | count', 'input text', '{}');
```

Build with: `wasm-pack build atp-wasm --target web`

---

## Language Server Protocol (LSP)

The `atp-lsp` crate provides a Language Server for AQL (v2):

- **Diagnostics** — real-time syntax error reporting as you type AQL
- **Notebook diagnostics** — validates AQL blocks inside `.atp.md` Markdown notebooks
- **Hover** — documentation for AQL keywords and stages
- **Completion v2** — context-aware auto-complete (after pipe, after stage keyword, after aggregate)
- **Workspace symbols** — search AQL keywords across the workspace

Start the server:

```bash
atp-lsp --stdio
```

Configure your editor to use `atp-lsp` as the language server for `.aql` and `.atp.md` files.

---

## Telemetry & Tracing

ATP includes opt-in usage telemetry and structured tracing:

```rust
use atp_core::telemetry;

// Usage telemetry (disabled by default)
telemetry::enable_usage_telemetry();
telemetry::record_usage("search", &["--format", "json"]);
let events = telemetry::export_usage_events();

// OpenTelemetry-style tracing (v1.4+)
use atp_core::{init_tracing, TracingConfig, TracingFormat, trace_pipeline_stage, TracedOperation};

let config = TracingConfig { enabled: true, format: TracingFormat::Json, ..Default::default() };
init_tracing(&config).unwrap();

// Trace pipeline stages
trace_pipeline_stage("search", 42, std::time::Duration::from_millis(15));

// Span-based tracing with parent context
let op = TracedOperation::begin("my_search", Some("parent_span_id"));
op.event("found 10 matches");
op.complete();
```

Telemetry is **disabled by default** and collects no data unless explicitly enabled. No data is transmitted externally.

---

## Semantic Search

Beyond regex, ATP supports TF-IDF-based semantic search for natural language queries:

```rust
use atp_core::semantic;

let index = semantic::build_index(&documents);
let results = semantic::search(&index, "error handling patterns", 10);
```

Semantic search complements pattern-based search when the exact syntax is unknown.

---

## Testing

ATP has comprehensive test coverage across all crates:

| Crate                    | Tests     | Description                                                                                                                                                                                                                                          |
| ------------------------ | --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `atp-core` (unit)        | 590       | Engines, AQL, output, pipeline, streaming, tracing, config, DAP, WASM, cache, diff, profile, schema, optimizer, rate-limit, git-search, snapshot, task-queue, patterns, checkpoint, log-sink, redact, dep-graph, template, workspace, hooks, metrics |
| `atp-core` (integration) | 30        | End-to-end workflow tests (grep, AQL, streaming, tracing, WASM, DAP, code intel)                                                                                                                                                                     |
| `atp-core` (property)    | 11        | Property-based tests (proptest) for grep, sed, AQL                                                                                                                                                                                                   |
| `atp-lsp`                | 25        | LSP protocol, diagnostics, hover, completion v2, workspace symbols, notebooks                                                                                                                                                                        |
| `atp-wasm`               | 15        | WASM bindings: search, transform, analyze, pipeline, AQL                                                                                                                                                                                             |
| `atp-core` (doc)         | 1         | Documentation examples                                                                                                                                                                                                                               |
| `atp-core` (fuzz)        | 7 targets | AQL, grep, sed, pipeline, config, diff, optimizer                                                                                                                                                                                                    |
| **Total**                | **~590**  | **All passing**                                                                                                                                                                                                                                      |

```bash
# Run all tests
cargo test --workspace

# Run benchmarks
cargo bench --bench atp_benchmarks

# Lint
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --check
```

---

## CI/CD

ATP ships with three GitHub Actions workflows:

- **`ci.yml`** — 8-job matrix: test, clippy, fmt, doc, MSRV, WASM, benchmarks, integration
- **`release.yml`** — Automated builds for 7 targets (Linux, macOS, Windows + ARM variants)
- **`cross.yml`** — Cross-compilation for 4 additional targets (musl, ARM, MIPS, RISC-V)

---

## References

- [grep, sed, awk — The Complete Guide](https://grep-sed-awk.com/) — history, examples, cheat sheets, and interactive playground
- [grep/awk/sed Quick Reference (York)](https://www-users.york.ac.uk/~mijp1/teaching/2nd_year_Comp_Lab/guides/grep_awk_sed.pdf) — concise academic guide
- *The AWK Programming Language*, 2nd Edition — Aho, Kernighan, Weinberger (2024)
- [GNU grep Manual](https://www.gnu.org/software/grep/manual/) — POSIX and GNU extensions
- [GNU sed Manual](https://www.gnu.org/software/sed/manual/) — stream editor reference
- [GNU awk (gawk) Manual](https://www.gnu.org/software/gawk/manual/) — the definitive awk reference

---

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only).

**Commercial licensing** is available for organizations that cannot comply with AGPL terms. Contact [opensource@nervosys.ai](mailto:opensource@nervosys.ai) for details.

## Contributing

Contributions welcome. Please open an issue or pull request on GitHub.

See [ROADMAP.md](ROADMAP.md) for planned features and [CHANGELOG.md](CHANGELOG.md) for release history.

---

*Built by [Nervosys](https://github.com/nervosys) — making tools for the agentic era.*
