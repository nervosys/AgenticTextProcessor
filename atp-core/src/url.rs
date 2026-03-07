// ---------------------------------------------------------------------------
// url.rs — URL parsing & manipulation
// ---------------------------------------------------------------------------
//
// Parse/build/normalize, encode/decode path & query components,
// resolve relative URLs, extract domain/TLD, URL validation.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A parsed URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    /// Scheme (e.g. "https").
    pub scheme: String,
    /// User info (e.g. "user:pass"), if present.
    pub userinfo: Option<String>,
    /// Host (e.g. "example.com").
    pub host: String,
    /// Port number, if present.
    pub port: Option<u16>,
    /// Path (e.g. "/path/to/resource").
    pub path: String,
    /// Query parameters as ordered pairs.
    pub query: Vec<(String, String)>,
    /// Fragment (after #).
    pub fragment: Option<String>,
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://", self.scheme)?;
        if let Some(ref ui) = self.userinfo {
            write!(f, "{ui}@")?;
        }
        write!(f, "{}", self.host)?;
        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }
        write!(f, "{}", self.path)?;
        if !self.query.is_empty() {
            let pairs: Vec<String> = self
                .query
                .iter()
                .map(|(k, v)| {
                    if v.is_empty() {
                        percent_encode(k)
                    } else {
                        format!("{}={}", percent_encode(k), percent_encode(v))
                    }
                })
                .collect();
            write!(f, "?{}", pairs.join("&"))?;
        }
        if let Some(ref frag) = self.fragment {
            write!(f, "#{frag}")?;
        }
        Ok(())
    }
}

impl Url {
    /// The full authority section (userinfo@host:port).
    pub fn authority(&self) -> String {
        let mut a = String::new();
        if let Some(ref ui) = self.userinfo {
            a.push_str(ui);
            a.push('@');
        }
        a.push_str(&self.host);
        if let Some(port) = self.port {
            a.push(':');
            a.push_str(&port.to_string());
        }
        a
    }

    /// Query as a HashMap (last value wins for duplicate keys).
    pub fn query_map(&self) -> HashMap<String, String> {
        self.query.iter().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a URL string into a `Url` struct.
pub fn parse(input: &str) -> Option<Url> {
    let s = input.trim();
    // Scheme
    let (scheme, rest) = s.split_once("://")?;
    if scheme.is_empty() {
        return None;
    }
    // Fragment
    let (before_frag, fragment) = if let Some((bf, f)) = rest.split_once('#') {
        (bf, Some(f.to_string()))
    } else {
        (rest, None)
    };
    // Query
    let (before_query, query_str) = if let Some((bq, qs)) = before_frag.split_once('?') {
        (bq, Some(qs))
    } else {
        (before_frag, None)
    };
    let query = query_str.map(parse_query).unwrap_or_default();
    // Authority and path
    let (authority, path) = if let Some(slash_pos) = before_query.find('/') {
        (&before_query[..slash_pos], &before_query[slash_pos..])
    } else {
        (before_query, "/")
    };
    // Userinfo
    let (userinfo, host_port) = if let Some((ui, hp)) = authority.split_once('@') {
        (Some(ui.to_string()), hp)
    } else {
        (None, authority)
    };
    // Host and port
    let (host, port) = if host_port.starts_with('[') {
        // IPv6
        if let Some(bracket_end) = host_port.find(']') {
            let h = &host_port[1..bracket_end];
            let rest = &host_port[bracket_end + 1..];
            let p = rest.strip_prefix(':').and_then(|s| s.parse::<u16>().ok());
            (h, p)
        } else {
            (host_port, None)
        }
    } else if let Some((h, p)) = host_port.rsplit_once(':') {
        if let Ok(port) = p.parse::<u16>() {
            (h, Some(port))
        } else {
            (host_port, None)
        }
    } else {
        (host_port, None)
    };

    Some(Url {
        scheme: scheme.to_lowercase(),
        userinfo,
        host: host.to_lowercase(),
        port,
        path: if path == "/" && before_query.find('/').is_none() {
            "/".to_string()
        } else {
            path.to_string()
        },
        query,
        fragment,
    })
}

// ---------------------------------------------------------------------------
// Query string
// ---------------------------------------------------------------------------

/// Parse a query string into key-value pairs.
pub fn parse_query(qs: &str) -> Vec<(String, String)> {
    qs.split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            if let Some((k, v)) = pair.split_once('=') {
                (percent_decode(k), percent_decode(v))
            } else {
                (percent_decode(pair), String::new())
            }
        })
        .collect()
}

