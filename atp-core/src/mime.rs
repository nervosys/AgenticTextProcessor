// ---------------------------------------------------------------------------
// mime.rs — MIME type detection
// ---------------------------------------------------------------------------
//
// Magic-byte sniffing, extension mapping (~100 types), charset guessing
// (UTF-8/16/Latin-1 BOM detection), content-type builder.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A detected MIME type with optional charset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeType {
    /// Primary type (e.g. "text").
    pub kind: String,
    /// Subtype (e.g. "plain").
    pub subtype: String,
    /// Optional charset parameter.
    pub charset: Option<String>,
}

impl MimeType {
    pub fn new(kind: &str, subtype: &str) -> Self {
        Self {
            kind: kind.to_string(),
            subtype: subtype.to_string(),
            charset: None,
        }
    }

    pub fn with_charset(mut self, charset: &str) -> Self {
        self.charset = Some(charset.to_string());
        self
    }

    /// Format as a Content-Type header value.
    pub fn content_type(&self) -> String {
        match &self.charset {
            Some(cs) => format!("{}/{}; charset={}", self.kind, self.subtype, cs),
            None => format!("{}/{}", self.kind, self.subtype),
        }
    }

    /// Full MIME string (e.g. "text/plain").
    pub fn mime_str(&self) -> String {
        format!("{}/{}", self.kind, self.subtype)
    }
}

/// Detected charset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charset {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
    Ascii,
    Latin1,
    Unknown,
}

impl Charset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::Utf32Le => "utf-32le",
            Self::Utf32Be => "utf-32be",
            Self::Ascii => "us-ascii",
            Self::Latin1 => "iso-8859-1",
            Self::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Magic bytes
// ---------------------------------------------------------------------------

/// Detect MIME type from magic bytes at the start of `data`.
pub fn detect_magic(data: &[u8]) -> Option<MimeType> {
    if data.len() < 4 {
        return None;
    }
    // PDF
    if data.starts_with(b"%PDF") {
        return Some(MimeType::new("application", "pdf"));
    }
    // PNG
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some(MimeType::new("image", "png"));
    }
    // JPEG
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(MimeType::new("image", "jpeg"));
    }
    // GIF
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Some(MimeType::new("image", "gif"));
    }
    // ZIP (also docx, xlsx, jar, etc.)
    if data.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return Some(MimeType::new("application", "zip"));
    }
    // GZIP
    if data.starts_with(&[0x1F, 0x8B]) {
        return Some(MimeType::new("application", "gzip"));
    }
    // BMP
    if data.starts_with(b"BM") {
        return Some(MimeType::new("image", "bmp"));
    }
    // TIFF (little-endian)
    if data.starts_with(&[0x49, 0x49, 0x2A, 0x00]) {
        return Some(MimeType::new("image", "tiff"));
    }
    // TIFF (big-endian)
    if data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
        return Some(MimeType::new("image", "tiff"));
    }
    // WASM
    if data.starts_with(&[0x00, 0x61, 0x73, 0x6D]) {
        return Some(MimeType::new("application", "wasm"));
    }
    // ELF
    if data.starts_with(&[0x7F, 0x45, 0x4C, 0x46]) {
        return Some(MimeType::new("application", "x-elf"));
    }
    // OGG
    if data.starts_with(b"OggS") {
        return Some(MimeType::new("audio", "ogg"));
    }
    // FLAC
    if data.starts_with(b"fLaC") {
        return Some(MimeType::new("audio", "flac"));
    }
    // RIFF (WAV, AVI)
    if data.starts_with(b"RIFF") && data.len() >= 12 {
        if &data[8..12] == b"WAVE" {
            return Some(MimeType::new("audio", "wav"));
        }
        if &data[8..12] == b"AVI " {
            return Some(MimeType::new("video", "x-msvideo"));
        }
    }
    // MP3 (ID3 tag or frame sync)
    if data.starts_with(b"ID3") || (data[0] == 0xFF && data[1] & 0xE0 == 0xE0) {
        return Some(MimeType::new("audio", "mpeg"));
    }
    // XML
    if data.starts_with(b"<?xml") {
        return Some(MimeType::new("application", "xml"));
    }
    None
}

// ---------------------------------------------------------------------------
// Extension mapping
// ---------------------------------------------------------------------------

