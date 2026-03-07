//! # Markdown — Parser & Renderer
//!
//! Parses a subset of Markdown into an AST, then renders to HTML or
//! plain text. Also extracts table-of-contents, links, and heading
//! structure.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AST types
// ---------------------------------------------------------------------------

/// A Markdown document's AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdDoc {
    pub nodes: Vec<MdNode>,
}

/// Node types in the Markdown AST.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MdNode {
    Heading { level: u8, text: String },
    Paragraph(String),
    CodeBlock { lang: String, code: String },
    BlockQuote(String),
    UnorderedList(Vec<String>),
    OrderedList(Vec<String>),
    HorizontalRule,
    Link { text: String, url: String },
    Image { alt: String, url: String },
    BlankLine,
}

/// Table of contents entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub slug: String,
}

/// Extracted link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdLink {
    pub text: String,
    pub url: String,
    pub is_image: bool,
}

/// Heading structure analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingStats {
    pub total: usize,
    pub by_level: [usize; 6],
    pub max_depth: u8,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse markdown text into an AST.
pub fn parse(input: &str) -> MdDoc {
    let mut nodes = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Blank line
        if trimmed.is_empty() {
            nodes.push(MdNode::BlankLine);
            i += 1;
            continue;
        }

        // Horizontal rule
        if is_horizontal_rule(trimmed) {
            nodes.push(MdNode::HorizontalRule);
            i += 1;
            continue;
        }

        // Heading (ATX style)
        if let Some(h) = parse_heading(trimmed) {
            nodes.push(h);
            i += 1;
            continue;
        }

        // Fenced code block
        if let Some(stripped) = trimmed.strip_prefix("```") {
            let lang = stripped.trim().to_string();
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                if lines[i].trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            nodes.push(MdNode::CodeBlock {
                lang,
                code: code_lines.join("\n"),
            });
            continue;
        }

        // Block quote
        if trimmed.starts_with('>') {
            let mut bq_lines = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start();
                let content = if let Some(rest) = l.strip_prefix('>') {
                    rest.trim_start()
                } else {
                    l
                };
                bq_lines.push(content);
                i += 1;
            }
            nodes.push(MdNode::BlockQuote(bq_lines.join("\n")));
            continue;
        }

        // Unordered list
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            let mut items = Vec::new();
            while i < lines.len() {
                let t = lines[i].trim();
                if let Some(rest) = t
                    .strip_prefix("- ")
                    .or_else(|| t.strip_prefix("* "))
                    .or_else(|| t.strip_prefix("+ "))
                {
                    items.push(rest.to_string());
                    i += 1;
                } else {
                    break;
                }
            }
            nodes.push(MdNode::UnorderedList(items));
            continue;
        }

        // Ordered list
        if is_ordered_item(trimmed) {
            let mut items = Vec::new();
            while i < lines.len() {
                let t = lines[i].trim();
                if let Some(rest) = strip_ordered_prefix(t) {
                    items.push(rest.to_string());
                    i += 1;
                } else {
                    break;
                }
            }
            nodes.push(MdNode::OrderedList(items));
            continue;
        }

        // Paragraph (collect contiguous non-blank lines)
        let mut para = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            let t = lines[i].trim();
            if t.starts_with('#')
                || t.starts_with("```")
                || t.starts_with('>')
                || t.starts_with("- ")
                || t.starts_with("* ")
                || is_horizontal_rule(t)
                || is_ordered_item(t)
            {
                break;
            }
            para.push(t);
            i += 1;
        }
        if !para.is_empty() {
            nodes.push(MdNode::Paragraph(para.join(" ")));
        }
    }

    MdDoc { nodes }
}

fn parse_heading(line: &str) -> Option<MdNode> {
    let bytes = line.as_bytes();
    let mut level = 0u8;
    for &b in bytes {
        if b == b'#' {
            level += 1;
        } else {
            break;
        }
    }
    if level == 0 || level > 6 {
        return None;
    }
    let rest = line[level as usize..].trim();
    if rest.is_empty() && level as usize == line.len() {
        return None; // bare # with nothing after
    }
    Some(MdNode::Heading {
        level,
        text: rest.trim_end_matches('#').trim().to_string(),
    })
}

fn is_horizontal_rule(line: &str) -> bool {
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3
        && (chars.iter().all(|&c| c == '-')
            || chars.iter().all(|&c| c == '*')
            || chars.iter().all(|&c| c == '_'))
}

fn is_ordered_item(line: &str) -> bool {
    strip_ordered_prefix(line).is_some()
}

