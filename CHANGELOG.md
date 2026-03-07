# Changelog

All notable changes to the Agentic Text Processor (ATP) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] — 2026-09-XX

### Added

- **Fuzzy**: Fuzzy string matching with Jaro, Jaro-Winkler, trigram similarity, bounded Levenshtein, hybrid scoring, configurable fuzzy search and best-match APIs. 19 tests.
- **Summarizer**: Text summarization with TF-IDF, TextRank, and Lead strategies, sentence scoring, configurable summary length, keyword extraction. 16 tests.
- **Casing**: Case conversion and detection for 10 styles (camelCase, PascalCase, snake_case, SCREAMING_SNAKE, kebab-case, SCREAMING-KEBAB, Train-Case, flatcase, Title Case, dot.case). 23 tests.
- **Text Wrap**: Text wrapping with word-wrap and hard-wrap modes, configurable indent, hanging indent, alignment, dedent, column layout. 16 tests.
- **MIME**: MIME type detection from magic bytes (16+ signatures), file extension mapping (~102 extensions), charset detection. 17 tests.
- **Graph**: Text graph analytics with co-occurrence and sentence similarity graphs, PageRank, TextRank keyword extraction, centrality, DOT export. 15 tests.
- **Shell**: Shell command analysis with shebang parsing, variable/command extraction, quoting/escaping, argument splitting, portability checking. 17 tests.
- **URL**: URL parsing and manipulation with normalization, relative URL resolution, percent encoding/decoding, domain/TLD extraction. 18 tests.
- **Table Extract**: Table detection and extraction from Markdown, ASCII box-drawing, and fixed-width text with auto-format detection. 17 tests.
- **Emoji**: Emoji detection and manipulation with 50-entry database, shortcode conversion, skin tone stripping, sentiment analysis, ZWJ-aware parsing. 17 tests.
- 10 new core modules: `fuzzy`, `summarizer`, `casing`, `text_wrap`, `mime`, `graph`, `shell`, `url`, `table_extract`, `emoji`.
- ~175 new unit tests across all new modules (total: 1235 tests).

### Changed

- `atp-core/src/lib.rs` now exports 79 modules (was 69) with full re-exports.
- Version bumped to 2.0.0 (major version bump for 10 new modules milestone).
## [1.9.0] — 2026-09-XX

### Added

- **Codec**: Multi-format encoder/decoder with 7 codecs (Base64, Hex, URL-encode, HTML entities, ROT13, Quoted-Printable, Percent-encode), auto-detection, unified encode/decode API. `CodecKind`, `encode`, `decode`, `detect_codec`. 17 tests.
- **Fingerprint**: Document fingerprinting with SimHash (64-bit LSH), MinHash (Jaccard estimation), shingle/shingle-set generation, rolling hashes, content-defined chunking, duplicate detection, term frequency analysis. `Fingerprint`, `FingerprintOptions`, `simhash`, `minhash`, `find_duplicates`. 17 tests.
- **Markdown**: Markdown parser and renderer with block-level AST (headings, paragraphs, code blocks, blockquotes, lists, horizontal rules), inline rendering (bold, italic, code, links), HTML/plain-text output, TOC generation, link extraction, heading stats, word count, slugify. `MdDoc`, `MdNode`, `parse`, `render_html`, `toc`. 17 tests.
- **Sampler**: Text sampling with 5 methods (Random, Reservoir, Systematic, Stratified, Weighted), deterministic xorshift64 PRNG, proportional stratification, weighted selection. `SampleMethod`, `Sample`, `reservoir`, `systematic`, `stratified`, `random_sample`, `weighted_sample`. 15 tests.
- **Tokenizer**: Multi-granularity tokenizer (Word, Sentence, Paragraph, FixedLen, Regex), BPE (Byte Pair Encoding) with vocabulary learning and sub-word tokenization, token statistics. `Granularity`, `Token`, `tokenize`, `learn_bpe`, `bpe_tokenize`, `token_stats`. 16 tests.
- **Spellcheck**: Spell checker with Levenshtein and Damerau-Levenshtein edit distance, Soundex phonetic encoding, ~250-word built-in dictionary, suggestion ranking, auto-correction, batch spell-check reports. `levenshtein`, `damerau_levenshtein`, `soundex`, `suggest`, `check`, `auto_correct`, `report`. 16 tests.
- **Calendar**: Date/time utilities with multi-format parsing (ISO-8601, US, EU, short), epoch conversion, date arithmetic, relative dates (today, yesterday, tomorrow, N days/weeks/months/years ago/from-now), ISO week calculation. `DateTime`, `Duration`, `DateFormat`, `RelativeDate`, `parse`, `format`. 18 tests.
- **Compress**: Text compression with 3 algorithms (RLE with escape-byte encoding, LZ77 with sliding window, Huffman coding with binary heap and bit packing), unified compress/decompress API, compression analysis with ratio/savings metrics. `Algorithm`, `compress`, `decompress`, `analyze`. 15 tests.
- **Color**: Color manipulation with hex/RGB/HSL parsing, RGB↔HSL conversion, WCAG 2.1 accessibility analysis (contrast ratio, normal/large text compliance), palette generation (complementary, analogous, triadic, n-spaced), lighten/darken/mix, ANSI-256 terminal output. `Rgb`, `Hsl`, `WcagLevel`, `AccessibilityReport`, `parse_color`, `accessibility`, `palette`. 18 tests.
- **Macro engine**: Text macro system with 11 step types (Replace, Prepend, Append, Upper, Lower, Trim, Delete, InsertAt, WrapLines, Call, If), variable substitution (`{{var}}` and `$1` positional), macro recorder, 3 built-in macros (quote, slugify, comment), recursion guard at depth 64. `MacroStep`, `Macro`, `MacroContext`, `MacroRecorder`, `execute`, `run_macro`. 16 tests.
- 10 new core modules: `codec`, `fingerprint`, `markdown`, `sampler`, `tokenizer`, `spellcheck`, `calendar`, `compress`, `color`, `macro_engine`.
- ~165 new unit tests across all new modules (total: 1062 tests).

