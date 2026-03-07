//! # Codec — Encode / Decode Transformations
//!
//! Provides encode and decode operations for common text encodings:
//! Base64, hex, URL-encoding, HTML entities, ROT13, quoted-printable,
//! and percent-encoding.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported codec kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodecKind {
    Base64,
    Hex,
    UrlEncode,
    HtmlEntities,
    Rot13,
    QuotedPrintable,
    PercentEncode,
}

impl CodecKind {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Base64 => "base64",
            Self::Hex => "hex",
            Self::UrlEncode => "url-encode",
            Self::HtmlEntities => "html-entities",
            Self::Rot13 => "rot13",
            Self::QuotedPrintable => "quoted-printable",
            Self::PercentEncode => "percent-encode",
        }
    }

    /// List all supported codecs.
    pub fn all() -> &'static [CodecKind] {
        &[
            Self::Base64,
            Self::Hex,
            Self::UrlEncode,
            Self::HtmlEntities,
            Self::Rot13,
            Self::QuotedPrintable,
            Self::PercentEncode,
        ]
    }
}

/// Result of an encode / decode operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecResult {
    pub codec: String,
    pub direction: String,
    pub input_len: usize,
    pub output_len: usize,
    pub output: String,
}

/// Options for batch codec operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecOptions {
    /// Line-wrap column for base64 (0 = no wrap).
    pub base64_line_len: usize,
    /// Use uppercase hex digits.
    pub hex_upper: bool,
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            base64_line_len: 76,
            hex_upper: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Base64
// ---------------------------------------------------------------------------

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Encode bytes to base64.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decode base64 to bytes.
pub fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if clean.is_empty() {
        return Ok(Vec::new());
    }
    if clean.len() % 4 != 0 {
        return Err("base64 length must be a multiple of 4".into());
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let mut vals = [0u8; 4];
        let mut pad = 0;
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'=' {
                pad += 1;
                vals[i] = 0;
            } else {
                vals[i] =
                    b64_val(b).ok_or_else(|| format!("invalid base64 byte: {}", b as char))?;
            }
        }
        let triple = (vals[0] as u32) << 18
            | (vals[1] as u32) << 12
            | (vals[2] as u32) << 6
            | vals[3] as u32;
        out.push((triple >> 16) as u8);
        if pad < 2 {
            out.push((triple >> 8) as u8);
        }
        if pad < 1 {
            out.push(triple as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Hex
// ---------------------------------------------------------------------------

/// Encode bytes to hex string.
pub fn hex_encode(data: &[u8], upper: bool) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for &b in data {
        if upper {
            out.push_str(&format!("{b:02X}"));
        } else {
            out.push_str(&format!("{b:02x}"));
        }
    }
    out
}

/// Decode hex string to bytes.
pub fn hex_decode(input: &str) -> Result<Vec<u8>, String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err("hex string must have even length".into());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])
            .ok_or_else(|| format!("invalid hex char: {}", bytes[i] as char))?;
        let lo = hex_nibble(bytes[i + 1])
            .ok_or_else(|| format!("invalid hex char: {}", bytes[i + 1] as char))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// URL encoding
// ---------------------------------------------------------------------------

/// URL-encode (RFC 3986 unreserved set preserved).
pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decode URL-encoded string.
pub fn url_decode(input: &str) -> Result<String, String> {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err("incomplete percent-encoding".into());
            }
            let hi = hex_nibble(bytes[i + 1]).ok_or("invalid percent-encoding")?;
            let lo = hex_nibble(bytes[i + 2]).ok_or("invalid percent-encoding")?;
            out.push((hi << 4) | lo);
            i += 3;
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| format!("invalid UTF-8: {e}"))
}

// ---------------------------------------------------------------------------
// HTML entities
// ---------------------------------------------------------------------------

/// Escape HTML special characters.
pub fn html_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Unescape HTML entities (named + numeric).
pub fn html_decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' {
            if let Some(semi) = chars[i..].iter().position(|&c| c == ';') {
                let entity: String = chars[i + 1..i + semi].iter().collect();
                let decoded = match entity.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some('\u{00A0}'),
                    _ if entity.starts_with('#') => parse_char_ref(&entity[1..]),
                    _ => None,
                };
                if let Some(ch) = decoded {
                    out.push(ch);
                    i += semi + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn parse_char_ref(s: &str) -> Option<char> {
    let n = if let Some(hex) = s.strip_prefix('x').or_else(|| s.strip_prefix('X')) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        s.parse::<u32>().ok()?
    };
    char::from_u32(n)
}

// ---------------------------------------------------------------------------
// ROT13
// ---------------------------------------------------------------------------

