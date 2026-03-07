//! Prometheus-style metrics collection.
//!
//! Counters, gauges, histograms with configurable bucket quantiles,
//! label dimensions, text-format export (OpenMetrics-compatible),
//! and pipeline instrumentation helpers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Label set: dimension key → value.
pub type Labels = BTreeMap<String, String>;

/// A metric identifier (name + labels).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MetricKey {
    pub name: String,
    pub labels: Labels,
}

impl MetricKey {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            labels: BTreeMap::new(),
        }
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    /// Format labels as Prometheus `{key="val",...}`.
    fn labels_str(&self) -> String {
        if self.labels.is_empty() {
            return String::new();
        }
        let pairs: Vec<String> = self
            .labels
            .iter()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect();
        format!("{{{}}}", pairs.join(","))
    }
}

/// Counter metric (monotonically increasing).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Counter {
    pub value: f64,
}

/// Gauge metric (can go up and down).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Gauge {
    pub value: f64,
}

/// Histogram bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    /// Upper bound (inclusive).
    pub le: f64,
    /// Count of observations ≤ le.
    pub count: u64,
}

/// Histogram metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Histogram {
    /// Buckets (sorted by le).
    pub buckets: Vec<HistogramBucket>,
    /// Sum of all observed values.
    pub sum: f64,
    /// Total count of observations.
    pub count: u64,
}

impl Histogram {
    /// Create a histogram with the given bucket boundaries.
    pub fn with_buckets(bounds: &[f64]) -> Self {
        let mut buckets: Vec<HistogramBucket> = bounds
            .iter()
            .map(|&le| HistogramBucket { le, count: 0 })
            .collect();
        buckets.push(HistogramBucket {
            le: f64::INFINITY,
            count: 0,
        });
        Self {
            buckets,
            sum: 0.0,
            count: 0,
        }
    }

    /// Observe a value.
    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
        for bucket in &mut self.buckets {
            if value <= bucket.le {
                bucket.count += 1;
            }
        }
    }

    /// Default buckets suitable for latency (in seconds).
    pub fn default_buckets() -> Vec<f64> {
        vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]
    }
}

/// What kind of metric this is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(Counter),
    Gauge(Gauge),
    Histogram(Histogram),
}

/// A metric with its help text and type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub key: MetricKey,
    pub help: String,
    pub value: MetricValue,
}

// ---------------------------------------------------------------------------
// MetricsRegistry
// ---------------------------------------------------------------------------

/// Thread-safe metrics registry.
#[derive(Debug, Clone)]
pub struct MetricsRegistry {
    metrics: Arc<Mutex<BTreeMap<MetricKey, MetricEntry>>>,
    help_texts: Arc<Mutex<BTreeMap<String, String>>>,
}