### Changed

- `atp-core/src/lib.rs` now exports 69 modules (was 59) with full re-exports.
- Version bumped to 1.9.0.

## [1.8.0] — 2026-09-XX

### Added

- **Converter**: Multi-format document converter with 6 formats (JSON, YAML, TOML, CSV, XML, INI), intermediate `DocValue` representation, bidirectional transforms, auto-detection, pretty-print options. `Converter`, `ConvertFormat`, `DocValue`, `ConvertOptions`. 17 tests.
- **Statistics**: Text statistics engine with character/word/sentence/paragraph counts, Shannon entropy, Flesch/Kincaid readability, word frequency, n-grams, Zipf analysis with R² fit, character distribution. `TextStats`, `Readability`, `WordFreq`, `Ngram`, `ZipfAnalysis`. 15 tests.
- **State machine**: Programmable state machine with 7 guard types (Always, Contains, Regex, StartsWith, EndsWith, All, Any, Not), 6 action types (Emit, SetVar, Increment, Log, Collect, Noop), builder pattern, reachability analysis, dead-end/cycle detection, Graphviz DOT export. `MachineSpec`, `MachineInstance`, `Guard`, `MachineAction`, `RunResult`. 16 tests.
- **I18n / Unicode**: Unicode normalisation (NFC/NFD/NFKC/NFKD) with decomposition tables, transliteration (~70 mappings), case folding with special ß/İ handling, grapheme cluster segmentation, script detection (13 scripts), locale-aware collation/sorting (Root/De/Sv/Tr). `NormForm`, `Script`, `Locale`, `Grapheme`. 16 tests.
- **Batch**: Transactional batch operations with 6 op types (Copy, Move, Delete, Create, Transform, Append), in-memory filesystem sandbox (`MemFs`), rollback support, dry-run mode, validation, error-on-stop policy. `Batch`, `BatchOp`, `BatchOptions`, `MemFs`, `BatchResult`. 15 tests.
- **Annotation**: Text annotation engine with span-based annotations, category taxonomy, overlap resolution (4 strategies: Allow, KeepFirst, KeepLongest, Merge), standoff/inline/JSON export, Cohen's kappa inter-annotator agreement, annotation statistics. `AnnotatedDoc`, `Annotation`, `Category`, `OverlapStrategy`. 16 tests.
- **Highlight**: Syntax highlighting with token-based tokenizer for 5+ languages (Rust, Python, JavaScript, C, Go), 4 output renderers (ANSI, HTML, SVG, Plain), dark/light themes, line-number gutters, keyword/type/function/macro detection. `Token`, `TokenKind`, `HighlightFormat`, `LangDef`, `Theme`. 16 tests.
- **Timeline**: Temporal event streams with timestamp parsing (Unix epoch + ISO-8601), interval bucketing (Second/Minute/Hour/Day), z-score anomaly detection (spike/drop/gap), sparkline rendering, duration histograms. `Event`, `Bucket`, `Anomaly`, `Granularity`, `HistBin`. 15 tests.
- **Tree**: Generic tree structure with typed payloads, DFS/BFS iterators, path queries, node add/remove, subtree map/filter/prune, tree diff (Added/Removed/Changed), merge, JSON serialisation, indented text rendering. `TreeNode`, `TreeDiff`, `DfsIter`, `BfsIter`, `IdGen`. 16 tests.
- **Validator**: Multi-format validation for email, URL, semver, UUID, IPv4, IPv6, JSON Schema subset (type/required/properties/min-max/pattern/enum), custom rule DSL (Matches, LengthBetween, OneOf, NotBlank, StartsWith, EndsWith, All, Any), batch validation with diagnostic reports. `ValidationResult`, `SchemaNode`, `ValidatorKind`, `Rule`. 16 tests.
- 10 new core modules: `converter`, `statistics`, `state_machine`, `i18n`, `batch`, `annotation`, `highlight`, `timeline`, `tree`, `validator`.
- ~158 new unit tests across all new modules (total: 899 tests).

