//! # ATP Core Library
//!
//! Core library for the Agentic Text Processor (ATP).
//! Provides strongly typed, deterministic text processing engines
//! with full ontology support for AI agent discoverability.

pub mod ai;
pub mod annotation;
pub mod archive;
pub mod async_io;
pub mod batch;
pub mod cache;
pub mod calendar;
pub mod casing;
pub mod changelog;
pub mod checkpoint;
pub mod code_intel;
pub mod codec;
pub mod color;
pub mod compat;
pub mod compliance;
pub mod compress;
pub mod config;
pub mod context;
pub mod converter;
pub mod dap;
pub mod data_table;
pub mod dep_graph;
pub mod diff;
pub mod distributed;
pub mod emoji;
pub mod encryption;
pub mod engine;
pub mod fingerprint;
pub mod formatter;
pub mod fuzzy;
pub mod git_search;
pub mod graph;
pub mod highlight;
pub mod hooks;
pub mod i18n;
pub mod index;
pub mod linter;
pub mod log_sink;
pub mod macro_engine;
pub mod markdown;
pub mod metrics;
pub mod mime;
pub mod notebook;
pub mod ontology;
pub mod optimizer;
pub mod output;
pub mod patterns;
pub mod plugin;
pub mod profile;
pub mod rate_limit;
pub mod redact;
pub mod remote;
pub mod report;
pub mod rewrite;
pub mod rule_engine;
pub mod sampler;
pub mod scheduler;
pub mod schema;
pub mod semantic;
pub mod shell;
pub mod snapshot;
pub mod spellcheck;
pub mod state_machine;
pub mod statistics;
pub mod summarizer;
pub mod table_extract;
pub mod task_queue;
pub mod telemetry;
pub mod template;
pub mod text_wrap;
pub mod timeline;
pub mod tokenizer;
pub mod traversal;
pub mod tree;
pub mod url;
pub mod validator;
pub mod wasm_runtime;
pub mod workspace;

// Re-export key types at crate root for convenience
pub use ai::{AiBackend, AiConfig, AiEngine, ChatMessage};
pub use async_io::{AsyncGrep, AsyncPipeline, DataSource, StreamProcessor};
pub use code_intel::{
    build_call_graph, build_scope_tree, CallEdge, CallGraph, CodeIntelligence, IntelBackend,
    Language, ScopeNode, StructuralQuery, Symbol, SymbolExtractor, SymbolKind,
};
pub use compat::{
    parse_awk_args, parse_grep_args, parse_sed_args, AwkCompat, GrepCompat, SedCompat,
};
pub use compliance::{
    generate_compliance_report, sha256_hash, sha256_hash_file, sha256_hash_files, AuditEvent,
    AuditLog, AuditOutcome, AuditSeverity, CmmcControl, ComplianceReport, DataClassification,
    DataMarking, DisseminationControl, FileIntegrityManifest,
};
pub use config::AtpConfig;
pub use dap::{
    AqlDebugger, Breakpoint, DapEvent, DapMessage, DapRequest, DapResponse, DebugSession,
    DebugState, StageSnapshot, WatchExpression,
};
pub use distributed::{
    DistributedPipeline, DistributedResult, MergeStrategy, NodeAssignment, NodeResult,
    PartitionStrategy, ScatterGatherPlan,
};
pub use engine::aql::{AqlEngine, AqlPipeline};
pub use engine::awk::AwkEngine;
pub use engine::grep::GrepEngine;
pub use engine::pipeline::{
    Pipeline, PipelineData, PipelineLine, StreamingConfig, StreamingPipeline,
};
pub use engine::sed::SedEngine;
pub use index::{FileEntry, IndexHit, IndexStats, SearchIndex};
pub use notebook::{CellKind, CellOutput, Notebook, NotebookCell, NotebookRunSummary};
pub use ontology::{Capability, CommandSpec, Ontology, TypeSpec};
pub use output::{
    AtpEnvelope, ChangeRecord, ContextLine, FileScope, Format, OutputFormatter, Provenance,
    SearchMatch, SearchResults, TransformResults,
};
pub use plugin::{
    AtpPlugin, FormatPlugin, PluginKind, PluginManifest, PluginPackage, PluginRecord,
    PluginRegistry, PluginSdk, PluginSummary, RegistryEntry, StagePlugin,
};
pub use remote::{DaemonConfig, RemoteExecutor, RemoteResult, RemoteTarget};
pub use telemetry::{
    init_tracing, trace_pipeline_stage, trace_search, BottleneckPhase, PhaseThroughput,
    SharedTelemetry, TelemetryReport, TelemetrySession, TracedOperation, TracingConfig,
    TracingFormat,
};
pub use traversal::{ScopeFilter, Walker};
pub use wasm_runtime::{
    build_test_wasm_module, BuiltinWasmEngine, WasmCapability, WasmEngine, WasmLimits, WasmModule,
    WasmPluginManifest, WasmResult, WasmRuntime,
};

