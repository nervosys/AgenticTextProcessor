//! # Archive-Aware Text Processing
//!
//! Stream entries from tar, zip, and gzip archives; search inside archives;
//! extract matching files. Pure-Rust implementations for basic formats.

use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported archive formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Tar,
    Gzip,
    Zip,
    /// Auto-detect based on magic bytes.
    Auto,
}

/// An entry within an archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Path / name of the entry within the archive.
    pub path: String,
    /// Uncompressed size in bytes (if known).
    pub size: Option<u64>,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// The raw content bytes.
    pub content: Vec<u8>,
}

impl ArchiveEntry {
    /// Get content as UTF-8 text (lossy).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.content).to_string()
    }

    /// Check if content contains a pattern.
    pub fn contains(&self, pattern: &str) -> bool {
        self.text().contains(pattern)
    }

    /// Check if content matches a regex.
    pub fn matches_regex(&self, pattern: &str) -> bool {
        regex::Regex::new(pattern)
            .map(|re| re.is_match(&self.text()))
            .unwrap_or(false)
    }
}

/// Search result within an archive.
#[derive(Debug, Clone)]
pub struct ArchiveSearchHit {
    /// Entry path.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The matching line text.
    pub content: String,
}

/// Archive processing statistics.
#[derive(Debug, Clone, Default)]
pub struct ArchiveStats {
    pub total_entries: usize,
    pub files: usize,
    pub directories: usize,
    pub total_bytes: u64,
    pub matched_entries: usize,
}

/// Errors from archive operations.
#[derive(Debug, Clone)]
pub enum ArchiveError {
    InvalidFormat(String),
    IoError(String),
    UnsupportedFormat(String),
    EntryNotFound(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::InvalidFormat(msg) => write!(f, "Invalid format: {msg}"),
            ArchiveError::IoError(msg) => write!(f, "IO error: {msg}"),
            ArchiveError::UnsupportedFormat(msg) => write!(f, "Unsupported: {msg}"),
            ArchiveError::EntryNotFound(msg) => write!(f, "Not found: {msg}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

/// Detect archive format from magic bytes.
pub fn detect_format(data: &[u8]) -> Option<ArchiveFormat> {
    if data.len() < 4 {
        return None;
    }
    // Gzip: 1f 8b
    if data[0] == 0x1f && data[1] == 0x8b {
        return Some(ArchiveFormat::Gzip);
    }
    // Zip: PK\x03\x04
    if data[0] == 0x50 && data[1] == 0x4b && data[2] == 0x03 && data[3] == 0x04 {
        return Some(ArchiveFormat::Zip);
    }
    // Tar: check for "ustar" at offset 257
    if data.len() > 262 && &data[257..262] == b"ustar" {
        return Some(ArchiveFormat::Tar);
    }
    None
}

// ---------------------------------------------------------------------------
// Tar support (simplified — handles basic POSIX tar)
// ---------------------------------------------------------------------------

/// Parse a tar archive from raw bytes.
pub fn parse_tar(data: &[u8]) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let mut entries = Vec::new();
    let mut pos = 0;

    while pos + 512 <= data.len() {
        let header = &data[pos..pos + 512];

        // Check if this is an empty block (end of archive)
        if header.iter().all(|&b| b == 0) {
            break;
        }

        // Extract filename (bytes 0..100)
        let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&header[..name_end])
            .trim()
            .to_string();

        // Extract size (bytes 124..136, octal)
        let size_str = String::from_utf8_lossy(&header[124..136])
            .trim()
            .to_string();
        let size_str = size_str.trim_matches('\0');
        let size = u64::from_str_radix(size_str.trim(), 8).unwrap_or(0);

        // File type (byte 156): '0' or '\0' = regular file, '5' = directory
        let type_flag = header[156];
        let is_dir = type_flag == b'5';

        pos += 512; // Move past header

        // Read content
        let content_end = pos + size as usize;
        let content = if content_end <= data.len() && !is_dir {
            data[pos..content_end].to_vec()
        } else {
            Vec::new()
        };

        if !name.is_empty() {
            entries.push(ArchiveEntry {
                path: name,
                size: Some(size),
                is_dir,
                content,
            });
        }

        // Advance past content (rounded up to 512-byte block)
        let blocks = (size as usize).div_ceil(512);
        pos += blocks * 512;
    }

    Ok(entries)
}

/// Create a tar archive from entries.
pub fn create_tar(entries: &[ArchiveEntry]) -> Vec<u8> {
    let mut data = Vec::new();

    for entry in entries {
        let mut header = [0u8; 512];

        // Name (bytes 0..100)
        let name_bytes = entry.path.as_bytes();
        let copy_len = name_bytes.len().min(100);
        header[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        // Mode (bytes 100..108): 0644
        header[100..107].copy_from_slice(b"0000644");

        // Size (bytes 124..136): octal
        let size = entry.content.len();
        let size_oct = format!("{:011o}", size);
        header[124..135].copy_from_slice(size_oct.as_bytes());

        // Type flag (byte 156): '0' = regular file
        header[156] = if entry.is_dir { b'5' } else { b'0' };

        // Magic (bytes 257..263): "ustar\0"
        header[257..263].copy_from_slice(b"ustar\0");

        // Compute checksum (bytes 148..156)
        // First, fill checksum field with spaces
        header[148..156].copy_from_slice(b"        ");
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let cksum_str = format!("{:06o}\0 ", checksum);
        let cksum_bytes = cksum_str.as_bytes();
        let ck_len = cksum_bytes.len().min(8);
        header[148..148 + ck_len].copy_from_slice(&cksum_bytes[..ck_len]);

        data.extend_from_slice(&header);

        // Content
        data.extend_from_slice(&entry.content);

        // Pad to 512-byte boundary
        let remainder = entry.content.len() % 512;
        if remainder > 0 {
            data.extend(std::iter::repeat(0u8).take(512 - remainder));
        }
    }

    // End-of-archive: two 512-byte blocks of zeros
    data.extend(std::iter::repeat(0u8).take(1024));
    data
}

// ---------------------------------------------------------------------------
// Gzip support (decode only, using flate2-compatible manual inflate is complex,
// so we provide a simple wrapper that works with uncompressed or pre-decoded data)
// ---------------------------------------------------------------------------

/// Simple gzip header check and payload extraction.
/// Returns the compressed payload (caller needs a deflate decoder for full decompression).
/// For this module, we support creating simple uncompressed "gzip" wrappers.
pub fn gzip_wrap(data: &[u8]) -> Vec<u8> {
    let mut out = vec![
        0x1f, // ID1
        0x8b, // ID2
        0x00, // CM = stored (no compression)
        0x00, // FLG
        0x00, 0x00, 0x00, 0x00, // MTIME
        0x00, // XFL
        0xff, // OS = unknown
    ];

    // For uncompressed data, we store blocks with BTYPE=00 (no compression)
    // This is a valid DEFLATE stream
    let chunks = data.chunks(65535);
    let chunk_vec: Vec<&[u8]> = chunks.collect();
    for (i, chunk) in chunk_vec.iter().enumerate() {
        let is_last = i == chunk_vec.len() - 1;
        out.push(if is_last { 0x01 } else { 0x00 }); // BFINAL flag
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes()); // NLEN
        out.extend_from_slice(chunk);
    }

    // CRC32 and ISIZE
    let crc = crc32(data);
    out.extend_from_slice(&crc.to_le_bytes());
    let isize = (data.len() as u32).to_le_bytes();
    out.extend_from_slice(&isize);

    out
}

/// Simple CRC32 (used for gzip footer).
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Search inside archives
// ---------------------------------------------------------------------------

/// Search for a text pattern inside all text entries of an archive.
pub fn search_archive(entries: &[ArchiveEntry], pattern: &str) -> Vec<ArchiveSearchHit> {
    let mut hits = Vec::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let text = entry.text();
        for (i, line) in text.lines().enumerate() {
            if line.contains(pattern) {
                hits.push(ArchiveSearchHit {
                    path: entry.path.clone(),
                    line: i + 1,
                    content: line.to_string(),
                });
            }
        }
    }
    hits
}

