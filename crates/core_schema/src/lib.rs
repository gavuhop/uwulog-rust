use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

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
        write!(f, "{}", self.as_str())
    }
}

impl LogLevel {
    pub const fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Unknown => "UNKNOWN",
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }

    pub fn parse_str(s: &str) -> Self {
        let clean = s.trim();
        if clean.eq_ignore_ascii_case("TRACE")
            || clean.eq_ignore_ascii_case("TRC")
            || clean.eq_ignore_ascii_case("VERBOSE")
        {
            LogLevel::Trace
        } else if clean.eq_ignore_ascii_case("DEBUG") || clean.eq_ignore_ascii_case("DBG") {
            LogLevel::Debug
        } else if clean.eq_ignore_ascii_case("INFO")
            || clean.eq_ignore_ascii_case("INF")
            || clean.eq_ignore_ascii_case("INFORMATION")
            || clean.eq_ignore_ascii_case("NOTICE")
        {
            LogLevel::Info
        } else if clean.eq_ignore_ascii_case("WARN")
            || clean.eq_ignore_ascii_case("WARNING")
            || clean.eq_ignore_ascii_case("WRN")
        {
            LogLevel::Warn
        } else if clean.eq_ignore_ascii_case("ERROR")
            || clean.eq_ignore_ascii_case("ERR")
            || clean.eq_ignore_ascii_case("CRITICAL")
            || clean.eq_ignore_ascii_case("CRIT")
        {
            LogLevel::Error
        } else if clean.eq_ignore_ascii_case("FATAL")
            || clean.eq_ignore_ascii_case("FTL")
            || clean.eq_ignore_ascii_case("EMERG")
            || clean.eq_ignore_ascii_case("EMERGENCY")
            || clean.eq_ignore_ascii_case("ALERT")
        {
            LogLevel::Fatal
        } else {
            LogLevel::Unknown
        }
    }
}

/// Các trường cơ sở chuẩn hóa trong toàn bộ hệ thống (Single Source of Truth)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StandardField {
    Timestamp,
    Level,
    Message,
    Id,
}

/// Phân loại kiểu dữ liệu của trường log
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldType {
    Time,
    Number,
    Text,
}

impl StandardField {
    /// Tên chuẩn tắc (Canonical Name)
    pub const fn canonical_name(&self) -> &'static str {
        match self {
            Self::Timestamp => "timestamp",
            Self::Level => "level",
            Self::Message => "message",
            Self::Id => "id",
        }
    }

    /// Tự động quy đổi mọi bí danh (aliases: ts, time, @timestamp, lvl, severity, msg...) về trường chuẩn
    pub fn from_alias(key: &str) -> Option<Self> {
        let clean = key.trim();
        match clean {
            "timestamp" | "time" | "ts" | "@timestamp" | "date" | "datetime" => {
                Some(Self::Timestamp)
            }
            "level" | "lvl" | "lv" | "severity" | "priority" => Some(Self::Level),
            "message" | "msg" | "text" | "body" => Some(Self::Message),
            "id" => Some(Self::Id),
            _ => None,
        }
    }

    /// Trả về danh sách 3 cột chuẩn hiển thị mặc định trên UI
    pub const fn default_columns() -> &'static [Self] {
        &[Self::Timestamp, Self::Level, Self::Message]
    }

    /// Kiểu dữ liệu mặc định của trường chuẩn
    pub const fn field_type(&self) -> FieldType {
        match self {
            Self::Timestamp => FieldType::Time,
            Self::Id => FieldType::Number,
            Self::Level | Self::Message => FieldType::Text,
        }
    }
}

/// Chuyển đổi an toàn serde_json::Value sang Cow<'_, str> không cấp phát thừa
pub fn value_to_cow(v: &serde_json::Value) -> Cow<'_, str> {
    match v {
        serde_json::Value::String(s) => Cow::Borrowed(s),
        serde_json::Value::Number(n) => Cow::Owned(n.to_string()),
        serde_json::Value::Bool(b) => Cow::Borrowed(if *b { "true" } else { "false" }),
        serde_json::Value::Null => Cow::Borrowed(""),
        other => Cow::Owned(other.to_string()),
    }
}

