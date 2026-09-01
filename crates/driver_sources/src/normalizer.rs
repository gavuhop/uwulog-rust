use std::collections::HashMap;
use uwu_core_schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};
use uwu_core_util::{contains_ignore_case, parse_iso_to_secs, strip_ansi};

pub struct LogNormalizer;

impl LogNormalizer {
    pub fn normalize(entry: RawLogEntry) -> LogEvent {
        match entry.payload {
            RawPayload::Json(v) => {
                let raw_text = v.to_string();
                Self::normalize_json(&entry.source_id, &v, &raw_text)
            }
            RawPayload::Text(s) => {
                let clean_ref = if s.contains('\x1b') {
                    strip_ansi(&s)
                } else {
                    s.clone()
                };
                let trimmed = clean_ref.trim();

                // 1. Fast-check JSON: Chỉ thử parse JSON nếu bắt đầu và kết thúc bằng cặp ngoặc {} hoặc []
                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        return Self::normalize_json(&entry.source_id, &json_val, &s);
                    }
                }

                // 2. Thử parse nếu là Windows Event XML
                if (trimmed.starts_with("<Event") || trimmed.starts_with("<System>"))
                    || (s.contains("<Event ") && s.contains("<System>"))
                {
                    if let Some(event) = Self::normalize_win_event_xml(&entry.source_id, &s) {
                        return event;
                    }
                }

                // 3. Fallback: Parse log dạng văn bản thuần (Unstructured Text)
                Self::normalize_unstructured_text(&entry.source_id, &s)
            }
            RawPayload::KeyValue(kv) => {
                let raw_text = format!("{:?}", kv);
                let mut fields = HashMap::new();
                let mut level = LogLevel::Unknown;
                let mut timestamp = String::new();
                let mut message = String::new();

                for (k, v) in kv {
                    let k_lower = k.to_lowercase();
                    if k_lower == "level" || k_lower == "lvl" || k_lower == "severity" {
                        level = LogLevel::parse_str(&v);
                    } else if k_lower == "timestamp" || k_lower == "time" || k_lower == "ts" {
                        timestamp = v.clone();
                    } else if k_lower == "message" || k_lower == "msg" || k_lower == "text" {
                        message = v.clone();
                    }
                    fields.insert(k, serde_json::Value::String(v));
                }

                if message.is_empty() {
                    message = raw_text.clone();
                }

                LogEvent::new(timestamp, level, entry.source_id, message, fields, raw_text)
            }
        }
    }

    fn flatten_json_value(
        prefix: &str,
        v: &serde_json::Value,
        out: &mut HashMap<String, serde_json::Value>,
    ) {
        if let Some(obj) = v.as_object() {
            for (k, child_val) in obj {
                let full_key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };

                if child_val.is_object() {
                    Self::flatten_json_value(&full_key, child_val, out);
                } else {
                    out.insert(full_key, child_val.clone());
                }
            }
        }
    }

    fn normalize_json(source_id: &str, v: &serde_json::Value, raw: &str) -> LogEvent {
        let mut fields = HashMap::new();
        let mut level = LogLevel::Unknown;
        let mut timestamp = String::new();
        let mut message = String::new();

        // 1. Phẳng hóa toàn bộ cây JSON object
        Self::flatten_json_value("", v, &mut fields);

        // 2. Nhận diện các trường đặc biệt từ mảng đã phẳng hóa (hỗ trợ cả JSON thông thường và Systemd Journald)
        if let Some(val) = fields
            .get("level")
            .or_else(|| fields.get("lvl"))
            .or_else(|| fields.get("severity"))
            .or_else(|| fields.get("PRIORITY"))
            .or_else(|| fields.get("priority"))
        {
            if let Some(s) = val.as_str() {
                // Hỗ trợ Syslog / Systemd Journald Priority số nguyên dạng string "0".."7"
                match s.trim() {
                    "0" | "1" | "2" | "3" => level = LogLevel::Error,
                    "4" => level = LogLevel::Warn,
                    "5" | "6" => level = LogLevel::Info,
                    "7" => level = LogLevel::Debug,
                    _ => level = LogLevel::parse_str(s),
                }
            } else if let Some(n) = val.as_u64() {
                match n {
                    0..=3 => level = LogLevel::Error,
                    4 => level = LogLevel::Warn,
                    5 | 6 => level = LogLevel::Info,
                    7 => level = LogLevel::Debug,
                    _ => level = LogLevel::Unknown,
                }
            }
        }

        if let Some(val) = fields
            .get("timestamp")
            .or_else(|| fields.get("time"))
            .or_else(|| fields.get("ts"))
            .or_else(|| fields.get("@timestamp"))
            .or_else(|| fields.get("__REALTIME_TIMESTAMP"))
            .or_else(|| fields.get("_SOURCE_REALTIME_TIMESTAMP"))
        {
            if let Some(s) = val.as_str() {
                // Xử lý timestamp microsecond của systemd journald (ví dụ "1724140800000000")
                if let Ok(usecs) = s.parse::<u64>() {
                    if usecs > 1_000_000_000_000_000 {
                        let secs = (usecs / 1_000_000) as i64;
                        let nsecs = ((usecs % 1_000_000) * 1_000) as u32;
                        if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                            timestamp = dt.to_rfc3339();
                        } else {
                            timestamp = s.to_string();
                        }
                    } else {
                        timestamp = s.to_string();
                    }
                } else {
                    timestamp = s.to_string();
                }
            } else if let Some(usecs) = val.as_u64() {
                if usecs > 1_000_000_000_000_000 {
                    let secs = (usecs / 1_000_000) as i64;
                    let nsecs = ((usecs % 1_000_000) * 1_000) as u32;
                    if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                        timestamp = dt.to_rfc3339();
                    } else {
                        timestamp = val.to_string();
                    }
                } else {
                    timestamp = val.to_string();
                }
            } else {
                timestamp = val.to_string();
            }
        }

        if let Some(val) = fields
            .get("message")
            .or_else(|| fields.get("msg"))
            .or_else(|| fields.get("text"))
            .or_else(|| fields.get("MESSAGE"))
        {
            if let Some(s) = val.as_str() {
                message = s.to_string();
            }
        }

        if message.is_empty() {
            message = raw.to_string();
        }

        LogEvent::new(timestamp, level, source_id, message, fields, raw)
    }

    fn normalize_win_event_xml(source_id: &str, xml: &str) -> Option<LogEvent> {
        let mut level = LogLevel::Info;
        let mut timestamp = String::new();
        let mut fields = HashMap::new();

        // Trích xuất Provider Name
        if let Some(idx) = xml.find("Provider Name='") {
            let rest = &xml[idx + 15..];
            if let Some(end) = rest.find('\'') {
                fields.insert("provider".to_string(), serde_json::json!(&rest[..end]));
            }
        }

        // Trích xuất EventID
        if let Some(idx) = xml.find("<EventID>") {
            let rest = &xml[idx + 9..];
            if let Some(end) = rest.find("</EventID>") {
                if let Ok(id_num) = rest[..end].parse::<u64>() {
                    fields.insert("event_id".to_string(), serde_json::json!(id_num));
                }
            }
        }

        // Trích xuất Level
        if let Some(idx) = xml.find("<Level>") {
            let rest = &xml[idx + 7..];
            if let Some(end) = rest.find("</Level>") {
                match &rest[..end] {
                    "1" | "2" => level = LogLevel::Error,
                    "3" => level = LogLevel::Warn,
                    "4" => level = LogLevel::Info,
                    "5" => level = LogLevel::Trace,
                    _ => level = LogLevel::Unknown,
                }
            }
        }

        // Trích xuất SystemTime
        if let Some(idx) = xml.find("SystemTime='") {
            let rest = &xml[idx + 12..];
            if let Some(end) = rest.find('\'') {
                timestamp = rest[..end].to_string();
            }
        }

        let message = format!(
            "Windows Event {}",
            fields
                .get("event_id")
                .unwrap_or(&serde_json::json!("Unknown"))
        );

        Some(LogEvent::new(
            timestamp, level, source_id, message, fields, xml,
        ))
    }

    fn normalize_unstructured_text(source_id: &str, raw: &str) -> LogEvent {
        let clean = strip_ansi(raw);
        let mut level = LogLevel::Unknown;

        // Trích xuất level không cấp phát heap (zero-allocation ASCII case-insensitive search)
        if contains_ignore_case(&clean, "ERROR")
            || contains_ignore_case(&clean, "[ERR]")
            || contains_ignore_case(&clean, "FATAL")
            || contains_ignore_case(&clean, "[FTL]")
            || contains_ignore_case(&clean, "CRITICAL")
        {
            level = LogLevel::Error;
        } else if contains_ignore_case(&clean, "WARN")
            || contains_ignore_case(&clean, "[WRN]")
            || contains_ignore_case(&clean, "WARNING")
        {
            level = LogLevel::Warn;
        } else if contains_ignore_case(&clean, "INFO")
            || contains_ignore_case(&clean, "[INF]")
            || contains_ignore_case(&clean, "NOTICE")
        {
            level = LogLevel::Info;
        } else if contains_ignore_case(&clean, "DEBUG") || contains_ignore_case(&clean, "[DBG]") {
            level = LogLevel::Debug;
        } else if contains_ignore_case(&clean, "TRACE")
            || contains_ignore_case(&clean, "[TRC]")
            || contains_ignore_case(&clean, "VERBOSE")
        {
            level = LogLevel::Trace;
        }

        let timestamp = extract_timestamp_from_text(&clean);

        LogEvent::new(
            timestamp,
            level,
            source_id,
            clean.clone(),
            HashMap::new(),
            clean,
        )
    }
}