### Changed

- `atp-core/src/lib.rs` now exports 59 modules (was 49) with full re-exports.
- Version bumped to 1.8.0.

## [1.7.0] — 2026-09-XX

### Added

- **Rule engine**: Declarative if-then rule engine with 11 condition types (Regex, Contains, StartsWith, EndsWith, FieldEquals, FieldMatches, LengthExceeds, LengthBelow, All, Any, Not), 9 action types (Tag, ReplaceFirst, ReplaceAll, Prepend, Append, Drop, Route, Alert, Chain), priority levels, conflict strategies, and statistics tracking. `RuleEngine`, `Rule`, `Condition`, `Action`, `ConflictStrategy`. 18 tests.
- **Formatter**: Table formatting with 4 border styles (None, Ascii, Unicode box-drawing, Markdown), column definitions with width/alignment/overflow control, word wrapping, text truncation, padding, indent normalization, and horizontal rules. `Table`, `ColumnDef`, `BorderStyle`, `Alignment`, `Overflow`. 16 tests.
- **Scheduler**: Cron-like scheduler with 5-field cron expressions (minute, hour, day-of-month, month, day-of-week), one-shot and repeating schedules, missed-run detection, execution history tracking. `Scheduler`, `CronExpr`, `ScheduleDef`, `TimePoint`. 16 tests.
- **Encryption**: Data-at-rest encryption with pure-Rust PBKDF2-HMAC-SHA256 key derivation, XOR stream cipher with SHA-256 keystream, HMAC-SHA256 authentication tags, JSON-serializable encrypted payloads. `EncryptionConfig`, `EncryptedPayload`, `Algorithm`, `Kdf`. 14 tests.
- **Changelog generator**: Conventional Commits parser with 12 commit types, scope extraction, breaking-change detection (! suffix and BREAKING CHANGE footer), semantic version bump suggestions (major/minor/patch), Markdown and JSON output rendering. `ConventionalCommit`, `SemVer`, `ChangelogEntry`. 16 tests.
- **Linter**: Configurable text/code linter with 4 severity levels (Error, Warning, Info, Hint), 7 built-in rule types (trailing whitespace, long lines, inconsistent EOL, TODO markers, tab indentation, consecutive blanks, regex match), file-type filtering, autofix support. `Linter`, `LintRule`, `Diagnostic`, `Fix`. 14 tests.
- **Data table**: In-memory columnar data table with typed columns (String, Int, Float, Bool), filter, sort, group-by aggregation (Sum, Avg, Min, Max, Count), inner join, pivot, column selection, CSV import/export. `DataTable`, `Column`, `Value`, `Aggregation`. 15 tests.
- **Rewrite engine**: Multi-pass rewrite rules with named rule sets, regex pattern + replacement template, scope constraints (All, LineRange, MatchingLines, ExcludeLines), max applications limit, dry-run preview, full rewrite history tracking. `RewriteEngine`, `RuleSet`, `RewriteRule`, `Scope`. 14 tests.
- **Archive processor**: Archive-aware text processing with tar creation/parsing, gzip wrapping with CRC32, format auto-detection via magic bytes, text search inside archives, regex search, glob-based entry extraction, extension grouping. `ArchiveEntry`, `ArchiveFormat`. 13 tests.
- **Report composer**: Multi-section report composition with 6 content block types (Text, List, Metrics, Table, Code, Divider), 4 output formats (Markdown, HTML, JSON, PlainText), template rendering with `{{var}}` placeholders, metric builders. `Report`, `Section`, `ReportMeta`, `OutputFormat`. 12 tests.
- 10 new core modules: `rule_engine`, `formatter`, `scheduler`, `encryption`, `changelog`, `linter`, `data_table`, `rewrite`, `archive`, `report`.
- ~148 new unit tests across all new modules (total: 741 tests).

### Changed

- `atp-core/src/lib.rs` now exports 49 modules (was 39) with full re-exports.
- Version bumped to 1.7.0.

## [1.6.0] — 2026-09-XX