/// Chuyển đổi một HashMap có chứa các key dạng dot-notation ("a.b.c") về cấu trúc JSON lồng nhau (nested JSON object)
pub fn unflatten_json(fields: &HashMap<String, serde_json::Value>) -> serde_json::Value {
    let mut root = serde_json::Map::new();

    for (k, v) in fields {
        if !k.contains('.') {
            root.insert(k.clone(), v.clone());
        } else {
            let parts: Vec<&str> = k.split('.').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                root.insert(k.clone(), v.clone());
                continue;
            }
            let mut curr = &mut root;
            let last_idx = parts.len() - 1;
            for (i, &part) in parts.iter().enumerate() {
                if i == last_idx {
                    curr.insert(part.to_string(), v.clone());
                } else {
                    let entry = curr
                        .entry(part.to_string())
                        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
                    if !entry.is_object() {
                        *entry = serde_json::Value::Object(serde_json::Map::new());
                    }
                    curr = entry.as_object_mut().unwrap();
                }
            }
        }
    }

    serde_json::Value::Object(root)
}

static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);

/// Sinh ID số tuần tự tăng dần (Monotonic ID) phục vụ định danh LogEvent
pub fn next_log_id() -> u64 {
    NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed)
}

/// Đặt lại bộ đếm ID (chủ yếu dùng cho unit test)
pub fn reset_log_id_counter(val: u64) {
    NEXT_LOG_ID.store(val, Ordering::Relaxed);
}

/// LogEvent đại diện cho 1 bản ghi log đã được chuẩn hóa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub id: u64,
    pub timestamp: String,
    #[serde(default)]
    pub timestamp_secs: Option<f64>,
    pub level: LogLevel,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
}

impl LogEvent {
    pub fn new(
        timestamp: impl Into<String>,
        level: LogLevel,
        message: impl Into<String>,
        fields: HashMap<String, serde_json::Value>,
    ) -> Self {
        let ts_str = timestamp.into();
        let timestamp_secs = uwu_core_util::parse_iso_to_secs(&ts_str);
        Self {
            id: next_log_id(),
            timestamp: ts_str,
            timestamp_secs,
            level,
            message: message.into(),
            fields,
        }
    }

    /// Khởi tạo hoặc gán ID chỉ định cho LogEvent
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }

    /// Tạo chuỗi hiển thị thô On-Demand (tiết kiệm bộ nhớ RAM)
    /// Tự động khôi phục cấu trúc JSON lồng nhau từ các trường dot-notation nếu có
    pub fn raw_display(&self) -> Cow<'_, str> {
        if self.fields.is_empty() {
            Cow::Borrowed(&self.message)
        } else {
            let unflattened = unflatten_json(&self.fields);
            Cow::Owned(serde_json::to_string(&unflattened).unwrap_or_default())
        }
    }

    /// Trả về tên key thực tế xuất hiện trong `fields` tương ứng với trường chuẩn (nếu có), hoặc trả về canonical_name của trường đó
    pub fn semantic_key(&self, field: StandardField) -> &str {
        self.fields
            .keys()
            .find(|k| StandardField::from_alias(k) == Some(field))
            .map(|s| s.as_str())
            .unwrap_or_else(|| field.canonical_name())
    }

    /// Lấy giá trị chuỗi (Zero-Alloc Cow) của bất kỳ trường chuẩn hay custom nào
    pub fn get_field_cow<'a>(&'a self, field_name: &str) -> Option<Cow<'a, str>> {
        if let Some(std_field) = StandardField::from_alias(field_name) {
            match std_field {
                StandardField::Timestamp => Some(Cow::Borrowed(&self.timestamp)),
                StandardField::Level => Some(Cow::Borrowed(self.level.as_str())),
                StandardField::Message => Some(Cow::Borrowed(&self.message)),
                StandardField::Id => Some(Cow::Owned(self.id.to_string())),
            }
        } else {
            self.fields.get(field_name).map(value_to_cow)
        }
    }

    /// Lấy giá trị số (f64) phục vụ lọc số và thời gian
    pub fn get_field_numeric(&self, field_name: &str, now: f64) -> Option<f64> {
        if let Some(std_field) = StandardField::from_alias(field_name) {
            return match std_field {
                StandardField::Timestamp => self
                    .timestamp_secs
                    .or_else(|| uwu_core_util::parse_numeric_value(&self.timestamp, now)),
                StandardField::Id => Some(self.id as f64),
                _ => None,
            };
        }
        if let Some(v) = self.fields.get(field_name) {
            if let Some(n) = v.as_f64() {
                return Some(n);
            }
            if let Some(n) = v.as_i64() {
                return Some(n as f64);
            }
            if let Some(n) = v.as_u64() {
                return Some(n as f64);
            }
            if let Some(s) = v.as_str() {
                return uwu_core_util::parse_numeric_value(s, now);
            }
        }
        None
    }

    /// Tìm kiếm Text tự do trên toàn bộ bản ghi log (Canonical Full-Text Search)
    pub fn matches_text(&self, needle: &str) -> bool {
        uwu_core_util::contains_ignore_case(&self.message, needle)
            || uwu_core_util::contains_ignore_case(self.level.as_str(), needle)
            || uwu_core_util::contains_ignore_case(&self.timestamp, needle)
            || self.fields.values().any(|v| match v {
                serde_json::Value::String(s) => uwu_core_util::contains_ignore_case(s, needle),
                serde_json::Value::Number(n) => {
                    uwu_core_util::contains_ignore_case(&n.to_string(), needle)
                }
                serde_json::Value::Bool(b) => {
                    uwu_core_util::contains_ignore_case(if *b { "true" } else { "false" }, needle)
                }
                serde_json::Value::Null => false,
                other => uwu_core_util::contains_ignore_case(&other.to_string(), needle),
            })
    }
}

