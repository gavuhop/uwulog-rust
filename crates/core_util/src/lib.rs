use chrono::{DateTime, Datelike, Local, TimeZone};
use std::time::{SystemTime, UNIX_EPOCH};

/// Trả về thời gian Unix timestamp hiện tại tính theo giây (f64)
pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Loại bỏ các mã màu và định dạng ANSI escape sequence khỏi chuỗi văn bản (Fast-path 0 heap alloc nếu không có mã ANSI/\r)
pub fn strip_ansi(s: &str) -> String {
    if !s.contains('\x1b') && !s.contains('\r') {
        return s.to_string();
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
    result
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

/// Chuyển đổi các định dạng ngày giờ phổ biến sang timestamp tính bằng giây (f64)
pub fn parse_iso_to_secs(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let s_norm = s.replace('T', " ").replace('/', "-").replace(',', ".");

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0);
    }

    let formats = [
        "%Y-%m-%d %H:%M:%S%.f %z",
        "%Y-%m-%d %H:%M:%S%.f%z",
        "%Y-%m-%d %H:%M:%S %z",
        "%Y-%m-%d %H:%M:%S%z",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%m-%d %H:%M:%S%.f", // Logcat
        "%m-%d %H:%M:%S",
        "%d/%m/%Y %H:%M:%S%.f",
        "%d/%m/%Y %H:%M:%S",
        "%d/%m/%y %H:%M:%S",
        "%d-%m-%Y %H:%M:%S",
        "%b %d %H:%M:%S", // Syslog
        "%d/%b/%Y:%H:%M:%S %z",
        "%Y-%m-%d",
    ];

    let current_year = Local::now().year();

    for fmt in formats {
        // try normal format
        if let Ok(dt) = DateTime::parse_from_str(&s_norm, fmt) {
            return Some(
                dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0,
            );
        }

        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s_norm, fmt) {
            if let Some(final_dt) = Local.from_local_datetime(&dt).single() {
                return Some(
                    final_dt.timestamp() as f64
                        + final_dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0,
                );
            }
        }

        // try format with current year
        if !fmt.contains("%Y") && !fmt.contains("%y") {
            let s_with_year = format!("{}-{}", current_year, s_norm);
            let fmt_with_year = format!("%Y-{}", fmt);
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s_with_year, &fmt_with_year) {
                if let Some(final_dt) = Local.from_local_datetime(&dt).single() {
                    return Some(
                        final_dt.timestamp() as f64
                            + final_dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0,
                    );
                }
            }
        }

        if let Ok(d) = chrono::NaiveDate::parse_from_str(&s_norm, fmt) {
            if let Some(final_dt) = Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single() {
                return Some(final_dt.timestamp() as f64);
            }
        }
    }

    None
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
}