### Added

- **Task queue with dependency DAG**: `TaskQueue` with priority scheduling (Critical→Background), topological sort via Kahn's algorithm, retry policies with exponential backoff, cancellation handles, and concurrent batch execution. `TaskDef`, `Priority`, `TaskState`, `RetryPolicy`, `QueueConfig`. 13 new tests.
- **Pattern registry**: `PatternRegistry` with 4 built-in pattern sets (log_parsing, security, pii, code_smells — 20 patterns total). Named-capture scanning, severity levels, tag-based filtering. `Pattern`, `PatternMatch`, `PatternSet`, `PatternSeverity`. 14 new tests.
- **Checkpoint / resume manager**: `CheckpointManager` with SHA-256 content-addressed checkpoints, name-indexed pipeline tracking, disk persistence (`~/.atp/checkpoints/`), automatic staleness detection (InputChanged, PipelineChanged), stage-level resume. `Checkpoint`, `CheckpointConfig`, `ResumeStatus`. 13 new tests.
- **Structured log sink**: `LogSink` with 5 format parsers (JSON, Syslog, CLF, Key-Value, Auto-detect). Severity-based filtering, time-range windowing, regex text search, alert rules with threshold triggers. `LogRecord`, `LogFilter`, `LogFormat`, `AlertRule`, `LogStats`. 14 new tests.
- **Sensitive data redactor**: `Redactor` with 8 built-in detection rules (email, SSN, credit card, phone, AWS key, API key, JWT, private key). 4 masking strategies (FullMask, PartialMask, Hash, Remove). Non-overlapping span merging. `RedactConfig`, `RedactResult`, `SensitiveKind`. 14 new tests.
- **Dependency graph analyzer**: `DepGraph` with adjacency/reverse-adjacency lists, cycle detection (DFS), topological sort, transitive closure (BFS), depth computation, Graphviz DOT export. Import extraction for Rust/Python/JS/C. `DepNode`, `GraphStats`, `ImportStatement`. 15 new tests.
- **Template engine**: Mustache-style template rendering with `{{var}}`, `{{{raw}}}`, `{{#if}}`, `{{#unless}}`, `{{#each}}`, `{{> partial}}` directives. Dotted path lookup, HTML escaping, partial registration. `TemplateEngine`, `Value`, `TemplateError`. 14 new tests.
- **Workspace aggregator**: `Workspace` with multi-project discovery (Cargo.toml, package.json, pyproject.toml, go.mod, pom.xml, etc.), 8 project kinds, recursive source counting, regex cross-project search. `WorkspaceConfig`, `Project`, `ProjectKind`, `WorkspaceStats`. 12 new tests.
- **Event bus / hook system**: `EventBus` with typed event dispatch, priority-ordered hook execution (First→Last), skip/abort control flow, event logging, hook statistics. 8 event kinds (PipelineStart/Complete/Error, StageStart/Complete/Error, MatchFound, TransformApplied, Custom). `HookDef`, `HookPriority`, `HookOutcome`, `Event`. 13 new tests.
- **Metrics registry**: Thread-safe `MetricsRegistry` with Counter, Gauge, and Histogram (configurable buckets) metric types. Label-keyed metrics, Prometheus text format export with HELP/TYPE headers. Pipeline instrumentation helpers. `MetricKey`, `MetricValue`. 14 new tests.
- 10 new core modules: `task_queue`, `patterns`, `checkpoint`, `log_sink`, `redact`, `dep_graph`, `template`, `workspace`, `hooks`, `metrics`.
- ~136 new unit tests across all new modules (total: ~590 tests + 1 doc-test).

### Changed

- `atp-core/src/lib.rs` now exports 39 modules (was 29) with full re-exports.
- Architecture diagram updated with new module layer.
- Version bumped to 1.6.0.

---

## [1.5.0] — 2026-08-XX

### Added

