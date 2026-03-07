//! # Report Composition Engine
//!
//! Multi-section reports from pipeline results, template rendering,
//! and output as Markdown, HTML, or structured JSON.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Report output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Html,
    Json,
    PlainText,
}

/// A key-value metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: MetricValue,
    pub unit: Option<String>,
}

/// Metric value variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Percentage(f64),
}

impl std::fmt::Display for MetricValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricValue::Int(n) => write!(f, "{n}"),
            MetricValue::Float(v) => write!(f, "{v:.2}"),
            MetricValue::Text(s) => write!(f, "{s}"),
            MetricValue::Bool(b) => write!(f, "{b}"),
            MetricValue::Percentage(p) => write!(f, "{p:.1}%"),
        }
    }
}

/// Table data for inclusion in report sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl ReportTable {
    pub fn new(headers: Vec<String>) -> Self {
        Self {
            headers,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }
}

/// Content block within a section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SectionContent {
    /// Free-form text paragraph.
    Text(String),
    /// Bullet list.
    List(Vec<String>),
    /// Key-value metrics.
    Metrics(Vec<Metric>),
    /// Tabular data.
    Table(ReportTable),
    /// Code block with optional language tag.
    Code {
        language: Option<String>,
        code: String,
    },
    /// A horizontal rule / divider.
    Divider,
}

/// A report section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section title.
    pub title: String,
    /// Heading level (1–6).
    pub level: u8,
    /// Content blocks in display order.
    pub content: Vec<SectionContent>,
}

impl Section {
    pub fn new(title: &str, level: u8) -> Self {
        Self {
            title: title.to_string(),
            level: level.clamp(1, 6),
            content: Vec::new(),
        }
    }

    pub fn add_text(&mut self, text: &str) -> &mut Self {
        self.content.push(SectionContent::Text(text.to_string()));
        self
    }

    pub fn add_list(&mut self, items: Vec<String>) -> &mut Self {
        self.content.push(SectionContent::List(items));
        self
    }

    pub fn add_metrics(&mut self, metrics: Vec<Metric>) -> &mut Self {
        self.content.push(SectionContent::Metrics(metrics));
        self
    }

    pub fn add_table(&mut self, table: ReportTable) -> &mut Self {
        self.content.push(SectionContent::Table(table));
        self
    }

    pub fn add_code(&mut self, code: &str, language: Option<&str>) -> &mut Self {
        self.content.push(SectionContent::Code {
            language: language.map(|s| s.to_string()),
            code: code.to_string(),
        });
        self
    }

    pub fn add_divider(&mut self) -> &mut Self {
        self.content.push(SectionContent::Divider);
        self
    }
}

/// Report metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMeta {
    pub title: String,
    pub author: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub extra: BTreeMap<String, String>,
}

impl ReportMeta {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            author: None,
            date: None,
            description: None,
            tags: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = Some(author.to_string());
        self
    }

    pub fn with_date(mut self, date: &str) -> Self {
        self.date = Some(date.to_string());
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

/// The full report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub meta: ReportMeta,
    pub sections: Vec<Section>,
}

impl Report {
    pub fn new(meta: ReportMeta) -> Self {
        Self {
            meta,
            sections: Vec::new(),
        }
    }

    pub fn add_section(&mut self, section: Section) -> &mut Self {
        self.sections.push(section);
        self
    }