/// Build a query string from key-value pairs.
pub fn build_query(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                percent_encode(k)
            } else {
                format!("{}={}", percent_encode(k), percent_encode(v))
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

// ---------------------------------------------------------------------------
// Percent encoding / decoding
// ---------------------------------------------------------------------------

/// Percent-encode a string (URL-safe).
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-decode a string.
pub fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(val);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/// Normalize a URL string (lowercase scheme/host, remove default ports,
/// remove trailing slash for non-root paths, sort query params).
pub fn normalize(url_str: &str) -> Option<String> {
    let mut url = parse(url_str)?;
    // Remove default ports
    match (url.scheme.as_str(), url.port) {
        ("http", Some(80)) | ("https", Some(443)) | ("ftp", Some(21)) => {
            url.port = None;
        }
        _ => {}
    }
    // Remove trailing slash (unless root)
    if url.path.len() > 1 && url.path.ends_with('/') {
        url.path = url.path.trim_end_matches('/').to_string();
        if url.path.is_empty() {
            url.path = "/".to_string();
        }
    }
    // Sort query params
    url.query.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    // Remove dot segments in path
    url.path = resolve_dot_segments(&url.path);
    Some(url.to_string())
}

fn resolve_dot_segments(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let result = out.join("/");
    if result.starts_with('/') || path.starts_with('/') {
        if result.starts_with('/') {
            result
        } else {
            format!("/{result}")
        }
    } else {
        result
    }
}

// ---------------------------------------------------------------------------
// Relative URL resolution
// ---------------------------------------------------------------------------

/// Resolve a relative URL against a base URL.
pub fn resolve(base: &str, relative: &str) -> Option<String> {
    if relative.contains("://") {
        return Some(relative.to_string());
    }
    let base_url = parse(base)?;
    if relative.starts_with("//") {
        return parse(&format!("{}:{relative}", base_url.scheme)).map(|u| u.to_string());
    }
    if relative.starts_with('/') {
        let mut new_url = base_url;
        new_url.path = relative.to_string();
        new_url.query.clear();
        new_url.fragment = None;
        return Some(new_url.to_string());
    }
    // Relative path
    let base_dir = if let Some(pos) = base_url.path.rfind('/') {
        &base_url.path[..=pos]
    } else {
        "/"
    };
    let combined = format!("{base_dir}{relative}");
    let mut new_url = base_url;
    new_url.path = resolve_dot_segments(&combined);
    new_url.query.clear();
    new_url.fragment = None;
    Some(new_url.to_string())
}

// ---------------------------------------------------------------------------
// Domain / TLD extraction
// ---------------------------------------------------------------------------

/// Extract the domain from a URL string.
pub fn domain(url_str: &str) -> Option<String> {
    parse(url_str).map(|u| u.host)
}

/// Extract the TLD (top-level domain) from a hostname.
pub fn tld(host: &str) -> Option<String> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 2 {
        // Check for known two-part TLDs
        let last_two = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        let two_part_tlds = [
            "co.uk", "co.jp", "co.kr", "com.br", "com.au", "com.cn",
            "org.uk", "net.au", "ac.uk", "gov.uk", "co.nz", "com.mx",
        ];
        if two_part_tlds.contains(&last_two.as_str()) {
            return Some(last_two);
        }
        Some(parts.last()?.to_string())
    } else {
        Some(host.to_string())
    }
}