/// RawPayload chứa dữ liệu chưa chuẩn hóa nhận từ Nguồn (Sources)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RawPayload {
    Text(String),
    Json(serde_json::Value),
    KeyValue(HashMap<String, String>),
}

/// RawLogEntry đại diện cho dữ liệu thô gửi qua async channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLogEntry {
    pub payload: RawPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parse_all_variants() {
        assert_eq!(LogLevel::parse_str("TRACE"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_str("trc"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_str("VERBOSE"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_str("DEBUG"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_str("dbg"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_str("INFO"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("inf"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("INFORMATION"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("Notice"), LogLevel::Info);
        assert_eq!(LogLevel::parse_str("WARN"), LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("warning"), LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("wrn"), LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("ERROR"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("err"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("CRITICAL"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("crit"), LogLevel::Error);
        assert_eq!(LogLevel::parse_str("FATAL"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("ftl"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("EMERG"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("emergency"), LogLevel::Fatal);
        assert_eq!(LogLevel::parse_str("ALERT"), LogLevel::Fatal);
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
    fn test_standard_field_alias_resolution() {
        assert_eq!(
            StandardField::from_alias("time"),
            Some(StandardField::Timestamp)
        );
        assert_eq!(
            StandardField::from_alias("ts"),
            Some(StandardField::Timestamp)
        );
        assert_eq!(
            StandardField::from_alias("@timestamp"),
            Some(StandardField::Timestamp)
        );
        assert_eq!(StandardField::from_alias("lvl"), Some(StandardField::Level));
        assert_eq!(
            StandardField::from_alias("severity"),
            Some(StandardField::Level)
        );
        assert_eq!(
            StandardField::from_alias("msg"),
            Some(StandardField::Message)
        );
        assert_eq!(
            StandardField::from_alias("text"),
            Some(StandardField::Message)
        );
        assert_eq!(StandardField::from_alias("custom_field"), None);
    }

    #[test]
    fn test_standard_field_type() {
        assert_eq!(StandardField::Timestamp.field_type(), FieldType::Time);
        assert_eq!(StandardField::Level.field_type(), FieldType::Text);
        assert_eq!(StandardField::Message.field_type(), FieldType::Text);
        assert_eq!(StandardField::Id.field_type(), FieldType::Number);
    }

    #[test]
    fn test_log_event_ssot_accessors() {
        let mut fields = HashMap::new();
        fields.insert("latency_ms".to_string(), serde_json::json!(125.5));
        fields.insert("user_id".to_string(), serde_json::json!("u99"));

        let event = LogEvent::new(
            "2026-08-20T10:00:00Z",
            LogLevel::Error,
            "Something failed badly",
            fields,
        );

        // Access via alias
        assert_eq!(event.get_field_cow("ts").unwrap(), "2026-08-20T10:00:00Z");
        assert_eq!(event.get_field_cow("lvl").unwrap(), "ERROR");
        assert_eq!(
            event.get_field_cow("msg").unwrap(),
            "Something failed badly"
        );
        assert_eq!(event.get_field_cow("latency_ms").unwrap(), "125.5");

        // Numeric access
        assert_eq!(event.get_field_numeric("latency_ms", 0.0), Some(125.5));
        assert!(event.get_field_numeric("ts", 0.0).is_some());
        assert_eq!(event.get_field_numeric("id", 0.0), Some(event.id as f64));
        assert_eq!(event.get_field_numeric("lvl", 0.0), None);
        assert_eq!(event.get_field_numeric("non_existent", 0.0), None);

        // Full text search
        assert!(event.matches_text("failed"));
        assert!(event.matches_text("error"));
        assert!(event.matches_text("u99"));
        assert!(!event.matches_text("non_existent_text"));
    }

    #[test]
    fn test_value_to_cow() {
        assert_eq!(value_to_cow(&serde_json::json!("hello")), "hello");
        assert_eq!(value_to_cow(&serde_json::json!(42)), "42");
        assert_eq!(value_to_cow(&serde_json::json!(true)), "true");
        assert_eq!(value_to_cow(&serde_json::json!(false)), "false");
        assert_eq!(value_to_cow(&serde_json::Value::Null), "");
        assert_eq!(value_to_cow(&serde_json::json!([1, 2])), "[1,2]");
    }

    #[test]
    fn test_raw_payload_variants() {
        let text_payload = RawPayload::Text("plain log".to_string());
        let json_payload = RawPayload::Json(serde_json::json!({"key": "val"}));
        let mut kv_map = HashMap::new();
        kv_map.insert("k".to_string(), "v".to_string());
        let kv_payload = RawPayload::KeyValue(kv_map);

        let entry1 = RawLogEntry {
            payload: text_payload,
        };
        let entry2 = RawLogEntry {
            payload: json_payload,
        };
        let entry3 = RawLogEntry {
            payload: kv_payload,
        };

        match entry1.payload {
            RawPayload::Text(s) => assert_eq!(s, "plain log"),
            _ => panic!("Expected text payload"),
        }
        match entry2.payload {
            RawPayload::Json(v) => assert_eq!(v["key"], "val"),
            _ => panic!("Expected json payload"),
        }
        match entry3.payload {
            RawPayload::KeyValue(m) => assert_eq!(m.get("k").unwrap(), "v"),
            _ => panic!("Expected key-value payload"),
        }
    }

    #[test]
    fn test_unflatten_json_nested() {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), serde_json::json!(201));
        fields.insert(
            "metadata.instance_id".to_string(),
            serde_json::json!("i-7e5c5d"),
        );
        fields.insert(
            "metadata.system.infra.cluster".to_string(),
            serde_json::json!("eu-central-cluster-a"),
        );
        fields.insert(
            "metadata.system.infra.zone".to_string(),
            serde_json::json!("us-west-2-c"),
        );
        fields.insert(
            "metadata.system.env".to_string(),
            serde_json::json!("production"),
        );
        fields.insert(
            "metadata.region".to_string(),
            serde_json::json!("us-west-2"),
        );
        fields.insert("metadata.version".to_string(), serde_json::json!("v2.0.1"));
        fields.insert("latency".to_string(), serde_json::json!(831));
        fields.insert("lv".to_string(), serde_json::json!("INFO"));

        let unflattened = unflatten_json(&fields);

        assert_eq!(unflattened["status"], 201);
        assert_eq!(unflattened["latency"], 831);
        assert_eq!(unflattened["lv"], "INFO");
        assert_eq!(unflattened["metadata"]["instance_id"], "i-7e5c5d");
        assert_eq!(unflattened["metadata"]["region"], "us-west-2");
        assert_eq!(unflattened["metadata"]["version"], "v2.0.1");
        assert_eq!(unflattened["metadata"]["system"]["env"], "production");
        assert_eq!(
            unflattened["metadata"]["system"]["infra"]["cluster"],
            "eu-central-cluster-a"
        );
        assert_eq!(
            unflattened["metadata"]["system"]["infra"]["zone"],
            "us-west-2-c"
        );
    }

    #[test]
    fn test_raw_display_unflattens_nested_keys() {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), serde_json::json!(201));
        fields.insert(
            "metadata.instance_id".to_string(),
            serde_json::json!("i-7e5c5d"),
        );
        fields.insert(
            "metadata.system.infra.cluster".to_string(),
            serde_json::json!("eu-central-cluster-a"),
        );

        let event = LogEvent::new("2026-09-07T11:47:52+07:00", LogLevel::Info, "test", fields);
        let raw = event.raw_display();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(parsed["status"], 201);
        assert_eq!(parsed["metadata"]["instance_id"], "i-7e5c5d");
        assert_eq!(
            parsed["metadata"]["system"]["infra"]["cluster"],
            "eu-central-cluster-a"
        );
    }

    #[test]
    fn test_raw_display_plain_text() {
        let event = LogEvent::new("", LogLevel::Unknown, "plain message text", HashMap::new());
        assert_eq!(event.raw_display(), "plain message text");
    }
}