    /// Render the report in the specified format.
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => self.render_markdown(),
            OutputFormat::Html => self.render_html(),
            OutputFormat::Json => self.render_json(),
            OutputFormat::PlainText => self.render_plain(),
        }
    }

    // ---- Markdown ----
    fn render_markdown(&self) -> String {
        let mut out = String::new();
        // Title
        out.push_str(&format!("# {}\n\n", self.meta.title));
        // Metadata
        if let Some(ref author) = self.meta.author {
            out.push_str(&format!("**Author:** {author}\n"));
        }
        if let Some(ref date) = self.meta.date {
            out.push_str(&format!("**Date:** {date}\n"));
        }
        if let Some(ref desc) = self.meta.description {
            out.push_str(&format!("\n> {desc}\n"));
        }
        out.push('\n');

        for section in &self.sections {
            let hashes = "#".repeat(section.level as usize);
            out.push_str(&format!("{hashes} {}\n\n", section.title));

            for block in &section.content {
                match block {
                    SectionContent::Text(t) => {
                        out.push_str(t);
                        out.push_str("\n\n");
                    }
                    SectionContent::List(items) => {
                        for item in items {
                            out.push_str(&format!("- {item}\n"));
                        }
                        out.push('\n');
                    }
                    SectionContent::Metrics(metrics) => {
                        out.push_str("| Metric | Value |\n|--------|-------|\n");
                        for m in metrics {
                            let unit = m.unit.as_deref().unwrap_or("");
                            out.push_str(&format!("| {} | {}{} |\n", m.name, m.value, unit));
                        }
                        out.push('\n');
                    }
                    SectionContent::Table(table) => {
                        // Header
                        out.push_str("| ");
                        out.push_str(&table.headers.join(" | "));
                        out.push_str(" |\n");
                        // Separator
                        out.push('|');
                        for _ in &table.headers {
                            out.push_str("------|");
                        }
                        out.push('\n');
                        // Rows
                        for row in &table.rows {
                            out.push_str("| ");
                            out.push_str(&row.join(" | "));
                            out.push_str(" |\n");
                        }
                        out.push('\n');
                    }
                    SectionContent::Code { language, code } => {
                        let lang = language.as_deref().unwrap_or("");
                        out.push_str(&format!("```{lang}\n{code}\n```\n\n"));
                    }
                    SectionContent::Divider => {
                        out.push_str("---\n\n");
                    }
                }
            }
        }
        out
    }

    // ---- HTML ----
    fn render_html(&self) -> String {
        let mut out = String::new();
        out.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        out.push_str(&format!(
            "  <title>{}</title>\n",
            html_escape(&self.meta.title)
        ));
        out.push_str(
            "  <style>body{font-family:sans-serif;max-width:900px;margin:0 auto;padding:20px}",
        );
        out.push_str("table{border-collapse:collapse;width:100%}th,td{border:1px solid #ddd;padding:8px;text-align:left}");
        out.push_str("th{background:#f4f4f4}pre{background:#f8f8f8;padding:12px;overflow-x:auto}");
        out.push_str(
            "blockquote{border-left:4px solid #ccc;margin:0;padding:8px 16px;color:#666}</style>\n",
        );
        out.push_str("</head>\n<body>\n");

        out.push_str(&format!("<h1>{}</h1>\n", html_escape(&self.meta.title)));
        if let Some(ref author) = self.meta.author {
            out.push_str(&format!(
                "<p><strong>Author:</strong> {}</p>\n",
                html_escape(author)
            ));
        }
        if let Some(ref date) = self.meta.date {
            out.push_str(&format!(
                "<p><strong>Date:</strong> {}</p>\n",
                html_escape(date)
            ));
        }
        if let Some(ref desc) = self.meta.description {
            out.push_str(&format!("<blockquote>{}</blockquote>\n", html_escape(desc)));
        }

        for section in &self.sections {
            let tag = format!("h{}", section.level.min(6));
            out.push_str(&format!("<{tag}>{}</{tag}>\n", html_escape(&section.title)));

            for block in &section.content {
                match block {
                    SectionContent::Text(t) => {
                        out.push_str(&format!("<p>{}</p>\n", html_escape(t)));
                    }
                    SectionContent::List(items) => {
                        out.push_str("<ul>\n");
                        for item in items {
                            out.push_str(&format!("  <li>{}</li>\n", html_escape(item)));
                        }
                        out.push_str("</ul>\n");
                    }
                    SectionContent::Metrics(metrics) => {
                        out.push_str("<table>\n<tr><th>Metric</th><th>Value</th></tr>\n");
                        for m in metrics {
                            let unit = m.unit.as_deref().unwrap_or("");
                            out.push_str(&format!(
                                "<tr><td>{}</td><td>{}{}</td></tr>\n",
                                html_escape(&m.name),
                                m.value,
                                html_escape(unit),
                            ));
                        }
                        out.push_str("</table>\n");
                    }
                    SectionContent::Table(table) => {
                        out.push_str("<table>\n<tr>");
                        for h in &table.headers {
                            out.push_str(&format!("<th>{}</th>", html_escape(h)));
                        }
                        out.push_str("</tr>\n");
                        for row in &table.rows {
                            out.push_str("<tr>");
                            for cell in row {
                                out.push_str(&format!("<td>{}</td>", html_escape(cell)));
                            }
                            out.push_str("</tr>\n");
                        }
                        out.push_str("</table>\n");
                    }
                    SectionContent::Code { language, code } => {
                        let cls = language
                            .as_ref()
                            .map(|l| format!(" class=\"language-{}\"", html_escape(l)))
                            .unwrap_or_default();
                        out.push_str(&format!(
                            "<pre><code{cls}>{}</code></pre>\n",
                            html_escape(code)
                        ));
                    }
                    SectionContent::Divider => {
                        out.push_str("<hr>\n");
                    }
                }
            }
        }

        out.push_str("</body>\n</html>\n");
        out
    }

    // ---- JSON ----
    fn render_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    // ---- Plain text ----
    fn render_plain(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.meta.title);
        out.push('\n');
        out.push_str(&"=".repeat(self.meta.title.len()));
        out.push_str("\n\n");

        if let Some(ref author) = self.meta.author {
            out.push_str(&format!("Author: {author}\n"));
        }
        if let Some(ref date) = self.meta.date {
            out.push_str(&format!("Date: {date}\n"));
        }
        if let Some(ref desc) = self.meta.description {
            out.push_str(&format!("{desc}\n"));
        }
        out.push('\n');

        for section in &self.sections {
            out.push_str(&section.title);
            out.push('\n');
            out.push_str(&"-".repeat(section.title.len()));
            out.push_str("\n\n");

            for block in &section.content {
                match block {
                    SectionContent::Text(t) => {
                        out.push_str(t);
                        out.push_str("\n\n");
                    }
                    SectionContent::List(items) => {
                        for item in items {
                            out.push_str(&format!("  * {item}\n"));
                        }
                        out.push('\n');
                    }
                    SectionContent::Metrics(metrics) => {
                        for m in metrics {
                            let unit = m.unit.as_deref().unwrap_or("");
                            out.push_str(&format!("  {}: {}{}\n", m.name, m.value, unit));
                        }
                        out.push('\n');
                    }
                    SectionContent::Table(table) => {
                        out.push_str(&table.headers.join("\t"));
                        out.push('\n');
                        for row in &table.rows {
                            out.push_str(&row.join("\t"));
                            out.push('\n');
                        }
                        out.push('\n');
                    }
                    SectionContent::Code { code, .. } => {
                        for line in code.lines() {
                            out.push_str(&format!("    {line}\n"));
                        }
                        out.push('\n');
                    }
                    SectionContent::Divider => {
                        out.push_str(&"-".repeat(40));
                        out.push_str("\n\n");
                    }
                }
            }
        }
        out
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Template rendering
// ---------------------------------------------------------------------------