/// Apply ROT13 cipher (self-inverse).
pub fn rot13(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'a'..='m' | 'A'..='M' => char::from(c as u8 + 13),
            'n'..='z' | 'N'..='Z' => char::from(c as u8 - 13),
            _ => c,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Quoted-Printable
// ---------------------------------------------------------------------------

/// Encode to quoted-printable (RFC 2045).
pub fn qp_encode(input: &str) -> String {
    let mut out = String::new();
    let mut col = 0;
    for &b in input.as_bytes() {
        let needs_encode = !(b == b'\t' || (0x20..=0x7E).contains(&b)) || b == b'=';
        let repr = if needs_encode {
            format!("={b:02X}")
        } else {
            String::from(b as char)
        };
        if col + repr.len() > 75 {
            out.push_str("=\r\n");
            col = 0;
        }
        out.push_str(&repr);
        col += repr.len();
        if b == b'\n' {
            col = 0;
        }
    }
    out
}

/// Decode quoted-printable.
pub fn qp_decode(input: &str) -> Result<String, String> {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            // soft line break
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
                continue;
            }
            if i + 2 < bytes.len() && bytes[i + 1] == b'\r' && bytes[i + 2] == b'\n' {
                i += 3;
                continue;
            }
            if i + 2 >= bytes.len() {
                return Err("incomplete QP escape".into());
            }
            let hi = hex_nibble(bytes[i + 1]).ok_or("invalid QP escape")?;
            let lo = hex_nibble(bytes[i + 2]).ok_or("invalid QP escape")?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| format!("invalid UTF-8: {e}"))
}

// ---------------------------------------------------------------------------
// Percent-encoding (full, not just URL-safe)
// ---------------------------------------------------------------------------

/// Percent-encode every non-ASCII-alphanumeric byte.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decode percent-encoded string.
pub fn percent_decode(input: &str) -> Result<String, String> {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err("incomplete percent-encoding".into());
            }
            let hi = hex_nibble(bytes[i + 1]).ok_or("invalid percent-encoding")?;
            let lo = hex_nibble(bytes[i + 2]).ok_or("invalid percent-encoding")?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| format!("invalid UTF-8: {e}"))
}

// ---------------------------------------------------------------------------
// Unified encode / decode
// ---------------------------------------------------------------------------

/// Encode `input` with the given codec.
pub fn encode(kind: CodecKind, input: &str, opts: &CodecOptions) -> CodecResult {
    let output = match kind {
        CodecKind::Base64 => {
            let raw = base64_encode(input.as_bytes());
            if opts.base64_line_len > 0 {
                wrap_lines(&raw, opts.base64_line_len)
            } else {
                raw
            }
        }
        CodecKind::Hex => hex_encode(input.as_bytes(), opts.hex_upper),
        CodecKind::UrlEncode => url_encode(input),
        CodecKind::HtmlEntities => html_encode(input),
        CodecKind::Rot13 => rot13(input),
        CodecKind::QuotedPrintable => qp_encode(input),
        CodecKind::PercentEncode => percent_encode(input),
    };
    CodecResult {
        codec: kind.label().to_string(),
        direction: "encode".into(),
        input_len: input.len(),
        output_len: output.len(),
        output,
    }
}

/// Decode `input` with the given codec.
pub fn decode(kind: CodecKind, input: &str) -> Result<CodecResult, String> {
    let output = match kind {
        CodecKind::Base64 => {
            let bytes = base64_decode(input)?;
            String::from_utf8(bytes).map_err(|e| format!("invalid UTF-8: {e}"))?
        }
        CodecKind::Hex => {
            let bytes = hex_decode(input)?;
            String::from_utf8(bytes).map_err(|e| format!("invalid UTF-8: {e}"))?
        }
        CodecKind::UrlEncode => url_decode(input)?,
        CodecKind::HtmlEntities => html_decode(input),
        CodecKind::Rot13 => rot13(input),
        CodecKind::QuotedPrintable => qp_decode(input)?,
        CodecKind::PercentEncode => percent_decode(input)?,
    };
    Ok(CodecResult {
        codec: kind.label().to_string(),
        direction: "decode".into(),
        input_len: input.len(),
        output_len: output.len(),
        output,
    })
}

fn wrap_lines(s: &str, cols: usize) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / cols);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && i % cols == 0 {
            out.push('\n');
        }
        out.push(ch);
    }
    out
}

/// Detect which codec was likely used to produce `input`.
pub fn detect_codec(input: &str) -> Option<CodecKind> {
    let trimmed = input.trim();
    // Try hex first (all hex chars, even length)
    if trimmed.len() >= 2
        && trimmed.len() % 2 == 0
        && trimmed.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Some(CodecKind::Hex);
    }
    // Base64 (alphanumeric, +, /, =, line breaks)
    if trimmed.len() >= 4
        && trimmed.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || b == b'+'
                || b == b'/'
                || b == b'='
                || b == b'\n'
                || b == b'\r'
        })
    {
        return Some(CodecKind::Base64);
    }
    // Percent-encoded
    if trimmed.contains('%') && trimmed.bytes().any(|b| b == b'%') {
        return Some(CodecKind::PercentEncode);
    }
    None
}

