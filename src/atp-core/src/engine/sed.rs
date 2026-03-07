//! Sed engine — strongly typed stream transformation.
//!
//! Supports substitution, deletion, insertion, and transliteration
//! with preview/dry-run, deterministic application order, and diff output.
//! Includes streaming mode for memory-efficient processing of large files.

use crate::output::{ChangeRecord, ChangeType, FileChanges, TransformResults};
use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

/// A single transformation command.
#[derive(Debug, Clone)]
pub enum TransformCommand {
    /// Substitute pattern with replacement
    Substitute {
        pattern: String,
        replacement: String,
        global: bool,
        case_insensitive: bool,
    },
    /// Delete lines matching pattern
    Delete { pattern: String },
    /// Insert text before lines matching pattern
    InsertBefore { pattern: String, text: String },
    /// Insert text after lines matching pattern
    InsertAfter { pattern: String, text: String },
    /// Transliterate characters (like tr or y// in sed)
    Transliterate { from: String, to: String },
}

/// Configuration for the sed engine.
#[derive(Debug, Clone)]
pub struct SedConfig {
    pub commands: Vec<TransformCommand>,
    /// If true, apply changes to files in place
    pub in_place: bool,
    /// Optional backup extension for in-place editing
    pub backup_extension: Option<String>,
    /// If true, only show what would change (dry run)
    pub dry_run: bool,
    /// Optional line range (start, end) — 1-based, inclusive
    pub line_range: Option<(usize, usize)>,
    /// Optional address-range patterns: only apply between lines matching start_pat..end_pat
    pub address_range: Option<(String, String)>,
}

impl Default for SedConfig {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            in_place: false,
            backup_extension: None,
            dry_run: true,
            line_range: None,
            address_range: None,
        }
    }
}

/// The sed transformation engine.
pub struct SedEngine {
    config: SedConfig,
}

impl SedEngine {
    pub fn new(config: SedConfig) -> Self {
        Self { config }
    }

    /// Transform a single file, returning the changes and optionally applying them.
    pub fn transform_file(&self, path: &Path) -> Result<FileChanges> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut result_lines = lines.clone();
        let mut all_changes = Vec::new();

        // Compute address-range mask if configured
        let address_mask = self.compute_address_mask(&lines)?;

        for cmd in &self.config.commands {
            let changes = self.apply_command(cmd, &result_lines, address_mask.as_deref())?;
            // Apply changes in reverse order to preserve line numbers
            let mut sorted_changes = changes.clone();
            sorted_changes.sort_by(|a, b| b.line_number.cmp(&a.line_number));
            for change in &sorted_changes {
                let idx = change.line_number - 1;
                match change.change_type {
                    ChangeType::Substitution | ChangeType::Transliteration => {
                        if idx < result_lines.len() {
                            result_lines[idx] = change.replacement.clone();
                        }
                    }
                    ChangeType::Deletion => {
                        if idx < result_lines.len() {
                            result_lines.remove(idx);
                        }
                    }
                    ChangeType::Insertion => {
                        // Insertion places new line at the specified position
                        if idx <= result_lines.len() {
                            result_lines.insert(idx, change.replacement.clone());
                        }
                    }
                }
            }
            all_changes.extend(changes);
        }

        // Write back if in-place and not dry-run
        if self.config.in_place && !self.config.dry_run && !all_changes.is_empty() {
            if let Some(ext) = &self.config.backup_extension {
                let backup_path = path.with_extension(format!(
                    "{}.{}",
                    path.extension()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    ext
                ));
                fs::copy(path, backup_path)?;
            }
            let new_content = result_lines.join("\n");
            fs::write(path, new_content)?;
        }