impl MetricsRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(BTreeMap::new())),
            help_texts: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Register a help text for a metric name.
    pub fn register_help(&self, name: &str, help: &str) {
        self.help_texts
            .lock()
            .unwrap()
            .insert(name.to_string(), help.to_string());
    }

    /// Increment a counter.
    pub fn counter_inc(&self, key: &MetricKey) {
        self.counter_add(key, 1.0);
    }

    /// Add to a counter.
    pub fn counter_add(&self, key: &MetricKey, value: f64) {
        let mut metrics = self.metrics.lock().unwrap();
        let help = self.get_help(&key.name);
        let entry = metrics.entry(key.clone()).or_insert_with(|| MetricEntry {
            key: key.clone(),
            help: help.clone(),
            value: MetricValue::Counter(Counter::default()),
        });
        if let MetricValue::Counter(ref mut c) = entry.value {
            c.value += value;
        }
    }

    /// Set a gauge value.
    pub fn gauge_set(&self, key: &MetricKey, value: f64) {
        let mut metrics = self.metrics.lock().unwrap();
        let help = self.get_help(&key.name);
        let entry = metrics.entry(key.clone()).or_insert_with(|| MetricEntry {
            key: key.clone(),
            help,
            value: MetricValue::Gauge(Gauge::default()),
        });
        if let MetricValue::Gauge(ref mut g) = entry.value {
            g.value = value;
        }
    }

    /// Increment a gauge.
    pub fn gauge_inc(&self, key: &MetricKey) {
        self.gauge_add(key, 1.0);
    }

    /// Add to a gauge.
    pub fn gauge_add(&self, key: &MetricKey, delta: f64) {
        let mut metrics = self.metrics.lock().unwrap();
        let help = self.get_help(&key.name);
        let entry = metrics.entry(key.clone()).or_insert_with(|| MetricEntry {
            key: key.clone(),
            help,
            value: MetricValue::Gauge(Gauge::default()),
        });
        if let MetricValue::Gauge(ref mut g) = entry.value {
            g.value += delta;
        }
    }

    /// Observe a histogram value.
    pub fn histogram_observe(&self, key: &MetricKey, value: f64) {
        self.histogram_observe_with_buckets(key, value, &Histogram::default_buckets());
    }

    /// Observe a histogram value with custom buckets.
    pub fn histogram_observe_with_buckets(
        &self,
        key: &MetricKey,
        value: f64,
        buckets: &[f64],
    ) {
        let mut metrics = self.metrics.lock().unwrap();
        let help = self.get_help(&key.name);
        let entry = metrics.entry(key.clone()).or_insert_with(|| MetricEntry {
            key: key.clone(),
            help,
            value: MetricValue::Histogram(Histogram::with_buckets(buckets)),
        });
        if let MetricValue::Histogram(ref mut h) = entry.value {
            h.observe(value);
        }
    }

    /// Get the current value of a counter.
    pub fn counter_value(&self, key: &MetricKey) -> Option<f64> {
        let metrics = self.metrics.lock().unwrap();
        metrics.get(key).and_then(|e| match &e.value {
            MetricValue::Counter(c) => Some(c.value),
            _ => None,
        })
    }

    /// Get the current value of a gauge.
    pub fn gauge_value(&self, key: &MetricKey) -> Option<f64> {
        let metrics = self.metrics.lock().unwrap();
        metrics.get(key).and_then(|e| match &e.value {
            MetricValue::Gauge(g) => Some(g.value),
            _ => None,
        })
    }

    /// Get a snapshot of all metrics.
    pub fn snapshot(&self) -> Vec<MetricEntry> {
        self.metrics.lock().unwrap().values().cloned().collect()
    }

    /// Export all metrics in Prometheus text format.
    pub fn export_text(&self) -> String {
        let metrics = self.metrics.lock().unwrap();
        let help_texts = self.help_texts.lock().unwrap();
        let mut output = String::new();

        // Group by metric name
        let mut by_name: BTreeMap<&str, Vec<&MetricEntry>> = BTreeMap::new();
        for entry in metrics.values() {
            by_name
                .entry(entry.key.name.as_str())
                .or_default()
                .push(entry);
        }

        for (name, entries) in &by_name {
            if let Some(help) = help_texts.get(*name) {
                output.push_str(&format!("# HELP {name} {help}\n"));
            }

            // Determine type from first entry
            let type_str = match &entries[0].value {
                MetricValue::Counter(_) => "counter",
                MetricValue::Gauge(_) => "gauge",
                MetricValue::Histogram(_) => "histogram",
            };
            output.push_str(&format!("# TYPE {name} {type_str}\n"));

            for entry in entries {
                let labels = entry.key.labels_str();
                match &entry.value {
                    MetricValue::Counter(c) => {
                        output.push_str(&format!("{name}{labels} {}\n", c.value));
                    }
                    MetricValue::Gauge(g) => {
                        output.push_str(&format!("{name}{labels} {}\n", g.value));
                    }
                    MetricValue::Histogram(h) => {
                        for bucket in &h.buckets {
                            let le = if bucket.le.is_infinite() {
                                "+Inf".to_string()
                            } else {
                                bucket.le.to_string()
                            };
                            let mut blabels = entry.key.labels.clone();
                            blabels.insert("le".to_string(), le);
                            let bkey = MetricKey {
                                name: format!("{name}_bucket"),
                                labels: blabels,
                            };
                            output.push_str(&format!(
                                "{}{} {}\n",
                                bkey.name,
                                bkey.labels_str(),
                                bucket.count
                            ));
                        }
                        output.push_str(&format!("{name}_sum{labels} {}\n", h.sum));
                        output.push_str(&format!("{name}_count{labels} {}\n", h.count));
                    }
                }
            }
        }

        output
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.metrics.lock().unwrap().clear();
    }

    fn get_help(&self, name: &str) -> String {
        self.help_texts
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Pipeline instrumentation helpers
// ---------------------------------------------------------------------------

/// Pre-built metric keys for pipeline instrumentation.
pub fn pipeline_stage_duration_key(stage: &str) -> MetricKey {
    MetricKey::new("atp_pipeline_stage_duration_seconds")
        .with_label("stage", stage)
}

pub fn pipeline_records_processed_key(stage: &str) -> MetricKey {
    MetricKey::new("atp_pipeline_records_processed_total")
        .with_label("stage", stage)
}

pub fn pipeline_errors_key(stage: &str) -> MetricKey {
    MetricKey::new("atp_pipeline_errors_total")
        .with_label("stage", stage)
}

pub fn search_matches_key(pattern: &str) -> MetricKey {
    MetricKey::new("atp_search_matches_total")
        .with_label("pattern", pattern)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("requests_total");
        reg.counter_inc(&key);
        reg.counter_inc(&key);
        reg.counter_add(&key, 3.0);
        assert_eq!(reg.counter_value(&key), Some(5.0));
    }

    #[test]
    fn test_counter_with_labels() {
        let reg = MetricsRegistry::new();
        let k1 = MetricKey::new("http_requests").with_label("method", "GET");
        let k2 = MetricKey::new("http_requests").with_label("method", "POST");
        reg.counter_inc(&k1);
        reg.counter_inc(&k1);
        reg.counter_inc(&k2);
        assert_eq!(reg.counter_value(&k1), Some(2.0));
        assert_eq!(reg.counter_value(&k2), Some(1.0));
    }

    #[test]
    fn test_gauge() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("temperature");
        reg.gauge_set(&key, 72.5);
        assert_eq!(reg.gauge_value(&key), Some(72.5));
        reg.gauge_set(&key, 73.0);
        assert_eq!(reg.gauge_value(&key), Some(73.0));
    }

    #[test]
    fn test_gauge_inc_dec() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("active_connections");
        reg.gauge_set(&key, 0.0);
        reg.gauge_inc(&key);
        reg.gauge_inc(&key);
        reg.gauge_add(&key, -1.0);
        assert_eq!(reg.gauge_value(&key), Some(1.0));
    }

    #[test]
    fn test_histogram() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("request_duration");
        let buckets = vec![0.1, 0.5, 1.0, 5.0];

        reg.histogram_observe_with_buckets(&key, 0.05, &buckets);
        reg.histogram_observe_with_buckets(&key, 0.3, &buckets);
        reg.histogram_observe_with_buckets(&key, 2.0, &buckets);

        let snap = reg.snapshot();
        let entry = snap.iter().find(|e| e.key == key).unwrap();
        if let MetricValue::Histogram(h) = &entry.value {
            assert_eq!(h.count, 3);
            assert!((h.sum - 2.35).abs() < 0.001);
            // le=0.1: count=1, le=0.5: count=2, le=1.0: count=2, le=5.0: count=3, le=+Inf: count=3
            assert_eq!(h.buckets[0].count, 1); // 0.1
            assert_eq!(h.buckets[1].count, 2); // 0.5
            assert_eq!(h.buckets[2].count, 2); // 1.0
            assert_eq!(h.buckets[3].count, 3); // 5.0
            assert_eq!(h.buckets[4].count, 3); // +Inf
        } else {
            panic!("expected histogram");
        }
    }

    #[test]
    fn test_text_export_counter() {
        let reg = MetricsRegistry::new();
        reg.register_help("requests", "Total requests");
        let key = MetricKey::new("requests");
        reg.counter_add(&key, 42.0);
        let text = reg.export_text();
        assert!(text.contains("# HELP requests Total requests"));
        assert!(text.contains("# TYPE requests counter"));
        assert!(text.contains("requests 42"));
    }

    #[test]
    fn test_text_export_gauge() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("temp").with_label("unit", "celsius");
        reg.gauge_set(&key, 22.5);
        let text = reg.export_text();
        assert!(text.contains("# TYPE temp gauge"));
        assert!(text.contains(r#"temp{unit="celsius"} 22.5"#));
    }

    #[test]
    fn test_text_export_histogram() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("latency");
        reg.histogram_observe_with_buckets(&key, 0.1, &[0.1, 1.0]);
        let text = reg.export_text();
        assert!(text.contains("# TYPE latency histogram"));
        assert!(text.contains("latency_bucket"));
        assert!(text.contains("latency_sum"));
        assert!(text.contains("latency_count"));
    }

    #[test]
    fn test_reset() {
        let reg = MetricsRegistry::new();
        let key = MetricKey::new("c");
        reg.counter_inc(&key);
        reg.reset();
        assert_eq!(reg.counter_value(&key), None);
    }

    #[test]
    fn test_snapshot() {
        let reg = MetricsRegistry::new();
        reg.counter_inc(&MetricKey::new("a"));
        reg.gauge_set(&MetricKey::new("b"), 1.0);
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn test_pipeline_instrumentation_keys() {
        let k = pipeline_stage_duration_key("grep");
        assert_eq!(k.name, "atp_pipeline_stage_duration_seconds");
        assert_eq!(k.labels.get("stage").unwrap(), "grep");

        let k2 = pipeline_records_processed_key("sort");
        assert_eq!(k2.name, "atp_pipeline_records_processed_total");

        let k3 = pipeline_errors_key("awk");
        assert_eq!(k3.name, "atp_pipeline_errors_total");

        let k4 = search_matches_key("foo.*bar");
        assert_eq!(k4.name, "atp_search_matches_total");
    }

    #[test]
    fn test_metric_key_labels_str() {
        let k = MetricKey::new("test");
        assert_eq!(k.labels_str(), "");

        let k2 = MetricKey::new("test")
            .with_label("a", "1")
            .with_label("b", "2");
        assert_eq!(k2.labels_str(), r#"{a="1",b="2"}"#);
    }

    #[test]
    fn test_default_histogram_buckets() {
        let buckets = Histogram::default_buckets();
        assert_eq!(buckets.len(), 11);
        assert!(buckets[0] < buckets[1]);
    }

    #[test]
    fn test_thread_safety() {
        let reg = MetricsRegistry::new();
        let reg2 = reg.clone();
        let key = MetricKey::new("concurrent");

        let handle = std::thread::spawn(move || {
            for _ in 0..100 {
                reg2.counter_inc(&MetricKey::new("concurrent"));
            }
        });

        for _ in 0..100 {
            reg.counter_inc(&key);
        }

        handle.join().unwrap();
        assert_eq!(reg.counter_value(&key), Some(200.0));
    }
}