/// Search using regex inside all text entries.
pub fn search_archive_regex(entries: &[ArchiveEntry], pattern: &str) -> Vec<ArchiveSearchHit> {
    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut hits = Vec::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let text = entry.text();
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                hits.push(ArchiveSearchHit {
                    path: entry.path.clone(),
                    line: i + 1,
                    content: line.to_string(),
                });
            }
        }
    }
    hits
}

/// Extract entries matching a glob-like pattern.
pub fn extract_matching(entries: &[ArchiveEntry], pattern: &str) -> Vec<ArchiveEntry> {
    entries
        .iter()
        .filter(|e| glob_match(pattern, &e.path))
        .cloned()
        .collect()
}

/// Very simple glob matcher supporting `*` and `?`.
fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_recursive(
        &pattern.chars().collect::<Vec<_>>(),
        0,
        &text.chars().collect::<Vec<_>>(),
        0,
    )
}

fn glob_match_recursive(pat: &[char], pi: usize, txt: &[char], ti: usize) -> bool {
    if pi == pat.len() && ti == txt.len() {
        return true;
    }
    if pi == pat.len() {
        return false;
    }

    match pat[pi] {
        '*' => {
            // Try matching zero or more characters
            for skip in 0..=(txt.len() - ti) {
                if glob_match_recursive(pat, pi + 1, txt, ti + skip) {
                    return true;
                }
            }
            false
        }
        '?' => {
            if ti < txt.len() {
                glob_match_recursive(pat, pi + 1, txt, ti + 1)
            } else {
                false
            }
        }
        c => {
            if ti < txt.len() && txt[ti] == c {
                glob_match_recursive(pat, pi + 1, txt, ti + 1)
            } else {
                false
            }
        }
    }
}

