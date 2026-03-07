//! # Calendar — Date / Time Processing
//!
//! Multi-format date parsing (ISO-8601, RFC-2822, common patterns),
//! formatting, arithmetic, duration computation, and relative dates.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A calendar date-time (UTC, no timezone offset tracking).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Duration between two date-times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duration {
    pub days: i64,
    pub hours: i32,
    pub minutes: i32,
    pub seconds: i32,
}

/// Calendar date format for parsing / formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DateFormat {
    /// `YYYY-MM-DD` (ISO-8601 date).
    Iso8601Date,
    /// `YYYY-MM-DDTHH:MM:SS` (ISO-8601 date-time).
    Iso8601DateTime,
    /// `DD Mon YYYY HH:MM:SS` (RFC-2822-like).
    Rfc2822,
    /// `MM/DD/YYYY`
    UsDate,
    /// `DD/MM/YYYY`
    EuDate,
    /// `YYYY/MM/DD`
    SlashDate,
    /// Unix epoch seconds.
    Epoch,
}

/// A relative date specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeDate {
    Today,
    Yesterday,
    Tomorrow,
    DaysAgo(i64),
    DaysFromNow(i64),
    WeeksAgo(i64),
    WeeksFromNow(i64),
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MONTH_DAYS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTH_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

// ---------------------------------------------------------------------------
// Core helpers
// ---------------------------------------------------------------------------

/// Check if a year is a leap year.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Days in a given month (1-indexed).
pub fn days_in_month(year: i32, month: u8) -> u8 {
    if month == 2 && is_leap_year(year) {
        29
    } else if (1..=12).contains(&month) {
        MONTH_DAYS[(month - 1) as usize]
    } else {
        0
    }
}

/// Day of year (1-indexed).
pub fn day_of_year(dt: &DateTime) -> u16 {
    let mut d = 0u16;
    for m in 1..dt.month {
        d += days_in_month(dt.year, m) as u16;
    }
    d + dt.day as u16
}

/// Day of week (0 = Monday, 6 = Sunday) via Tomohiko Sakamoto's algorithm.
pub fn day_of_week(dt: &DateTime) -> u8 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut y = dt.year;
    if dt.month < 3 {
        y -= 1;
    }
    let dow = (y + y / 4 - y / 100 + y / 400 + t[(dt.month - 1) as usize] + dt.day as i32) % 7;
    // Result: 0=Sunday..6=Saturday → convert to 0=Monday..6=Sunday
    ((dow + 6) % 7) as u8
}

/// Day of week name.
pub fn day_name(dow: u8) -> &'static str {
    match dow {
        0 => "Monday",
        1 => "Tuesday",
        2 => "Wednesday",
        3 => "Thursday",
        4 => "Friday",
        5 => "Saturday",
        6 => "Sunday",
        _ => "Unknown",
    }
}

/// Is the date valid?
pub fn is_valid(dt: &DateTime) -> bool {
    dt.month >= 1
        && dt.month <= 12
        && dt.day >= 1
        && dt.day <= days_in_month(dt.year, dt.month)
        && dt.hour <= 23
        && dt.minute <= 59
        && dt.second <= 59
}

// ---------------------------------------------------------------------------
// Conversion to/from epoch
// ---------------------------------------------------------------------------

/// Convert DateTime to Unix epoch seconds (from 1970-01-01T00:00:00).
pub fn to_epoch(dt: &DateTime) -> i64 {
    let mut days: i64 = 0;
    if dt.year >= 1970 {
        for y in 1970..dt.year {
            days += if is_leap_year(y) { 366 } else { 365 };
        }
    } else {
        for y in dt.year..1970 {
            days -= if is_leap_year(y) { 366 } else { 365 };
        }
    }
    days += day_of_year(dt) as i64 - 1;
    days * 86400 + dt.hour as i64 * 3600 + dt.minute as i64 * 60 + dt.second as i64
}