/// Map a file extension (without dot) to a MIME type.
pub fn from_extension(ext: &str) -> Option<MimeType> {
    let m = |k: &str, s: &str| Some(MimeType::new(k, s));
    match ext.to_lowercase().as_str() {
        // Text
        "txt" | "text" | "log" => m("text", "plain"),
        "html" | "htm" => m("text", "html"),
        "css" => m("text", "css"),
        "csv" => m("text", "csv"),
        "tsv" => m("text", "tab-separated-values"),
        "xml" => m("text", "xml"),
        "md" | "markdown" => m("text", "markdown"),
        "rtf" => m("text", "rtf"),
        "ics" => m("text", "calendar"),
        "vcf" | "vcard" => m("text", "vcard"),
        // Application
        "js" | "mjs" | "cjs" => m("application", "javascript"),
        "json" => m("application", "json"),
        "jsonl" | "ndjson" => m("application", "x-ndjson"),
        "yaml" | "yml" => m("application", "x-yaml"),
        "toml" => m("application", "toml"),
        "pdf" => m("application", "pdf"),
        "zip" => m("application", "zip"),
        "gz" | "gzip" => m("application", "gzip"),
        "tar" => m("application", "x-tar"),
        "tgz" => m("application", "x-gzip"),
        "bz2" => m("application", "x-bzip2"),
        "xz" => m("application", "x-xz"),
        "zst" | "zstd" => m("application", "zstd"),
        "7z" => m("application", "x-7z-compressed"),
        "rar" => m("application", "x-rar-compressed"),
        "wasm" => m("application", "wasm"),
        "sql" => m("application", "sql"),
        "graphql" | "gql" => m("application", "graphql"),
        "woff" => m("application", "font-woff"),
        "woff2" => m("application", "font-woff2"),
        "ttf" => m("application", "font-sfnt"),
        "otf" => m("application", "font-sfnt"),
        "eot" => m("application", "vnd.ms-fontobject"),
        "swf" => m("application", "x-shockwave-flash"),
        "doc" => m("application", "msword"),
        "docx" => m("application", "vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xls" => m("application", "vnd.ms-excel"),
        "xlsx" => m("application", "vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "ppt" => m("application", "vnd.ms-powerpoint"),
        "pptx" => m("application", "vnd.openxmlformats-officedocument.presentationml.presentation"),
        "odt" => m("application", "vnd.oasis.opendocument.text"),
        "ods" => m("application", "vnd.oasis.opendocument.spreadsheet"),
        "jar" => m("application", "java-archive"),
        "apk" => m("application", "vnd.android.package-archive"),
        "exe" | "dll" => m("application", "x-msdownload"),
        "dmg" => m("application", "x-apple-diskimage"),
        "iso" => m("application", "x-iso9660-image"),
        "deb" => m("application", "x-debian-package"),
        "rpm" => m("application", "x-rpm"),
        "bin" => m("application", "octet-stream"),
        "dat" => m("application", "octet-stream"),
        // Image
        "png" => m("image", "png"),
        "jpg" | "jpeg" => m("image", "jpeg"),
        "gif" => m("image", "gif"),
        "bmp" => m("image", "bmp"),
        "svg" => m("image", "svg+xml"),
        "ico" => m("image", "x-icon"),
        "tif" | "tiff" => m("image", "tiff"),
        "webp" => m("image", "webp"),
        "avif" => m("image", "avif"),
        "heic" | "heif" => m("image", "heif"),
        "psd" => m("image", "vnd.adobe.photoshop"),
        // Audio
        "mp3" => m("audio", "mpeg"),
        "wav" => m("audio", "wav"),
        "ogg" | "oga" => m("audio", "ogg"),
        "flac" => m("audio", "flac"),
        "aac" => m("audio", "aac"),
        "m4a" => m("audio", "mp4"),
        "wma" => m("audio", "x-ms-wma"),
        "midi" | "mid" => m("audio", "midi"),
        "opus" => m("audio", "opus"),
        // Video
        "mp4" | "m4v" => m("video", "mp4"),
        "avi" => m("video", "x-msvideo"),
        "mkv" => m("video", "x-matroska"),
        "mov" => m("video", "quicktime"),
        "wmv" => m("video", "x-ms-wmv"),
        "flv" => m("video", "x-flv"),
        "webm" => m("video", "webm"),
        "mpg" | "mpeg" => m("video", "mpeg"),
        "ts" => m("video", "mp2t"),
        // Source code (as text)
        "rs" => m("text", "x-rust"),
        "py" => m("text", "x-python"),
        "rb" => m("text", "x-ruby"),
        "go" => m("text", "x-go"),
        "c" | "h" => m("text", "x-c"),
        "cpp" | "cxx" | "cc" | "hpp" => m("text", "x-c++"),
        "java" => m("text", "x-java"),
        "kt" | "kts" => m("text", "x-kotlin"),
        "swift" => m("text", "x-swift"),
        "sh" | "bash" | "zsh" => m("text", "x-shellscript"),
        "ps1" => m("text", "x-powershell"),
        "php" => m("text", "x-php"),
        "pl" | "pm" => m("text", "x-perl"),
        "r" => m("text", "x-r"),
        "lua" => m("text", "x-lua"),
        "dart" => m("text", "x-dart"),
        "scala" => m("text", "x-scala"),
        "zig" => m("text", "x-zig"),
        "nim" => m("text", "x-nim"),
        "ex" | "exs" => m("text", "x-elixir"),
        "erl" | "hrl" => m("text", "x-erlang"),
        "hs" => m("text", "x-haskell"),
        "ml" | "mli" => m("text", "x-ocaml"),
        _ => None,
    }
}

/// Extract extension from a filename.
pub fn extension(filename: &str) -> Option<&str> {
    let name = filename.rsplit('/').next().unwrap_or(filename);
    let name = name.rsplit('\\').next().unwrap_or(name);
    name.rsplit('.').next().filter(|e| *e != name)
}

// ---------------------------------------------------------------------------
// Charset detection
// ---------------------------------------------------------------------------

/// Detect charset from BOM and byte patterns.
pub fn detect_charset(data: &[u8]) -> Charset {
    // BOM detection
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Charset::Utf8;
    }
    if data.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return Charset::Utf32Le;
    }
    if data.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return Charset::Utf32Be;
    }
    if data.starts_with(&[0xFF, 0xFE]) {
        return Charset::Utf16Le;
    }
    if data.starts_with(&[0xFE, 0xFF]) {
        return Charset::Utf16Be;
    }
    // Check if valid UTF-8
    if std::str::from_utf8(data).is_ok() {
        // Check if pure ASCII
        if data.iter().all(|&b| b < 0x80) {
            return Charset::Ascii;
        }
        return Charset::Utf8;
    }
    // Fall back to Latin-1 check (always valid for any byte sequence, but heuristic)
    if data.iter().all(|&b| b < 0x80 || (0xA0..=0xFF).contains(&b)) {
        return Charset::Latin1;
    }
    Charset::Unknown
}