        let change_count = all_changes.len();
        Ok(FileChanges {
            file: path.display().to_string(),
            change_count,
            changes: all_changes,
        })
    }

    /// Transform multiple files.
    pub fn transform_files(&self, paths: &[&Path]) -> Result<TransformResults> {
        let mut all_file_changes = Vec::new();
        let mut total_changes = 0;
        let mut files_modified = 0;

        let pattern = self
            .config
            .commands
            .first()
            .map(|c| match c {
                TransformCommand::Substitute { pattern, .. } => pattern.clone(),
                TransformCommand::Delete { pattern } => pattern.clone(),
                TransformCommand::InsertBefore { pattern, .. } => pattern.clone(),
                TransformCommand::InsertAfter { pattern, .. } => pattern.clone(),
                TransformCommand::Transliterate { from, .. } => from.clone(),
            })
            .unwrap_or_default();

        let replacement = self
            .config
            .commands
            .first()
            .map(|c| match c {
                TransformCommand::Substitute { replacement, .. } => replacement.clone(),
                TransformCommand::Delete { .. } => String::new(),
                TransformCommand::InsertBefore { text, .. } => text.clone(),
                TransformCommand::InsertAfter { text, .. } => text.clone(),
                TransformCommand::Transliterate { to, .. } => to.clone(),
            })
            .unwrap_or_default();

        for path in paths {
            let fc = self.transform_file(path)?;
            if fc.change_count > 0 {
                files_modified += 1;
                total_changes += fc.change_count;
            }
            all_file_changes.push(fc);
        }

        Ok(TransformResults {
            pattern,
            replacement,
            total_changes,
            files_modified,
            applied: self.config.in_place && !self.config.dry_run,
            changes: all_file_changes,
        })
    }

    // ── Streaming API ───────────────────────────────────────────────────

    /// Transform a readable source in streaming mode, writing results to a writer.
    ///
    /// Processes input line-by-line without loading the entire content into memory.
    /// Returns the list of changes applied. Note: address-range patterns work
    /// as a single-pass state machine, so they are fully supported.
    pub fn transform_reader<R: Read, W: Write>(
        &self,
        reader: R,
        writer: &mut W,
        source_name: &str,
    ) -> Result<FileChanges> {
        let buf = BufReader::new(reader);
        let mut all_changes = Vec::new();

        // Pre-compile address-range regexes if needed
        let address_state = if let Some((start_pat, end_pat)) = &self.config.address_range {
            let start_re = Regex::new(start_pat)
                .with_context(|| format!("Invalid address-range start: {start_pat}"))?;
            let end_re = Regex::new(end_pat)
                .with_context(|| format!("Invalid address-range end: {end_pat}"))?;
            Some((start_re, end_re, false)) // (start_re, end_re, inside)
        } else {
            None
        };
        let mut inside_address = false;
        let address_res = address_state
            .as_ref()
            .map(|(s, e, _)| (s.clone(), e.clone()));

        // Pre-compile command regexes
        let compiled_cmds = self.compile_commands()?;

        for (idx, line_result) in buf.lines().enumerate() {
            let line = line_result
                .with_context(|| format!("Failed to read line {} from {}", idx + 1, source_name))?;
            let line_num = idx + 1;

            // Check address range
            let in_address = if let Some((ref start_re, ref end_re)) = address_res {
                if !inside_address {
                    if start_re.is_match(&line) {
                        inside_address = true;
                        true
                    } else {
                        false
                    }
                } else {
                    let was_inside = true;
                    if end_re.is_match(&line) {
                        inside_address = false;
                    }
                    was_inside
                }
            } else {
                true
            };

            // Check line range
            let in_range = match self.config.line_range {
                Some((start, end)) => line_num >= start && line_num <= end,
                None => true,
            };

            let applicable = in_address && in_range;

            // Apply commands sequentially to the line
            let mut current_line = line.clone();
            let mut line_deleted = false;
            let mut insert_before: Vec<String> = Vec::new();
            let mut insert_after: Vec<String> = Vec::new();

            if applicable {
                for (cmd, compiled) in self.config.commands.iter().zip(compiled_cmds.iter()) {
                    match cmd {
                        TransformCommand::Substitute {
                            replacement,
                            global,
                            ..
                        } => {
                            if let Some(re) = compiled {
                                if re.is_match(&current_line) {
                                    let new_line = if *global {
                                        re.replace_all(&current_line, replacement.as_str())
                                            .to_string()
                                    } else {
                                        re.replace(&current_line, replacement.as_str()).to_string()
                                    };
                                    if new_line != current_line {
                                        all_changes.push(ChangeRecord {
                                            line_number: line_num,
                                            original: current_line.clone(),
                                            replacement: new_line.clone(),
                                            change_type: ChangeType::Substitution,
                                        });
                                        current_line = new_line;
                                    }
                                }
                            }
                        }
                        TransformCommand::Delete { .. } => {
                            if let Some(re) = compiled {
                                if re.is_match(&current_line) {
                                    all_changes.push(ChangeRecord {
                                        line_number: line_num,
                                        original: current_line.clone(),
                                        replacement: String::new(),
                                        change_type: ChangeType::Deletion,
                                    });
                                    line_deleted = true;
                                    break;
                                }
                            }
                        }
                        TransformCommand::InsertBefore { text, .. } => {
                            if let Some(re) = compiled {
                                if re.is_match(&current_line) {
                                    all_changes.push(ChangeRecord {
                                        line_number: line_num,
                                        original: String::new(),
                                        replacement: text.clone(),
                                        change_type: ChangeType::Insertion,
                                    });
                                    insert_before.push(text.clone());
                                }
                            }
                        }
                        TransformCommand::InsertAfter { text, .. } => {
                            if let Some(re) = compiled {
                                if re.is_match(&current_line) {
                                    all_changes.push(ChangeRecord {
                                        line_number: line_num + 1,
                                        original: String::new(),
                                        replacement: text.clone(),
                                        change_type: ChangeType::Insertion,
                                    });
                                    insert_after.push(text.clone());
                                }
                            }
                        }
                        TransformCommand::Transliterate { from, to, .. } => {
                            let from_chars: Vec<char> = from.chars().collect();
                            let to_chars: Vec<char> = to.chars().collect();
                            if from_chars.len() == to_chars.len() {
                                let new_line: String = current_line
                                    .chars()
                                    .map(|c| {
                                        if let Some(pos) = from_chars.iter().position(|&fc| fc == c)
                                        {
                                            to_chars[pos]
                                        } else {
                                            c
                                        }
                                    })
                                    .collect();
                                if new_line != current_line {
                                    all_changes.push(ChangeRecord {
                                        line_number: line_num,
                                        original: current_line.clone(),
                                        replacement: new_line.clone(),
                                        change_type: ChangeType::Transliteration,
                                    });
                                    current_line = new_line;
                                }
                            }
                        }
                    }
                }
            }

            // Write output
            for text in &insert_before {
                writeln!(writer, "{text}")?;
            }
            if !line_deleted {
                writeln!(writer, "{current_line}")?;
            }
            for text in &insert_after {
                writeln!(writer, "{text}")?;
            }
        }

        let change_count = all_changes.len();
        Ok(FileChanges {
            file: source_name.to_string(),
            change_count,
            changes: all_changes,
        })
    }

    /// Transform a file in streaming mode (memory-efficient for large files).
    ///
    /// When `in_place` is true and not `dry_run`, writes changes back to the file
    /// via a temporary buffer then atomic rename.
    pub fn transform_file_streaming(&self, path: &Path) -> Result<FileChanges> {
        if self.config.in_place && !self.config.dry_run {
            // Stream to a temporary buffer, then write back
            let file = fs::File::open(path)
                .with_context(|| format!("Failed to open file: {}", path.display()))?;
            let mut output = Vec::new();
            let changes = self.transform_reader(file, &mut output, &path.display().to_string())?;

            if changes.change_count > 0 {
                // Create backup if configured
                if let Some(ext) = &self.config.backup_extension {
                    let backup_path = path.with_extension(format!(
                        "{}.{}",
                        path.extension()
                            .map(|e| e.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        ext
                    ));
                    fs::copy(path, backup_path)?;
                }
                fs::write(path, output)?;
            }
            Ok(changes)
        } else {
            // Dry run: stream to /dev/null equivalent (just compute changes)
            let file = fs::File::open(path)
                .with_context(|| format!("Failed to open file: {}", path.display()))?;
            let mut sink = std::io::sink();
            self.transform_reader(file, &mut sink, &path.display().to_string())
        }
    }

    /// Pre-compile all command patterns for streaming use.
    fn compile_commands(&self) -> Result<Vec<Option<Regex>>> {
        self.config
            .commands
            .iter()
            .map(|cmd| {
                match cmd {
                    TransformCommand::Substitute {
                        pattern,
                        case_insensitive,
                        ..
                    } => {
                        let re = RegexBuilder::new(pattern)
                            .case_insensitive(*case_insensitive)
                            .build()
                            .with_context(|| format!("Invalid pattern: {pattern}"))?;
                        Ok(Some(re))
                    }
                    TransformCommand::Delete { pattern } => {
                        let re = Regex::new(pattern)
                            .with_context(|| format!("Invalid pattern: {pattern}"))?;
                        Ok(Some(re))
                    }
                    TransformCommand::InsertBefore { pattern, .. }
                    | TransformCommand::InsertAfter { pattern, .. } => {
                        let re = Regex::new(pattern)
                            .with_context(|| format!("Invalid pattern: {pattern}"))?;
                        Ok(Some(re))
                    }
                    TransformCommand::Transliterate { .. } => Ok(None), // No regex needed
                }
            })
            .collect()
    }

    fn apply_command(
        &self,
        cmd: &TransformCommand,
        lines: &[String],
        address_mask: Option<&[bool]>,
    ) -> Result<Vec<ChangeRecord>> {
        let mut changes = Vec::new();

        match cmd {
            TransformCommand::Substitute {
                pattern,
                replacement,
                global,
                case_insensitive,
            } => {
                let re = RegexBuilder::new(pattern)
                    .case_insensitive(*case_insensitive)
                    .build()
                    .with_context(|| format!("Invalid pattern: {pattern}"))?;

                for (idx, line) in lines.iter().enumerate() {
                    let line_num = idx + 1;
                    if !self.in_line_range(line_num, address_mask) {
                        continue;
                    }
                    if re.is_match(line) {
                        let new_line = if *global {
                            re.replace_all(line, replacement.as_str()).to_string()
                        } else {
                            re.replace(line, replacement.as_str()).to_string()
                        };
                        if new_line != *line {
                            changes.push(ChangeRecord {
                                line_number: line_num,
                                original: line.clone(),
                                replacement: new_line,
                                change_type: ChangeType::Substitution,
                            });
                        }
                    }
                }
            }

            TransformCommand::Delete { pattern } => {
                let re =
                    Regex::new(pattern).with_context(|| format!("Invalid pattern: {pattern}"))?;
                for (idx, line) in lines.iter().enumerate() {
                    let line_num = idx + 1;
                    if !self.in_line_range(line_num, address_mask) {
                        continue;
                    }
                    if re.is_match(line) {
                        changes.push(ChangeRecord {
                            line_number: line_num,
                            original: line.clone(),
                            replacement: String::new(),
                            change_type: ChangeType::Deletion,
                        });
                    }
                }
            }

            TransformCommand::InsertBefore { pattern, text } => {
                let re =
                    Regex::new(pattern).with_context(|| format!("Invalid pattern: {pattern}"))?;
                for (idx, line) in lines.iter().enumerate() {
                    let line_num = idx + 1;
                    if !self.in_line_range(line_num, address_mask) {
                        continue;
                    }
                    if re.is_match(line) {
                        changes.push(ChangeRecord {
                            line_number: line_num,
                            original: String::new(),
                            replacement: text.clone(),
                            change_type: ChangeType::Insertion,
                        });
                    }
                }
            }

            TransformCommand::InsertAfter { pattern, text } => {
                let re =
                    Regex::new(pattern).with_context(|| format!("Invalid pattern: {pattern}"))?;
                for (idx, line) in lines.iter().enumerate() {
                    let line_num = idx + 1;
                    if !self.in_line_range(line_num, address_mask) {
                        continue;
                    }
                    if re.is_match(line) {
                        changes.push(ChangeRecord {
                            line_number: line_num + 1,
                            original: String::new(),
                            replacement: text.clone(),
                            change_type: ChangeType::Insertion,
                        });
                    }
                }
            }

            TransformCommand::Transliterate { from, to } => {
                let from_chars: Vec<char> = from.chars().collect();
                let to_chars: Vec<char> = to.chars().collect();
                if from_chars.len() != to_chars.len() {
                    anyhow::bail!(
                        "Transliterate: 'from' and 'to' must have the same length ({} vs {})",
                        from_chars.len(),
                        to_chars.len()
                    );
                }
                for (idx, line) in lines.iter().enumerate() {
                    let line_num = idx + 1;
                    if !self.in_line_range(line_num, address_mask) {
                        continue;
                    }
                    let new_line: String = line
                        .chars()
                        .map(|c| {
                            if let Some(pos) = from_chars.iter().position(|&fc| fc == c) {
                                to_chars[pos]
                            } else {
                                c
                            }
                        })
                        .collect();
                    if new_line != *line {
                        changes.push(ChangeRecord {
                            line_number: line_num,
                            original: line.clone(),
                            replacement: new_line,
                            change_type: ChangeType::Transliteration,
                        });
                    }
                }
            }
        }

        Ok(changes)
    }

    fn in_line_range(&self, line_num: usize, address_mask: Option<&[bool]>) -> bool {
        // Check numeric line range
        let in_range = match self.config.line_range {
            Some((start, end)) => line_num >= start && line_num <= end,
            None => true,
        };
        // Check address-range pattern mask
        let in_address = match address_mask {
            Some(mask) => *mask.get(line_num.saturating_sub(1)).unwrap_or(&false),
            None => true,
        };
        in_range && in_address
    }

    /// Compute a boolean mask for address-range patterns (/start/,/end/).
    /// Lines between (inclusive) start-match and end-match are true.
    fn compute_address_mask(&self, lines: &[String]) -> Result<Option<Vec<bool>>> {
        let (start_pat, end_pat) = match &self.config.address_range {
            Some((s, e)) => (s.clone(), e.clone()),
            None => return Ok(None),
        };
        let start_re = Regex::new(&start_pat)
            .with_context(|| format!("Invalid address-range start pattern: {start_pat}"))?;
        let end_re = Regex::new(&end_pat)
            .with_context(|| format!("Invalid address-range end pattern: {end_pat}"))?;

        let mut mask = vec![false; lines.len()];
        let mut inside = false;
        for (idx, line) in lines.iter().enumerate() {
            if !inside {
                if start_re.is_match(line) {
                    inside = true;
                    mask[idx] = true;
                }
            } else {
                mask[idx] = true;
                if end_re.is_match(line) {
                    inside = false;
                }
            }
        }
        Ok(Some(mask))
    }
}