// v1.5.0 re-exports
pub use cache::{CacheConfig, CacheEntry, CacheStats, PipelineCache};
pub use diff::{DiffOp, DiffOpKind, DiffResult, UnifiedLine};
pub use git_search::{
    BlameInfo, ChangeStatus, ChangedFile, GitSearchConfig, GitSearchResult, LineRange,
};
pub use optimizer::{
    format_plan, optimize, stage_desc, OptimizationRule, QueryPlan, StageDesc, StageKind,
};
pub use profile::{
    analyze_profile, build_profile, measure_stage, OptimizationSuggestion, PipelineProfile,
    StageProfile, SuggestedAction, SuggestionSeverity,
};
pub use rate_limit::{
    CircuitBreaker, CircuitState, RateLimitConfig, RateLimitError, RateLimitStats,
    RateLimitedExecutor, TokenBucket,
};
pub use schema::{SchemaEntry, SchemaRegistry};
pub use snapshot::{Snapshot, SnapshotConfig, SnapshotHarness, SnapshotMetadata, SnapshotVerdict};

// v1.6.0 re-exports
pub use checkpoint::{Checkpoint, CheckpointConfig, CheckpointManager, ResumeStatus};
pub use dep_graph::{DepGraph, DepNode, GraphStats, ImportKind, ImportStatement};
pub use hooks::{Event, EventBus, EventKind, HookDef, HookOutcome, HookPriority};
pub use log_sink::{AlertRule, LogFilter, LogFormat, LogRecord, LogSink, Severity};
pub use metrics::{Counter, Gauge, Histogram, MetricKey, MetricValue, MetricsRegistry};
pub use patterns::{Pattern, PatternMatch, PatternRegistry, PatternSet, PatternSeverity};
pub use redact::{MaskStrategy, RedactConfig, RedactResult, Redactor, SensitiveKind};
pub use task_queue::{Priority, QueueConfig, TaskDef, TaskQueue, TaskState};
pub use template::{TemplateEngine, TemplateError, Value};
pub use workspace::{Project, ProjectKind, Workspace, WorkspaceConfig, WorkspaceStats};

// v1.7.0 re-exports
pub use archive::{ArchiveEntry, ArchiveFormat, ArchiveSearchHit, ArchiveStats};
pub use changelog::{
    BumpKind, ChangelogEntry, ChangelogStats, CommitType, ConventionalCommit, SemVer,
};
pub use data_table::{Aggregation, Column, ColumnType, DataTable, Row, Value as DataValue};
pub use encryption::{Algorithm, EncryptedPayload, EncryptionConfig, Kdf};
pub use formatter::{Alignment, BorderStyle, ColumnDef, Overflow, Table};
pub use linter::{Diagnostic, FileFilter, Fix, LintRule, Linter, Severity as LintSeverity};
pub use report::{Metric, OutputFormat, Report, ReportMeta, ReportTable, Section, SectionContent};
pub use rewrite::{RewriteEngine, RewriteResult, RewriteRule, RuleSet, Scope};
pub use rule_engine::{
    Action, Condition, ConflictStrategy, Rule, RuleEngine, RuleEngineConfig, RuleResult,
};
pub use scheduler::{CronExpr, ScheduleDef, Scheduler, SchedulerStats, TimePoint};

// v1.8.0 re-exports
pub use annotation::{AnnotatedDoc, Annotation, Category, OverlapStrategy};
pub use batch::{Batch, BatchOp, BatchOptions, BatchResult, MemFs};
pub use converter::{ConvertFormat, ConvertOptions, Converter, DocValue};
pub use highlight::{
    detect_language, highlight, tokenize, HighlightFormat, HighlightOptions, LangDef, Theme, Token,
    TokenKind,
};
pub use i18n::{
    collation_key, grapheme_clusters, locale_sort, normalize, transliterate, Grapheme, Locale,
    NormForm, Script,
};
pub use state_machine::{
    Guard, MachineAction, MachineInstance, MachineSpec, RunResult, StateDef, Transition,
};
pub use statistics::{
    analyze as analyze_text, char_distribution, ngrams, word_frequency, zipf_analysis,
    CharDistribution, Ngram, Readability, TextStats, WordFreq, ZipfAnalysis,
};
pub use timeline::{
    bucket_events, detect_anomalies, duration_histogram, parse_events, sparkline, summary_report,
    Anomaly, AnomalyKind, Bucket, Event as TimelineEvent, Granularity, HistBin,
};
pub use tree::{
    bfs, dfs, from_json as tree_from_json, merge, to_indented, to_json as tree_to_json, TreeDiff,
    TreeNode,
};
pub use validator::{
    validate_batch, validate_email, validate_ipv4, validate_ipv6, validate_json_schema,
    validate_semver, validate_url, validate_uuid, BatchEntry, SchemaNode, SchemaType,
    ValidationResult, ValidatorKind,
};

