use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Cấp độ log chuẩn hóa
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Unknown,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Unknown => write!(f, "UNKNOWN"),
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

impl LogLevel {
    pub fn parse_str(s: &str) -> Self {
        let clean = s.trim().to_uppercase();
        match clean.as_str() {
            "TRACE" | "TRC" | "VERBOSE" => LogLevel::Trace,
            "DEBUG" | "DBG" => LogLevel::Debug,
            "INFO" | "INF" | "INFORMATION" | "NOTICE" => LogLevel::Info,
            "WARN" | "WARNING" | "WRN" => LogLevel::Warn,
            "ERROR" | "ERR" | "CRITICAL" | "CRIT" => LogLevel::Error,
            "FATAL" | "FTL" | "EMERG" | "EMERGENCY" | "ALERT" => LogLevel::Fatal,
            _ => LogLevel::Unknown,
        }
    }
}

/// LogEvent đại diện cho 1 bản ghi log đã được chuẩn hóa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub id: Uuid,
    pub timestamp: String,
    pub level: LogLevel,
    pub source_id: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub raw: String,
}

impl LogEvent {
    pub fn new(
        timestamp: impl Into<String>,
        level: LogLevel,
        source_id: impl Into<String>,
        message: impl Into<String>,
        fields: HashMap<String, serde_json::Value>,
        raw: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: timestamp.into(),
            level,
            source_id: source_id.into(),
            message: message.into(),
            fields,
            raw: raw.into(),
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), serde_json::json!(self.id.to_string()));
        map.insert("timestamp".to_string(), serde_json::json!(&self.timestamp));
        map.insert(
            "level".to_string(),
            serde_json::json!(self.level.to_string()),
        );
        map.insert("source_id".to_string(), serde_json::json!(&self.source_id));
        map.insert("message".to_string(), serde_json::json!(&self.message));
        map.insert("raw".to_string(), serde_json::json!(&self.raw));

        for (k, v) in &self.fields {
            map.insert(k.clone(), v.clone());
        }

        serde_json::Value::Object(map)
    }
}

/// RawPayload chứa dữ liệu chưa chuẩn hóa nhận từ Nguồn (Sources)
#[derive(Debug, Clone)]
pub enum RawPayload {
    Text(String),
    Json(serde_json::Value),
    KeyValue(HashMap<String, String>),
}

/// RawLogEntry đại diện cho dữ liệu thô gửi qua async channel
#[derive(Debug, Clone)]
pub struct RawLogEntry {
    pub source_id: String,
    pub payload: RawPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parse_all_variants() {
        // Trace variants
        assert_eq!(LogLevel::parse_str("TRACE"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_str("trc"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_str("VERBOSE"), LogLevel::Trace);

        // Debug variants
        assert_eq!(LogLevel::parse_str("DEBUG"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_str("dbg"), LogLevel::Debug);

        // Info variants
        assert_eq!(LogLevel::parse_str("INFO"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("inf"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("INFORMATION"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("Notice"), LogLevel::Info);

        // Warn variants
        assert_eq!(LogLevel::parse_str("WARN"), LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("warning"), LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("wrn"), LogLevel::Warn);

        // Error variants
        assert_eq!(LogLevel::parse_str("ERROR"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("err"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("CRITICAL"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("crit"), LogLevel::Error);

        // Fatal variants
        assert_eq!(LogLevel::parse_str("FATAL"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("ftl"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("EMERG"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("emergency"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("ALERT"), LogLevel::Fatal);

        // Unknown / Fallback
        assert_eq!(LogLevel::parse_str("CUSTOM_LEVEL"), LogLevel::Unknown);
        assert_eq!(LogLevel::parse_str(""), LogLevel::Unknown);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
        assert_eq!(LogLevel::Fatal.to_string(), "FATAL");
        assert_eq!(LogLevel::Unknown.to_string(), "UNKNOWN");
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Unknown < LogLevel::Trace);
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Fatal);
    }

    #[test]
    fn test_log_event_creation_and_json_value() {
        let mut fields = HashMap::new();
        fields.insert("user_id".to_string(), serde_json::json!("u123"));
        fields.insert("latency".to_string(), serde_json::json!(125));

        let event = LogEvent::new(
            "2026-08-20T10:00:00Z",
            LogLevel::Error,
            "test_source",
            "Something failed",
            fields,
            "[ERROR] Something failed",
        );

        assert_eq!(event.timestamp, "2026-08-20T10:00:00Z");
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.source_id, "test_source");
        assert_eq!(event.message, "Something failed");
        assert_eq!(event.raw, "[ERROR] Something failed");
        assert!(!event.id.is_nil());

        let json_val = event.to_json_value();
        assert_eq!(json_val.get("level").unwrap(), &serde_json::json!("ERROR"));
        assert_eq!(
            json_val.get("source_id").unwrap(),
            &serde_json::json!("test_source")
        );
        assert_eq!(json_val.get("user_id").unwrap(), &serde_json::json!("u123"));
        assert_eq!(json_val.get("latency").unwrap(), &serde_json::json!(125));
    }

    #[test]
    fn test_raw_payload_variants() {
        let text_payload = RawPayload::Text("plain log".to_string());
        let json_payload = RawPayload::Json(serde_json::json!({"key": "val"}));
        let mut kv_map = HashMap::new();
        kv_map.insert("k".to_string(), "v".to_string());
        let kv_payload = RawPayload::KeyValue(kv_map);

        let entry = RawLogEntry {
            source_id: "proc:test".to_string(),
            payload: text_payload,
        };
        assert_eq!(entry.source_id, "proc:test");

        match entry.payload {
            RawPayload::Text(s) => assert_eq!(s, "plain log"),
            _ => panic!("Expected text payload"),
        }

        let _ = (json_payload, kv_payload);
    }
}