/// Simple template engine: replaces `{{key}}` placeholders with values.
pub fn render_template(template: &str, vars: &BTreeMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

/// Render a report section from a template string and variables.
pub fn section_from_template(
    title: &str,
    level: u8,
    template: &str,
    vars: &BTreeMap<String, String>,
) -> Section {
    let text = render_template(template, vars);
    let mut section = Section::new(title, level);
    section.add_text(&text);
    section
}

// ---------------------------------------------------------------------------
// Convenience builders
// ---------------------------------------------------------------------------

/// Create a metric with an integer value.
pub fn int_metric(name: &str, value: i64, unit: Option<&str>) -> Metric {
    Metric {
        name: name.to_string(),
        value: MetricValue::Int(value),
        unit: unit.map(|s| s.to_string()),
    }
}

/// Create a metric with a float value.
pub fn float_metric(name: &str, value: f64, unit: Option<&str>) -> Metric {
    Metric {
        name: name.to_string(),
        value: MetricValue::Float(value),
        unit: unit.map(|s| s.to_string()),
    }
}

/// Create a percentage metric.
pub fn pct_metric(name: &str, value: f64) -> Metric {
    Metric {
        name: name.to_string(),
        value: MetricValue::Percentage(value),
        unit: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> Report {
        let meta = ReportMeta::new("Test Report")
            .with_author("ATP")
            .with_date("2026-03-06")
            .with_description("A test report");

        let mut report = Report::new(meta);

        let mut summary = Section::new("Summary", 2);
        summary.add_text("This is a test report.");
        summary.add_metrics(vec![
            int_metric("Files", 42, None),
            pct_metric("Coverage", 87.5),
        ]);
        report.add_section(summary);

        let mut details = Section::new("Details", 2);
        details.add_list(vec!["Item A".into(), "Item B".into()]);
        let mut table = ReportTable::new(vec!["Name".into(), "Status".into()]);
        table.add_row(vec!["Alpha".into(), "Pass".into()]);
        table.add_row(vec!["Beta".into(), "Fail".into()]);
        details.add_table(table);
        details.add_code("fn main() {}", Some("rust"));
        details.add_divider();
        report.add_section(details);

        report
    }

    #[test]
    fn test_markdown_render() {
        let report = sample_report();
        let md = report.render(OutputFormat::Markdown);
        assert!(md.contains("# Test Report"));
        assert!(md.contains("**Author:** ATP"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("| Files | 42 |"));
        assert!(md.contains("- Item A"));
        assert!(md.contains("```rust"));
        assert!(md.contains("---"));
    }

    #[test]
    fn test_html_render() {
        let report = sample_report();
        let html = report.render(OutputFormat::Html);
        assert!(html.contains("<h1>Test Report</h1>"));
        assert!(html.contains("<h2>Summary</h2>"));
        assert!(html.contains("<table>"));
        assert!(html.contains("<li>Item A</li>"));
        assert!(html.contains("<pre>"));
        assert!(html.contains("<hr>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_json_render() {
        let report = sample_report();
        let json = report.render(OutputFormat::Json);
        assert!(json.contains("\"title\""));
        assert!(json.contains("Test Report"));
        assert!(json.contains("Summary"));
    }

    #[test]
    fn test_plain_render() {
        let report = sample_report();
        let plain = report.render(OutputFormat::PlainText);
        assert!(plain.contains("Test Report"));
        assert!(plain.contains("Author: ATP"));
        assert!(plain.contains("Summary"));
        assert!(plain.contains("* Item A"));
    }

    #[test]
    fn test_template_render() {
        let mut vars = BTreeMap::new();
        vars.insert("name".into(), "World".into());
        vars.insert("count".into(), "42".into());
        let result = render_template("Hello {{name}}, you have {{count}} items.", &vars);
        assert_eq!(result, "Hello World, you have 42 items.");
    }

    #[test]
    fn test_section_from_template() {
        let mut vars = BTreeMap::new();
        vars.insert("status".into(), "complete".into());
        let section = section_from_template("Status", 3, "Build is {{status}}.", &vars);
        assert_eq!(section.title, "Status");
        assert_eq!(section.level, 3);
        match &section.content[0] {
            SectionContent::Text(t) => assert_eq!(t, "Build is complete."),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_report_table() {
        let mut table = ReportTable::new(vec!["A".into(), "B".into()]);
        table.add_row(vec!["1".into(), "2".into()]);
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows.len(), 1);
    }

    #[test]
    fn test_metric_display() {
        assert_eq!(MetricValue::Int(42).to_string(), "42");
        assert_eq!(MetricValue::Percentage(87.5).to_string(), "87.5%");
        assert_eq!(MetricValue::Bool(true).to_string(), "true");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<b>test</b>"), "&lt;b&gt;test&lt;/b&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_empty_report() {
        let meta = ReportMeta::new("Empty");
        let report = Report::new(meta);
        let md = report.render(OutputFormat::Markdown);
        assert!(md.contains("# Empty"));
    }

    #[test]
    fn test_section_builder() {
        let mut s = Section::new("Test", 2);
        s.add_text("Hello").add_list(vec!["A".into()]).add_divider();
        assert_eq!(s.content.len(), 3);
    }

    #[test]
    fn test_report_meta_builder() {
        let meta = ReportMeta::new("Title")
            .with_author("Me")
            .with_date("2026-01-01")
            .with_description("Desc");
        assert_eq!(meta.author.as_deref(), Some("Me"));
        assert_eq!(meta.date.as_deref(), Some("2026-01-01"));
    }
}
