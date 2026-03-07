//! # Timeline – Temporal Event Streams
//!
//! Parse timestamps from log lines, bucket events into intervals, detect
//! anomalies and bursts, render sparklines, and compute duration histograms.

use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A parsed event with a timestamp.
#[derive(Debug, Clone)]
pub struct Event {
    pub timestamp: i64, // Unix epoch seconds
    pub label: String,
    pub value: f64,
}

/// Bucket granularity for grouping events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Second,
    Minute,
    Hour,
    Day,
}

impl Granularity {
    pub fn seconds(self) -> i64 {
        match self {
            Self::Second => 1,
            Self::Minute => 60,
            Self::Hour => 3600,
            Self::Day => 86400,
        }
    }
}

/// A time bucket with aggregated data.
#[derive(Debug, Clone)]
pub struct Bucket {
    pub start: i64,
    pub end: i64,
    pub count: usize,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl Bucket {
    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}

/// An anomaly detected in the timeline.
#[derive(Debug, Clone)]
pub struct Anomaly {
    pub bucket_start: i64,
    pub kind: AnomalyKind,
    pub score: f64,
    pub description: String,
}

/// Kind of anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyKind {
    Spike,
    Drop,
    Gap,
    Burst,
}

impl fmt::Display for AnomalyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spike => write!(f, "spike"),
            Self::Drop => write!(f, "drop"),
            Self::Gap => write!(f, "gap"),
            Self::Burst => write!(f, "burst"),
        }
    }
}

/// Duration histogram entry.
#[derive(Debug, Clone)]
pub struct HistBin {
    pub range_start: f64,
    pub range_end: f64,
    pub count: usize,
}

// ---------------------------------------------------------------------------
// Timestamp parsing
// ---------------------------------------------------------------------------

/// Parse a Unix-epoch integer from a string.
pub fn parse_epoch(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok()
}

/// Try to parse a timestamp from a log line.
///
/// Supports:
///   - Leading Unix epoch (integer)
///   - ISO-8601–like: `YYYY-MM-DDThh:mm:ss` or `YYYY-MM-DD hh:mm:ss`
///
/// Returns `(epoch_seconds, rest_of_line)`.
pub fn parse_timestamp(line: &str) -> Option<(i64, &str)> {
    let trimmed = line.trim();

    // Try leading integer (epoch)
    if let Some(first) = trimmed.split_whitespace().next() {
        if let Ok(epoch) = first.parse::<i64>() {
            if epoch > 946_684_800 && epoch < 4_102_444_800 {
                // between 2000 and 2100
                let rest = trimmed[first.len()..].trim();
                return Some((epoch, rest));
            }
        }
    }

    // Try ISO-8601-like
    if trimmed.len() >= 19 {
        let candidate = &trimmed[..19];
        if let Some(epoch) = parse_iso_like(candidate) {
            let rest = trimmed[19..].trim_start_matches(|c: char| {
                c == 'Z' || c == '+' || c == '-' || c.is_ascii_digit() || c == ':'
            });
            return Some((epoch, rest.trim()));
        }
    }

    None
}

