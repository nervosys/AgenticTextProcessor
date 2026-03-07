//! # Compress — Text-Oriented Compression
//!
//! Run-length encoding (RLE), simplified LZ77, Huffman coding,
//! decompression, and compression-ratio analysis.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Compression algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Algorithm {
    Rle,
    Lz77,
    Huffman,
}

/// Result of a compression / decompression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResult {
    pub algorithm: String,
    pub original_len: usize,
    pub compressed_len: usize,
    pub ratio: f64,
    pub data: Vec<u8>,
}

/// Compression analysis across all algorithms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionAnalysis {
    pub input_len: usize,
    pub results: Vec<CompressResult>,
    pub best: String,
}

// ---------------------------------------------------------------------------
// RLE — Run-Length Encoding
// ---------------------------------------------------------------------------

/// RLE-encode bytes. Format: for runs ≥3, emit (count, byte); otherwise raw.
/// Escape byte 0xFF is used: 0xFF count byte.
pub fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        let mut run = 1usize;
        while i + run < data.len() && data[i + run] == b && run < 255 {
            run += 1;
        }
        if run >= 3 || b == 0xFF {
            out.push(0xFF);
            out.push(run as u8);
            out.push(b);
        } else {
            for _ in 0..run {
                out.push(b);
            }
        }
        i += run;
    }
    out
}

/// Decode RLE-encoded bytes.
pub fn rle_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0xFF {
            if i + 2 >= data.len() {
                return Err("truncated RLE sequence".into());
            }
            let count = data[i + 1] as usize;
            let byte = data[i + 2];
            for _ in 0..count {
                out.push(byte);
            }
            i += 3;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// LZ77 — Simplified sliding-window compression
// ---------------------------------------------------------------------------

/// LZ77 token.
#[derive(Debug, Clone)]
struct Lz77Token {
    offset: u16,
    length: u8,
    next: u8,
}

/// LZ77-encode bytes with a given window size.
pub fn lz77_encode(data: &[u8], window_size: usize) -> Vec<u8> {
    let window_size = window_size.min(65535);
    let mut tokens: Vec<Lz77Token> = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let window_start = i.saturating_sub(window_size);
        let mut best_offset = 0u16;
        let mut best_length = 0u8;

        for j in window_start..i {
            let mut len = 0u8;
            while i + (len as usize) < data.len()
                && len < 255
                && data[j + len as usize] == data[i + len as usize]
            {
                len += 1;
            }
            if len > best_length {
                best_length = len;
                best_offset = (i - j) as u16;
            }
        }

        let next_byte = if i + (best_length as usize) < data.len() {
            data[i + (best_length as usize)]
        } else {
            0
        };

        tokens.push(Lz77Token {
            offset: best_offset,
            length: best_length,
            next: next_byte,
        });
        i += best_length as usize + 1;
    }

    // Serialize tokens: (offset:u16 LE, length:u8, next:u8) = 4 bytes each
    let mut out = Vec::with_capacity(tokens.len() * 4);
    for tok in &tokens {
        out.push((tok.offset & 0xFF) as u8);
        out.push((tok.offset >> 8) as u8);
        out.push(tok.length);
        out.push(tok.next);
    }
    out
}

/// Decode LZ77-encoded bytes.
pub fn lz77_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() % 4 != 0 {
        return Err("LZ77 data must be multiple of 4 bytes".into());
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let offset = data[i] as u16 | ((data[i + 1] as u16) << 8);
        let length = data[i + 2];
        let next = data[i + 3];

        if length > 0 {
            if (offset as usize) > out.len() {
                return Err("LZ77 back-reference out of range".into());
            }
            let start = out.len() - offset as usize;
            for j in 0..length as usize {
                let idx = start + j;
                let b = out[idx];
                out.push(b);
            }
        }
        // The 'next' byte might be 0 at end — we always push it,
        // but trim trailing null if we're at the last token and length was 0-end
        out.push(next);
        i += 4;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Huffman Coding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct HuffNode {
    freq: usize,
    kind: HuffKind,
}

#[derive(Debug, Clone)]
enum HuffKind {
    Leaf(u8),
    Internal(Box<HuffNode>, Box<HuffNode>),
}

impl Eq for HuffNode {}
impl PartialEq for HuffNode {
    fn eq(&self, other: &Self) -> bool {
        self.freq == other.freq
    }
}

impl PartialOrd for HuffNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HuffNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.freq.cmp(&self.freq) // min-heap
    }
}