/// Compute archive statistics.
pub fn archive_stats(entries: &[ArchiveEntry]) -> ArchiveStats {
    let mut stats = ArchiveStats {
        total_entries: entries.len(),
        ..Default::default()
    };
    for e in entries {
        if e.is_dir {
            stats.directories += 1;
        } else {
            stats.files += 1;
        }
        stats.total_bytes += e.size.unwrap_or(e.content.len() as u64);
    }
    stats
}

/// List all entry paths in an archive.
pub fn list_entries(entries: &[ArchiveEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.path.as_str()).collect()
}

/// Group entries by file extension.
pub fn group_by_extension(entries: &[ArchiveEntry]) -> BTreeMap<String, Vec<&ArchiveEntry>> {
    let mut map: BTreeMap<String, Vec<&ArchiveEntry>> = BTreeMap::new();
    for e in entries {
        let ext = e
            .path
            .rsplit('.')
            .next()
            .map(|s| format!(".{s}"))
            .unwrap_or_else(|| "(none)".to_string());
        map.entry(ext).or_default().push(e);
    }
    map
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<ArchiveEntry> {
        vec![
            ArchiveEntry {
                path: "readme.txt".into(),
                size: Some(13),
                is_dir: false,
                content: b"Hello, world!".to_vec(),
            },
            ArchiveEntry {
                path: "src/main.rs".into(),
                size: Some(22),
                is_dir: false,
                content: b"fn main() { // TODO }\n".to_vec(),
            },
            ArchiveEntry {
                path: "src/".into(),
                size: Some(0),
                is_dir: true,
                content: Vec::new(),
            },
            ArchiveEntry {
                path: "data.csv".into(),
                size: Some(9),
                is_dir: false,
                content: b"a,b,c\n1,2,3".to_vec(),
            },
        ]
    }

    #[test]
    fn test_entry_text() {
        let e = &sample_entries()[0];
        assert_eq!(e.text(), "Hello, world!");
    }

    #[test]
    fn test_entry_contains() {
        let e = &sample_entries()[1];
        assert!(e.contains("TODO"));
        assert!(!e.contains("FIXME"));
    }

    #[test]
    fn test_search_archive() {
        let entries = sample_entries();
        let hits = search_archive(&entries, "TODO");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "src/main.rs");
    }

    #[test]
    fn test_search_regex() {
        let entries = sample_entries();
        let hits = search_archive_regex(&entries, r"fn\s+main");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.txt", "readme.txt"));
        assert!(glob_match("src/*.rs", "src/main.rs"));
        assert!(!glob_match("*.rs", "readme.txt"));
        assert!(glob_match("?ata.csv", "data.csv"));
    }

    #[test]
    fn test_extract_matching() {
        let entries = sample_entries();
        let rs_files = extract_matching(&entries, "*.rs");
        assert_eq!(rs_files.len(), 1);
        assert_eq!(rs_files[0].path, "src/main.rs");
    }

    #[test]
    fn test_archive_stats() {
        let entries = sample_entries();
        let stats = archive_stats(&entries);
        assert_eq!(stats.total_entries, 4);
        assert_eq!(stats.files, 3);
        assert_eq!(stats.directories, 1);
    }

    #[test]
    fn test_list_entries() {
        let entries = sample_entries();
        let paths = list_entries(&entries);
        assert_eq!(paths.len(), 4);
        assert!(paths.contains(&"readme.txt"));
    }

    #[test]
    fn test_group_by_extension() {
        let entries = sample_entries();
        let groups = group_by_extension(&entries);
        assert!(groups.contains_key(".txt"));
        assert!(groups.contains_key(".rs"));
        assert!(groups.contains_key(".csv"));
    }

    #[test]
    fn test_tar_roundtrip() {
        let entries = vec![
            ArchiveEntry {
                path: "hello.txt".into(),
                size: Some(5),
                is_dir: false,
                content: b"hello".to_vec(),
            },
            ArchiveEntry {
                path: "world.txt".into(),
                size: Some(5),
                is_dir: false,
                content: b"world".to_vec(),
            },
        ];
        let tar_data = create_tar(&entries);
        let parsed = parse_tar(&tar_data).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path, "hello.txt");
        assert_eq!(parsed[0].content, b"hello");
        assert_eq!(parsed[1].path, "world.txt");
    }

    #[test]
    fn test_detect_gzip() {
        let data = gzip_wrap(b"test data");
        assert_eq!(detect_format(&data), Some(ArchiveFormat::Gzip));
    }

    #[test]
    fn test_crc32() {
        // Known CRC32 of empty data
        assert_eq!(crc32(b""), 0x0000_0000);
        // CRC32 of "123456789" should be 0xCBF43926
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn test_detect_tar() {
        let entries = vec![ArchiveEntry {
            path: "a.txt".into(),
            size: Some(1),
            is_dir: false,
            content: b"x".to_vec(),
        }];
        let tar = create_tar(&entries);
        assert_eq!(detect_format(&tar), Some(ArchiveFormat::Tar));
    }
}