// v1.9.0 re-exports
pub use calendar::{
    add_days, day_of_week, difference, format as calendar_format, from_epoch, is_leap_year,
    parse as calendar_parse, parse_relative, resolve_relative, to_epoch, DateFormat, DateTime,
    Duration, RelativeDate,
};
pub use codec::{
    decode, detect_codec, encode, supported_codecs, CodecKind, CodecOptions, CodecResult,
};
pub use color::{
    accessibility, analogous, complementary, contrast_ratio, darken, hsl_to_rgb, lighten, mix,
    palette, parse_color, parse_hex, relative_luminance, rgb_to_hsl, to_ansi256, to_hex, triadic,
    wcag_large, wcag_normal, AccessibilityReport, ColorFormat, Hsl, Rgb, WcagLevel,
};
pub use compress::{
    analyze as compress_analyze, compress, decompress, huffman_decode, huffman_encode, lz77_decode,
    lz77_encode, rle_decode, rle_encode, Algorithm as CompressAlgorithm, CompressResult,
    CompressionAnalysis,
};
pub use fingerprint::{
    chunk_boundaries, find_duplicates, fnv1a, jaccard_similarity, minhash, minhash_similarity,
    rolling_hashes, shingle, simhash, simhash_similarity, summary_report as fingerprint_report,
    term_frequency, Fingerprint, FingerprintOptions, Shingle, Similarity,
};
pub use macro_engine::{
    builtin_comment, builtin_quote, builtin_slugify, execute as macro_execute, register_builtins,
    run_macro, run_macro_with, substitute, substitute_positional, Macro, MacroContext,
    MacroRecorder, MacroResult, MacroStep,
};
pub use markdown::{
    extract_links, heading_stats, parse as markdown_parse, render_html, render_plain, slugify, toc,
    word_count as md_word_count, HeadingStats, MdDoc, MdLink, MdNode, TocEntry,
};
pub use sampler::{
    random_sample, reservoir, sample, stratified, summary as sampler_summary, systematic,
    weighted_sample, weighted_sample_with, Sample, SampleMethod, SampleOptions, SampleSummary,
};
pub use spellcheck::{
    auto_correct, check as spell_check, check_with_dict, damerau_levenshtein, levenshtein,
    report as spell_report, soundex, sounds_alike, suggest, SpellIssue, SpellOptions, SpellResult,
    Suggestion,
};
pub use tokenizer::{
    bpe_tokenize, learn_bpe, stats as token_stats, tokenize as text_tokenize, tokenize_fixed,
    tokenize_paragraphs, tokenize_regex, tokenize_sentences, tokenize_words,
    Granularity as TokenGranularity, Token as TextToken, TokenStats, TokenizerOptions, Vocabulary,
};

// v2.0.0 re-exports
pub use casing::{convert as case_convert, detect as case_detect, CaseStyle};
pub use emoji::{
    contains_emoji, count as emoji_count, emoji_to_shortcode, extract as emoji_extract,
    find_emojis, sentiment as emoji_sentiment, shortcode_to_emoji, strip_skin_tones, EmojiMatch,
    Sentiment as EmojiSentiment,
};
pub use fuzzy::{
    best_match, fuzzy_search, jaro, jaro_winkler, trigram_similarity, FuzzyConfig, FuzzyMatch,
};
pub use graph::{
    centrality, cooccurrence, pagerank, textrank, to_dot, Centrality, Edge as GraphEdge,
    RankResult, TextGraph,
};
pub use mime::{
    detect as mime_detect, detect_charset, detect_magic, from_extension, Charset, MimeType,
};
pub use shell::{
    analyze as shell_analyze, check_portability, cmd_escape, parse_shebang, split_args,
    PortabilityHint, ShellAnalysis, ShellKind,
};
pub use summarizer::{
    extract_keywords, summarize, summarize_default, Keyword, Strategy as SummaryStrategy, Summary,
    SummaryConfig,
};
pub use table_extract::{
    detect_boundaries, detect_tables, parse_ascii, parse_fixed_width,
    parse_markdown as parse_markdown_table, DetectedTable, Row as TableRow,
    Table as ExtractedTable, TableFormat,
};
pub use text_wrap::{
    columns as text_columns, dedent, hanging_indent, indent as text_indent, word_wrap, wrap, Align,
    WrapOptions,
};
pub use url::{
    domain, normalize as url_normalize, parse as url_parse, percent_decode, percent_encode,
    resolve as url_resolve, tld, Url,
};

/// ATP version constant
pub const ATP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// ATP tool identifier
pub const ATP_TOOL_ID: &str = "atp";