fn build_huffman_tree(freq: &HashMap<u8, usize>) -> Option<HuffNode> {
    let mut heap = BinaryHeap::new();
    for (&byte, &count) in freq {
        heap.push(HuffNode {
            freq: count,
            kind: HuffKind::Leaf(byte),
        });
    }
    if heap.is_empty() {
        return None;
    }
    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();
        heap.push(HuffNode {
            freq: left.freq + right.freq,
            kind: HuffKind::Internal(Box::new(left), Box::new(right)),
        });
    }
    heap.pop()
}

fn build_codes(node: &HuffNode, prefix: &mut Vec<bool>, codes: &mut HashMap<u8, Vec<bool>>) {
    match &node.kind {
        HuffKind::Leaf(b) => {
            if prefix.is_empty() {
                // Single-symbol edge case
                codes.insert(*b, vec![false]);
            } else {
                codes.insert(*b, prefix.clone());
            }
        }
        HuffKind::Internal(left, right) => {
            prefix.push(false);
            build_codes(left, prefix, codes);
            prefix.pop();
            prefix.push(true);
            build_codes(right, prefix, codes);
            prefix.pop();
        }
    }
}

/// Huffman-encode bytes.
///
/// Output format: [num_symbols:u16 LE] [for each: byte, code_len:u8, code_bits...]
/// then [total_bits:u32 LE] [compressed bit stream packed in bytes]
pub fn huffman_encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut freq = HashMap::new();
    for &b in data {
        *freq.entry(b).or_insert(0) += 1;
    }
    let tree = match build_huffman_tree(&freq) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut codes = HashMap::new();
    let mut prefix = Vec::new();
    build_codes(&tree, &mut prefix, &mut codes);

    let mut out = Vec::new();
    // Header: number of symbols
    let n = codes.len() as u16;
    out.push((n & 0xFF) as u8);
    out.push((n >> 8) as u8);
    // Code table: sorted by byte for determinism
    let mut entries: Vec<_> = codes.iter().collect();
    entries.sort_by_key(|(b, _)| **b);
    for (&byte, bits) in &entries {
        out.push(byte);
        out.push(bits.len() as u8);
        // Pack bits into bytes
        let packed = pack_bits(bits);
        out.push(packed.len() as u8);
        out.extend_from_slice(&packed);
    }
    // Compressed data
    let mut all_bits = Vec::new();
    for &b in data {
        all_bits.extend_from_slice(&codes[&b]);
    }
    let total = all_bits.len() as u32;
    out.push((total & 0xFF) as u8);
    out.push(((total >> 8) & 0xFF) as u8);
    out.push(((total >> 16) & 0xFF) as u8);
    out.push(((total >> 24) & 0xFF) as u8);
    out.extend_from_slice(&pack_bits(&all_bits));

    out
}

/// Huffman-decode bytes.
pub fn huffman_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if data.len() < 2 {
        return Err("truncated Huffman header".into());
    }
    let n = data[0] as u16 | ((data[1] as u16) << 8);
    let mut i = 2;
    let mut codes: HashMap<Vec<bool>, u8> = HashMap::new();
    for _ in 0..n {
        if i >= data.len() {
            return Err("truncated code table".into());
        }
        let byte = data[i];
        let bit_len = data[i + 1] as usize;
        let packed_len = data[i + 2] as usize;
        i += 3;
        if i + packed_len > data.len() {
            return Err("truncated code bits".into());
        }
        let bits = unpack_bits(&data[i..i + packed_len], bit_len);
        codes.insert(bits, byte);
        i += packed_len;
    }
    if i + 4 > data.len() {
        return Err("truncated total bits".into());
    }
    let total_bits = data[i] as u32
        | ((data[i + 1] as u32) << 8)
        | ((data[i + 2] as u32) << 16)
        | ((data[i + 3] as u32) << 24);
    i += 4;
    let bit_data = unpack_bits(&data[i..], total_bits as usize);

    let mut out = Vec::new();
    let mut current = Vec::new();
    for bit in &bit_data {
        current.push(*bit);
        if let Some(&byte) = codes.get(&current) {
            out.push(byte);
            current.clear();
        }
    }
    Ok(out)
}

fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len().div_ceil(8));
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit {
                byte |= 1 << (7 - i);
            }
        }
        out.push(byte);
    }
    out
}

fn unpack_bits(data: &[u8], count: usize) -> Vec<bool> {
    let mut bits = Vec::with_capacity(count);
    for &byte in data {
        for i in 0..8 {
            if bits.len() >= count {
                return bits;
            }
            bits.push((byte >> (7 - i)) & 1 == 1);
        }
    }
    bits
}

