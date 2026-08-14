use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Cấp độ log chuẩn hóa
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Unknown,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Fatal => write!(f, "FATAL"),
            LogLevel::Unknown => write!(f, "UNKNOWN"),
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
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub source_id: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub raw: String,
}

impl LogEvent {
    pub fn new(
        timestamp: DateTime<Utc>,
        level: LogLevel,
        source_id: impl Into<String>,
        message: impl Into<String>,
        fields: HashMap<String, serde_json::Value>,
        raw: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp,
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
        map.insert(
            "timestamp".to_string(),
            serde_json::json!(self.timestamp.to_rfc3339()),
        );
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
