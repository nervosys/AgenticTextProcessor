//! Async I/O — streaming text processing with tokio.
//!
//! Provides async equivalents of the core engines for non-blocking file I/O,
//! HTTP/URL sources, and streaming pipeline execution.
//!
//! # Architecture
//!
//! - [`AsyncGrep`]: Async grep with streaming results via channels
//! - [`AsyncPipeline`]: Execute AQL pipelines over async data sources
//! - [`DataSource`]: Unified abstraction for files, URLs, and stdin
//! - [`StreamProcessor`]: Process lines as an async stream

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;

/// Data source abstraction — files, URLs, or stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DataSource {
    /// Local file path.
    File { path: String },
    /// HTTP/HTTPS URL.
    Url { url: String },
    /// Standard input (for piping).
    Stdin,
}

impl DataSource {
    /// Parse a string into a DataSource — auto-detect file vs URL.
    pub fn parse(input: &str) -> Self {
        if input == "-" {
            DataSource::Stdin
        } else if input.starts_with("http://") || input.starts_with("https://") {
            DataSource::Url {
                url: input.to_string(),
            }
        } else {
            DataSource::File {
                path: input.to_string(),
            }
        }
    }

    /// Resolve the source name for display purposes.
    pub fn name(&self) -> &str {
        match self {
            DataSource::File { path } => path,
            DataSource::Url { url } => url,
            DataSource::Stdin => "<stdin>",
        }
    }
}

/// A single line from a data source with metadata.
#[derive(Debug, Clone)]
pub struct SourceLine {
    /// Where this line came from.
    pub source: String,
    /// 1-based line number.
    pub line_number: usize,
    /// The line content.
    pub content: String,
}

/// Async stream processor — reads lines from any async source.
pub struct StreamProcessor;

impl StreamProcessor {
    /// Read all lines from an async reader into SourceLines.
    pub async fn read_lines<R: AsyncRead + Unpin>(
        reader: R,
        source_name: &str,
    ) -> Result<Vec<SourceLine>> {
        let buf = BufReader::new(reader);
        let mut lines_stream = buf.lines();
        let mut result = Vec::new();
        let mut line_num = 0usize;

        while let Some(line) = lines_stream.next_line().await? {
            line_num += 1;
            result.push(SourceLine {
                source: source_name.to_string(),
                line_number: line_num,
                content: line,
            });
        }

        Ok(result)
    }

    /// Stream lines from an async reader, sending them through a channel.
    pub async fn stream_lines<R: AsyncRead + Unpin + Send + 'static>(
        reader: R,
        source_name: String,
        tx: mpsc::Sender<SourceLine>,
    ) -> Result<usize> {
        let buf = BufReader::new(reader);
        let mut lines_stream = buf.lines();
        let mut count = 0usize;

        while let Some(line) = lines_stream.next_line().await? {
            count += 1;
            let source_line = SourceLine {
                source: source_name.clone(),
                line_number: count,
                content: line,
            };
            if tx.send(source_line).await.is_err() {
                break; // receiver dropped
            }
        }

        Ok(count)
    }
}

/// Fetch content from a URL asynchronously.
pub async fn fetch_url(url: &str) -> Result<String> {
    let resp = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to fetch URL: {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status} fetching {url}");
    }

    resp.text()
        .await
        .with_context(|| format!("Failed to read body from {url}"))
}

/// Open a DataSource and return its contents as text.
pub async fn read_source(source: &DataSource) -> Result<String> {
    match source {
        DataSource::File { path } => tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read file: {path}")),
        DataSource::Url { url } => fetch_url(url).await,
        DataSource::Stdin => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("Failed to read stdin")?;
            Ok(buf)
        }
    }
}

/// Async grep — search across multiple sources concurrently.
pub struct AsyncGrep;

impl AsyncGrep {
    /// Search across multiple data sources concurrently using the grep engine.
    ///
    /// Returns all matches from all sources, collected in order.
    pub async fn search_sources(
        pattern: &str,
        sources: &[DataSource],
        case_insensitive: bool,
    ) -> Result<Vec<crate::SearchMatch>> {
        use crate::engine::grep::{GrepConfig, GrepEngine};

        let mut handles = Vec::new();

        for source in sources {
            let source = source.clone();
            let pattern = pattern.to_string();
            let ci = case_insensitive;

            handles.push(tokio::spawn(async move {
                let text = read_source(&source).await?;
                let cfg = GrepConfig {
                    pattern,
                    case_sensitive: !ci,
                    ..GrepConfig::default()
                };
                let eng = GrepEngine::new(cfg)?;
                let cursor = std::io::Cursor::new(text.as_bytes());
                eng.search_reader(cursor, source.name())
            }));
        }

        let mut all_matches = Vec::new();
        for handle in handles {
            let matches = handle.await??;
            all_matches.extend(matches);
        }

        Ok(all_matches)
    }
}

