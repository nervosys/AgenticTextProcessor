//! # Text Annotation Engine
//!
//! Span-based labels/tags with overlap resolution, category taxonomies,
//! export to standoff, inline, and JSON formats, and inter-annotator
//! agreement computation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single text span annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    /// Unique identifier.
    pub id: String,
    /// Character offset start (inclusive).
    pub start: usize,
    /// Character offset end (exclusive).
    pub end: usize,
    /// Label/category.
    pub label: String,
    /// Annotator identifier.
    pub annotator: Option<String>,
    /// Free-form attributes.
    pub attrs: BTreeMap<String, String>,
}

impl Annotation {
    pub fn new(id: &str, start: usize, end: usize, label: &str) -> Self {
        Self {
            id: id.into(),
            start,
            end,
            label: label.into(),
            annotator: None,
            attrs: BTreeMap::new(),
        }
    }

    pub fn with_annotator(mut self, annotator: &str) -> Self {
        self.annotator = Some(annotator.into());
        self
    }

    pub fn with_attr(mut self, key: &str, value: &str) -> Self {
        self.attrs.insert(key.into(), value.into());
        self
    }

    /// Length of the span.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Is the span empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Does this span overlap with another?
    pub fn overlaps(&self, other: &Annotation) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Extract the text covered by this annotation.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        let chars: Vec<char> = source.chars().collect();
        let start = self.start.min(chars.len());
        let end = self.end.min(chars.len());
        // Convert char offsets to byte offsets
        let byte_start: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
        let byte_end: usize = chars[..end].iter().map(|c| c.len_utf8()).sum();
        &source[byte_start..byte_end]
    }
}

/// A taxonomy category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub children: Vec<Category>,
}

impl Category {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            description: None,
            color: None,
            children: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_child(mut self, child: Category) -> Self {
        self.children.push(child);
        self
    }

    /// Flatten taxonomy to list of label names.
    pub fn flatten(&self) -> Vec<String> {
        let mut result = vec![self.name.clone()];
        for child in &self.children {
            result.extend(child.flatten());
        }
        result
    }
}

/// An annotated document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedDoc {
    pub text: String,
    pub annotations: Vec<Annotation>,
    pub taxonomy: Option<Vec<Category>>,
}

/// Overlap resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapStrategy {
    /// Allow overlapping annotations.
    Allow,
    /// Keep the first (earlier start, then longer).
    KeepFirst,
    /// Keep the longest span.
    KeepLongest,
    /// Merge overlapping spans with same label.
    Merge,
}

// ---------------------------------------------------------------------------
// AnnotatedDoc implementation
// ---------------------------------------------------------------------------

impl AnnotatedDoc {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            annotations: Vec::new(),
            taxonomy: None,
        }
    }

    pub fn add(&mut self, ann: Annotation) {
        self.annotations.push(ann);
    }

    pub fn set_taxonomy(&mut self, taxonomy: Vec<Category>) {
        self.taxonomy = Some(taxonomy);
    }

    /// Get annotations by label.
    pub fn by_label(&self, label: &str) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.label == label)
            .collect()
    }

    /// Get annotations by annotator.
    pub fn by_annotator(&self, annotator: &str) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.annotator.as_deref() == Some(annotator))
            .collect()
    }

    /// Find annotations overlapping a position.
    pub fn at_position(&self, pos: usize) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.start <= pos && pos < a.end)
            .collect()
    }

    /// Resolve overlapping annotations.
    pub fn resolve_overlaps(&mut self, strategy: OverlapStrategy) {
        match strategy {
            OverlapStrategy::Allow => {}
            OverlapStrategy::KeepFirst => {
                self.annotations
                    .sort_by_key(|a| (a.start, std::cmp::Reverse(a.len())));
                let mut kept = Vec::new();
                for ann in &self.annotations {
                    let overlaps = kept.iter().any(|k: &Annotation| k.overlaps(ann));
                    if !overlaps {
                        kept.push(ann.clone());
                    }
                }
                self.annotations = kept;
            }
            OverlapStrategy::KeepLongest => {
                self.annotations
                    .sort_by(|a, b| b.len().cmp(&a.len()).then(a.start.cmp(&b.start)));
                let mut kept = Vec::new();
                for ann in &self.annotations {
                    let overlaps = kept.iter().any(|k: &Annotation| k.overlaps(ann));
                    if !overlaps {
                        kept.push(ann.clone());
                    }
                }
                kept.sort_by_key(|a| a.start);
                self.annotations = kept;
            }
            OverlapStrategy::Merge => {
                self.annotations.sort_by_key(|a| (a.label.clone(), a.start));
                let mut merged = Vec::new();
                let mut i = 0;
                while i < self.annotations.len() {
                    let mut current = self.annotations[i].clone();
                    while i + 1 < self.annotations.len()
                        && self.annotations[i + 1].label == current.label
                        && self.annotations[i + 1].start <= current.end
                    {
                        i += 1;
                        current.end = current.end.max(self.annotations[i].end);
                    }
                    merged.push(current);
                    i += 1;
                }
                merged.sort_by_key(|a| a.start);
                self.annotations = merged;
            }
        }
    }

    /// Label distribution counts.
    pub fn label_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for ann in &self.annotations {
            *counts.entry(ann.label.clone()).or_insert(0) += 1;
        }
        counts
    }
}