- **Incremental compilation cache**: `PipelineCache` with SHA-256 content-addressed keys, LRU eviction, TTL expiry, JSON-based persistence to disk. `CacheConfig`, `CacheEntry`, `CacheStats`. 10 new tests.
- **Structured diff engine**: `diff_json()` for recursive JSON comparison, `diff_text()` with LCS-based text diff, `format_unified()` for unified diff output, `apply_patch()` for RFC 6902-style patch application. `DiffOp`, `DiffOpKind`, `DiffResult`, `UnifiedLine` types. 14 new tests.
- **Profile-guided optimization hints**: `analyze_profile()` with 4 rules — filter-before-sort reorder, dead-stage detection, expensive identity detection, late selective filter. `StageProfile`, `PipelineProfile`, `OptimizationSuggestion`. 9 new tests.
- **Schema registry & versioned output**: `SchemaRegistry` with 10 built-in JSON Schema (draft 2020-12) definitions for all output types (`SearchResults`, `TransformResults`, `AnalysisResults`, `PipelineResults`, `ExplainResult`, `ComplianceReport`, `AtpEnvelope`, `DiffResult`, `PipelineProfile`, `CacheStats`). `SchemaEntry` with `since_version`. 8 new tests.
- **Query plan optimizer**: Cost-based AQL optimization pass — predicate pushdown, dead-stage elimination, duplicate elimination. `QueryPlan`, `StageDesc`, `StageKind`, `OptimizationRule`. `format_plan()` for explain-style output. 10 new tests.
- **Rate-limited pipelines**: `TokenBucket` rate limiter, `CircuitBreaker` with closed/open/half-open states, `RateLimitedExecutor` with cancellation support. `RateLimitConfig`, `RateLimitStats`, `RateLimitError`. 14 new tests.
- **Git-aware search**: `changed_files()`, `changed_line_ranges()`, `blame_file()`, `git_search()` using git CLI. `GitSearchConfig` (base_ref, changed_only, include_blame), `ChangedFile`, `ChangeStatus`, `LineRange`, `BlameInfo`. `is_line_changed()` and `filter_to_changed()` utilities. 13 new tests.
- **Snapshot testing harness**: `SnapshotHarness` with create/verify/update workflow, output normalization (UUIDs, timestamps, durations), SHA-256 content hashing. `SnapshotConfig`, `Snapshot`, `SnapshotMetadata`, `SnapshotVerdict`. 14 new tests.
- **Comprehensive fuzzing round**: 4 new `cargo-fuzz` targets — `fuzz_pipeline` (YAML/DSL parser), `fuzz_config` (TOML project config), `fuzz_diff` (JSON diff + text diff + patch), `fuzz_optimizer` (query plan optimizer). Total: 7 fuzz targets.
- 8 new core modules: `cache`, `diff`, `profile`, `schema`, `optimizer`, `rate_limit`, `git_search`, `snapshot`.
- ~90 new unit tests across all new modules (total: ~535 tests + 1 doc-test).

### Changed

- `atp-core/src/lib.rs` now exports 29 modules (was 21) with full re-exports.
- Architecture diagram updated with new module layer.
- Version bumped to 1.5.0.

---

## [1.4.0] — 2026-07-XX

### Added

- **Aho-Corasick multi-pattern grep**: Hardware-accelerated multi-pattern matching via `aho-corasick` crate. `GrepEngine` builds an Aho-Corasick automaton when multiple patterns are provided, with fast-path integration in `search_file`, `search_reader`, and `search_file_mmap`. 4 new grep tests.
- **OpenTelemetry-style tracing**: `TracingConfig`, `TracingFormat` (Text/Json/Compact), `init_tracing()` subscriber setup, `TracedOperation` (begin/complete/event with parent span IDs), `trace_pipeline_stage()`, `trace_search()`. 6 new tracing tests.
- **Streaming pipelines**: `StreamingConfig` (channel_capacity, batch_size), `StreamingPipeline` with async tokio mpsc channel-based execution for large-data workloads. 6 new streaming tests.
- **LSP v2**: Context-aware completion (`CompletionContext`: Empty, AfterPipe, AfterStageKeyword, AfterAggregate, General), workspace symbol search, notebook diagnostics for `.atp.md` files with embedded AQL validation. 10 new LSP tests (25 total).
- **TUI v2**: 4 new tabs (Index, Debug, Notebook, Distributed) for a total of 10 tabs with dedicated execute methods for each.
- **Real WASM engine**: `WasmEngine` trait (`validate`/`execute`), `BuiltinWasmEngine` with WASM binary format parsing (magic bytes, version, LEB128 section traversal, export section discovery), `build_test_wasm_module()` helper, `encode_leb128()`/`decode_leb128()` utilities. 7 new tests.
- **Project `.atp.toml`**: `PipelineDefaults` (channel_capacity, batch_size, saved pipelines), `ScopeDefaults` (roots, extensions, follow_symlinks), `find_project_config()` directory walk-up discovery, integrated into `AtpConfig::load()`. 5 new config tests.
- **DAP wire protocol**: `DapMessage` enum (Request/Response/Event) with Content-Length framed JSON encoding/decoding. `DapRequest::new()`, `DapResponse::success()`/`error()`, `DapEvent::stopped()`/`output()`/`terminated()` convenience constructors. 8 new wire protocol tests.
- **End-to-end integration tests**: 10 new tests covering streaming pipelines, Aho-Corasick matching, tracing initialization, WASM engine roundtrip, project config merging, DAP wire roundtrip, AQL pipeline explain, code intel symbols, and debugger sessions. Integration total: 30 tests.
- Workspace dependencies: `aho-corasick = "1"`, `tracing = "0.1"`, `tracing-subscriber = "0.3"`.