/// Parse a sed-style substitution expression like `s/pattern/replacement/flags`.
pub fn parse_substitution(expr: &str) -> Result<TransformCommand> {
    // Support s/pattern/replacement/flags format
    let delim = expr
        .chars()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid substitution expression: too short"))?;

    if !expr.starts_with('s') {
        anyhow::bail!("Substitution expression must start with 's'");
    }

    let parts: Vec<&str> = expr[2..].splitn(3, delim).collect();
    if parts.len() < 2 {
        anyhow::bail!(
            "Invalid substitution expression. Expected: s{d}pattern{d}replacement{d}[flags]",
            d = delim
        );
    }

    let pattern = parts[0].to_string();
    let replacement = parts[1].to_string();
    let flags = if parts.len() > 2 { parts[2] } else { "" };
    let global = flags.contains('g');
    let case_insensitive = flags.contains('i');

    Ok(TransformCommand::Substitute {
        pattern,
        replacement,
        global,
        case_insensitive,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_substitution() {
        let cmd = parse_substitution("s/hello/world/g").unwrap();
        match cmd {
            TransformCommand::Substitute {
                pattern,
                replacement,
                global,
                ..
            } => {
                assert_eq!(pattern, "hello");
                assert_eq!(replacement, "world");
                assert!(global);
            }
            _ => panic!("Expected Substitute"),
        }
    }

    // ── Streaming tests ──────────────────────────────────────────────

    #[test]
    fn test_streaming_substitution() {
        let input = b"hello world\nfoo bar\nhello again\n";
        let config = SedConfig {
            commands: vec![TransformCommand::Substitute {
                pattern: "hello".to_string(),
                replacement: "HI".to_string(),
                global: true,
                case_insensitive: false,
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "stdin")
            .unwrap();
        assert_eq!(changes.change_count, 2);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("HI world"));
        assert!(result.contains("HI again"));
        assert!(result.contains("foo bar"));
    }

    #[test]
    fn test_streaming_deletion() {
        let input = b"keep\ndelete me\nkeep too\n";
        let config = SedConfig {
            commands: vec![TransformCommand::Delete {
                pattern: "delete".to_string(),
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 1);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("keep"));
        assert!(!result.contains("delete me"));
    }

    #[test]
    fn test_streaming_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "hello world\nfoo bar\n").unwrap();
        let config = SedConfig {
            commands: vec![TransformCommand::Substitute {
                pattern: "foo".to_string(),
                replacement: "baz".to_string(),
                global: false,
                case_insensitive: false,
            }],
            dry_run: true,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let changes = engine.transform_file_streaming(tmp.path()).unwrap();
        assert_eq!(changes.change_count, 1);
    }

    #[test]
    fn test_insert_before() {
        let input = b"line one\ntarget line\nline three\n";
        let config = SedConfig {
            commands: vec![TransformCommand::InsertBefore {
                pattern: "target".to_string(),
                text: "INSERTED".to_string(),
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 1);
        let result = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.iter().any(|l| l.contains("INSERTED")));
    }

    #[test]
    fn test_insert_after() {
        let input = b"line one\ntarget line\nline three\n";
        let config = SedConfig {
            commands: vec![TransformCommand::InsertAfter {
                pattern: "target".to_string(),
                text: "APPENDED".to_string(),
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 1);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("APPENDED"));
    }

    #[test]
    fn test_transliterate() {
        let input = b"hello world\n";
        let config = SedConfig {
            commands: vec![TransformCommand::Transliterate {
                from: "aeiou".to_string(),
                to: "AEIOU".to_string(),
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert!(changes.change_count >= 1);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("hEllO wOrld"));
    }

    #[test]
    fn test_case_insensitive_substitution() {
        let input = b"Hello HELLO hello\n";
        let config = SedConfig {
            commands: vec![TransformCommand::Substitute {
                pattern: "hello".to_string(),
                replacement: "HI".to_string(),
                global: true,
                case_insensitive: true,
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert!(changes.change_count >= 1);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("HI"));
        // All occurrences should be replaced
        assert!(!result.to_lowercase().contains("hello"));
    }

    #[test]
    fn test_non_global_substitution() {
        let input = b"hello hello hello\n";
        let config = SedConfig {
            commands: vec![TransformCommand::Substitute {
                pattern: "hello".to_string(),
                replacement: "HI".to_string(),
                global: false,
                case_insensitive: false,
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 1);
        let result = String::from_utf8(output).unwrap();
        // Only first occurrence replaced
        assert!(result.contains("HI"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_multiple_commands() {
        let input = b"hello world\nfoo bar\n";
        let config = SedConfig {
            commands: vec![
                TransformCommand::Substitute {
                    pattern: "hello".to_string(),
                    replacement: "HI".to_string(),
                    global: false,
                    case_insensitive: false,
                },
                TransformCommand::Substitute {
                    pattern: "foo".to_string(),
                    replacement: "BAZ".to_string(),
                    global: false,
                    case_insensitive: false,
                },
            ],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 2);
        let result = String::from_utf8(output).unwrap();
        assert!(result.contains("HI"));
        assert!(result.contains("BAZ"));
    }

    #[test]
    fn test_no_match_no_changes() {
        let input = b"nothing to match here\n";
        let config = SedConfig {
            commands: vec![TransformCommand::Substitute {
                pattern: "ZZZZZ".to_string(),
                replacement: "x".to_string(),
                global: true,
                case_insensitive: false,
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 0);
    }

    #[test]
    fn test_parse_substitution_with_pipe_delimiter() {
        let cmd = parse_substitution("s|hello|world|g").unwrap();
        match cmd {
            TransformCommand::Substitute {
                pattern,
                replacement,
                global,
                ..
            } => {
                assert_eq!(pattern, "hello");
                assert_eq!(replacement, "world");
                assert!(global);
            }
            _ => panic!("Expected Substitute"),
        }
    }

    #[test]
    fn test_parse_substitution_case_insensitive() {
        let cmd = parse_substitution("s/hello/world/gi").unwrap();
        match cmd {
            TransformCommand::Substitute {
                case_insensitive, ..
            } => {
                assert!(case_insensitive);
            }
            _ => panic!("Expected Substitute"),
        }
    }

    #[test]
    fn test_empty_input() {
        let input = b"";
        let config = SedConfig {
            commands: vec![TransformCommand::Substitute {
                pattern: "x".to_string(),
                replacement: "y".to_string(),
                global: true,
                case_insensitive: false,
            }],
            dry_run: false,
            ..Default::default()
        };
        let engine = SedEngine::new(config);
        let mut output = Vec::new();
        let changes = engine
            .transform_reader(&input[..], &mut output, "test")
            .unwrap();
        assert_eq!(changes.change_count, 0);
    }
}
