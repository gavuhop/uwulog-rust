pub mod command;

use chrono::{DateTime, Datelike, Local, TimeZone};
use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

/// Trả về thời gian Unix timestamp hiện tại tính theo giây (f64)
pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Loại bỏ các mã màu và định dạng ANSI escape sequence khỏi chuỗi văn bản (Fast-path Zero-alloc Cow nếu không có mã ANSI/\r)
pub fn strip_ansi(s: &str) -> Cow<'_, str> {
    if !s.contains('\x1b') && !s.contains('\r') {
        return Cow::Borrowed(s);
    }
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            continue;
        }
        if c == '\x1b' {
            if let Some(&'[') = chars.peek() {
                let _ = chars.next(); // consume '['
                while let Some(&next_c) = chars.peek() {
                    let _ = chars.next();
                    if ('@'..='~').contains(&next_c) {
                        break;
                    }
                }
                continue;
            }
        }
        result.push(c);
    }
    Cow::Owned(result)
}

/// Helper tìm kiếm chuỗi không phân biệt hoa thường với 0 heap allocation
#[inline]
pub fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }
    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    haystack_bytes
        .windows(needle_bytes.len())
        .any(|window| window.eq_ignore_ascii_case(needle_bytes))
}

/// Chuyển đổi giá trị token thành số f64.
/// Hỗ trợ số nguyên, số thực, thời gian tuyệt đối và các đơn vị thời gian tương đối: now-5m, now-1h, now+30s...
pub fn parse_numeric_value(s: &str, now: f64) -> Option<f64> {
    let s_trim = s.trim();
    if s_trim.eq_ignore_ascii_case("now") {
        return Some(now);
    }

    // 1. Hỗ trợ biểu thức relative timestamp: now-5m, now-1h, now+30s, now - 5m... (0 allocation & an toàn UTF-8)
    if s_trim.len() >= 3 && s_trim.as_bytes()[..3].eq_ignore_ascii_case(b"now") {
        let after_now = s_trim[3..].trim();
        if let Some(rest) = after_now.strip_prefix('-') {
            let rest = rest.trim();
            if let Some(secs) = parse_duration_to_secs(rest) {
                return Some(now - secs);
            }
        } else if let Some(rest) = after_now.strip_prefix('+') {
            let rest = rest.trim();
            if let Some(secs) = parse_duration_to_secs(rest) {
                return Some(now + secs);
            }
        }
    }

    // 2. Parse số thực / số nguyên thuần túy
    if let Ok(v) = s_trim.parse::<f64>() {
        return Some(v);
    }

    // 3. Parse duration thuần túy như "5m", "1h", "30s" -> now - secs
    if let Some(secs) = parse_duration_to_secs(s_trim) {
        return Some(now - secs);
    }

    parse_iso_to_secs(s)
}

/// Chuyển đổi duration dạng chuỗi (ví dụ "5s", "10m", "2h", "1d", "1w", "1M", "1y") thành số giây
pub fn parse_duration_to_secs(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let last_char = s.chars().last().unwrap_or(' ');
    if "smhdwyM".contains(last_char) {
        let num_part = &s[..s.len() - 1];
        if let Ok(n) = num_part.parse::<f64>() {
            let secs = match last_char {
                's' => n,
                'm' => n * 60.0,
                'h' => n * 3600.0,
                'd' => n * 86400.0,
                'w' => n * 604800.0,
                'M' => n * 2629746.0,
                'y' => n * 31536000.0,
                _ => 0.0,
            };
            return Some(secs);
        }
    }
    if let Ok(n) = s.parse::<f64>() {
        return Some(n);
    }
    None
}

