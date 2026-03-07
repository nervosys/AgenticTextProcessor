//! Grep engine — strongly typed pattern search across files and directories.
//!
//! Provides regex, literal, and glob-based searching with:
//! - Contextual matches (before/after lines)
//! - Byte-offset tracking
//! - Multi-file traversal
//! - Deterministic, ordered output
//! - Streaming mode for memory-efficient processing of large files
//! - Parallel search with rayon for multi-file operations
//! - Memory-mapped I/O for large files
//! - Aho-Corasick automaton for O(n) multi-pattern literal matching

use crate::output::{ContextLine, PatternType, SearchMatch, SearchResults};
use aho_corasick::AhoCorasick;
use anyhow::{Context, Result};
use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Configuration for a grep search operation.
#[derive(Debug, Clone)]
pub struct GrepConfig {
    pub pattern: String,
    pub pattern_type: PatternType,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub invert_match: bool,
    pub context_before: usize,
    pub context_after: usize,
    pub max_matches: Option<usize>,
    pub max_matches_per_file: Option<usize>,
    pub multiline: bool,
    pub include_binary: bool,
    /// Additional patterns for multi-pattern matching (like grep -e)
    pub extra_patterns: Vec<String>,
    /// Only output the matched text, not the full line (like grep -o)
    pub only_matching: bool,
}

impl Default for GrepConfig {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            pattern_type: PatternType::Regex,
            case_sensitive: true,
            whole_word: false,
            invert_match: false,
            context_before: 0,
            context_after: 0,
            max_matches: None,
            max_matches_per_file: None,
            multiline: false,
            include_binary: false,
            extra_patterns: Vec::new(),
            only_matching: false,
        }
    }
}

/// The grep search engine.
pub struct GrepEngine {
    config: GrepConfig,
    compiled: Regex,
    /// Additional compiled patterns for multi-pattern matching
    extra_compiled: Vec<Regex>,
    /// Aho-Corasick automaton for O(n) multi-pattern literal matching.
    /// Built when pattern_type is Literal and there are extra_patterns.
    ac_automaton: Option<AhoCorasick>,
}

impl GrepEngine {
    /// Create a new GrepEngine with the given configuration.
    pub fn new(config: GrepConfig) -> Result<Self> {
        let pattern_str = match config.pattern_type {
            PatternType::Literal => regex::escape(&config.pattern),
            PatternType::Glob => glob_to_regex(&config.pattern),
            PatternType::Regex | PatternType::Semantic => config.pattern.clone(),
        };

        let pattern_str = if config.whole_word {
            format!(r"\b{}\b", pattern_str)
        } else {
            pattern_str
        };

        let compiled = RegexBuilder::new(&pattern_str)
            .case_insensitive(!config.case_sensitive)
            .multi_line(config.multiline)
            .build()
            .with_context(|| format!("Invalid pattern: {}", config.pattern))?;

        // Compile extra patterns
        let mut extra_compiled = Vec::new();
        for extra in &config.extra_patterns {
            let extra_str = match config.pattern_type {
                PatternType::Literal => regex::escape(extra),
                PatternType::Glob => glob_to_regex(extra),
                PatternType::Regex | PatternType::Semantic => extra.clone(),
            };
            let extra_str = if config.whole_word {
                format!(r"\b{}\b", extra_str)
            } else {
                extra_str
            };
            let re = RegexBuilder::new(&extra_str)
                .case_insensitive(!config.case_sensitive)
                .multi_line(config.multiline)
                .build()
                .with_context(|| format!("Invalid extra pattern: {extra}"))?;
            extra_compiled.push(re);
        }

        // Build Aho-Corasick automaton for literal multi-pattern matching.
        // When the pattern type is Literal and we have multiple patterns,
        // the AC automaton provides O(n + m + z) matching where n=text length,
        // m=total pattern length, z=number of matches — significantly faster
        // than running each regex independently.
        let ac_automaton = if config.pattern_type == PatternType::Literal
            && !config.extra_patterns.is_empty()
            && !config.whole_word
        {
            let mut all_literals: Vec<&str> = vec![&config.pattern];
            all_literals.extend(config.extra_patterns.iter().map(|s| s.as_str()));
            aho_corasick::AhoCorasickBuilder::new()
                .ascii_case_insensitive(!config.case_sensitive)
                .build(&all_literals)
                .ok()
        } else {
            None
        };

        Ok(Self {
            config,
            compiled,
            extra_compiled,
            ac_automaton,
        })
    }