/// Convert Unix epoch seconds to DateTime.
pub fn from_epoch(epoch: i64) -> DateTime {
    let mut rem = epoch;
    let mut year = 1970i32;
    if rem >= 0 {
        loop {
            let ydays = if is_leap_year(year) { 366 } else { 365 };
            if rem < ydays * 86400 {
                break;
            }
            rem -= ydays * 86400;
            year += 1;
        }
    } else {
        loop {
            year -= 1;
            let ydays = if is_leap_year(year) { 366 } else { 365 };
            rem += ydays * 86400;
            if rem >= 0 {
                break;
            }
        }
    }
    let day_seconds = rem;
    let mut yday = (day_seconds / 86400) as u16;
    let time_rem = day_seconds % 86400;
    let hour = (time_rem / 3600) as u8;
    let minute = ((time_rem % 3600) / 60) as u8;
    let second = (time_rem % 60) as u8;

    let mut month = 1u8;
    loop {
        let dim = days_in_month(year, month);
        if yday < dim as u16 {
            break;
        }
        yday -= dim as u16;
        month += 1;
    }
    let day = yday as u8 + 1;

    DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a date string, trying multiple formats.
pub fn parse(input: &str) -> Option<DateTime> {
    let trimmed = input.trim();
    parse_iso8601(trimmed)
        .or_else(|| parse_us_date(trimmed))
        .or_else(|| parse_eu_date(trimmed))
        .or_else(|| parse_slash_date(trimmed))
        .or_else(|| parse_rfc2822(trimmed))
        .or_else(|| parse_epoch_str(trimmed))
}

fn parse_int<T: std::str::FromStr>(s: &str) -> Option<T> {
    s.parse().ok()
}

fn parse_iso8601(s: &str) -> Option<DateTime> {
    // YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS
    if s.len() < 10 {
        return None;
    }
    let year: i32 = parse_int(&s[..4])?;
    if s.as_bytes().get(4)? != &b'-' {
        return None;
    }
    let month: u8 = parse_int(&s[5..7])?;
    if s.as_bytes().get(7)? != &b'-' {
        return None;
    }
    let day: u8 = parse_int(&s[8..10])?;
    let (hour, minute, second) =
        if s.len() >= 19 && (s.as_bytes()[10] == b'T' || s.as_bytes()[10] == b' ') {
            let h: u8 = parse_int(&s[11..13])?;
            let m: u8 = parse_int(&s[14..16])?;
            let sc: u8 = parse_int(&s[17..19])?;
            (h, m, sc)
        } else {
            (0, 0, 0)
        };
    let dt = DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    };
    if is_valid(&dt) {
        Some(dt)
    } else {
        None
    }
}

fn parse_us_date(s: &str) -> Option<DateTime> {
    // MM/DD/YYYY
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 || parts[2].len() != 4 {
        return None;
    }
    let month: u8 = parse_int(parts[0])?;
    let day: u8 = parse_int(parts[1])?;
    let year: i32 = parse_int(parts[2])?;
    let dt = DateTime {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
    };
    if is_valid(&dt) {
        Some(dt)
    } else {
        None
    }
}

fn parse_eu_date(s: &str) -> Option<DateTime> {
    // DD.MM.YYYY
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 || parts[2].len() != 4 {
        return None;
    }
    let day: u8 = parse_int(parts[0])?;
    let month: u8 = parse_int(parts[1])?;
    let year: i32 = parse_int(parts[2])?;
    let dt = DateTime {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
    };
    if is_valid(&dt) {
        Some(dt)
    } else {
        None
    }
}

fn parse_slash_date(s: &str) -> Option<DateTime> {
    // YYYY/MM/DD
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 || parts[0].len() != 4 {
        return None;
    }
    let year: i32 = parse_int(parts[0])?;
    let month: u8 = parse_int(parts[1])?;
    let day: u8 = parse_int(parts[2])?;
    let dt = DateTime {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
    };
    if is_valid(&dt) {
        Some(dt)
    } else {
        None
    }
}

