use crate::schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};
use std::collections::HashMap;

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

        // 2. Nhận diện các trường đặc biệt từ mảng đã phẳng hóa
        if let Some(val) = fields
            .get("level")
            .or_else(|| fields.get("lvl"))
            .or_else(|| fields.get("severity"))
        {
            if let Some(s) = val.as_str() {
                level = LogLevel::parse_str(s);
            }
        }

        if let Some(val) = fields
            .get("timestamp")
            .or_else(|| fields.get("time"))
            .or_else(|| fields.get("ts"))
            .or_else(|| fields.get("@timestamp"))
        {
            if let Some(s) = val.as_str() {
                timestamp = s.to_string();
            } else {
                timestamp = val.to_string();
            }
        }

        if let Some(val) = fields
            .get("message")
            .or_else(|| fields.get("msg"))
            .or_else(|| fields.get("text"))
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

        LogEvent::new("", level, source_id, clean.clone(), HashMap::new(), clean)
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
    fn test_strip_ansi() {
        let ansi_text = "\x1b[31m[ERROR]\x1b[0m Connection failed";
        assert_eq!(strip_ansi(ansi_text), "[ERROR] Connection failed");
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
}