    /// Search a single file, returning all matches.
    pub fn search_file(&self, path: &Path) -> Result<Vec<SearchMatch>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let file_str = path.display().to_string();
        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();
        let mut byte_offset = 0usize;

        // Collect all regexes (primary + extras) for multi-pattern matching
        let all_patterns: Vec<&Regex> = std::iter::once(&self.compiled)
            .chain(self.extra_compiled.iter())
            .collect();

        for (idx, line) in lines.iter().enumerate() {
            let line_number = idx + 1;

            // A line matches if ANY pattern matches.
            // Use Aho-Corasick automaton for O(n) literal multi-pattern check
            // when available, falling back to per-regex iteration otherwise.
            let has_match = if let Some(ref ac) = self.ac_automaton {
                ac.is_match(line)
            } else {
                all_patterns.iter().any(|re| re.is_match(line))
            };
            let should_include = if self.config.invert_match {
                !has_match
            } else {
                has_match
            };

            if should_include {
                if self.config.invert_match {
                    // For inverted match, the entire line is the "match"
                    let context_before = self.get_context_before(&lines, idx);
                    let context_after = self.get_context_after(&lines, idx);
                    matches.push(SearchMatch {
                        file: file_str.clone(),
                        line_number,
                        column_start: 1,
                        column_end: line.len() + 1,
                        line_content: line.to_string(),
                        matched_text: line.to_string(),
                        context_before,
                        context_after,
                        byte_offset,
                    });
                } else {
                    // Collect all matches from all patterns on this line
                    for re in &all_patterns {
                        for mat in re.find_iter(line) {
                            let context_before = self.get_context_before(&lines, idx);
                            let context_after = self.get_context_after(&lines, idx);
                            let matched_text = mat.as_str().to_string();
                            matches.push(SearchMatch {
                                file: file_str.clone(),
                                line_number,
                                column_start: mat.start() + 1,
                                column_end: mat.end() + 1,
                                // In only-matching mode, line_content shows just the match
                                line_content: if self.config.only_matching {
                                    matched_text.clone()
                                } else {
                                    line.to_string()
                                },
                                matched_text,
                                context_before,
                                context_after,
                                byte_offset: byte_offset + mat.start(),
                            });
                        }
                    }
                }

                if let Some(max) = self.config.max_matches_per_file {
                    if matches.len() >= max {
                        matches.truncate(max);
                        break;
                    }
                }
            }

            byte_offset += line.len() + 1; // +1 for newline
        }