/// List all supported codec labels.
pub fn supported_codecs() -> Vec<&'static str> {
    CodecKind::all().iter().map(|k| k.label()).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let input = "Hello, World!";
        let encoded = base64_encode(input.as_bytes());
        assert_eq!(encoded, "SGVsbG8sIFdvcmxkIQ==");
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, input.as_bytes());
    }

    #[test]
    fn test_base64_empty() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_base64_padding_variants() {
        // 1 byte → 4 chars with ==
        let e1 = base64_encode(b"a");
        assert_eq!(e1, "YQ==");
        assert_eq!(base64_decode(&e1).unwrap(), b"a");
        // 2 bytes → 4 chars with =
        let e2 = base64_encode(b"ab");
        assert_eq!(e2, "YWI=");
        assert_eq!(base64_decode(&e2).unwrap(), b"ab");
    }

    #[test]
    fn test_hex_roundtrip() {
        let data = b"\x00\xff\x42";
        let lower = hex_encode(data, false);
        assert_eq!(lower, "00ff42");
        let upper = hex_encode(data, true);
        assert_eq!(upper, "00FF42");
        assert_eq!(hex_decode(&lower).unwrap(), data);
        assert_eq!(hex_decode(&upper).unwrap(), data);
    }

    #[test]
    fn test_hex_errors() {
        assert!(hex_decode("0").is_err());
        assert!(hex_decode("GG").is_err());
    }

    #[test]
    fn test_url_encode_decode() {
        let input = "hello world & foo=bar";
        let encoded = url_encode(input);
        assert!(encoded.contains("%20"));
        assert!(encoded.contains("%26"));
        let decoded = url_decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_url_decode_plus() {
        assert_eq!(url_decode("a+b").unwrap(), "a b");
    }

    #[test]
    fn test_html_entities_roundtrip() {
        let input = "<div class=\"test\">&</div>";
        let enc = html_encode(input);
        assert_eq!(enc, "&lt;div class=&quot;test&quot;&gt;&amp;&lt;/div&gt;");
        let dec = html_decode(&enc);
        assert_eq!(dec, input);
    }

    #[test]
    fn test_html_numeric_entity() {
        assert_eq!(html_decode("&#65;"), "A");
        assert_eq!(html_decode("&#x41;"), "A");
    }

    #[test]
    fn test_rot13_self_inverse() {
        let input = "Hello World 123!";
        let enc = rot13(input);
        assert_eq!(enc, "Uryyb Jbeyq 123!");
        assert_eq!(rot13(&enc), input);
    }

    #[test]
    fn test_qp_roundtrip() {
        let input = "Subject: =?UTF-8?Q? résumé";
        let enc = qp_encode(input);
        let dec = qp_decode(&enc).unwrap();
        // QP preserves printable ASCII, encodes high bytes
        assert_eq!(dec, input);
    }

    #[test]
    fn test_qp_soft_linebreak() {
        let decoded = qp_decode("line1=\r\nline2").unwrap();
        assert_eq!(decoded, "line1line2");
    }

    #[test]
    fn test_percent_encode_decode() {
        let input = "a/b c";
        let enc = percent_encode(input);
        assert_eq!(enc, "a%2Fb%20c");
        let dec = percent_decode(&enc).unwrap();
        assert_eq!(dec, input);
    }

    #[test]
    fn test_unified_encode_decode() {
        let opts = CodecOptions::default();
        for &kind in CodecKind::all() {
            let input = "test data 123";
            let enc = encode(kind, input, &opts);
            assert_eq!(enc.direction, "encode");
            let dec = decode(kind, &enc.output).unwrap();
            assert_eq!(dec.direction, "decode");
            assert_eq!(dec.output, input, "roundtrip failed for {:?}", kind);
        }
    }

    #[test]
    fn test_detect_codec() {
        assert_eq!(detect_codec("48656c6c6f"), Some(CodecKind::Hex));
        assert_eq!(detect_codec("SGVsbG8="), Some(CodecKind::Base64));
        assert_eq!(
            detect_codec("hello%20world"),
            Some(CodecKind::PercentEncode)
        );
    }

    #[test]
    fn test_supported_codecs() {
        let codecs = supported_codecs();
        assert_eq!(codecs.len(), 7);
        assert!(codecs.contains(&"base64"));
    }

    #[test]
    fn test_wrap_lines() {
        let s = "ABCDEFGHIJ";
        let wrapped = wrap_lines(s, 4);
        assert_eq!(wrapped, "ABCD\nEFGH\nIJ");
    }
}