### Changed

- TUI now has 10 tabs (was 6): added Index, Debug, Notebook, Distributed.
- LSP completion uses context-aware v2 handler.
- Architecture diagram updated with streaming pipelines, tracing, config modules.
- README.md fully refreshed: badges, crate table, commands reference (28 commands), testing section (~445 tests).
- Version bumped to 1.4.0.

---

## [1.3.0] — 2026-06-XX

### Added

- **Tree-sitter code intelligence**: `IntelBackend` enum (Regex/TreeSitter), `SymbolExtractor` trait, `ScopeNode` with `scope_at_line()`/`scope_path_at_line()`/`flatten()`, `build_scope_tree()`, `CallEdge`/`CallGraph` with `callees_of()`/`callers_of()`/`roots()`/`leaves()`, `build_call_graph()`.
- **Incremental file index**: `SearchIndex` with trigram-accelerated full-text search, SHA-256 content hashing, incremental `update_file()`/`remove_file()`, regex search, glob file matching, JSON persistence (`save()`/`load()`). `FileEntry`, `IndexHit`, `IndexStats` types. CLI `index` command (alias `idx`) with `build`, `search`, `status`, `update`, `files` subactions.
- **DAP integration (AQL debugger)**: `AqlDebugger` with step-through pipeline debugging, `Breakpoint` (stage/conditional), `DebugSession`, `StageSnapshot` with I/O sampling, `WatchExpression`, breakpoint management, session summaries. CLI `debug` command (alias `dbg`) with `step`, `run`, `info` modes.
- **WASM plugin runtime**: `WasmRuntime` with capability-based security model (`WasmCapability`: FileRead, FileWrite, Network, Environment, Process, Stdio), `WasmPluginManifest` (TOML), `WasmModule` loading/validation, sandboxed `execute()`, security auditing. `WasmLimits` (memory/time/output bounds).
- **Notebook / literate mode**: `Notebook` parser for Markdown with embedded AQL `\`\`\`aql` code blocks, cell metadata attributes, `execute_cell()`/`execute_all()`, `render()` with output insertion, `save()`/`load()`. `CellKind` (Markdown/Aql/Output), `CellOutput`, `NotebookRunSummary`. CLI `notebook` command (alias `nb`/`literate`) with `run`, `render`, `info` actions.
- **Distributed pipelines**: `ScatterGatherPlan` with 5 partition strategies (ByFile, ByLineRange, BySample, Broadcast, ByFileType) and 7 merge strategies (Concat, ConcatSort, ConcatDedup, Sum, Average, FirstNonEmpty, MergeFrequency). `DistributedPipeline` executor, `NodeAssignment`, `NodeResult`, `DistributedResult`. CLI `distributed` command (alias `dist`/`scatter`).
- **Flaky telemetry test fix**: Serialized all global-state telemetry tests with `USAGE_TEST_MUTEX` to eliminate race conditions.
- 5 new core modules: `index` (~420 LOC), `dap` (~380 LOC), `wasm_runtime` (~380 LOC), `notebook` (~430 LOC), `distributed` (~400 LOC).
- 4 new CLI commands: `index`, `debug`, `notebook`, `distributed`.
- 67 new unit tests across all new modules (total: 390 tests).

### Changed

- CLI now has 28 subcommands (was 24).
- `code_intel` module enhanced with scope trees, call graphs, and `SymbolExtractor` trait.
- Version bumped to 1.3.0.

---

## [1.2.0] — 2026-05-XX

### Added

