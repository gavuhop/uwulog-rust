use chrono::{DateTime, Datelike, Local, TimeZone};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Chuyển đổi giá trị token thành số f64.
/// Hỗ trợ số nguyên, số thực và các đơn vị thời gian tương đối: now-5m, now-1h...
pub fn parse_numeric_value(s: &str, now: f64) -> Option<f64> {
    let s_low = s.to_lowercase();
    let s_trim = s_low.trim();
    if s_trim == "now" {
        return Some(now);
    }

    if let Ok(v) = s_trim.parse::<f64>() {
        return Some(v);
    }

    if !s_trim.is_empty() {
        let last_char = s_trim.chars().last().unwrap_or(' ');
        if "smhdwyM".contains(last_char) {
            let num_part = &s_trim[..s_trim.len() - 1];
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
                return Some(now - secs);
            }
        }
    }

    parse_iso_to_secs(s)
}

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