// ---------------------------------------------------------------------------
// Export formats
// ---------------------------------------------------------------------------

/// Export as standoff format (one annotation per line: id, start, end, label, text).
pub fn to_standoff(doc: &AnnotatedDoc) -> String {
    let mut out = String::new();
    for ann in &doc.annotations {
        let text = ann.text(&doc.text).replace('\n', " ");
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            ann.id, ann.start, ann.end, ann.label, text
        ));
    }
    out
}

/// Export as inline annotations (insert markers).
pub fn to_inline(doc: &AnnotatedDoc) -> String {
    let chars: Vec<char> = doc.text.chars().collect();
    let mut events: Vec<(usize, bool, String)> = Vec::new();

    for ann in &doc.annotations {
        events.push((ann.start, true, format!("[{}:", ann.label)));
        events.push((ann.end, false, "]".into()));
    }
    // Sort: position, then closes before opens at same position
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut out = String::new();
    let mut pos = 0;
    for (offset, _is_open, marker) in &events {
        while pos < *offset && pos < chars.len() {
            out.push(chars[pos]);
            pos += 1;
        }
        out.push_str(marker);
    }
    while pos < chars.len() {
        out.push(chars[pos]);
        pos += 1;
    }
    out
}

/// Export as JSON.
pub fn to_json(doc: &AnnotatedDoc) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_default()
}

/// Import from JSON.
pub fn from_json(json: &str) -> Result<AnnotatedDoc, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Inter-annotator agreement
// ---------------------------------------------------------------------------

/// Cohen's kappa between two annotators on a token-level basis.
pub fn cohens_kappa(doc: &AnnotatedDoc, annotator_a: &str, annotator_b: &str) -> f64 {
    let len = doc.text.chars().count();
    if len == 0 {
        return 0.0;
    }

    let labels_a = build_label_array(doc, annotator_a, len);
    let labels_b = build_label_array(doc, annotator_b, len);

    let mut agree = 0usize;
    for i in 0..len {
        if labels_a[i] == labels_b[i] {
            agree += 1;
        }
    }

    let po = agree as f64 / len as f64;

    // Expected agreement: sum of P(a=label)*P(b=label)
    let mut freq_a: BTreeMap<&str, usize> = BTreeMap::new();
    let mut freq_b: BTreeMap<&str, usize> = BTreeMap::new();
    for i in 0..len {
        *freq_a.entry(&labels_a[i]).or_insert(0) += 1;
        *freq_b.entry(&labels_b[i]).or_insert(0) += 1;
    }
    let mut all_labels: Vec<&str> = freq_a.keys().copied().collect();
    for k in freq_b.keys() {
        if !all_labels.contains(k) {
            all_labels.push(k);
        }
    }

    let pe: f64 = all_labels
        .iter()
        .map(|&label| {
            let pa = *freq_a.get(label).unwrap_or(&0) as f64 / len as f64;
            let pb = *freq_b.get(label).unwrap_or(&0) as f64 / len as f64;
            pa * pb
        })
        .sum();

    if (1.0 - pe).abs() < 1e-10 {
        1.0
    } else {
        (po - pe) / (1.0 - pe)
    }
}