/// Streaming AQL pipeline — process data through AQL stages asynchronously.
pub struct AsyncPipeline;

impl AsyncPipeline {
    /// Execute an AQL query on data from a source.
    pub async fn execute(
        query: &str,
        source: &DataSource,
    ) -> Result<crate::output::PipelineResults> {
        use crate::engine::aql;

        let text = read_source(source).await?;
        let pipeline = aql::parse(query)?;

        // Write to temp file for AQL execution
        let dir = std::env::temp_dir();
        let tmp_path = dir.join(format!("atp_async_{}.txt", std::process::id()));
        tokio::fs::write(&tmp_path, &text).await?;

        let mut engine = aql::AqlEngine::new();
        let result = engine.execute(&pipeline, &[tmp_path.as_path()]);

        // Clean up temp file (ignore errors)
        let _ = tokio::fs::remove_file(&tmp_path).await;

        result
    }

    /// Execute an AQL query on multiple sources concurrently.
    pub async fn execute_multi(
        query: &str,
        sources: &[DataSource],
    ) -> Result<Vec<(String, crate::output::PipelineResults)>> {
        let mut handles = Vec::new();

        for source in sources {
            let source = source.clone();
            let query = query.to_string();

            handles.push(tokio::spawn(async move {
                let name = source.name().to_string();
                let result = AsyncPipeline::execute(&query, &source).await?;
                Ok::<_, anyhow::Error>((name, result))
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await??);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_source_parse_file() {
        let ds = DataSource::parse("foo.txt");
        assert!(matches!(ds, DataSource::File { path } if path == "foo.txt"));
    }

    #[test]
    fn test_data_source_parse_url() {
        let ds = DataSource::parse("https://example.com/data.txt");
        assert!(matches!(ds, DataSource::Url { url } if url == "https://example.com/data.txt"));
    }

    #[test]
    fn test_data_source_parse_stdin() {
        let ds = DataSource::parse("-");
        assert!(matches!(ds, DataSource::Stdin));
    }

    #[test]
    fn test_data_source_name() {
        assert_eq!(DataSource::parse("foo.txt").name(), "foo.txt");
        assert_eq!(
            DataSource::parse("https://example.com").name(),
            "https://example.com"
        );
        assert_eq!(DataSource::parse("-").name(), "<stdin>");
    }

    #[tokio::test]
    async fn test_stream_processor_read_lines() {
        let data = b"line1\nline2\nline3\n";
        let cursor = tokio::io::BufReader::new(&data[..]);
        let lines = StreamProcessor::read_lines(cursor, "test").await.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].content, "line1");
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[2].content, "line3");
    }

    #[tokio::test]
    async fn test_stream_lines_channel() {
        let data = b"alpha\nbeta\ngamma\n";
        let (tx, mut rx) = mpsc::channel(10);

        let cursor = tokio::io::BufReader::new(&data[..]);
        let count = StreamProcessor::stream_lines(cursor, "test".into(), tx)
            .await
            .unwrap();
        assert_eq!(count, 3);

        let mut received = Vec::new();
        while let Some(line) = rx.recv().await {
            received.push(line.content);
        }
        assert_eq!(received, vec!["alpha", "beta", "gamma"]);
    }

    #[tokio::test]
    async fn test_read_source_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(
            &mut std::fs::File::create(tmp.path()).unwrap(),
            b"hello async",
        )
        .unwrap();
        let ds = DataSource::File {
            path: tmp.path().to_string_lossy().to_string(),
        };
        let text = read_source(&ds).await.unwrap();
        assert_eq!(text, "hello async");
    }

    #[tokio::test]
    async fn test_async_pipeline_with_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(
            &mut std::fs::File::create(tmp.path()).unwrap(),
            b"hello world\nfoo bar\nhello again\n",
        )
        .unwrap();
        let ds = DataSource::File {
            path: tmp.path().to_string_lossy().to_string(),
        };
        let results = AsyncPipeline::execute("find \"hello\"", &ds).await.unwrap();
        // final_output is a JSON array
        if let serde_json::Value::Array(arr) = &results.final_output {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected JSON array");
        }
    }
}