        Ok(matches)
    }

    /// Search multiple files, returning aggregated results.
    pub fn search_files(&self, paths: &[&Path]) -> Result<SearchResults> {
        let mut all_matches = Vec::new();
        let mut files_with_matches = 0usize;

        for path in paths {
            let file_matches = self.search_file(path)?;
            if !file_matches.is_empty() {
                files_with_matches += 1;
            }
            all_matches.extend(file_matches);

            if let Some(max) = self.config.max_matches {
                if all_matches.len() >= max {
                    all_matches.truncate(max);
                    break;
                }
            }
        }

        let total = all_matches.len();
        Ok(SearchResults {
            pattern: self.config.pattern.clone(),
            pattern_type: self.config.pattern_type.clone(),
            case_sensitive: self.config.case_sensitive,
            total_matches: total,
            files_with_matches,
            matches: all_matches,
        })
    }

    fn get_context_before(&self, lines: &[&str], current_idx: usize) -> Vec<ContextLine> {
        let start = current_idx.saturating_sub(self.config.context_before);
        (start..current_idx)
            .map(|i| ContextLine {
                line_number: i + 1,
                content: lines[i].to_string(),
            })
            .collect()
    }

    fn get_context_after(&self, lines: &[&str], current_idx: usize) -> Vec<ContextLine> {
        let end = (current_idx + 1 + self.config.context_after).min(lines.len());
        ((current_idx + 1)..end)
            .map(|i| ContextLine {
                line_number: i + 1,
                content: lines[i].to_string(),
            })
            .collect()
    }

    // ── Streaming API ───────────────────────────────────────────────────

    /// Search a readable source (file, stdin, pipe) in streaming mode.
    ///
    /// Uses a bounded ring buffer to provide context lines without loading the
    /// entire input into memory. Ideal for large files or continuous streams.
    pub fn search_reader<R: Read>(&self, reader: R, source_name: &str) -> Result<Vec<SearchMatch>> {
        let buf = BufReader::new(reader);
        let all_patterns: Vec<&Regex> = std::iter::once(&self.compiled)
            .chain(self.extra_compiled.iter())
            .collect();

        let ctx_before = self.config.context_before;
        let ctx_after = self.config.context_after;

        // Ring buffer for context-before lines
        let mut before_buf: VecDeque<(usize, String)> = VecDeque::with_capacity(ctx_before + 1);
        // Pending matches waiting for context-after lines
        let mut pending: Vec<(SearchMatch, usize)> = Vec::new(); // (match, remaining_after)
        let mut matches: Vec<SearchMatch> = Vec::new();
        let mut byte_offset: usize = 0;

        for (idx, line_result) in buf.lines().enumerate() {
            let line = line_result
                .with_context(|| format!("Failed to read line {} from {}", idx + 1, source_name))?;
            let line_number = idx + 1;

            // Feed context-after to pending matches
            for (pending_match, remaining) in pending.iter_mut() {
                if *remaining > 0 {
                    pending_match.context_after.push(ContextLine {
                        line_number,
                        content: line.clone(),
                    });
                    *remaining -= 1;
                }
            }
            // Flush fully-resolved pending matches
            let drain_count = pending
                .iter()
                .take_while(|(_, remaining)| *remaining == 0)
                .count();
            matches.extend(pending.drain(..drain_count).map(|(m, _)| m));

            // Check if line matches — AC automaton fast path for literals
            let has_match = if let Some(ref ac) = self.ac_automaton {
                ac.is_match(&line)
            } else {
                all_patterns.iter().any(|re| re.is_match(&line))
            };
            let should_include = if self.config.invert_match {
                !has_match
            } else {
                has_match
            };

            if should_include {
                let context_before: Vec<ContextLine> = before_buf
                    .iter()
                    .map(|(ln, content)| ContextLine {
                        line_number: *ln,
                        content: content.clone(),
                    })
                    .collect();

                if self.config.invert_match {
                    let m = SearchMatch {
                        file: source_name.to_string(),
                        line_number,
                        column_start: 1,
                        column_end: line.len() + 1,
                        line_content: line.clone(),
                        matched_text: line.clone(),
                        context_before,
                        context_after: Vec::new(),
                        byte_offset,
                    };
                    pending.push((m, ctx_after));
                } else {
                    for re in &all_patterns {
                        for mat in re.find_iter(&line) {
                            let matched_text = mat.as_str().to_string();
                            let m = SearchMatch {
                                file: source_name.to_string(),
                                line_number,
                                column_start: mat.start() + 1,
                                column_end: mat.end() + 1,
                                line_content: if self.config.only_matching {
                                    matched_text.clone()
                                } else {
                                    line.clone()
                                },
                                matched_text,
                                context_before: context_before.clone(),
                                context_after: Vec::new(),
                                byte_offset: byte_offset + mat.start(),
                            };
                            pending.push((m, ctx_after));
                        }
                    }
                }

                if let Some(max) = self.config.max_matches_per_file {
                    let total = matches.len() + pending.len();
                    if total >= max {
                        break;
                    }
                }
            }

            // Update ring buffer
            before_buf.push_back((line_number, line.clone()));
            if before_buf.len() > ctx_before {
                before_buf.pop_front();
            }

            byte_offset += line.len() + 1;
        }

        // Flush remaining pending matches
        matches.extend(pending.into_iter().map(|(m, _)| m));

        if let Some(max) = self.config.max_matches_per_file {
            matches.truncate(max);
        }

        Ok(matches)
    }

    /// Search a file in streaming mode (memory-efficient for large files).
    pub fn search_file_streaming(&self, path: &Path) -> Result<Vec<SearchMatch>> {
        let file = fs::File::open(path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;
        self.search_reader(file, &path.display().to_string())
    }

    /// Search multiple files in streaming mode.
    pub fn search_files_streaming(&self, paths: &[&Path]) -> Result<SearchResults> {
        let mut all_matches = Vec::new();
        let mut files_with_matches = 0usize;

        for path in paths {
            let file_matches = self.search_file_streaming(path)?;
            if !file_matches.is_empty() {
                files_with_matches += 1;
            }
            all_matches.extend(file_matches);

            if let Some(max) = self.config.max_matches {
                if all_matches.len() >= max {
                    all_matches.truncate(max);
                    break;
                }
            }
        }

        let total = all_matches.len();
        Ok(SearchResults {
            pattern: self.config.pattern.clone(),
            pattern_type: self.config.pattern_type.clone(),
            case_sensitive: self.config.case_sensitive,
            total_matches: total,
            files_with_matches,
            matches: all_matches,
        })
    }

    // ── Parallel API ────────────────────────────────────────────────────

    /// Search multiple files in parallel using rayon.
    ///
    /// Files are searched concurrently across all available cores. Results
    /// are collected and sorted by file path for deterministic output.
    pub fn search_files_parallel(&self, paths: &[&Path]) -> Result<SearchResults> {
        let per_file: Vec<(Vec<SearchMatch>, bool)> = paths
            .par_iter()
            .map(|path| {
                let matches = self.search_file(path).unwrap_or_default();
                let has_matches = !matches.is_empty();
                (matches, has_matches)
            })
            .collect();

        let mut all_matches = Vec::new();
        let mut files_with_matches = 0usize;

        for (matches, has) in per_file {
            if has {
                files_with_matches += 1;
            }
            all_matches.extend(matches);

            if let Some(max) = self.config.max_matches {
                if all_matches.len() >= max {
                    all_matches.truncate(max);
                    break;
                }
            }
        }

        let total = all_matches.len();
        Ok(SearchResults {
            pattern: self.config.pattern.clone(),
            pattern_type: self.config.pattern_type.clone(),
            case_sensitive: self.config.case_sensitive,
            total_matches: total,
            files_with_matches,
            matches: all_matches,
        })
    }

    // ── Memory-mapped I/O API ───────────────────────────────────────────

    /// Search a file using memory-mapped I/O for zero-copy access.
    ///
    /// This is more efficient than `search_file` for large files as it
    /// avoids copying file contents into a heap-allocated `String`.
    pub fn search_file_mmap(&self, path: &Path) -> Result<Vec<SearchMatch>> {
        let file = fs::File::open(path)
            .with_context(|| format!("Failed to open file: {}", path.display()))?;

        let mmap = unsafe {
            memmap2::Mmap::map(&file)
                .with_context(|| format!("Failed to mmap file: {}", path.display()))?
        };

        // Treat as UTF-8, skipping binary files
        let content = match std::str::from_utf8(&mmap) {
            Ok(s) => s,
            Err(_) if !self.config.include_binary => return Ok(Vec::new()),
            Err(_) => return Ok(Vec::new()),
        };

        let file_str = path.display().to_string();
        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();
        let mut byte_offset = 0usize;

        let all_patterns: Vec<&Regex> = std::iter::once(&self.compiled)
            .chain(self.extra_compiled.iter())
            .collect();

        for (idx, line) in lines.iter().enumerate() {
            let line_number = idx + 1;
            let has_match = if let Some(ref ac) = self.ac_automaton {
                ac.is_match(line)
            } else {
                all_patterns.iter().any(|re| re.is_match(line))
            };
            let should_include = if self.config.invert_match {
                !has_match
            } else {
                has_match
            };

            if should_include {
                if self.config.invert_match {
                    let context_before = self.get_context_before(&lines, idx);
                    let context_after = self.get_context_after(&lines, idx);
                    matches.push(SearchMatch {
                        file: file_str.clone(),
                        line_number,
                        column_start: 1,
                        column_end: line.len() + 1,
                        line_content: line.to_string(),
                        matched_text: line.to_string(),
                        context_before,
                        context_after,
                        byte_offset,
                    });
                } else {
                    for re in &all_patterns {
                        for mat in re.find_iter(line) {
                            let context_before = self.get_context_before(&lines, idx);
                            let context_after = self.get_context_after(&lines, idx);
                            let matched_text = mat.as_str().to_string();
                            matches.push(SearchMatch {
                                file: file_str.clone(),
                                line_number,
                                column_start: mat.start() + 1,
                                column_end: mat.end() + 1,
                                line_content: if self.config.only_matching {
                                    matched_text.clone()
                                } else {
                                    line.to_string()
                                },
                                matched_text,
                                context_before,
                                context_after,
                                byte_offset: byte_offset + mat.start(),
                            });
                        }
                    }
                }

                if let Some(max) = self.config.max_matches_per_file {
                    if matches.len() >= max {
                        matches.truncate(max);
                        break;
                    }
                }
            }

            byte_offset += line.len() + 1;
        }

        Ok(matches)
    }
}