- **Async I/O & streaming**: Tokio-based async file processing, URL data sources (`http://`, `https://`), channel-based line streaming, concurrent multi-source grep (`AsyncGrep`), async AQL pipeline execution (`AsyncPipeline`).
- **AI/LLM integration**: Natural language to AQL translation, AQL query explanation, query suggestion, and semantic search via Ollama or OpenAI backends. CLI `ai` command (alias `llm`) with `--nl`, `--explain`, `--suggest`, `--semantic` flags.
- **Code intelligence**: Regex-based symbol extraction for 14 languages (Rust, Python, JS/TS, Go, Java, C/C++, Ruby, Shell, etc.). Symbol kinds: function, method, class, struct, enum, trait, module, constant, variable, import, type. Structural queries with name pattern, kind, language, visibility, doc-comment filters. CLI `symbols` command (alias `sym`/`code`).
- **Remote execution**: SSH-based distributed ATP command execution on remote hosts. Sequential and parallel (rayon) multi-host execution. Host parsing (`user@host:port`), host files, identity file support. CLI `remote` command (alias `ssh`).
- **TUI overhaul**: File browser sidebar (Ctrl+B toggle), AQL tab with live validation and history (Up/Down), Symbols tab with structural query input (`fn:pattern`, `struct:*`), focus cycling (Ctrl+F) between input/results/file browser, 6 tabs total.
- **Plugin SDK & marketplace**: Plugin scaffolding (`atp plugin init`), manifest validation (`atp plugin check`), local install/uninstall, plugin info display. `PluginSdk` with `scaffold_stage()`, `scaffold_format()`, `validate_manifest()`, `package_info()`. `RegistryEntry` type for future marketplace integration. CLI `plugin` command (alias `plug`) with 6 subcommands.
- **Crates.io publication prep**: Version specifiers on all `atp-core` path dependencies, `cargo publish --dry-run` verified, proper workspace metadata inheritance.
- 4 new core modules: `async_io` (280 LOC), `ai` (300 LOC), `code_intel` (480 LOC), `remote` (290 LOC).
- 5 new CLI commands: `ai`, `remote`, `symbols`, `plugin` (with 6 subcommands).
- 34+ new unit tests across new modules.

### Changed

- CLI now has 24 subcommands (was 19).
- TUI now has 6 tabs (was 4): Search, Transform, Analyze, Pipeline, AQL, Symbols.
- TUI has split-pane layout with toggleable file browser.
- Version bumped to 1.2.0.
- Workspace dependencies: added `tokio 1`, `reqwest 0.12`, `futures 0.3`, `async-trait 0.1`.

---

## [1.1.0] — 2026-04-XX

### Added

- **AQL v2**: Variables (`let x = ...`), conditionals (`if ... then ... else ... end`), group-by with aggregations (`group by field N aggregate count, sum N, avg N, min N, max N`), user-defined functions (`def name ... end`, `call name`).
- **Configuration system**: Global `~/.atp/config.toml` and per-project `.atprc` with merge semantics. CLI `config` command (`--show`, `--init`, `--path`).
- **MCP server mode**: Model Context Protocol server over JSON-RPC 2.0 stdio with 4 tools (`search`, `transform`, `query`, `analyze`). CLI `mcp` command (alias `serve`).
- **Performance**: Parallel file search via `rayon` (`search_files_parallel`). Memory-mapped I/O via `memmap2` (`search_file_mmap`).
- **VS Code extension**: TextMate grammar for AQL syntax highlighting. LSP client connecting to `atp-lsp`. Extension at `editors/vscode/`.
- **Benchmark regression CI gate**: `benchmark-action/github-action-benchmark` with 120% alert threshold and fail-on-alert.
- **Property-based testing**: 11 proptest tests covering grep, sed, and AQL engines.
- **Fuzz harness**: 3 `cargo-fuzz` targets (`fuzz_aql`, `fuzz_grep`, `fuzz_sed`) with seed corpus.
- **Packaging**: Homebrew formula, Scoop manifest (Windows), Debian packaging (control/rules/changelog/copyright), RPM spec.
- 14 new AQL v2 unit tests, 7 config unit tests, 11 property tests.

### Changed

- CLI now has 19 subcommands (added `config`/`cfg` and `mcp`/`serve`).
- AQL engine now supports 20 stage types (was 15).
- Test count increased from 256 to 287 tests.
- ROADMAP.md updated with Milestone 10.

---

## [1.0.0] — 2026-03-XX

### Added

- **WASM crate** (`atp-wasm`): Browser-ready bindings via `wasm-bindgen` for grep, sed, awk, AQL, semantic search, and ontology.
- **LSP crate** (`atp-lsp`): Language Server Protocol server for AQL files with completion, hover, diagnostics, and formatting.
- **Telemetry system**: Remote telemetry with CloudWatch-style payloads, 10+ event types, session tracking, and opt-in privacy model (`ATP_TELEMETRY=1`).
- **Semantic search**: TF-IDF / BM25 document scoring via `PatternType::Semantic`, document ranking, and relevance thresholds.
- **CI/CD pipelines**: GitHub Actions for lint, test, MSRV, miri, coverage, release, and cross-compilation.
- **Criterion benchmarks**: Performance benchmarks for grep, sed, awk, AQL, and semantic engines.
- **Shell completions**: Generator for bash, zsh, fish, PowerShell, and elvish.
- **Man page generation**: `clap_mangen`-based man page output.
- **Integration test suite**: 21 cross-cutting integration tests.
- **Documentation site**: mdBook with 27 pages covering installation, usage, AQL, architecture, and API reference.
- **Cross-compilation**: `Cross.toml` for `aarch64-linux`, `armv7-linux`, `x86_64-musl`, `aarch64-macos`, and `x86_64-windows`.
- **crates.io metadata**: Full publication-ready metadata and AGPL-3.0-only license across all crates.