fn strip_ordered_prefix(line: &str) -> Option<&str> {
    let idx = line.find('.')?;
    let num_part = &line[..idx];
    if num_part.is_empty() || !num_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let rest = &line[idx + 1..];
    Some(rest.trim_start())
}

// ---------------------------------------------------------------------------
// Inline parsing helpers
// ---------------------------------------------------------------------------

/// Extract inline links from text: `[text](url)`.
fn extract_inline_links(text: &str) -> Vec<MdLink> {
    let mut links = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let is_image = i > 0 && chars[i - 1] == '!';
        if chars[i] == '[' {
            if let Some(close_bracket) = chars[i..].iter().position(|&c| c == ']') {
                let cb = i + close_bracket;
                if cb + 1 < chars.len() && chars[cb + 1] == '(' {
                    if let Some(close_paren) = chars[cb + 1..].iter().position(|&c| c == ')') {
                        let cp = cb + 1 + close_paren;
                        let link_text: String = chars[i + 1..cb].iter().collect();
                        let url: String = chars[cb + 2..cp].iter().collect();
                        links.push(MdLink {
                            text: link_text,
                            url,
                            is_image,
                        });
                        i = cp + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    links
}

/// Render inline markdown to HTML (bold, italic, code, links).
fn render_inline_html(text: &str) -> String {
    let mut out = text.to_string();
    // Code spans (backtick)
    out = replace_inline_pattern(&out, '`', '`', "<code>", "</code>");
    // Bold **text**
    out = replace_double_pattern(&out, "**", "<strong>", "</strong>");
    // Bold __text__
    out = replace_double_pattern(&out, "__", "<strong>", "</strong>");
    // Italic *text* (but not **)
    out = replace_single_emphasis(&out, '*', "<em>", "</em>");
    // Italic _text_
    out = replace_single_emphasis(&out, '_', "<em>", "</em>");
    // Links [text](url)
    out = render_inline_links(&out);
    out
}

fn replace_inline_pattern(
    text: &str,
    open: char,
    close: char,
    html_open: &str,
    html_close: &str,
) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == open {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == close) {
                let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                result.push_str(html_open);
                result.push_str(&inner);
                result.push_str(html_close);
                i += end + 2;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

fn replace_double_pattern(text: &str, marker: &str, html_open: &str, html_close: &str) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(marker) {
        result.push_str(&rest[..start]);
        let after = &rest[start + marker.len()..];
        if let Some(end) = after.find(marker) {
            result.push_str(html_open);
            result.push_str(&after[..end]);
            result.push_str(html_close);
            rest = &after[end + marker.len()..];
        } else {
            result.push_str(marker);
            rest = after;
        }
    }
    result.push_str(rest);
    result
}

fn replace_single_emphasis(text: &str, marker: char, html_open: &str, html_close: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == marker {
            // Don't match if next char is same marker (that would be bold)
            if i + 1 < chars.len() && chars[i + 1] == marker {
                result.push(chars[i]);
                i += 1;
                continue;
            }
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == marker) {
                let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                result.push_str(html_open);
                result.push_str(&inner);
                result.push_str(html_close);
                i += end + 2;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

fn render_inline_links(text: &str) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(lb) = rest.find('[') {
        result.push_str(&rest[..lb]);
        let after = &rest[lb + 1..];
        if let Some(rb) = after.find(']') {
            let link_text = &after[..rb];
            let after_rb = &after[rb + 1..];
            if after_rb.starts_with('(') {
                if let Some(rp) = after_rb.find(')') {
                    let url = &after_rb[1..rp];
                    // Check for image
                    if result.ends_with('!') {
                        result.pop();
                        result.push_str(&format!(r#"<img src="{url}" alt="{link_text}">"#));
                    } else {
                        result.push_str(&format!(r#"<a href="{url}">{link_text}</a>"#));
                    }
                    rest = &after_rb[rp + 1..];
                    continue;
                }
            }
            result.push('[');
            rest = after;
        } else {
            result.push('[');
            rest = after;
        }
    }
    result.push_str(rest);
    result
}

// ---------------------------------------------------------------------------
// Slug generation
// ---------------------------------------------------------------------------

/// Generate a URL-safe slug from heading text.
pub fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

/// Render AST to HTML.
pub fn render_html(doc: &MdDoc) -> String {
    let mut out = String::new();
    for node in &doc.nodes {
        match node {
            MdNode::Heading { level, text } => {
                let slug = slugify(text);
                let inner = render_inline_html(text);
                out.push_str(&format!(r#"<h{level} id="{slug}">{inner}</h{level}>"#));
                out.push('\n');
            }
            MdNode::Paragraph(text) => {
                let inner = render_inline_html(text);
                out.push_str(&format!("<p>{inner}</p>\n"));
            }
            MdNode::CodeBlock { lang, code } => {
                let cls = if lang.is_empty() {
                    String::new()
                } else {
                    format!(r#" class="language-{lang}""#)
                };
                let escaped = html_escape(code);
                out.push_str(&format!("<pre><code{cls}>{escaped}</code></pre>\n"));
            }
            MdNode::BlockQuote(text) => {
                let inner = render_inline_html(text);
                out.push_str(&format!("<blockquote><p>{inner}</p></blockquote>\n"));
            }
            MdNode::UnorderedList(items) => {
                out.push_str("<ul>\n");
                for item in items {
                    let inner = render_inline_html(item);
                    out.push_str(&format!("  <li>{inner}</li>\n"));
                }
                out.push_str("</ul>\n");
            }
            MdNode::OrderedList(items) => {
                out.push_str("<ol>\n");
                for item in items {
                    let inner = render_inline_html(item);
                    out.push_str(&format!("  <li>{inner}</li>\n"));
                }
                out.push_str("</ol>\n");
            }
            MdNode::HorizontalRule => {
                out.push_str("<hr>\n");
            }
            MdNode::Link { text, url } => {
                out.push_str(&format!(r#"<a href="{url}">{text}</a>"#));
                out.push('\n');
            }
            MdNode::Image { alt, url } => {
                out.push_str(&format!(r#"<img src="{url}" alt="{alt}">"#));
                out.push('\n');
            }
            MdNode::BlankLine => {}
        }
    }
    out
}

/// Render AST to plain text.
pub fn render_plain(doc: &MdDoc) -> String {
    let mut out = String::new();
    for node in &doc.nodes {
        match node {
            MdNode::Heading { level, text } => {
                let prefix = "#".repeat(*level as usize);
                out.push_str(&format!("{prefix} {text}\n\n"));
            }
            MdNode::Paragraph(text) => {
                out.push_str(text);
                out.push_str("\n\n");
            }
            MdNode::CodeBlock { code, .. } => {
                for line in code.lines() {
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            MdNode::BlockQuote(text) => {
                for line in text.lines() {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            MdNode::UnorderedList(items) => {
                for item in items {
                    out.push_str("  • ");
                    out.push_str(item);
                    out.push('\n');
                }
                out.push('\n');
            }
            MdNode::OrderedList(items) => {
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&format!("  {}. ", i + 1));
                    out.push_str(item);
                    out.push('\n');
                }
                out.push('\n');
            }
            MdNode::HorizontalRule => {
                out.push_str("---\n\n");
            }
            MdNode::Link { text, url } | MdNode::Image { alt: text, url } => {
                out.push_str(&format!("{text} ({url})\n"));
            }
            MdNode::BlankLine => {
                out.push('\n');
            }
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ---------------------------------------------------------------------------
// Extractors
// ---------------------------------------------------------------------------

/// Extract table of contents from Markdown.
pub fn toc(doc: &MdDoc) -> Vec<TocEntry> {
    doc.nodes
        .iter()
        .filter_map(|n| match n {
            MdNode::Heading { level, text } => Some(TocEntry {
                level: *level,
                text: text.clone(),
                slug: slugify(text),
            }),
            _ => None,
        })
        .collect()
}

/// Extract all links (including images) from Markdown text.
pub fn extract_links(input: &str) -> Vec<MdLink> {
    let doc = parse(input);
    let mut links = Vec::new();
    for node in &doc.nodes {
        match node {
            MdNode::Link { text, url } => links.push(MdLink {
                text: text.clone(),
                url: url.clone(),
                is_image: false,
            }),
            MdNode::Image { alt, url } => links.push(MdLink {
                text: alt.clone(),
                url: url.clone(),
                is_image: true,
            }),
            MdNode::Paragraph(text) | MdNode::Heading { text, .. } | MdNode::BlockQuote(text) => {
                links.extend(extract_inline_links(text));
            }
            MdNode::UnorderedList(items) | MdNode::OrderedList(items) => {
                for item in items {
                    links.extend(extract_inline_links(item));
                }
            }
            _ => {}
        }
    }
    links
}

/// Analyse heading structure.
pub fn heading_stats(doc: &MdDoc) -> HeadingStats {
    let mut stats = HeadingStats {
        total: 0,
        by_level: [0; 6],
        max_depth: 0,
    };
    for node in &doc.nodes {
        if let MdNode::Heading { level, .. } = node {
            stats.total += 1;
            if *level >= 1 && *level <= 6 {
                stats.by_level[(*level - 1) as usize] += 1;
            }
            if *level > stats.max_depth {
                stats.max_depth = *level;
            }
        }
    }
    stats
}

/// Word count of the document (ignoring code blocks).
pub fn word_count(doc: &MdDoc) -> usize {
    let mut count = 0;
    for node in &doc.nodes {
        match node {
            MdNode::Paragraph(text) | MdNode::Heading { text, .. } | MdNode::BlockQuote(text) => {
                count += text.split_whitespace().count();
            }
            MdNode::UnorderedList(items) | MdNode::OrderedList(items) => {
                for item in items {
                    count += item.split_whitespace().count();
                }
            }
            _ => {}
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        let doc = parse("# Title\n## Subtitle\n### Third");
        assert_eq!(doc.nodes.len(), 3);
        match &doc.nodes[0] {
            MdNode::Heading { level, text } => {
                assert_eq!(*level, 1);
                assert_eq!(text, "Title");
            }
            _ => panic!("expected heading"),
        }
    }

    #[test]
    fn test_parse_paragraph() {
        let doc = parse("Hello world.\nSecond line.\n\nNew paragraph.");
        let paras: Vec<_> = doc
            .nodes
            .iter()
            .filter(|n| matches!(n, MdNode::Paragraph(_)))
            .collect();
        assert_eq!(paras.len(), 2);
    }

    #[test]
    fn test_parse_code_block() {
        let doc = parse("```rust\nfn main() {}\n```");
        match &doc.nodes[0] {
            MdNode::CodeBlock { lang, code } => {
                assert_eq!(lang, "rust");
                assert_eq!(code, "fn main() {}");
            }
            _ => panic!("expected code block"),
        }
    }

    #[test]
    fn test_parse_blockquote() {
        let doc = parse("> Hello\n> World");
        match &doc.nodes[0] {
            MdNode::BlockQuote(text) => assert_eq!(text, "Hello\nWorld"),
            _ => panic!("expected blockquote"),
        }
    }

    #[test]
    fn test_parse_unordered_list() {
        let doc = parse("- alpha\n- beta\n- gamma");
        match &doc.nodes[0] {
            MdNode::UnorderedList(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], "alpha");
            }
            _ => panic!("expected ul"),
        }
    }

    #[test]
    fn test_parse_ordered_list() {
        let doc = parse("1. first\n2. second\n3. third");
        match &doc.nodes[0] {
            MdNode::OrderedList(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], "first");
            }
            _ => panic!("expected ol"),
        }
    }

    #[test]
    fn test_parse_horizontal_rule() {
        let doc = parse("---");
        assert!(matches!(doc.nodes[0], MdNode::HorizontalRule));
        let doc2 = parse("***");
        assert!(matches!(doc2.nodes[0], MdNode::HorizontalRule));
    }

    #[test]
    fn test_render_html_heading() {
        let doc = parse("# Hello");
        let html = render_html(&doc);
        assert!(html.contains(r#"<h1 id="hello">Hello</h1>"#));
    }

    #[test]
    fn test_render_html_paragraph_with_bold() {
        let doc = parse("This is **bold** text.");
        let html = render_html(&doc);
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_render_html_code_block() {
        let doc = parse("```js\nconsole.log(1);\n```");
        let html = render_html(&doc);
        assert!(html.contains(r#"class="language-js""#));
        assert!(html.contains("console.log(1);"));
    }

    #[test]
    fn test_render_plain() {
        let doc = parse("# Title\n\nHello world.");
        let plain = render_plain(&doc);
        assert!(plain.contains("# Title"));
        assert!(plain.contains("Hello world."));
    }

    #[test]
    fn test_toc() {
        let doc = parse("# One\n## Two\n### Three");
        let entries = toc(&doc);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, 1);
        assert_eq!(entries[0].slug, "one");
    }

    #[test]
    fn test_extract_links() {
        let links = extract_links("Check [this](https://example.com) out.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
    }

    #[test]
    fn test_heading_stats() {
        let doc = parse("# A\n## B\n## C\n### D");
        let stats = heading_stats(&doc);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.by_level[0], 1);
        assert_eq!(stats.by_level[1], 2);
        assert_eq!(stats.max_depth, 3);
    }

    #[test]
    fn test_word_count() {
        let doc = parse("# Hello World\n\nThis is a test.\n\n```rust\ncode here\n```");
        let wc = word_count(&doc);
        assert_eq!(wc, 6); // "Hello World" + "This is a test."
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("API Reference"), "api-reference");
    }

    #[test]
    fn test_inline_code() {
        let doc = parse("Use `code` here");
        let html = render_html(&doc);
        assert!(html.contains("<code>code</code>"));
    }
}