fn parse_iso_like(s: &str) -> Option<i64> {
    // YYYY-MM-DDThh:mm:ss or YYYY-MM-DD hh:mm:ss
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if bytes[10] != b'T' && bytes[10] != b' ' {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }

    let year: i64 = s[0..4].parse().ok()?;
    let month: i64 = s[5..7].parse().ok()?;
    let day: i64 = s[8..10].parse().ok()?;
    let hour: i64 = s[11..13].parse().ok()?;
    let min: i64 = s[14..16].parse().ok()?;
    let sec: i64 = s[17..19].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if !(0..=23).contains(&hour) || !(0..=59).contains(&min) || !(0..=59).contains(&sec) {
        return None;
    }

    // Simplified days-since-epoch (sufficient for bucketing)
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let month_days = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for md in &month_days[..(month - 1) as usize] {
        days += *md as i64;
    }
    days += day - 1;

    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Parse events from multi-line log text.
pub fn parse_events(text: &str) -> Vec<Event> {
    text.lines()
        .filter_map(|line| {
            let (ts, rest) = parse_timestamp(line)?;
            Some(Event {
                timestamp: ts,
                label: rest.to_string(),
                value: 1.0,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Bucketing
// ---------------------------------------------------------------------------

/// Bucket events into fixed-width time intervals.
pub fn bucket_events(events: &[Event], granularity: Granularity) -> Vec<Bucket> {
    if events.is_empty() {
        return Vec::new();
    }

    let step = granularity.seconds();
    let min_ts = events.iter().map(|e| e.timestamp).min().unwrap();
    let max_ts = events.iter().map(|e| e.timestamp).max().unwrap();

    let bucket_start = (min_ts / step) * step;
    let bucket_end = ((max_ts / step) + 1) * step;

    let mut map: BTreeMap<i64, Bucket> = BTreeMap::new();
    let mut t = bucket_start;
    while t < bucket_end {
        map.insert(
            t,
            Bucket {
                start: t,
                end: t + step,
                count: 0,
                sum: 0.0,
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
        );
        t += step;
    }

    for event in events {
        let key = (event.timestamp / step) * step;
        if let Some(b) = map.get_mut(&key) {
            b.count += 1;
            b.sum += event.value;
            if event.value < b.min {
                b.min = event.value;
            }
            if event.value > b.max {
                b.max = event.value;
            }
        }
    }

    // Fix min/max for empty buckets
    for b in map.values_mut() {
        if b.count == 0 {
            b.min = 0.0;
            b.max = 0.0;
        }
    }

    map.into_values().collect()
}

// ---------------------------------------------------------------------------
// Anomaly detection
// ---------------------------------------------------------------------------

/// Detect anomalies using z-score on bucket counts.
pub fn detect_anomalies(buckets: &[Bucket], z_threshold: f64) -> Vec<Anomaly> {
    if buckets.len() < 3 {
        return Vec::new();
    }

    let counts: Vec<f64> = buckets.iter().map(|b| b.count as f64).collect();
    let mean = counts.iter().sum::<f64>() / counts.len() as f64;
    let variance = counts.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / counts.len() as f64;
    let std_dev = variance.sqrt();

    if std_dev < 1e-9 {
        return Vec::new();
    }

    let mut anomalies = Vec::new();

    for (i, bucket) in buckets.iter().enumerate() {
        let z = (counts[i] - mean) / std_dev;

        if z > z_threshold {
            anomalies.push(Anomaly {
                bucket_start: bucket.start,
                kind: AnomalyKind::Spike,
                score: z,
                description: format!("count {} is {:.1}σ above mean {:.1}", bucket.count, z, mean),
            });
        } else if z < -z_threshold {
            anomalies.push(Anomaly {
                bucket_start: bucket.start,
                kind: AnomalyKind::Drop,
                score: z.abs(),
                description: format!(
                    "count {} is {:.1}σ below mean {:.1}",
                    bucket.count,
                    z.abs(),
                    mean
                ),
            });
        }
    }

    // Detect gaps (consecutive empty buckets)
    let mut gap_start: Option<usize> = None;
    for (i, bucket) in buckets.iter().enumerate() {
        if bucket.count == 0 {
            if gap_start.is_none() {
                gap_start = Some(i);
            }
        } else if let Some(start) = gap_start {
            let gap_len = i - start;
            if gap_len >= 3 {
                anomalies.push(Anomaly {
                    bucket_start: buckets[start].start,
                    kind: AnomalyKind::Gap,
                    score: gap_len as f64,
                    description: format!("{} consecutive empty buckets", gap_len),
                });
            }
            gap_start = None;
        }
    }

    anomalies
}

// ---------------------------------------------------------------------------
// Sparkline
// ---------------------------------------------------------------------------

const SPARK_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Generate a sparkline string from bucket counts.
pub fn sparkline(buckets: &[Bucket]) -> String {
    if buckets.is_empty() {
        return String::new();
    }

    let counts: Vec<f64> = buckets.iter().map(|b| b.count as f64).collect();
    let max = counts.iter().cloned().fold(0.0_f64, f64::max);
    if max < 1e-9 {
        return SPARK_CHARS[0].to_string().repeat(buckets.len());
    }

    counts
        .iter()
        .map(|&c| {
            let idx = ((c / max) * 7.0).round() as usize;
            SPARK_CHARS[idx.min(7)]
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Duration histogram
// ---------------------------------------------------------------------------

/// Compute a duration histogram from pairs of consecutive events.
pub fn duration_histogram(events: &[Event], bin_count: usize) -> Vec<HistBin> {
    if events.len() < 2 || bin_count == 0 {
        return Vec::new();
    }

    let mut durations: Vec<f64> = events
        .windows(2)
        .map(|w| (w[1].timestamp - w[0].timestamp) as f64)
        .collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min_d = durations[0];
    let max_d = durations[durations.len() - 1];
    let range = max_d - min_d;
    if range < 1e-9 {
        return vec![HistBin {
            range_start: min_d,
            range_end: max_d,
            count: durations.len(),
        }];
    }

    let bin_width = range / bin_count as f64;
    let mut bins: Vec<HistBin> = (0..bin_count)
        .map(|i| HistBin {
            range_start: min_d + i as f64 * bin_width,
            range_end: min_d + (i + 1) as f64 * bin_width,
            count: 0,
        })
        .collect();

    for d in &durations {
        let idx = (((d - min_d) / bin_width) as usize).min(bin_count - 1);
        bins[idx].count += 1;
    }

    bins
}

// ---------------------------------------------------------------------------
// Summary report
// ---------------------------------------------------------------------------

/// Format a human-readable timeline summary.
pub fn summary_report(events: &[Event], granularity: Granularity) -> String {
    let mut out = String::new();
    out.push_str(&format!("Timeline Summary ({} events)\n", events.len()));
    out.push_str(&"─".repeat(40));
    out.push('\n');

    if events.is_empty() {
        out.push_str("No events.\n");
        return out;
    }

    let min_ts = events.iter().map(|e| e.timestamp).min().unwrap();
    let max_ts = events.iter().map(|e| e.timestamp).max().unwrap();
    out.push_str(&format!(
        "Time range: {} → {} ({} s)\n",
        min_ts,
        max_ts,
        max_ts - min_ts
    ));

    let buckets = bucket_events(events, granularity);
    out.push_str(&format!("Buckets ({:?}): {}\n", granularity, buckets.len()));

    let spark = sparkline(&buckets);
    out.push_str(&format!("Sparkline: {spark}\n"));

    let anomalies = detect_anomalies(&buckets, 2.0);
    out.push_str(&format!("Anomalies: {}\n", anomalies.len()));
    for a in &anomalies {
        out.push_str(&format!(
            "  [{} @ {}] {}\n",
            a.kind, a.bucket_start, a.description
        ));
    }

    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_events(timestamps: &[i64]) -> Vec<Event> {
        timestamps
            .iter()
            .map(|&ts| Event {
                timestamp: ts,
                label: String::new(),
                value: 1.0,
            })
            .collect()
    }

    #[test]
    fn test_parse_epoch() {
        assert_eq!(parse_epoch("1700000000"), Some(1700000000));
        assert!(parse_epoch("not_a_number").is_none());
    }

    #[test]
    fn test_parse_timestamp_epoch() {
        let (ts, rest) = parse_timestamp("1700000000 some event").unwrap();
        assert_eq!(ts, 1700000000);
        assert_eq!(rest, "some event");
    }

    #[test]
    fn test_parse_timestamp_iso() {
        let (ts, _rest) = parse_timestamp("2024-01-15T10:30:00 event").unwrap();
        assert!(ts > 0);
    }

    #[test]
    fn test_parse_events() {
        let text = "1700000000 start\n1700000060 middle\n1700000120 end\n";
        let events = parse_events(text);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_bucket_events_second() {
        let events = make_events(&[100, 100, 101, 102, 105]);
        let buckets = bucket_events(&events, Granularity::Second);
        assert!(!buckets.is_empty());
        assert_eq!(buckets.iter().map(|b| b.count).sum::<usize>(), 5);
    }

    #[test]
    fn test_bucket_events_minute() {
        let events = make_events(&[1700000000, 1700000030, 1700000060, 1700000061]);
        let buckets = bucket_events(&events, Granularity::Minute);
        assert!(!buckets.is_empty());
        assert_eq!(buckets.iter().map(|b| b.count).sum::<usize>(), 4);
    }

    #[test]
    fn test_bucket_avg() {
        let b = Bucket {
            start: 0,
            end: 60,
            count: 4,
            sum: 10.0,
            min: 1.0,
            max: 4.0,
        };
        assert!((b.avg() - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_sparkline_basic() {
        let events = make_events(&[0, 0, 0, 1, 2, 3, 3, 3, 3, 3, 3]);
        let buckets = bucket_events(&events, Granularity::Second);
        let spark = sparkline(&buckets);
        assert!(!spark.is_empty());
    }

    #[test]
    fn test_sparkline_empty() {
        assert_eq!(sparkline(&[]), "");
    }

    #[test]
    fn test_detect_anomalies_spike() {
        // Create a distribution with one big spike
        let mut events = make_events(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        // Add 20 more events at timestamp 5 to create a spike
        for _ in 0..20 {
            events.push(Event {
                timestamp: 5,
                label: String::new(),
                value: 1.0,
            });
        }
        let buckets = bucket_events(&events, Granularity::Second);
        let anomalies = detect_anomalies(&buckets, 2.0);
        assert!(anomalies.iter().any(|a| a.kind == AnomalyKind::Spike));
    }

    #[test]
    fn test_duration_histogram() {
        let events = make_events(&[0, 10, 20, 30, 100]);
        let hist = duration_histogram(&events, 3);
        assert!(!hist.is_empty());
        assert_eq!(hist.iter().map(|b| b.count).sum::<usize>(), 4);
    }

    #[test]
    fn test_duration_histogram_empty() {
        let hist = duration_histogram(&[], 5);
        assert!(hist.is_empty());
    }

    #[test]
    fn test_summary_report() {
        let events = make_events(&[1700000000, 1700000060, 1700000120]);
        let report = summary_report(&events, Granularity::Minute);
        assert!(report.contains("3 events"));
        assert!(report.contains("Sparkline"));
    }

    #[test]
    fn test_granularity_seconds() {
        assert_eq!(Granularity::Second.seconds(), 1);
        assert_eq!(Granularity::Minute.seconds(), 60);
        assert_eq!(Granularity::Hour.seconds(), 3600);
        assert_eq!(Granularity::Day.seconds(), 86400);
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2000));
        assert!(is_leap(2024));
        assert!(!is_leap(1900));
        assert!(!is_leap(2023));
    }
}