/// Trích xuất timestamp cơ bản ở đầu dòng plain text (nếu có)
pub fn extract_timestamp_from_text(s: &str) -> String {
    let s_trimmed = s.trim_start();
    if s_trimmed.is_empty() {
        return String::new();
    }

    // 1. Kiểm tra định dạng có ngoặc vuông ở đầu: [2026-08-23T11:00:00Z] hoặc [2026-08-23 11:00:00]
    if s_trimmed.starts_with('[') {
        if let Some(close_bracket) = s_trimmed.find(']') {
            let candidate = s_trimmed[1..close_bracket].trim();
            if parse_iso_to_secs(candidate).is_some() {
                return candidate.to_string();
            }
        }
    }

    // 2. Kiểm tra phần đầu chuỗi: lấy token đầu tiên hoặc 2 token đầu (Date + Time)
    let mut words = s_trimmed.split_whitespace();
    if let Some(first) = words.next() {
        // Tránh parse nhầm token level như [INFO] hay WARN:
        if !first.starts_with('[') {
            // Trường hợp ISO timestamp liền: 2026-08-23T11:00:00.123Z
            if first.len() >= 10 && parse_iso_to_secs(first).is_some() {
                return first.to_string();
            }

            // Trường hợp Date Time cách nhau dấu cách: 2026-08-23 11:00:00
            if let Some(second) = words.next() {
                let combined_len = first.len() + 1 + second.len();
                if combined_len <= 35 && s_trimmed.len() >= combined_len {
                    let candidate = &s_trimmed[..combined_len];
                    if parse_iso_to_secs(candidate).is_some() {
                        return candidate.to_string();
                    }
                }
            }
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_json() {
        let json_payload = serde_json::json!({
            "timestamp": "2026-08-14T10:00:00Z",
            "level": "ERROR",
            "message": "Database connection lost",
            "db_id": 42
        });

        let entry = RawLogEntry {
            source_id: "test:json".to_string(),
            payload: RawPayload::Json(json_payload),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.message, "Database connection lost");
        assert_eq!(event.fields.get("db_id").unwrap(), &serde_json::json!(42));
        assert_eq!(event.timestamp, "2026-08-14T10:00:00Z");
    }

    #[test]
    fn test_normalize_unstructured_text() {
        let text = "[WARN] High memory usage detected: 85%";
        let entry = RawLogEntry {
            source_id: "file:app.log".to_string(),
            payload: RawPayload::Text(text.to_string()),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.message, text);
        assert_eq!(event.timestamp, "");
    }

    #[test]
    fn test_timestamp_preservation() {
        let custom_ts = "2026-08-15T21:45:00.123456+07:00";
        let json_payload = serde_json::json!({
            "time": custom_ts,
            "level": "INFO",
            "message": "User logged in"
        });
        let entry = RawLogEntry {
            source_id: "api".to_string(),
            payload: RawPayload::Json(json_payload),
        };
        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.timestamp, custom_ts);
    }

    #[test]
    fn test_flatten_nested_json_keys() {
        let nested_json = serde_json::json!({
            "level": "WARN",
            "message": "User action",
            "metadata": {
                "system": {
                    "env": "production",
                    "cluster": "k8s-us-west"
                },
                "latency_ms": 120
            }
        });

        let entry = RawLogEntry {
            source_id: "test:nested".to_string(),
            payload: RawPayload::Json(nested_json),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(
            event.fields.get("metadata.system.env").unwrap(),
            &serde_json::json!("production")
        );
        assert_eq!(
            event.fields.get("metadata.system.cluster").unwrap(),
            &serde_json::json!("k8s-us-west")
        );
        assert_eq!(
            event.fields.get("metadata.latency_ms").unwrap(),
            &serde_json::json!(120)
        );
    }

    #[test]
    fn test_normalize_win_event_xml() {
        let xml = "<Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'>\
            <System>\
                <Provider Name='Microsoft-Windows-Security-Auditing' />\
                <EventID>4624</EventID>\
                <Level>2</Level>\
                <TimeCreated SystemTime='2026-08-20T08:30:00.0000000Z' />\
            </System>\
        </Event>";

        let entry = RawLogEntry {
            source_id: "winevent:Security".to_string(),
            payload: RawPayload::Text(xml.to_string()),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.level, LogLevel::Error); // Level 2 is Error in WinEvent
        assert_eq!(event.timestamp, "2026-08-20T08:30:00.0000000Z");
        assert_eq!(
            event.fields.get("event_id").unwrap(),
            &serde_json::json!(4624)
        );
        assert_eq!(
            event.fields.get("provider").unwrap(),
            &serde_json::json!("Microsoft-Windows-Security-Auditing")
        );
        assert_eq!(event.message, "Windows Event 4624");
    }

    #[test]
    fn test_normalize_key_value_payload() {
        let mut map = HashMap::new();
        map.insert("level".to_string(), "INFO".to_string());
        map.insert("msg".to_string(), "Service started".to_string());

        let entry = RawLogEntry {
            source_id: "kv:source".to_string(),
            payload: RawPayload::KeyValue(map),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.source_id, "kv:source");
        assert!(event.raw.contains("Service started"));
    }

    #[test]
    fn test_normalize_json_field_aliases() {
        // Test alias lvl + msg + ts
        let json_payload = serde_json::json!({
            "lvl": "CRIT",
            "msg": "Fatal storage error",
            "ts": "1724140800"
        });

        let entry = RawLogEntry {
            source_id: "test:aliases".to_string(),
            payload: RawPayload::Json(json_payload),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.message, "Fatal storage error");
        assert_eq!(event.timestamp, "1724140800");

        // Test alias severity + text + @timestamp
        let json_payload2 = serde_json::json!({
            "severity": "WARNING",
            "text": "High CPU",
            "@timestamp": "2026-08-20T10:00:00Z"
        });

        let event2 = LogNormalizer::normalize(RawLogEntry {
            source_id: "test:elastic".to_string(),
            payload: RawPayload::Json(json_payload2),
        });
        assert_eq!(event2.level, LogLevel::Warn);
        assert_eq!(event2.message, "High CPU");
        assert_eq!(event2.timestamp, "2026-08-20T10:00:00Z");

        // Test fallback to raw when no message field
        let json_payload3 = serde_json::json!({
            "code": 404
        });
        let event3 = LogNormalizer::normalize(RawLogEntry {
            source_id: "test:empty_msg".to_string(),
            payload: RawPayload::Json(json_payload3),
        });
        assert!(event3.message.contains("404"));
    }

    #[test]
    fn test_normalize_unstructured_text_with_timestamp() {
        // 1. Bracketed timestamp
        let log1 = "[2026-08-23 14:30:00] [ERROR] Database deadlock detected";
        let event1 = LogNormalizer::normalize(RawLogEntry {
            source_id: "app".to_string(),
            payload: RawPayload::Text(log1.to_string()),
        });
        assert_eq!(event1.level, LogLevel::Error);
        assert_eq!(event1.timestamp, "2026-08-23 14:30:00");

        // 2. ISO timestamp at start
        let log2 = "2026-08-23T14:30:00Z INFO Server listening on port 8080";
        let event2 = LogNormalizer::normalize(RawLogEntry {
            source_id: "app".to_string(),
            payload: RawPayload::Text(log2.to_string()),
        });
        assert_eq!(event2.level, LogLevel::Info);
        assert_eq!(event2.timestamp, "2026-08-23T14:30:00Z");

        // 3. Fast-check: text that starts with JSON bracket in RawPayload::Text
        let json_text = "{\"level\":\"WARN\",\"message\":\"Low disk space\",\"free_mb\":50}";
        let event3 = LogNormalizer::normalize(RawLogEntry {
            source_id: "app".to_string(),
            payload: RawPayload::Text(json_text.to_string()),
        });
        assert_eq!(event3.level, LogLevel::Warn);
        assert_eq!(event3.message, "Low disk space");
        assert_eq!(
            event3.fields.get("free_mb").unwrap(),
            &serde_json::json!(50)
        );
    }

    #[test]
    fn test_normalize_systemd_journald_json() {
        let journald_payload = serde_json::json!({
            "MESSAGE": "Started User Manager for UID 1000.",
            "PRIORITY": "6",
            "__REALTIME_TIMESTAMP": "1724140800000000",
            "_SYSTEMD_UNIT": "user@1000.service"
        });

        let event = LogNormalizer::normalize(RawLogEntry {
            source_id: "journald:system".to_string(),
            payload: RawPayload::Json(journald_payload),
        });

        assert_eq!(event.level, LogLevel::Info); // Priority 6 is Info
        assert_eq!(event.message, "Started User Manager for UID 1000.");
        assert!(event.timestamp.contains("2024"));
        assert_eq!(
            event.fields.get("_SYSTEMD_UNIT").unwrap(),
            &serde_json::json!("user@1000.service")
        );
    }
}