/// Chuẩn hóa chuỗi timestamp: thay T→space, /→-, ,→. — Zero-alloc (Cow::Borrowed) nếu chuỗi không chứa ký tự cần thay
#[inline]
fn normalize_timestamp_str(s: &str) -> Cow<'_, str> {
    if s.contains('T') || s.contains('/') || s.contains(',') {
        Cow::Owned(s.replace('T', " ").replace('/', "-").replace(',', "."))
    } else {
        Cow::Borrowed(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimestampFormat {
    Rfc3339,
    DateTimeTz(&'static str),
    NaiveDateTime(&'static str),
    NaiveDateTimeYearPrefixed(&'static str),
    NaiveDate(&'static str),
    EpochSeconds,
    EpochMillis,
    EpochNanos,
}

pub const STANDARD_FORMATS: &[(&str, bool, bool)] = &[
    ("%Y-%m-%d %H:%M:%S%.f %z", true, false),
    ("%Y-%m-%d %H:%M:%S%.f%z", true, false),
    ("%Y-%m-%d %H:%M:%S %z", true, false),
    ("%Y-%m-%d %H:%M:%S%z", true, false),
    ("%Y-%m-%d %H:%M:%S%.f", false, false),
    ("%Y-%m-%d %H:%M:%S", false, false),
    ("%Y-%m-%d %H:%M", false, false),
    ("%m-%d %H:%M:%S%.f", false, true), // Logcat
    ("%m-%d %H:%M:%S", false, true),
    ("%d/%m/%Y %H:%M:%S%.f", false, false),
    ("%d/%m/%Y %H:%M:%S", false, false),
    ("%d/%m/%y %H:%M:%S", false, false),
    ("%d-%m-%Y %H:%M:%S", false, false),
    ("%b %d %H:%M:%S", false, true), // Syslog
    ("%d/%b/%Y:%H:%M:%S %z", true, false),
    ("%Y-%m-%d", false, false),
];

/// Nhận diện định dạng timestamp từ chuỗi và trả về cả Format cùng giá trị epoch seconds (f64).
/// Logic chuyển đổi nằm tập trung trong `parse_with_format` — hàm này chỉ xác định format rồi delegate.
pub fn detect_timestamp_format(s: &str) -> Option<(TimestampFormat, f64)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // 1. RFC3339
    if DateTime::parse_from_rfc3339(s).is_ok() {
        let ts = parse_with_format(s, TimestampFormat::Rfc3339)?;
        return Some((TimestampFormat::Rfc3339, ts));
    }

    // 2. Epoch số nguyên / số thực
    if let Ok(n) = s.parse::<f64>() {
        if (1_000_000_000.0..3_000_000_000.0).contains(&n) {
            return Some((TimestampFormat::EpochSeconds, n));
        }
        if (1_000_000_000_000.0..3_000_000_000_000.0).contains(&n) {
            return Some((TimestampFormat::EpochMillis, n / 1_000.0));
        }
        if (1_000_000_000_000_000_000.0..3_000_000_000_000_000_000.0).contains(&n) {
            return Some((TimestampFormat::EpochNanos, n / 1_000_000_000.0));
        }
    }

    // 3. Duyệt STANDARD_FORMATS, delegate parse cho parse_with_format
    for &(fmt, has_tz, needs_year_prefix) in STANDARD_FORMATS {
        if has_tz {
            let candidate = TimestampFormat::DateTimeTz(fmt);
            if let Some(ts) = parse_with_format(s, candidate) {
                return Some((candidate, ts));
            }
        } else if needs_year_prefix {
            let candidate = TimestampFormat::NaiveDateTimeYearPrefixed(fmt);
            if let Some(ts) = parse_with_format(s, candidate) {
                return Some((candidate, ts));
            }
        } else {
            let candidate = TimestampFormat::NaiveDateTime(fmt);
            if let Some(ts) = parse_with_format(s, candidate) {
                return Some((candidate, ts));
            }
            let date_candidate = TimestampFormat::NaiveDate(fmt);
            if let Some(ts) = parse_with_format(s, date_candidate) {
                return Some((date_candidate, ts));
            }
        }
    }

    None
}

/// Fast-Path: Parse timestamp trực tiếp bằng format đã biết trong O(1) không duyệt lặp.
/// Dùng `normalize_timestamp_str` (Cow) để tránh heap allocation khi chuỗi không cần chuẩn hóa.
pub fn parse_with_format(s: &str, fmt: TimestampFormat) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    match fmt {
        TimestampFormat::Rfc3339 => {
            let dt = DateTime::parse_from_rfc3339(s).ok()?;
            Some(dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0)
        }
        TimestampFormat::EpochSeconds => s.parse::<f64>().ok(),
        TimestampFormat::EpochMillis => s.parse::<f64>().ok().map(|n| n / 1_000.0),
        TimestampFormat::EpochNanos => s.parse::<f64>().ok().map(|n| n / 1_000_000_000.0),
        TimestampFormat::DateTimeTz(pat) => {
            let s_norm = normalize_timestamp_str(s);
            let dt = DateTime::parse_from_str(&s_norm, pat).ok()?;
            Some(dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0)
        }
        TimestampFormat::NaiveDateTime(pat) => {
            let s_norm = normalize_timestamp_str(s);
            let dt = chrono::NaiveDateTime::parse_from_str(&s_norm, pat).ok()?;
            let final_dt = Local.from_local_datetime(&dt).single()?;
            Some(
                final_dt.timestamp() as f64
                    + final_dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0,
            )
        }
        TimestampFormat::NaiveDateTimeYearPrefixed(pat) => {
            let s_norm = normalize_timestamp_str(s);
            let current_year = Local::now().year();
            let s_with_year = format!("{}-{}", current_year, s_norm);
            let fmt_with_year = format!("%Y-{}", pat);
            let dt = chrono::NaiveDateTime::parse_from_str(&s_with_year, &fmt_with_year).ok()?;
            let final_dt = Local.from_local_datetime(&dt).single()?;
            Some(
                final_dt.timestamp() as f64
                    + final_dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0,
            )
        }
        TimestampFormat::NaiveDate(pat) => {
            let s_norm = normalize_timestamp_str(s);
            let d = chrono::NaiveDate::parse_from_str(&s_norm, pat).ok()?;
            let opt_dt = d.and_hms_opt(0, 0, 0)?;
            let final_dt = Local.from_local_datetime(&opt_dt).single()?;
            Some(final_dt.timestamp() as f64)
        }
    }
}

/// Chuyển đổi các định dạng ngày giờ phổ biến sang timestamp tính bằng giây (f64)
pub fn parse_iso_to_secs(s: &str) -> Option<f64> {
    detect_timestamp_format(s).map(|(_, ts)| ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_secs() {
        let ts = now_secs();
        assert!(ts > 1_700_000_000.0); // Timestamp after 2023
    }

    #[test]
    fn test_parse_numeric_value_units() {
        let now = 1_000_000.0;

        // Base cases
        assert_eq!(parse_numeric_value("now", now), Some(now));
        assert_eq!(parse_numeric_value("42", now), Some(42.0));
        assert_eq!(parse_numeric_value("3.5", now), Some(3.5));

        // Time units relative to now
        assert_eq!(parse_numeric_value("10s", now), Some(now - 10.0));
        assert_eq!(parse_numeric_value("5m", now), Some(now - 300.0));
        assert_eq!(parse_numeric_value("2h", now), Some(now - 7200.0));
        assert_eq!(parse_numeric_value("1d", now), Some(now - 86400.0));
        assert_eq!(parse_numeric_value("1w", now), Some(now - 604800.0));
        assert_eq!(parse_numeric_value("1M", now), Some(now - 2629746.0));
        assert_eq!(parse_numeric_value("1y", now), Some(now - 31536000.0));

        // Multi-byte Unicode & Emoji safety (must not panic on UTF-8 char boundary)
        assert_eq!(parse_numeric_value("🚀10", now), None);
        assert_eq!(parse_numeric_value("đồng", now), None);
        assert_eq!(parse_numeric_value("🔥", now), None);
        assert_eq!(parse_numeric_value("á123", now), None);
        assert_eq!(parse_numeric_value("đ", now), None);

        // Invalid strings
        assert_eq!(parse_numeric_value("not_a_number", now), None);
        assert_eq!(parse_numeric_value("", now), None);
    }

    #[test]
    fn test_parse_iso_to_secs_formats() {
        // RFC3339
        let rfc3339 = "2026-08-20T10:00:00Z";
        assert!(parse_iso_to_secs(rfc3339).is_some());

        // Standard space-separated
        let std_fmt = "2026-08-20 10:00:00.123";
        assert!(parse_iso_to_secs(std_fmt).is_some());

        // Slash format
        let slash_fmt = "2026/08/20 10:00:00";
        assert!(parse_iso_to_secs(slash_fmt).is_some());

        // Date only
        let date_only = "2026-08-20";
        assert!(parse_iso_to_secs(date_only).is_some());

        // Empty
        assert_eq!(parse_iso_to_secs(""), None);
    }

    #[test]
    fn test_strip_ansi_complex() {
        // Truecolor 24-bit ANSI
        let truecolor = "\x1b[38;2;255;100;50mHello TrueColor\x1b[0m";
        assert_eq!(strip_ansi(truecolor), "Hello TrueColor");

        // Bold + multiple style codes + \r
        let multi_style = "\x1b[1;32;40m[INFO]\x1b[0m Line with return\r\n";
        assert_eq!(strip_ansi(multi_style), "[INFO] Line with return\n");

        // Plain string without ANSI
        let plain = "Simple plain text";
        assert_eq!(strip_ansi(plain), "Simple plain text");
    }

    #[test]
    fn test_contains_ignore_case() {
        assert!(contains_ignore_case("Hello World", "world"));
        assert!(contains_ignore_case("ERROR: something broke", "error"));
        assert!(!contains_ignore_case("INFO: ok", "error"));
        assert!(contains_ignore_case("anything", ""));
    }

    #[test]
    fn test_detect_and_fast_parse_timestamp() {
        // RFC3339
        let (fmt_rfc, ts_rfc) = detect_timestamp_format("2026-08-20T10:00:00Z").unwrap();
        assert_eq!(fmt_rfc, TimestampFormat::Rfc3339);
        assert_eq!(
            parse_with_format("2026-08-20T10:00:00Z", fmt_rfc),
            Some(ts_rfc)
        );

        // Epoch seconds
        let (fmt_sec, ts_sec) = detect_timestamp_format("1724148000.5").unwrap();
        assert_eq!(fmt_sec, TimestampFormat::EpochSeconds);
        assert_eq!(ts_sec, 1724148000.5);
        assert_eq!(
            parse_with_format("1724148000.5", fmt_sec),
            Some(1724148000.5)
        );

        // Epoch millis
        let (fmt_ms, ts_ms) = detect_timestamp_format("1724148000000").unwrap();
        assert_eq!(fmt_ms, TimestampFormat::EpochMillis);
        assert_eq!(ts_ms, 1724148000.0);
        assert_eq!(
            parse_with_format("1724148000000", fmt_ms),
            Some(1724148000.0)
        );

        // Standard space format
        let (fmt_std, ts_std) = detect_timestamp_format("2026-08-20 10:00:00.123").unwrap();
        assert!(matches!(fmt_std, TimestampFormat::NaiveDateTime(_)));
        assert_eq!(
            parse_with_format("2026-08-20 10:00:00.123", fmt_std),
            Some(ts_std)
        );

        // Format switch / dev change test: Parse fail triggers redetection
        let old_fmt = fmt_rfc;
        let new_str = "1724148000.5";
        // Parse with wrong format returns None -> triggers redetect
        assert_eq!(parse_with_format(new_str, old_fmt), None);
        let (new_fmt, new_ts) = detect_timestamp_format(new_str).unwrap();
        assert_eq!(new_fmt, TimestampFormat::EpochSeconds);
        assert_eq!(new_ts, 1724148000.5);
    }
}