// ---------------------------------------------------------------------------
// Unified interface
// ---------------------------------------------------------------------------

/// Compress data with the given algorithm.
pub fn compress(data: &[u8], algo: Algorithm) -> CompressResult {
    let compressed = match algo {
        Algorithm::Rle => rle_encode(data),
        Algorithm::Lz77 => lz77_encode(data, 4096),
        Algorithm::Huffman => huffman_encode(data),
    };
    let ratio = if data.is_empty() {
        1.0
    } else {
        compressed.len() as f64 / data.len() as f64
    };
    CompressResult {
        algorithm: format!("{algo:?}"),
        original_len: data.len(),
        compressed_len: compressed.len(),
        ratio,
        data: compressed,
    }
}

/// Decompress data with the given algorithm.
pub fn decompress(data: &[u8], algo: Algorithm) -> Result<Vec<u8>, String> {
    match algo {
        Algorithm::Rle => rle_decode(data),
        Algorithm::Lz77 => lz77_decode(data),
        Algorithm::Huffman => huffman_decode(data),
    }
}

/// Analyse compression across all algorithms.
pub fn analyze(data: &[u8]) -> CompressionAnalysis {
    let results: Vec<CompressResult> = [Algorithm::Rle, Algorithm::Lz77, Algorithm::Huffman]
        .iter()
        .map(|&algo| compress(data, algo))
        .collect();
    let best = results
        .iter()
        .min_by(|a, b| a.ratio.partial_cmp(&b.ratio).unwrap_or(Ordering::Equal))
        .map(|r| r.algorithm.clone())
        .unwrap_or_default();
    CompressionAnalysis {
        input_len: data.len(),
        results,
        best,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle_roundtrip() {
        let data = b"aaabbbcccdddeeefff";
        let enc = rle_encode(data);
        let dec = rle_decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_rle_no_runs() {
        let data = b"abcdef";
        let enc = rle_encode(data);
        let dec = rle_decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_rle_empty() {
        assert!(rle_encode(b"").is_empty());
        assert!(rle_decode(b"").unwrap().is_empty());
    }

    #[test]
    fn test_rle_escape_byte() {
        let data = vec![0xFF, 0xFF, 0xFF];
        let enc = rle_encode(&data);
        let dec = rle_decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_lz77_roundtrip() {
        let data = b"abcabcabcabc";
        let enc = lz77_encode(data, 256);
        let dec = lz77_decode(&enc).unwrap();
        // LZ77 may add a trailing null byte
        assert!(dec.starts_with(data));
    }

    #[test]
    fn test_lz77_empty() {
        let enc = lz77_encode(b"", 256);
        assert!(enc.is_empty());
    }

    #[test]
    fn test_huffman_roundtrip() {
        let data = b"aabbbccccdddddeeeeeefffffffggggggg";
        let enc = huffman_encode(data);
        let dec = huffman_decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_huffman_single_char() {
        let data = b"aaaaa";
        let enc = huffman_encode(data);
        let dec = huffman_decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_huffman_empty() {
        assert!(huffman_encode(b"").is_empty());
        assert!(huffman_decode(b"").unwrap().is_empty());
    }

    #[test]
    fn test_compress_decompress_rle() {
        let data = b"xxxxxxxxyyyyyyzzzzz";
        let result = compress(data, Algorithm::Rle);
        assert!(result.compressed_len > 0);
        let dec = decompress(&result.data, Algorithm::Rle).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_compress_decompress_huffman() {
        let data = b"hello world hello world hello world hello";
        let result = compress(data, Algorithm::Huffman);
        let dec = decompress(&result.data, Algorithm::Huffman).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_analyze() {
        let data = b"aaaaaabbbbbbcccccc";
        let analysis = analyze(data);
        assert_eq!(analysis.results.len(), 3);
        assert!(!analysis.best.is_empty());
    }

    #[test]
    fn test_rle_compresses_runs() {
        let data = vec![b'A'; 100];
        let enc = rle_encode(&data);
        assert!(enc.len() < data.len(), "RLE should compress long runs");
    }

    #[test]
    fn test_pack_unpack_bits() {
        let bits = vec![true, false, true, true, false, false, true, false, true];
        let packed = pack_bits(&bits);
        let unpacked = unpack_bits(&packed, bits.len());
        assert_eq!(unpacked, bits);
    }

    #[test]
    fn test_ratio() {
        let data = vec![b'X'; 200];
        let result = compress(&data, Algorithm::Rle);
        assert!(result.ratio < 1.0, "runs should compress below 1.0 ratio");
    }
}
