# Benchmarks

ATP uses [Criterion.rs](https://github.com/bheisler/criterion.rs) for statistical benchmarking.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p atp-core

# Run specific benchmark group
cargo bench -p atp-core -- grep
cargo bench -p atp-core -- sed
cargo bench -p atp-core -- awk
cargo bench -p atp-core -- aql
cargo bench -p atp-core -- semantic
```

## Benchmark Groups

### Grep

| Benchmark               | Input Size         |
| ----------------------- | ------------------ |
| `grep/search_reader`    | 100, 1K, 10K lines |
| `grep/case_insensitive` | 100, 1K, 10K lines |
| `grep/context_lines`    | 100, 1K, 10K lines |

### Sed

| Benchmark        | Input Size         |
| ---------------- | ------------------ |
| `sed/substitute` | 100, 1K, 10K lines |
| `sed/delete`     | 100, 1K, 10K lines |

### Awk

| Benchmark                | Input Size         |
| ------------------------ | ------------------ |
| `awk/process_reader_csv` | 100, 1K, 10K lines |

### AQL

| Benchmark                 | Input Size         |
| ------------------------- | ------------------ |
| `aql/parse_simple`        | N/A (parsing only) |
| `aql/parse_complex`       | N/A (parsing only) |
| `aql/execute_find`        | 100, 1K, 10K lines |
| `aql/execute_multi_stage` | 1K lines           |

### Semantic Search

| Benchmark        | Input Size        |
| ---------------- | ----------------- |
| `semantic/query` | 100, 1K documents |

## Viewing Results

Criterion generates HTML reports in `target/criterion/`:

```bash
# Open the report after running benchmarks
open target/criterion/report/index.html
```

Reports include:
- Statistical analysis with confidence intervals
- Throughput measurements
- Comparison between runs (regression detection)
- Violin plots and PDF estimates