fn build_label_array(doc: &AnnotatedDoc, annotator: &str, len: usize) -> Vec<String> {
    let mut labels = vec!["O".to_string(); len];
    for ann in &doc.annotations {
        if ann.annotator.as_deref() == Some(annotator) {
            for label in labels.iter_mut().take(ann.end.min(len)).skip(ann.start) {
                *label = ann.label.clone();
            }
        }
    }
    labels
}

/// Annotation statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationStats {
    pub total: usize,
    pub labels: BTreeMap<String, usize>,
    pub avg_span_length: f64,
    pub overlap_count: usize,
    pub annotators: Vec<String>,
}

/// Compute annotation statistics.
pub fn stats(doc: &AnnotatedDoc) -> AnnotationStats {
    let labels = doc.label_counts();
    let total = doc.annotations.len();
    let avg_span_length = if total > 0 {
        doc.annotations.iter().map(|a| a.len()).sum::<usize>() as f64 / total as f64
    } else {
        0.0
    };

    let mut overlaps = 0;
    for i in 0..doc.annotations.len() {
        for j in (i + 1)..doc.annotations.len() {
            if doc.annotations[i].overlaps(&doc.annotations[j]) {
                overlaps += 1;
            }
        }
    }

    let mut annotators: Vec<String> = doc
        .annotations
        .iter()
        .filter_map(|a| a.annotator.clone())
        .collect();
    annotators.sort();
    annotators.dedup();

    AnnotationStats {
        total,
        labels,
        avg_span_length,
        overlap_count: overlaps,
        annotators,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> AnnotatedDoc {
        let mut doc = AnnotatedDoc::new("The quick brown fox jumps over the lazy dog.");
        doc.add(Annotation::new("a1", 0, 3, "DET"));
        doc.add(Annotation::new("a2", 4, 9, "ADJ"));
        doc.add(Annotation::new("a3", 10, 15, "ADJ"));
        doc.add(Annotation::new("a4", 16, 19, "NOUN"));
        doc
    }

    #[test]
    fn test_annotation_text() {
        let doc = sample_doc();
        assert_eq!(doc.annotations[0].text(&doc.text), "The");
        assert_eq!(doc.annotations[1].text(&doc.text), "quick");
        assert_eq!(doc.annotations[3].text(&doc.text), "fox");
    }

    #[test]
    fn test_annotation_overlaps() {
        let a = Annotation::new("1", 0, 5, "X");
        let b = Annotation::new("2", 3, 8, "Y");
        let c = Annotation::new("3", 5, 10, "Z");
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn test_by_label() {
        let doc = sample_doc();
        assert_eq!(doc.by_label("ADJ").len(), 2);
        assert_eq!(doc.by_label("NOUN").len(), 1);
    }

    #[test]
    fn test_at_position() {
        let doc = sample_doc();
        let at5 = doc.at_position(5);
        assert_eq!(at5.len(), 1);
        assert_eq!(at5[0].label, "ADJ");
    }

    #[test]
    fn test_resolve_keep_first() {
        let mut doc = AnnotatedDoc::new("abcdefgh");
        doc.add(Annotation::new("1", 0, 4, "X"));
        doc.add(Annotation::new("2", 2, 6, "Y")); // overlaps with 1
        doc.add(Annotation::new("3", 6, 8, "Z")); // no overlap
        doc.resolve_overlaps(OverlapStrategy::KeepFirst);
        assert_eq!(doc.annotations.len(), 2);
        assert_eq!(doc.annotations[0].id, "1");
        assert_eq!(doc.annotations[1].id, "3");
    }

    #[test]
    fn test_resolve_keep_longest() {
        let mut doc = AnnotatedDoc::new("abcdefgh");
        doc.add(Annotation::new("1", 0, 3, "X")); // len 3
        doc.add(Annotation::new("2", 1, 6, "Y")); // len 5 — longest
        doc.resolve_overlaps(OverlapStrategy::KeepLongest);
        assert_eq!(doc.annotations.len(), 1);
        assert_eq!(doc.annotations[0].id, "2");
    }

    #[test]
    fn test_resolve_merge() {
        let mut doc = AnnotatedDoc::new("abcdefgh");
        doc.add(Annotation::new("1", 0, 4, "X"));
        doc.add(Annotation::new("2", 3, 7, "X")); // same label, overlapping
        doc.resolve_overlaps(OverlapStrategy::Merge);
        assert_eq!(doc.annotations.len(), 1);
        assert_eq!(doc.annotations[0].start, 0);
        assert_eq!(doc.annotations[0].end, 7);
    }

    #[test]
    fn test_standoff_export() {
        let doc = sample_doc();
        let standoff = to_standoff(&doc);
        assert!(standoff.contains("a1\t0\t3\tDET\tThe"));
        assert!(standoff.contains("a4\t16\t19\tNOUN\tfox"));
    }

    #[test]
    fn test_inline_export() {
        let mut doc = AnnotatedDoc::new("Hello World");
        doc.add(Annotation::new("1", 0, 5, "GREET"));
        let inline = to_inline(&doc);
        assert!(inline.contains("[GREET:Hello]"));
    }

    #[test]
    fn test_json_roundtrip() {
        let doc = sample_doc();
        let json = to_json(&doc);
        let doc2 = from_json(&json).unwrap();
        assert_eq!(doc.annotations.len(), doc2.annotations.len());
        assert_eq!(doc.text, doc2.text);
    }

    #[test]
    fn test_label_counts() {
        let doc = sample_doc();
        let counts = doc.label_counts();
        assert_eq!(counts["ADJ"], 2);
        assert_eq!(counts["DET"], 1);
    }

    #[test]
    fn test_stats() {
        let doc = sample_doc();
        let s = stats(&doc);
        assert_eq!(s.total, 4);
        assert!(s.avg_span_length > 0.0);
        assert_eq!(s.overlap_count, 0);
    }

    #[test]
    fn test_cohens_kappa_perfect() {
        let mut doc = AnnotatedDoc::new("abcdef");
        doc.add(Annotation::new("1", 0, 3, "X").with_annotator("A"));
        doc.add(Annotation::new("2", 0, 3, "X").with_annotator("B"));
        let kappa = cohens_kappa(&doc, "A", "B");
        assert!(kappa > 0.99); // perfect agreement
    }

    #[test]
    fn test_cohens_kappa_different() {
        let mut doc = AnnotatedDoc::new("abcdef");
        doc.add(Annotation::new("1", 0, 3, "X").with_annotator("A"));
        doc.add(Annotation::new("2", 3, 6, "Y").with_annotator("B"));
        let kappa = cohens_kappa(&doc, "A", "B");
        assert!(kappa < 0.5); // poor agreement
    }

    #[test]
    fn test_taxonomy() {
        let tax = Category::new("NER")
            .with_child(Category::new("PERSON"))
            .with_child(Category::new("ORG").with_child(Category::new("GOV")));
        let flat = tax.flatten();
        assert!(flat.contains(&"NER".to_string()));
        assert!(flat.contains(&"PERSON".to_string()));
        assert!(flat.contains(&"GOV".to_string()));
    }

    #[test]
    fn test_annotation_attrs() {
        let ann = Annotation::new("1", 0, 5, "ENTITY")
            .with_attr("confidence", "0.95")
            .with_annotator("model-v1");
        assert_eq!(ann.attrs["confidence"], "0.95");
        assert_eq!(ann.annotator.as_deref(), Some("model-v1"));
    }
}