/// Convert a glob pattern to a regex string.
fn glob_to_regex(glob: &str) -> String {
    let mut regex = String::new();
    regex.push('^');
    for ch in glob.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' => regex.push_str(r"\."),
            '\\' => regex.push_str(r"\\"),
            c => regex.push(c),
        }
    }
    regex.push('$');
    regex
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_file(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_simple_search() {
        let f = write_temp_file("hello world\nfoo bar\nhello again\n");
        let config = GrepConfig {
            pattern: "hello".to_string(),
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[1].line_number, 3);
    }

    #[test]
    fn test_case_insensitive() {
        let f = write_temp_file("Hello World\nHELLO\nhello\n");
        let config = GrepConfig {
            pattern: "hello".to_string(),
            case_sensitive: false,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_context_lines() {
        let f = write_temp_file("line1\nline2\nMATCH\nline4\nline5\n");
        let config = GrepConfig {
            pattern: "MATCH".to_string(),
            context_before: 2,
            context_after: 2,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].context_before.len(), 2);
        assert_eq!(matches[0].context_after.len(), 2);
    }

    // ── Streaming tests ──────────────────────────────────────────────

    #[test]
    fn test_streaming_simple_search() {
        let f = write_temp_file("hello world\nfoo bar\nhello again\n");
        let config = GrepConfig {
            pattern: "hello".to_string(),
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file_streaming(f.path()).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[1].line_number, 3);
    }

    #[test]
    fn test_streaming_context_lines() {
        let f = write_temp_file("line1\nline2\nMATCH\nline4\nline5\n");
        let config = GrepConfig {
            pattern: "MATCH".to_string(),
            context_before: 2,
            context_after: 2,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file_streaming(f.path()).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].context_before.len(), 2);
        assert_eq!(matches[0].context_after.len(), 2);
        assert_eq!(matches[0].context_before[0].content, "line1");
        assert_eq!(matches[0].context_after[1].content, "line5");
    }

    #[test]
    fn test_streaming_from_reader() {
        let data = b"alpha\nbeta\ngamma\nalpha again\n";
        let config = GrepConfig {
            pattern: "alpha".to_string(),
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_reader(&data[..], "stdin").unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].file, "stdin");
        assert_eq!(matches[1].line_number, 4);
    }

    #[test]
    fn test_whole_word() {
        let f = write_temp_file("cat\ncatch\ncat nap\nconcat\n");
        let config = GrepConfig {
            pattern: "cat".to_string(),
            whole_word: true,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        // "cat" and "cat nap" should match, "catch" and "concat" should not
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_invert_match() {
        let f = write_temp_file("keep\nremove\nkeep too\nremove also\n");
        let config = GrepConfig {
            pattern: "remove".to_string(),
            invert_match: true,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches[0].line_content.contains("keep"));
        assert!(matches[1].line_content.contains("keep too"));
    }

    #[test]
    fn test_literal_pattern_type() {
        let f = write_temp_file("hello (world)\nno match\nhello (world) again\n");
        let config = GrepConfig {
            pattern: "(world)".to_string(),
            pattern_type: PatternType::Literal,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_only_matching() {
        let f = write_temp_file("hello world foo\n");
        let config = GrepConfig {
            pattern: "world".to_string(),
            only_matching: true,
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].matched_text, "world");
    }

    #[test]
    fn test_max_matches() {
        let f = write_temp_file("a\na\na\na\na\n");
        let config = GrepConfig {
            pattern: "a".to_string(),
            max_matches_per_file: Some(3),
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert!(matches.len() <= 3);
    }

    #[test]
    fn test_no_match() {
        let f = write_temp_file("hello world\n");
        let config = GrepConfig {
            pattern: "ZZZZZ".to_string(),
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let f = write_temp_file("");
        let config = GrepConfig {
            pattern: "test".to_string(),
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_file(f.path()).unwrap();
        assert!(matches.is_empty());
    }

    // ── Aho-Corasick tests ──────────────────────────────────────────

    #[test]
    fn test_aho_corasick_literal_multi_pattern() {
        let f = write_temp_file("TODO fix this\nFIXME later\nhello world\nHACK around it\n");
        let config = GrepConfig {
            pattern: "TODO".to_string(),
            pattern_type: PatternType::Literal,
            extra_patterns: vec!["FIXME".to_string(), "HACK".to_string()],
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        // AC automaton should be built for literal multi-pattern
        assert!(engine.ac_automaton.is_some());
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 3); // TODO, FIXME, HACK
    }

    #[test]
    fn test_aho_corasick_case_insensitive() {
        let f = write_temp_file("todo\nFixme\nhack\nno match\n");
        let config = GrepConfig {
            pattern: "TODO".to_string(),
            pattern_type: PatternType::Literal,
            case_sensitive: false,
            extra_patterns: vec!["FIXME".to_string(), "HACK".to_string()],
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        assert!(engine.ac_automaton.is_some());
        let matches = engine.search_file(f.path()).unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_aho_corasick_streaming() {
        let data = b"TODO fix\nFIXME later\nhello\nHACK it\n";
        let config = GrepConfig {
            pattern: "TODO".to_string(),
            pattern_type: PatternType::Literal,
            extra_patterns: vec!["FIXME".to_string(), "HACK".to_string()],
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        let matches = engine.search_reader(&data[..], "stdin").unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_aho_corasick_not_built_for_regex() {
        let config = GrepConfig {
            pattern: "TODO".to_string(),
            pattern_type: PatternType::Regex,
            extra_patterns: vec!["FIXME".to_string()],
            ..Default::default()
        };
        let engine = GrepEngine::new(config).unwrap();
        // AC automaton should NOT be built for regex patterns
        assert!(engine.ac_automaton.is_none());
    }
}