fn parse_rfc2822(s: &str) -> Option<DateTime> {
    // e.g. "06 Mar 2026 14:30:00" or "Mon, 06 Mar 2026 14:30:00"
    let parts: Vec<&str> = s.split_whitespace().collect();
    // Try with/without day-of-week prefix
    let (day_s, mon_s, year_s, time_s) = if parts.len() == 4 {
        (parts[0], parts[1], parts[2], Some(parts[3]))
    } else if parts.len() == 5 {
        // "Mon, 06 Mar 2026 14:30:00"
        (parts[1], parts[2], parts[3], Some(parts[4]))
    } else if parts.len() == 3 {
        (parts[0], parts[1], parts[2], None)
    } else {
        return None;
    };
    let day: u8 = parse_int(day_s)?;
    let month = month_from_name(mon_s)?;
    let year: i32 = parse_int(year_s)?;
    let (hour, minute, second) = if let Some(ts) = time_s {
        parse_time(ts)?
    } else {
        (0, 0, 0)
    };
    let dt = DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    };
    if is_valid(&dt) {
        Some(dt)
    } else {
        None
    }
}

fn month_from_name(s: &str) -> Option<u8> {
    let lower = s.to_lowercase();
    for (i, &name) in MONTH_NAMES.iter().enumerate() {
        if lower == name.to_lowercase() {
            return Some(i as u8 + 1);
        }
    }
    for (i, &name) in MONTH_FULL.iter().enumerate() {
        if lower == name.to_lowercase() {
            return Some(i as u8 + 1);
        }
    }
    None
}

fn parse_time(s: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let h: u8 = parse_int(parts[0])?;
    let m: u8 = parse_int(parts[1])?;
    let s: u8 = if parts.len() > 2 {
        parse_int(parts[2])?
    } else {
        0
    };
    Some((h, m, s))
}

fn parse_epoch_str(s: &str) -> Option<DateTime> {
    let epoch: i64 = s.parse().ok()?;
    // Reasonable epoch range: 1970-01-01 to 2100-01-01 approx
    if epoch.abs() > 4_102_444_800 {
        return None;
    }
    Some(from_epoch(epoch))
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Format date-time to a string with the given format.
pub fn format(dt: &DateTime, fmt: DateFormat) -> String {
    match fmt {
        DateFormat::Iso8601Date => format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day),
        DateFormat::Iso8601DateTime => format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
        ),
        DateFormat::Rfc2822 => {
            let dow = day_of_week(dt);
            let dow_name = &day_name(dow)[..3];
            let mon_name = MONTH_NAMES[(dt.month - 1) as usize];
            format!(
                "{}, {:02} {} {:04} {:02}:{:02}:{:02}",
                dow_name, dt.day, mon_name, dt.year, dt.hour, dt.minute, dt.second
            )
        }
        DateFormat::UsDate => format!("{:02}/{:02}/{:04}", dt.month, dt.day, dt.year),
        DateFormat::EuDate => format!("{:02}.{:02}.{:04}", dt.day, dt.month, dt.year),
        DateFormat::SlashDate => format!("{:04}/{:02}/{:02}", dt.year, dt.month, dt.day),
        DateFormat::Epoch => format!("{}", to_epoch(dt)),
    }
}

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

/// Add (or subtract) days to a date-time.
pub fn add_days(dt: &DateTime, days: i64) -> DateTime {
    from_epoch(to_epoch(dt) + days * 86400)
}

/// Add (or subtract) hours.
pub fn add_hours(dt: &DateTime, hours: i64) -> DateTime {
    from_epoch(to_epoch(dt) + hours * 3600)
}

/// Add (or subtract) minutes.
pub fn add_minutes(dt: &DateTime, minutes: i64) -> DateTime {
    from_epoch(to_epoch(dt) + minutes * 60)
}

