//! Criterion benchmarks for ATP core engines.
//!
//! Run with: `cargo bench -p atp-core`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::io::Cursor;

use atp_core::engine::awk::AwkConfig;
use atp_core::engine::grep::{GrepConfig, GrepEngine};
use atp_core::engine::pipeline::{PipelineData, PipelineLine};
use atp_core::engine::sed::{SedConfig, SedEngine, TransformCommand};
use atp_core::output::PatternType;

// ---------------------------------------------------------------------------
// Test data generators
// ---------------------------------------------------------------------------

fn generate_text(num_lines: usize) -> String {
    let mut text = String::new();
    for i in 0..num_lines {
        if i % 10 == 0 {
            text.push_str(&format!("TODO: fix issue #{i} in the codebase\n"));
        } else if i % 7 == 0 {
            text.push_str(&format!("ERROR: something went wrong at line {i}\n"));
        } else {
            text.push_str(&format!(
                "This is line number {i} with some regular content.\n"
            ));
        }
    }
    text
}

fn generate_csv(num_lines: usize) -> String {
    let mut csv = String::from("name,age,city,score\n");
    let names = ["Alice", "Bob", "Charlie", "Diana", "Eve"];
    let cities = ["New York", "London", "Tokyo", "Paris", "Berlin"];
    for i in 0..num_lines {
        csv.push_str(&format!(
            "{},{},{},{}\n",
            names[i % 5],
            20 + (i % 50),
            cities[i % 5],
            (i * 17 + 3) % 100
        ));
    }
    csv
}

// ---------------------------------------------------------------------------
// Grep benchmarks
// ---------------------------------------------------------------------------