/// Extract the registered domain (domain + TLD) from a hostname.
pub fn registered_domain(host: &str) -> Option<String> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() < 2 {
        return Some(host.to_string());
    }
    let tld_str = tld(host)?;
    let tld_parts = tld_str.split('.').count();
    if parts.len() > tld_parts {
        let domain_start = parts.len() - tld_parts - 1;
        Some(parts[domain_start..].join("."))
    } else {
        Some(host.to_string())
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Check if a string is a valid URL.
pub fn is_valid(s: &str) -> bool {
    parse(s).is_some()
}

/// Check if a URL uses HTTPS.
pub fn is_https(s: &str) -> bool {
    parse(s).is_some_and(|u| u.scheme == "https")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let u = parse("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.host, "example.com");
        assert_eq!(u.path, "/path");
        assert_eq!(u.query, vec![("q".into(), "1".into())]);
        assert_eq!(u.fragment, Some("frag".to_string()));
    }

    #[test]
    fn test_parse_with_port() {
        let u = parse("http://localhost:8080/api").unwrap();
        assert_eq!(u.host, "localhost");
        assert_eq!(u.port, Some(8080));
    }

    #[test]
    fn test_parse_with_userinfo() {
        let u = parse("ftp://user:pass@ftp.example.com/files").unwrap();
        assert_eq!(u.userinfo, Some("user:pass".to_string()));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse("not a url").is_none());
    }

    #[test]
    fn test_to_string_roundtrip() {
        let input = "https://example.com/path?key=value#sec";
        let u = parse(input).unwrap();
        let output = u.to_string();
        assert!(output.contains("example.com"));
        assert!(output.contains("key=value"));
    }

    #[test]
    fn test_percent_encode() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("a+b"), "a%2Bb");
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("a+b"), "a b");
    }

    #[test]
    fn test_normalize() {
        let n = normalize("HTTP://Example.COM:80/path/./a/../b/").unwrap();
        assert!(n.starts_with("http://example.com/"));
        assert!(!n.contains(":80"));
    }

    #[test]
    fn test_resolve_absolute() {
        let r = resolve("https://a.com/foo", "https://b.com/bar").unwrap();
        assert!(r.contains("b.com"));
    }

    #[test]
    fn test_resolve_relative() {
        let r = resolve("https://a.com/dir/page", "other").unwrap();
        assert!(r.contains("/dir/other"));
    }

    #[test]
    fn test_resolve_absolute_path() {
        let r = resolve("https://a.com/dir/page", "/root").unwrap();
        assert!(r.contains("/root"));
    }

    #[test]
    fn test_domain_extraction() {
        assert_eq!(domain("https://www.example.com/path"), Some("www.example.com".into()));
    }

    #[test]
    fn test_tld() {
        assert_eq!(tld("www.example.com"), Some("com".into()));
        assert_eq!(tld("www.example.co.uk"), Some("co.uk".into()));
    }

    #[test]
    fn test_registered_domain() {
        assert_eq!(registered_domain("www.example.com"), Some("example.com".into()));
        assert_eq!(registered_domain("sub.example.co.uk"), Some("example.co.uk".into()));
    }

    #[test]
    fn test_is_valid() {
        assert!(is_valid("https://example.com"));
        assert!(!is_valid("not a url"));
    }

    #[test]
    fn test_is_https() {
        assert!(is_https("https://secure.example.com"));
        assert!(!is_https("http://insecure.example.com"));
    }

    #[test]
    fn test_parse_query() {
        let q = parse_query("a=1&b=2&c=3");
        assert_eq!(q.len(), 3);
        assert_eq!(q[0], ("a".into(), "1".into()));
    }

    #[test]
    fn test_build_query() {
        let q = build_query(&[("key".into(), "val ue".into())]);
        assert_eq!(q, "key=val%20ue");
    }
}