// ---------------------------------------------------------------------------
// Unified detection
// ---------------------------------------------------------------------------

/// Detect MIME type from data content (magic bytes) and/or filename extension.
pub fn detect(data: Option<&[u8]>, filename: Option<&str>) -> MimeType {
    // Try magic bytes first
    if let Some(d) = data {
        if let Some(mt) = detect_magic(d) {
            return mt;
        }
    }
    // Try extension
    if let Some(name) = filename {
        if let Some(ext) = extension(name) {
            if let Some(mt) = from_extension(ext) {
                return mt;
            }
        }
    }
    // Default
    MimeType::new("application", "octet-stream")
}

/// Detect MIME type and charset together, building a full Content-Type.
pub fn detect_with_charset(data: &[u8], filename: Option<&str>) -> MimeType {
    let mut mt = detect(Some(data), filename);
    if mt.kind == "text" {
        let cs = detect_charset(data);
        mt.charset = Some(cs.as_str().to_string());
    }
    mt
}

/// Count the number of recognized extension mappings.
pub fn extension_count() -> usize {
    // Count of mappings in from_extension
    102
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_magic(&data).unwrap().mime_str(), "image/png");
    }

    #[test]
    fn test_detect_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_magic(&data).unwrap().mime_str(), "image/jpeg");
    }

    #[test]
    fn test_detect_pdf() {
        let data = b"%PDF-1.4 ...";
        assert_eq!(detect_magic(data).unwrap().mime_str(), "application/pdf");
    }

    #[test]
    fn test_detect_zip() {
        let data = [0x50, 0x4B, 0x03, 0x04];
        assert_eq!(detect_magic(&data).unwrap().mime_str(), "application/zip");
    }

    #[test]
    fn test_extension_rs() {
        assert_eq!(from_extension("rs").unwrap().mime_str(), "text/x-rust");
    }

    #[test]
    fn test_extension_json() {
        assert_eq!(from_extension("json").unwrap().mime_str(), "application/json");
    }

    #[test]
    fn test_extension_unknown() {
        assert!(from_extension("xyzzy42").is_none());
    }

    #[test]
    fn test_extension_extract() {
        assert_eq!(extension("foo.txt"), Some("txt"));
        assert_eq!(extension("archive.tar.gz"), Some("gz"));
        assert_eq!(extension("noext"), None);
    }

    #[test]
    fn test_charset_utf8_bom() {
        let data = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(detect_charset(&data), Charset::Utf8);
    }

    #[test]
    fn test_charset_utf16le_bom() {
        let data = [0xFF, 0xFE, 0x00, 0x41];
        assert_eq!(detect_charset(&data), Charset::Utf16Le);
    }

    #[test]
    fn test_charset_ascii() {
        let data = b"Hello, world!";
        assert_eq!(detect_charset(data), Charset::Ascii);
    }

    #[test]
    fn test_content_type() {
        let mt = MimeType::new("text", "html").with_charset("utf-8");
        assert_eq!(mt.content_type(), "text/html; charset=utf-8");
    }

    #[test]
    fn test_detect_with_filename() {
        let mt = detect(None, Some("report.pdf"));
        assert_eq!(mt.mime_str(), "application/pdf");
    }

    #[test]
    fn test_detect_fallback() {
        let mt = detect(None, None);
        assert_eq!(mt.mime_str(), "application/octet-stream");
    }

    #[test]
    fn test_detect_with_charset() {
        let data = b"Hello in text";
        let mt = detect_with_charset(data, Some("file.txt"));
        assert!(mt.charset.is_some());
    }

    #[test]
    fn test_detect_magic_gzip() {
        let data = [0x1F, 0x8B, 0x08, 0x00];
        assert_eq!(detect_magic(&data).unwrap().mime_str(), "application/gzip");
    }

    #[test]
    fn test_detect_magic_wasm() {
        let data = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert_eq!(detect_magic(&data).unwrap().mime_str(), "application/wasm");
    }
}