fn bench_grep(c: &mut Criterion) {
    let mut group = c.benchmark_group("grep");

    for size in [100, 1_000, 10_000] {
        let text = generate_text(size);

        group.bench_with_input(BenchmarkId::new("search_reader", size), &text, |b, text| {
            let config = GrepConfig {
                pattern: "TODO".to_string(),
                pattern_type: PatternType::Regex,
                ..Default::default()
            };
            let engine = GrepEngine::new(config).unwrap();
            b.iter(|| {
                let cursor = Cursor::new(black_box(text.as_bytes()));
                engine.search_reader(cursor, "bench").unwrap()
            });
        });

        group.bench_with_input(
            BenchmarkId::new("search_reader_case_insensitive", size),
            &text,
            |b, text| {
                let config = GrepConfig {
                    pattern: "error".to_string(),
                    pattern_type: PatternType::Regex,
                    case_sensitive: false,
                    ..Default::default()
                };
                let engine = GrepEngine::new(config).unwrap();
                b.iter(|| {
                    let cursor = Cursor::new(black_box(text.as_bytes()));
                    engine.search_reader(cursor, "bench").unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("search_reader_context", size),
            &text,
            |b, text| {
                let config = GrepConfig {
                    pattern: "TODO".to_string(),
                    pattern_type: PatternType::Regex,
                    context_before: 2,
                    context_after: 2,
                    ..Default::default()
                };
                let engine = GrepEngine::new(config).unwrap();
                b.iter(|| {
                    let cursor = Cursor::new(black_box(text.as_bytes()));
                    engine.search_reader(cursor, "bench").unwrap()
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Sed benchmarks
// ---------------------------------------------------------------------------

fn bench_sed(c: &mut Criterion) {
    let mut group = c.benchmark_group("sed");

    for size in [100, 1_000, 10_000] {
        let text = generate_text(size);

        group.bench_with_input(
            BenchmarkId::new("transform_reader_substitute", size),
            &text,
            |b, text| {
                let config = SedConfig {
                    commands: vec![TransformCommand::Substitute {
                        pattern: "TODO".to_string(),
                        replacement: "DONE".to_string(),
                        global: true,
                        case_insensitive: false,
                    }],
                    dry_run: true,
                    ..Default::default()
                };
                let engine = SedEngine::new(config);
                b.iter(|| {
                    let cursor = Cursor::new(black_box(text.as_bytes()));
                    let mut output = Vec::new();
                    engine
                        .transform_reader(cursor, &mut output, "bench")
                        .unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("transform_reader_delete", size),
            &text,
            |b, text| {
                let config = SedConfig {
                    commands: vec![TransformCommand::Delete {
                        pattern: "TODO".to_string(),
                    }],
                    dry_run: true,
                    ..Default::default()
                };
                let engine = SedEngine::new(config);
                b.iter(|| {
                    let cursor = Cursor::new(black_box(text.as_bytes()));
                    let mut output = Vec::new();
                    engine
                        .transform_reader(cursor, &mut output, "bench")
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Awk benchmarks
// ---------------------------------------------------------------------------

fn bench_awk(c: &mut Criterion) {
    let mut group = c.benchmark_group("awk");

    for size in [100, 1_000, 10_000] {
        let csv = generate_csv(size);

        group.bench_with_input(
            BenchmarkId::new("process_reader_csv", size),
            &csv,
            |b, csv| {
                use atp_core::engine::awk::{AwkEngine, Condition, Rule};
                let config = AwkConfig {
                    field_separator: ",".to_string(),
                    has_header: true,
                    rules: vec![Rule {
                        pattern: None,
                        condition: None,
                        select_fields: vec![1, 2, 3],
                        computed_fields: Vec::new(),
                    }],
                    ..Default::default()
                };
                let engine = AwkEngine::new(config).unwrap();
                b.iter(|| {
                    let cursor = Cursor::new(black_box(csv.as_bytes()));
                    engine.process_reader(cursor, "bench", 0).unwrap()
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// AQL benchmarks
// ---------------------------------------------------------------------------

fn bench_aql(c: &mut Criterion) {
    let mut group = c.benchmark_group("aql");

    // Parse benchmarks
    group.bench_function("parse_simple", |b| {
        b.iter(|| atp_core::engine::aql::parse(black_box(r#"find "TODO""#)).unwrap());
    });

    group.bench_function("parse_complex", |b| {
        b.iter(|| {
            atp_core::engine::aql::parse(black_box(
                r#"find "error" ignore_case | sort by line desc | take 10 | count"#,
            ))
            .unwrap()
        });
    });

    // Execute benchmarks
    for size in [100, 1_000, 10_000] {
        let text = generate_text(size);
        let pipeline = atp_core::engine::aql::parse(r#"find "TODO""#).unwrap();

        group.bench_with_input(BenchmarkId::new("execute_find", size), &text, |b, text| {
            b.iter(|| {
                let data = PipelineData {
                    lines: text
                        .lines()
                        .enumerate()
                        .map(|(i, line)| PipelineLine {
                            source_file: "bench".to_string(),
                            source_line: i + 1,
                            content: line.to_string(),
                            fields: Vec::new(),
                        })
                        .collect(),
                    source_files: Vec::new(),
                };
                let mut engine = atp_core::engine::aql::AqlEngine::new();
                engine.execute_on_data(black_box(&pipeline), data).unwrap()
            });
        });
    }

    // Pipeline with multiple stages
    let text = generate_text(1_000);
    let multi_pipeline =
        atp_core::engine::aql::parse(r#"find "TODO" | sort | unique | count"#).unwrap();

    group.bench_function("execute_multi_stage_1000", |b| {
        b.iter(|| {
            let data = PipelineData {
                lines: text
                    .lines()
                    .enumerate()
                    .map(|(i, line)| PipelineLine {
                        source_file: "bench".to_string(),
                        source_line: i + 1,
                        content: line.to_string(),
                        fields: Vec::new(),
                    })
                    .collect(),
                source_files: Vec::new(),
            };
            let mut engine = atp_core::engine::aql::AqlEngine::new();
            engine
                .execute_on_data(black_box(&multi_pipeline), data)
                .unwrap()
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Semantic search benchmarks
// ---------------------------------------------------------------------------

fn bench_semantic(c: &mut Criterion) {
    let mut group = c.benchmark_group("semantic");

    for size in [100, 1_000] {
        let text = generate_text(size);
        let mut index = atp_core::semantic::TfIdfIndex::new();
        for (i, line) in text.lines().enumerate() {
            index.add_document("bench", i + 1, line);
        }
        index.finalize();

        group.bench_with_input(BenchmarkId::new("query", size), &index, |b, index| {
            b.iter(|| index.query(black_box("TODO fix issue"), 10));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_grep,
    bench_sed,
    bench_awk,
    bench_aql,
    bench_semantic,
);
criterion_main!(benches);
