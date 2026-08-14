use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uwu_schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};

pub struct LogNormalizer;

impl LogNormalizer {
    pub fn normalize(entry: RawLogEntry) -> LogEvent {
        let raw_text = match &entry.payload {
            RawPayload::Text(s) => s.clone(),
            RawPayload::Json(v) => v.to_string(),
            RawPayload::KeyValue(kv) => format!("{:?}", kv),
        };

        // 1. Thử parse nếu là JSON
        if let RawPayload::Json(ref v) = entry.payload {
            return Self::normalize_json(&entry.source_id, v, &raw_text);
        }

        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&raw_text) {
            return Self::normalize_json(&entry.source_id, &json_val, &raw_text);
        }

        // 2. Thử parse nếu là Windows Event XML
        if raw_text.contains("<Event ") || raw_text.contains("<System>") {
            if let Some(event) = Self::normalize_win_event_xml(&entry.source_id, &raw_text) {
                return event;
            }
        }

        // 3. Fallback: Parse log dạng văn bản thuần (Unstructured Text)
        Self::normalize_unstructured_text(&entry.source_id, &raw_text)
    }

    fn normalize_json(source_id: &str, v: &serde_json::Value, raw: &str) -> LogEvent {
        let mut fields = HashMap::new();
        let mut level = LogLevel::Unknown;
        let mut timestamp = Utc::now();
        let mut message = String::new();

        if let Some(obj) = v.as_object() {
            for (k, val) in obj {
                let k_lower = k.to_lowercase();

                // Nhận diện Level
                if k_lower == "level" || k_lower == "lvl" || k_lower == "severity" {
                    if let Some(s) = val.as_str() {
                        level = LogLevel::parse_str(s);
                    }
                }

                // Nhận diện Timestamp
                if k_lower == "timestamp"
                    || k_lower == "time"
                    || k_lower == "ts"
                    || k_lower == "@timestamp"
                {
                    if let Some(s) = val.as_str() {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                            timestamp = dt.with_timezone(&Utc);
                        }
                    }
                }

                // Nhận diện Message
                if k_lower == "message" || k_lower == "msg" || k_lower == "text" {
                    if let Some(s) = val.as_str() {
                        message = s.to_string();
                    }
                }

                fields.insert(k.clone(), val.clone());
            }
        }

        if message.is_empty() {
            message = raw.to_string();
        }

        LogEvent::new(timestamp, level, source_id, message, fields, raw)
    }

    fn normalize_win_event_xml(source_id: &str, xml: &str) -> Option<LogEvent> {
        let mut level = LogLevel::Info;
        let mut timestamp = Utc::now();
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
                if let Ok(dt) = DateTime::parse_from_rfc3339(&rest[..end]) {
                    timestamp = dt.with_timezone(&Utc);
                }
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

        // Trích xuất level bằng từ khóa đơn giản
        let upper = clean.to_uppercase();
        if upper.contains("ERROR") || upper.contains("[ERR]") || upper.contains("FATAL") {
            level = LogLevel::Error;
        } else if upper.contains("WARN") || upper.contains("[WRN]") {
            level = LogLevel::Warn;
        } else if upper.contains("INFO") || upper.contains("[INF]") {
            level = LogLevel::Info;
        } else if upper.contains("DEBUG") || upper.contains("[DBG]") {
            level = LogLevel::Debug;
        } else if upper.contains("TRACE") {
            level = LogLevel::Trace;
        }

        LogEvent::new(
            Utc::now(),
            level,
            source_id,
            clean.clone(),
            HashMap::new(),
            clean,
        )
    }
}

pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            continue;
        }
        if c == '\x1b' {
            if let Some(&'[') = chars.peek() {
                let _ = chars.next();
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
    }

    #[test]
    fn test_strip_ansi() {
        let ansi_text = "\x1b[31m[ERROR]\x1b[0m Connection failed";
        assert_eq!(strip_ansi(ansi_text), "[ERROR] Connection failed");
    }
}