/// Compute the duration between two date-times.
pub fn difference(a: &DateTime, b: &DateTime) -> Duration {
    let diff = to_epoch(b) - to_epoch(a);
    let abs_diff = diff.unsigned_abs();
    let sign = if diff < 0 { -1i64 } else { 1 };
    let total_secs = abs_diff;
    let days = (total_secs / 86400) as i64 * sign;
    let rem = (total_secs % 86400) as i32;
    let hours = (rem / 3600) * sign as i32;
    let minutes = ((rem % 3600) / 60) * sign as i32;
    let seconds = (rem % 60) * sign as i32;
    Duration {
        days,
        hours,
        minutes,
        seconds,
    }
}

/// Format a duration as a human-readable string.
pub fn format_duration(d: &Duration) -> String {
    let mut parts = Vec::new();
    if d.days != 0 {
        parts.push(format!("{}d", d.days));
    }
    if d.hours != 0 {
        parts.push(format!("{}h", d.hours));
    }
    if d.minutes != 0 {
        parts.push(format!("{}m", d.minutes));
    }
    if d.seconds != 0 || parts.is_empty() {
        parts.push(format!("{}s", d.seconds));
    }
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Relative dates
// ---------------------------------------------------------------------------

/// Resolve a relative date against a reference date.
pub fn resolve_relative(rel: &RelativeDate, reference: &DateTime) -> DateTime {
    match rel {
        RelativeDate::Today => *reference,
        RelativeDate::Yesterday => add_days(reference, -1),
        RelativeDate::Tomorrow => add_days(reference, 1),
        RelativeDate::DaysAgo(n) => add_days(reference, -(*n)),
        RelativeDate::DaysFromNow(n) => add_days(reference, *n),
        RelativeDate::WeeksAgo(n) => add_days(reference, -(*n) * 7),
        RelativeDate::WeeksFromNow(n) => add_days(reference, *n * 7),
    }
}

/// Parse a natural-language relative date string.
pub fn parse_relative(s: &str) -> Option<RelativeDate> {
    let lower = s.trim().to_lowercase();
    if lower == "today" {
        return Some(RelativeDate::Today);
    }
    if lower == "yesterday" {
        return Some(RelativeDate::Yesterday);
    }
    if lower == "tomorrow" {
        return Some(RelativeDate::Tomorrow);
    }
    // "N days ago", "N weeks ago"
    let parts: Vec<&str> = lower.split_whitespace().collect();
    if parts.len() == 3 && parts[2] == "ago" {
        let n: i64 = parts[0].parse().ok()?;
        match parts[1] {
            "day" | "days" => return Some(RelativeDate::DaysAgo(n)),
            "week" | "weeks" => return Some(RelativeDate::WeeksAgo(n)),
            _ => {}
        }
    }
    // "in N days", "in N weeks"
    if parts.len() == 3 && parts[0] == "in" {
        let n: i64 = parts[1].parse().ok()?;
        match parts[2] {
            "day" | "days" => return Some(RelativeDate::DaysFromNow(n)),
            "week" | "weeks" => return Some(RelativeDate::WeeksFromNow(n)),
            _ => {}
        }
    }
    None
}

/// ISO week number (1-indexed).
pub fn iso_week(dt: &DateTime) -> u8 {
    let yday = day_of_year(dt) as i32;
    let dow = day_of_week(dt) as i32; // 0=Mon
    let week = (yday - dow + 10) / 7;
    week.max(1) as u8
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leap_year() {
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2023));
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2023, 1), 31);
    }

    #[test]
    fn test_epoch_roundtrip() {
        let dt = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 14,
            minute: 30,
            second: 0,
        };
        let epoch = to_epoch(&dt);
        let back = from_epoch(epoch);
        assert_eq!(back, dt);
    }

    #[test]
    fn test_epoch_origin() {
        let dt = DateTime {
            year: 1970,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        };
        assert_eq!(to_epoch(&dt), 0);
    }

    #[test]
    fn test_parse_iso8601_date() {
        let dt = parse("2026-03-06").unwrap();
        assert_eq!(dt.year, 2026);
        assert_eq!(dt.month, 3);
        assert_eq!(dt.day, 6);
    }

    #[test]
    fn test_parse_iso8601_datetime() {
        let dt = parse("2026-03-06T14:30:00").unwrap();
        assert_eq!(dt.hour, 14);
        assert_eq!(dt.minute, 30);
    }

    #[test]
    fn test_parse_us_date() {
        let dt = parse("03/06/2026").unwrap();
        assert_eq!(dt.month, 3);
        assert_eq!(dt.day, 6);
    }

    #[test]
    fn test_parse_eu_date() {
        let dt = parse("06.03.2026").unwrap();
        assert_eq!(dt.day, 6);
        assert_eq!(dt.month, 3);
    }

    #[test]
    fn test_parse_rfc2822() {
        let dt = parse("06 Mar 2026 14:30:00").unwrap();
        assert_eq!(dt.year, 2026);
        assert_eq!(dt.month, 3);
    }

    #[test]
    fn test_format_iso() {
        let dt = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 0,
            minute: 0,
            second: 0,
        };
        assert_eq!(format(&dt, DateFormat::Iso8601Date), "2026-03-06");
    }

    #[test]
    fn test_format_rfc2822() {
        let dt = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 14,
            minute: 30,
            second: 0,
        };
        let s = format(&dt, DateFormat::Rfc2822);
        assert!(s.contains("Mar"));
        assert!(s.contains("2026"));
    }

    #[test]
    fn test_add_days() {
        let dt = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 0,
            minute: 0,
            second: 0,
        };
        let next = add_days(&dt, 1);
        assert_eq!(next.day, 7);
        let prev = add_days(&dt, -6);
        assert_eq!(prev.month, 2);
        assert_eq!(prev.day, 28);
    }

    #[test]
    fn test_difference() {
        let a = DateTime {
            year: 2026,
            month: 3,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        };
        let b = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 12,
            minute: 0,
            second: 0,
        };
        let d = difference(&a, &b);
        assert_eq!(d.days, 5);
        assert_eq!(d.hours, 12);
    }

    #[test]
    fn test_day_of_week() {
        // 2026-03-06 is a Friday
        let dt = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 0,
            minute: 0,
            second: 0,
        };
        let dow = day_of_week(&dt);
        assert_eq!(day_name(dow), "Friday");
    }

    #[test]
    fn test_parse_relative() {
        assert_eq!(parse_relative("today"), Some(RelativeDate::Today));
        assert_eq!(parse_relative("3 days ago"), Some(RelativeDate::DaysAgo(3)));
        assert_eq!(
            parse_relative("in 2 weeks"),
            Some(RelativeDate::WeeksFromNow(2))
        );
    }

    #[test]
    fn test_resolve_relative() {
        let ref_dt = DateTime {
            year: 2026,
            month: 3,
            day: 6,
            hour: 0,
            minute: 0,
            second: 0,
        };
        let yesterday = resolve_relative(&RelativeDate::Yesterday, &ref_dt);
        assert_eq!(yesterday.day, 5);
    }

    #[test]
    fn test_format_duration() {
        let d = Duration {
            days: 2,
            hours: 3,
            minutes: 15,
            seconds: 0,
        };
        assert_eq!(format_duration(&d), "2d 3h 15m");
    }

    #[test]
    fn test_iso_week() {
        let dt = DateTime {
            year: 2026,
            month: 1,
            day: 5,
            hour: 0,
            minute: 0,
            second: 0,
        };
        let w = iso_week(&dt);
        assert!(w >= 1 && w <= 53);
    }
}