### Changed

- All clippy warnings resolved (0 warnings across 6 crates).
- Flaky telemetry test hardened against parallel test interference.
- ROADMAP.md updated to reflect all 9 milestones.
- Architecture diagram updated to include `atp-wasm`, `atp-lsp`, semantic, and telemetry modules.
- Test coverage table updated to 225 tests + 1 doc-test.

### Fixed

- `DataClassification` and `DisseminationControl` now use `#[derive(Default)]` instead of manual impls.
- Manual prefix stripping in compat.rs replaced with `strip_prefix()`.
- Redundant closures, unused imports, and collapsible-if warnings cleaned up across all crates.

---

## [0.7.0] — 2026-03-XX

### Added

- **Streaming execution**: `search_reader`, `transform_reader`, `process_reader` for large file processing without full memory load.
- **AQL REPL**: Interactive shell with 10 dot-commands (`.help`, `.history`, `.clear`, `.format`, etc.) and persistent history.
- **Watch mode**: Live file monitoring with `notify` crate, configurable debounce, and `--clear` flag.
- **GUI Apply button**: In-place file transformation with confirmation dialog and backup support.
- **Improved function detection**: Multi-language support (Rust, Python, Go, Java, C/C++, JavaScript/TypeScript, Ruby), decorators, and attributes.
- **Plugin system**: `AtpPlugin` trait, TOML manifest format, `PluginRegistry` with directory scanning.
- 8 plugin unit tests.

---

## [0.6.0] — 2026-03-XX

### Added

- **Backwards compatibility binaries**: `atp-grep`, `atp-sed`, `atp-awk` with POSIX flag parsing.
- POSIX-to-ATP translation layer (`compat.rs`) for grep, sed, and awk arguments.
- Auto-format detection: JSON when piped, human-readable when TTY.
- 22 compat unit tests.

---

## [0.5.0] — 2026-03-XX

### Added

- **Regulatory compliance module** (`compliance.rs`):
  - FIPS 180-4 SHA-256 hashing with NIST test vector verification.
  - Structured audit logging per NIST SP 800-53 (AU-3/AU-8).
  - CUI data classification markings per NIST SP 800-171.
  - CMMC 2.0 control mapping (12 controls across 7 domains).
  - Compliance report generation.
- `DataMarking` field on `AtpEnvelope` for output classification.
- Provenance input hashing (`with_input_hash`).
- CLI `compliance` command (report/controls/integrity sub-commands).
- COMPLIANCE.md documentation.
- 11 compliance unit tests.

---

## [0.4.0] — 2026-03-XX

### Added

- AQL validation in `validate` command.
- `query`, `validate`, `context` commands added to ontology.
- JSONL output format (true line-delimited).
- TUI: Transform, Analyze, and Pipeline tab execution.
- GUI: Analyze and Pipeline tabs.
- 10 awk unit tests, 9 pipeline unit tests, 6 traversal unit tests.

### Improved

- `explain` command now supports query/scope/validate/context.

---

## [0.3.0] — 2026-03-XX

### Added

- **AQL — ATP Query Language**: Keyword-based syntax with 15 stage types and 11 modifiers.
- Full tokenizer, recursive-descent parser, AST, and evaluator.
- CLI `query` command with `--explain` and `--validate` flags.
- Stdin piping and shell pipe chaining support.
- SYNTAX.md formal reference.
- 28 AQL unit tests + 1 doc-test.

---

## [0.2.0] — 2026-03-XX

### Added

- **CLI commands** (10): search, transform, analyze, pipeline, query, ontology, explain, scope, validate, context.
- Multiple aliases per command for discoverability.
- Global `--format` flag (json, json-pretty, jsonl, yaml, csv, human).

---

## [0.1.0] — 2026-03-XX

### Added

- **Core engines**: Grep (14 config options), Sed (5 command types), Awk (12 expression types, 9 conditions, 8 aggregations).
- **Pipeline engine**: 9 stage types, DSL parser, YAML input.
- **File traversal**: gitignore-aware, glob patterns, depth limits.
- **Context extraction**: Function, block, indent, line range, and full-file modes.
- **Output formatting**: JSON, JSON-pretty, YAML, CSV, human-readable with `AtpEnvelope` provenance.
- **Ontology system**: Self-describing capabilities, commands, types, and errors for AI agent discovery.
- **TUI**: Terminal interface with Search, Transform, Analyze, Pipeline tabs.
- **GUI**: Native desktop application with egui.
- 6 grep + 4 sed + 14 context unit tests.
