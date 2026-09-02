use std::collections::HashMap;
use uwu_core_schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};
use uwu_core_util::strip_ansi;

struct DetectedSemanticFields {
    level: LogLevel,
    timestamp: String,
    message: String,
}

pub struct LogNormalizer;

impl LogNormalizer {
    pub fn normalize(entry: RawLogEntry) -> LogEvent {
        match entry.payload {
            RawPayload::Json(v) => Self::normalize_json(&v),
            RawPayload::Text(s) => {
                let has_ansi = s.contains('\x1b') || s.contains('\r');
                let clean = if has_ansi { strip_ansi(&s) } else { s };
                let trimmed = clean.trim();

                // 1. Fast-check JSON: Chỉ thử parse JSON nếu bắt đầu và kết thúc bằng cặp ngoặc {} hoặc []
                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        return Self::normalize_json(&json_val);
                    }
                }

                // 2. Plain text thuần túy: Không nhận dạng level/timestamp giả định
                LogEvent::new(String::new(), LogLevel::Unknown, clean, HashMap::new())
            }
            RawPayload::KeyValue(kv) => {
                let fields: HashMap<String, serde_json::Value> = kv
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect();
                let detected = Self::detect_semantic_fields(&fields);

                LogEvent::new(detected.timestamp, detected.level, detected.message, fields)
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

    fn detect_semantic_fields(
        fields: &HashMap<String, serde_json::Value>,
    ) -> DetectedSemanticFields {
        let mut timestamp_key = None;
        let mut level_key = None;
        let mut message_key = None;

        // Nhận diện các trường đặc biệt từ mảng đã phẳng hóa 1 lần duy nhất bằng SSOT
        for k in fields.keys() {
            if let Some(std_field) = uwu_core_schema::StandardField::from_alias(k) {
                match std_field {
                    uwu_core_schema::StandardField::Level if level_key.is_none() => {
                        level_key = Some(k.clone());
                    }
                    uwu_core_schema::StandardField::Timestamp if timestamp_key.is_none() => {
                        timestamp_key = Some(k.clone());
                    }
                    uwu_core_schema::StandardField::Message if message_key.is_none() => {
                        message_key = Some(k.clone());
                    }
                    _ => {}
                }
            }
        }

        let level = if let Some(ref lk) = level_key {
            if let Some(val) = fields.get(lk) {
                if let Some(s) = val.as_str() {
                    LogLevel::parse_str(s)
                } else {
                    LogLevel::parse_str(&val.to_string())
                }
            } else {
                LogLevel::Unknown
            }
        } else {
            LogLevel::Unknown
        };

        let timestamp = if let Some(ref tk) = timestamp_key {
            if let Some(val) = fields.get(tk) {
                if let Some(s) = val.as_str() {
                    s.to_string()
                } else {
                    val.to_string()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let message = if let Some(ref mk) = message_key {
            if let Some(val) = fields.get(mk) {
                if let Some(s) = val.as_str() {
                    s.to_string()
                } else {
                    val.to_string()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        DetectedSemanticFields {
            level,
            timestamp,
            message,
        }
    }

    fn normalize_json(v: &serde_json::Value) -> LogEvent {
        let mut fields = HashMap::new();
        Self::flatten_json_value("", v, &mut fields);
        let detected = Self::detect_semantic_fields(&fields);

        LogEvent::new(detected.timestamp, detected.level, detected.message, fields)
    }
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
            payload: RawPayload::Text(text.to_string()),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.level, LogLevel::Unknown);
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
    fn test_normalize_key_value_payload() {
        let mut map = HashMap::new();
        map.insert("level".to_string(), "INFO".to_string());
        map.insert("msg".to_string(), "Service started".to_string());

        let entry = RawLogEntry {
            payload: RawPayload::KeyValue(map),
        };

        let event = LogNormalizer::normalize(entry);
        assert_eq!(event.message, "Service started");
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
            payload: RawPayload::Json(json_payload2),
        });
        assert_eq!(event2.level, LogLevel::Warn);
        assert_eq!(event2.message, "High CPU");
        assert_eq!(event2.timestamp, "2026-08-20T10:00:00Z");

        // Test custom fields when no message field
        let json_payload3 = serde_json::json!({
            "code": 404
        });
        let event3 = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(json_payload3),
        });
        assert_eq!(event3.fields.get("code").unwrap(), &serde_json::json!(404));
    }

    #[test]
    fn test_normalize_text_with_json() {
        let json_text = "{\"level\":\"WARN\",\"message\":\"Low disk space\",\"free_mb\":50}";
        let event = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Text(json_text.to_string()),
        });
        assert_eq!(event.level, LogLevel::Warn);
        assert_eq!(event.message, "Low disk space");
        assert_eq!(event.fields.get("free_mb").unwrap(), &serde_json::json!(50));
    }
}
