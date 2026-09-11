use uwu_core_schema::{LogColor, LogEvent, LogFields, RawLogEntry, RawPayload};
use uwu_core_util::strip_ansi;

struct DetectedSemanticFields {
    color: LogColor,
    timestamp: String,
    message: String,
}

pub struct LogNormalizer;

impl LogNormalizer {
    pub fn normalize(entry: RawLogEntry) -> LogEvent {
        match entry.payload {
            RawPayload::Json(v) => Self::normalize_json(&v),
            RawPayload::Text(s) => {
                let clean = strip_ansi(&s);
                let trimmed = clean.trim();

                // 1. Fast-check JSON: Chỉ thử parse JSON nếu bắt đầu và kết thúc bằng cặp ngoặc {} hoặc []
                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        return Self::normalize_json(&json_val);
                    }
                }

                // 2. Plain text thuần túy: Tái sử dụng chuỗi gốc nếu không có ANSI
                let message = match clean {
                    std::borrow::Cow::Borrowed(_) => s,
                    std::borrow::Cow::Owned(owned) => owned,
                };

                LogEvent::new(String::new(), LogColor::Default, message, LogFields::new())
            }
            RawPayload::KeyValue(kv) => {
                let fields: LogFields = kv
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::String(v)))
                    .collect();
                let detected = Self::detect_semantic_fields(&fields);

                LogEvent::new(detected.timestamp, detected.color, detected.message, fields)
            }
        }
    }

    fn flatten_json_value(prefix: &str, v: &serde_json::Value, out: &mut LogFields) {
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

    fn detect_semantic_fields(fields: &LogFields) -> DetectedSemanticFields {
        let mut timestamp_val = None;
        let mut level_val = None;
        let mut message_val = None;

        // Nhận diện các trường đặc biệt trong 1 lượt duyệt trực tiếp con trỏ tham chiếu (Zero-alloc & không HashMap lookup lần 2)
        for (k, val) in fields {
            if let Some(std_field) = uwu_core_schema::StandardField::from_alias(k) {
                match std_field {
                    uwu_core_schema::StandardField::Level if level_val.is_none() => {
                        level_val = Some(val);
                    }
                    uwu_core_schema::StandardField::Timestamp if timestamp_val.is_none() => {
                        timestamp_val = Some(val);
                    }
                    uwu_core_schema::StandardField::Message if message_val.is_none() => {
                        message_val = Some(val);
                    }
                    _ => {}
                }
            }
        }

        let color = if let Some(val) = level_val {
            LogColor::from_value(val)
        } else {
            LogColor::Default
        };

        let timestamp = if let Some(val) = timestamp_val {
            if let Some(s) = val.as_str() {
                s.to_string()
            } else {
                val.to_string()
            }
        } else {
            String::new()
        };

        let message = if let Some(val) = message_val {
            if let Some(s) = val.as_str() {
                s.to_string()
            } else {
                val.to_string()
            }
        } else {
            String::new()
        };

        DetectedSemanticFields {
            color,
            timestamp,
            message,
        }
    }

    fn normalize_json(v: &serde_json::Value) -> LogEvent {
        let mut fields = LogFields::new();
        Self::flatten_json_value("", v, &mut fields);
        fields.shrink_to_fit();
        let detected = Self::detect_semantic_fields(&fields);

        LogEvent::new(detected.timestamp, detected.color, detected.message, fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
        assert_eq!(event.color, LogColor::Red);
        assert_eq!(event.message, "Database connection lost");
        assert_eq!(
            event.fields.get("level").unwrap(),
            &serde_json::json!("ERROR")
        );
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
        assert_eq!(event.color, LogColor::Default);
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
        assert_eq!(event.color, LogColor::Green);
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
        assert_eq!(event.color, LogColor::Yellow);
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

        let raw_json: serde_json::Value = serde_json::from_str(&event.raw_display()).unwrap();
        assert_eq!(raw_json["metadata"]["system"]["env"], "production");
        assert_eq!(raw_json["metadata"]["system"]["cluster"], "k8s-us-west");
        assert_eq!(raw_json["metadata"]["latency_ms"], 120);
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
        assert_eq!(event.color, LogColor::Green);
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
        assert_eq!(event.color, LogColor::Red);
        assert_eq!(event.message, "Fatal storage error");
        assert_eq!(event.timestamp, "1724140800");
        assert_eq!(event.fields.get("lvl").unwrap(), &serde_json::json!("CRIT"));

        // Test alias severity + text + @timestamp
        let json_payload2 = serde_json::json!({
            "severity": "WARNING",
            "text": "High CPU",
            "@timestamp": "2026-08-20T10:00:00Z"
        });

        let event2 = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(json_payload2),
        });
        assert_eq!(event2.color, LogColor::Yellow);
        assert_eq!(event2.message, "High CPU");
        assert_eq!(event2.timestamp, "2026-08-20T10:00:00Z");
        assert_eq!(
            event2.fields.get("severity").unwrap(),
            &serde_json::json!("WARNING")
        );

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
        assert_eq!(event.color, LogColor::Yellow);
        assert_eq!(event.message, "Low disk space");
        assert_eq!(event.fields.get("free_mb").unwrap(), &serde_json::json!(50));
    }

    #[test]
    fn test_normalize_diverse_level_keys_and_values() {
        // 1. Python / Logback loglevel / log_level keys
        let event_py = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "log_level": "DEBUG",
                "message": "Checking cache"
            })),
        });
        assert_eq!(event_py.color, LogColor::Cyan);

        let event_trace = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "loglevel": "TRACE",
                "message": "Packet dump"
            })),
        });
        assert_eq!(event_trace.color, LogColor::Gray);

        // 2. Syslog notice & Bunyan numeric
        let event_notice = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "priority": "NOTICE",
                "message": "Security policy loaded"
            })),
        });
        assert_eq!(event_notice.color, LogColor::Blue);

        let event_bunyan = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "lvl": 50,
                "msg": "Fatal storage error"
            })),
        });
        assert_eq!(event_bunyan.color, LogColor::Red);

        // 3. Without a level key, color is Default (no message prefix guessing)
        let event_no_level = LogNormalizer::normalize(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "message": "[ERROR] Payment gateway unreachable",
                "attempt": 3
            })),
        });
        assert_eq!(event_no_level.color, LogColor::Default);
    }
}
