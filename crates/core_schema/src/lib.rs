use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub mod fields;
pub use fields::{Iter, IterMut, Keys, LogFields, Values};

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
            "level" | "lvl" | "lv" | "severity" | "priority" | "loglevel" | "log_level"
            | "log.level" | "levelname" => Some(Self::Level),
            "message" | "msg" | "text" | "body" | "log" | "content" => Some(Self::Message),
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

/// Gom nhóm các trường theo cụm (cluster prefix) dựa trên thứ tự xuất hiện tự nhiên đầu tiên:
/// - Các trường cùng prefix (vd: metadata.*) được gom lại liền kề nhau ngay tại vị trí xuất hiện đầu tiên của prefix đó.
/// - Không đẩy cả cụm xuống cuối danh sách (giữ nguyên vị trí tự nhiên).
/// - Không sắp xếp theo bảng chữ cái toàn bộ danh sách (các trường độc lập giữ nguyên thứ tự).
/// - Bên trong cụm lồng nhau, các nhánh con được sắp xếp để các nhánh cùng cấp nằm cạnh nhau.
pub fn cluster_fields<'a, V>(
    fields: impl IntoIterator<Item = (&'a String, V)>,
) -> Vec<(&'a String, V)> {
    let mut order_of_prefixes = Vec::new();
    let mut groups: HashMap<&str, Vec<(&'a String, V)>> = HashMap::new();

    for item in fields {
        let prefix = if let Some(dot_idx) = item.0.find('.') {
            &item.0[..dot_idx]
        } else {
            item.0.as_str()
        };

        if !groups.contains_key(prefix) {
            order_of_prefixes.push(prefix);
        }
        groups.entry(prefix).or_default().push(item);
    }

    let mut result = Vec::new();
    for prefix in order_of_prefixes {
        if let Some(mut items) = groups.remove(prefix) {
            if items.len() > 1 && items.iter().any(|(k, _)| k.contains('.')) {
                items.sort_unstable_by(|a, b| a.0.cmp(b.0));
            }
            result.extend(items);
        }
    }
    result
}

/// Chuyển đổi một danh sách các trường có thứ tự dạng dot-notation về cấu trúc JSON lồng nhau (nested JSON object)
pub fn unflatten_json_ordered<'a>(
    fields: impl IntoIterator<Item = (&'a str, &'a serde_json::Value)>,
) -> serde_json::Value {
    let mut root = serde_json::Map::new();

    for (k, v) in fields {
        if !k.contains('.') {
            root.insert(k.to_string(), v.clone());
        } else {
            let parts: Vec<&str> = k.split('.').filter(|s| !s.is_empty()).collect();
            if parts.is_empty() {
                root.insert(k.to_string(), v.clone());
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

/// Chuyển đổi một tập hợp các trường có chứa các key dạng dot-notation ("a.b.c") về cấu trúc JSON lồng nhau (nested JSON object)
/// Tự động gom nhóm các trường cùng cụm theo thứ tự xuất hiện tự nhiên
pub fn unflatten_json<'a>(
    fields: impl IntoIterator<Item = (&'a String, &'a serde_json::Value)>,
) -> serde_json::Value {
    let clustered = cluster_fields(fields);
    unflatten_json_ordered(clustered.into_iter().map(|(k, v)| (k.as_str(), v)))
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

/// Màu sắc định dạng hiển thị của app (App Metadata Key - Không thuộc dữ liệu log thô)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LogColor {
    Red,
    Yellow,
    Green,
    Gray,
    #[default]
    Default,
}

impl LogColor {
    pub fn from_value(val: &serde_json::Value) -> Self {
        match val {
            serde_json::Value::String(s) => Self::from_severity_str(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::from_number(i)
                } else {
                    LogColor::Default
                }
            }
            _ => LogColor::Default,
        }
    }

    pub fn from_number(n: i64) -> Self {
        match n {
            // Bunyan / Pino / Zap levels
            60 | 50 => LogColor::Red,
            40 => LogColor::Yellow,
            30 => LogColor::Green,
            20 | 10 => LogColor::Gray,

            // Syslog RFC 5424 (0=Emerg, 1=Alert, 2=Crit, 3=Err, 4=Warn, 5=Notice, 6=Info, 7=Debug)
            0..=3 => LogColor::Red,
            4 => LogColor::Yellow,
            6 => LogColor::Green,
            5 | 7 => LogColor::Gray,

            // HTTP Status Codes
            500..=599 => LogColor::Red,
            400..=499 => LogColor::Yellow,
            200..=299 => LogColor::Green,
            300..=399 => LogColor::Gray,

            _ => LogColor::Default,
        }
    }

    pub fn from_severity_str(s: &str) -> Self {
        let clean = s.trim();
        if let Ok(num) = clean.parse::<i64>() {
            let col = Self::from_number(num);
            if col != LogColor::Default {
                return col;
            }
        }

        if clean.eq_ignore_ascii_case("ERROR")
            || clean.eq_ignore_ascii_case("ERR")
            || clean.eq_ignore_ascii_case("CRITICAL")
            || clean.eq_ignore_ascii_case("CRIT")
            || clean.eq_ignore_ascii_case("FATAL")
            || clean.eq_ignore_ascii_case("FTL")
            || clean.eq_ignore_ascii_case("EMERG")
            || clean.eq_ignore_ascii_case("EMERGENCY")
            || clean.eq_ignore_ascii_case("ALERT")
            || clean.eq_ignore_ascii_case("PANIC")
            || clean.eq_ignore_ascii_case("FAIL")
            || clean.eq_ignore_ascii_case("FAILED")
            || clean.eq_ignore_ascii_case("FAILURE")
            || clean.eq_ignore_ascii_case("RED")
        {
            LogColor::Red
        } else if clean.eq_ignore_ascii_case("WARN")
            || clean.eq_ignore_ascii_case("WARNING")
            || clean.eq_ignore_ascii_case("WRN")
            || clean.eq_ignore_ascii_case("YELLOW")
        {
            LogColor::Yellow
        } else if clean.eq_ignore_ascii_case("INFO")
            || clean.eq_ignore_ascii_case("INF")
            || clean.eq_ignore_ascii_case("INFORMATION")
            || clean.eq_ignore_ascii_case("SUCCESS")
            || clean.eq_ignore_ascii_case("OK")
            || clean.eq_ignore_ascii_case("GREEN")
        {
            LogColor::Green
        } else if clean.eq_ignore_ascii_case("DEBUG")
            || clean.eq_ignore_ascii_case("DBG")
            || clean.eq_ignore_ascii_case("TRACE")
            || clean.eq_ignore_ascii_case("TRC")
            || clean.eq_ignore_ascii_case("VERBOSE")
            || clean.eq_ignore_ascii_case("FINE")
            || clean.eq_ignore_ascii_case("FINER")
            || clean.eq_ignore_ascii_case("FINEST")
            || clean.eq_ignore_ascii_case("SILLY")
            || clean.eq_ignore_ascii_case("NOTICE")
            || clean.eq_ignore_ascii_case("AUDIT")
            || clean.eq_ignore_ascii_case("NOTE")
            || clean.eq_ignore_ascii_case("GRAY")
            || clean.eq_ignore_ascii_case("GREY")
            || clean.eq_ignore_ascii_case("CYAN")
            || clean.eq_ignore_ascii_case("BLUE")
        {
            LogColor::Gray
        } else {
            LogColor::Default
        }
    }
}

/// LogEvent đại diện cho 1 bản ghi log đã được chuẩn hóa
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub id: u64,
    pub color: LogColor,
    pub timestamp: String,
    #[serde(default)]
    pub timestamp_secs: Option<f64>,
    pub message: String,
    pub fields: LogFields,
}

impl LogEvent {
    pub fn new(
        timestamp: impl Into<String>,
        color: LogColor,
        message: impl Into<String>,
        fields: impl Into<LogFields>,
    ) -> Self {
        let ts_str = timestamp.into();
        let timestamp_secs = uwu_core_util::parse_iso_to_secs(&ts_str);
        Self {
            id: next_log_id(),
            color,
            timestamp: ts_str,
            timestamp_secs,
            message: message.into(),
            fields: fields.into(),
        }
    }

    /// Khởi tạo hoặc gán ID chỉ định cho LogEvent
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }

    /// Tạo chuỗi hiển thị thô On-Demand (tiết kiệm bộ nhớ RAM)
    /// Tự động khôi phục cấu trúc JSON lồng nhau từ các trường dot-notation theo cụm
    pub fn raw_display(&self) -> Cow<'_, str> {
        if self.fields.is_empty() {
            Cow::Borrowed(&self.message)
        } else {
            let unflattened = unflatten_json(&self.fields);
            Cow::Owned(serde_json::to_string(&unflattened).unwrap_or_default())
        }
    }

    /// Tạo chuỗi JSON hiển thị định dạng Beauty (Pretty / Indented) On-Demand
    /// Tự động khôi phục cấu trúc JSON lồng nhau từ các trường dot-notation theo cụm
    pub fn beauty_display(&self) -> Cow<'_, str> {
        if self.fields.is_empty() {
            Cow::Borrowed(&self.message)
        } else {
            let unflattened = unflatten_json(&self.fields);
            Cow::Owned(serde_json::to_string_pretty(&unflattened).unwrap_or_default())
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

    /// Trả về cặp (key, value) thực tế trong `fields` tương ứng với trường chuẩn (nếu có)
    pub fn raw_field_for_standard(
        &self,
        field: StandardField,
    ) -> Option<(&str, &serde_json::Value)> {
        self.fields
            .iter()
            .find(|(k, _)| StandardField::from_alias(k) == Some(field))
            .map(|(k, v)| (k.as_str(), v))
    }

    /// Lấy giá trị chuỗi (Zero-Alloc Cow) của bất kỳ trường chuẩn hay custom nào:
    /// 1. Nếu `field_name` khớp chính xác một key trong `self.fields`: trả về giá trị thực tế của trường đó.
    /// 2. Nếu không có key chính xác nhưng `field_name` là trường chuẩn hoặc alias của trường chuẩn:
    ///    - Ưu tiên tìm trường tương ứng thực tế trong `self.fields` (ví dụ: cột là "level" nhưng log có "lvl: warning" -> trả về "warning").
    ///    - Nếu log không chứa trường đó trong `fields` (ví dụ: plain text log), fallback về giá trị chuẩn hóa trong struct (`self.timestamp`, `self.message`, `self.id`).
    pub fn get_field_cow<'a>(&'a self, field_name: &str) -> Option<Cow<'a, str>> {
        // 1. Kiểm tra trực tiếp key trong fields (O(1) lookup)
        if let Some(v) = self.fields.get(field_name) {
            return Some(value_to_cow(v));
        }

        // 2. Nếu không có key chính xác, kiểm tra xem có phải alias của trường chuẩn không
        if let Some(std_field) = StandardField::from_alias(field_name) {
            // Tìm xem trong fields có key nào khác là alias của std_field không
            if let Some((_, v)) = self.raw_field_for_standard(std_field) {
                return Some(value_to_cow(v));
            }

            // Fallback về trường chuẩn hóa trong struct nếu fields không có
            return match std_field {
                StandardField::Timestamp => {
                    if !self.timestamp.is_empty() {
                        Some(Cow::Borrowed(&self.timestamp))
                    } else {
                        None
                    }
                }
                StandardField::Message => {
                    if !self.message.is_empty() {
                        Some(Cow::Borrowed(&self.message))
                    } else {
                        None
                    }
                }
                StandardField::Id => Some(Cow::Owned(self.id.to_string())),
                _ => None,
            };
        }

        None
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
    fn test_log_color_classification_strings() {
        assert_eq!(LogColor::from_severity_str("ERROR"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("err"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("CRITICAL"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("fatal"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("panic"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("fail"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("red"), LogColor::Red);

        assert_eq!(LogColor::from_severity_str("WARN"), LogColor::Yellow);
        assert_eq!(LogColor::from_severity_str("warning"), LogColor::Yellow);
        assert_eq!(LogColor::from_severity_str("wrn"), LogColor::Yellow);

        assert_eq!(LogColor::from_severity_str("INFO"), LogColor::Green);
        assert_eq!(LogColor::from_severity_str("information"), LogColor::Green);
        assert_eq!(LogColor::from_severity_str("success"), LogColor::Green);
        assert_eq!(LogColor::from_severity_str("ok"), LogColor::Green);

        assert_eq!(LogColor::from_severity_str("NOTICE"), LogColor::Gray);
        assert_eq!(LogColor::from_severity_str("audit"), LogColor::Gray);

        assert_eq!(LogColor::from_severity_str("DEBUG"), LogColor::Gray);
        assert_eq!(LogColor::from_severity_str("dbg"), LogColor::Gray);

        assert_eq!(LogColor::from_severity_str("TRACE"), LogColor::Gray);
        assert_eq!(LogColor::from_severity_str("verbose"), LogColor::Gray);
        assert_eq!(LogColor::from_severity_str("silly"), LogColor::Gray);

        assert_eq!(LogColor::from_severity_str("CUSTOM"), LogColor::Default);
        assert_eq!(LogColor::from_severity_str(""), LogColor::Default);
    }

    #[test]
    fn test_log_color_classification_numbers() {
        // Bunyan / Pino
        assert_eq!(LogColor::from_number(60), LogColor::Red);
        assert_eq!(LogColor::from_number(50), LogColor::Red);
        assert_eq!(LogColor::from_number(40), LogColor::Yellow);
        assert_eq!(LogColor::from_number(30), LogColor::Green);
        assert_eq!(LogColor::from_number(20), LogColor::Gray);
        assert_eq!(LogColor::from_number(10), LogColor::Gray);

        // Syslog
        assert_eq!(LogColor::from_number(3), LogColor::Red);
        assert_eq!(LogColor::from_number(4), LogColor::Yellow);
        assert_eq!(LogColor::from_number(5), LogColor::Gray);
        assert_eq!(LogColor::from_number(6), LogColor::Green);
        assert_eq!(LogColor::from_number(7), LogColor::Gray);

        // HTTP status codes
        assert_eq!(LogColor::from_number(500), LogColor::Red);
        assert_eq!(LogColor::from_number(404), LogColor::Yellow);
        assert_eq!(LogColor::from_number(200), LogColor::Green);
        assert_eq!(LogColor::from_number(304), LogColor::Gray);

        // Stringified numbers
        assert_eq!(LogColor::from_severity_str("50"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("40"), LogColor::Yellow);
        assert_eq!(LogColor::from_severity_str("30"), LogColor::Green);
        assert_eq!(LogColor::from_severity_str("10"), LogColor::Gray);

        // Memory footprint: exact 1 byte, Copy, zero heap allocation
        assert_eq!(std::mem::size_of::<LogColor>(), 1);
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
        assert_eq!(
            StandardField::from_alias("log"),
            Some(StandardField::Message)
        );
        assert_eq!(
            StandardField::from_alias("content"),
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
            LogColor::Red,
            "Something failed badly",
            fields,
        );

        // Access via alias
        assert_eq!(event.get_field_cow("ts").unwrap(), "2026-08-20T10:00:00Z");
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

        let event = LogEvent::new("2026-09-07T11:47:52+07:00", LogColor::Green, "test", fields);
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
        let event = LogEvent::new("", LogColor::Default, "plain message text", HashMap::new());
        assert_eq!(event.raw_display(), "plain message text");
    }

    #[test]
    fn test_beauty_display() {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), serde_json::json!(201));
        fields.insert(
            "metadata.instance_id".to_string(),
            serde_json::json!("i-7e5c5d"),
        );

        let event = LogEvent::new("2026-09-07T11:47:52+07:00", LogColor::Green, "test", fields);
        let beauty = event.beauty_display();
        assert!(beauty.contains('\n'));
        assert!(beauty.contains("\"status\": 201"));
        assert!(beauty.contains("\"metadata\": {"));
        assert!(beauty.contains("\"instance_id\": \"i-7e5c5d\""));
    }

    #[test]
    fn test_raw_display_preserves_raw_data_and_types() {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), serde_json::json!(500));
        fields.insert("level".to_string(), serde_json::json!(30));

        let event = LogEvent::new(
            "2026-09-07T11:47:52+07:00",
            LogColor::Red,
            "error occurred",
            fields,
        );
        let raw = event.raw_display();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["level"], 30);
        assert_eq!(parsed["status"], 500);

        let mut fields_alias = HashMap::new();
        fields_alias.insert("lvl".to_string(), serde_json::json!("notice"));
        let event_alias = LogEvent::new(
            "2026-09-07T11:47:52+07:00",
            LogColor::Green,
            "ok",
            fields_alias,
        );
        let raw_alias = event_alias.raw_display();
        let parsed_alias: serde_json::Value = serde_json::from_str(&raw_alias).unwrap();
        assert_eq!(parsed_alias["lvl"], "notice");
    }

    #[test]
    fn test_get_field_cow_hierarchical_resolution() {
        let mut fields = HashMap::new();
        fields.insert("lvl".to_string(), serde_json::json!("warning"));
        fields.insert("ts".to_string(), serde_json::json!("1724140800"));
        fields.insert("user_id".to_string(), serde_json::json!(42));

        let event = LogEvent::new("1724140800", LogColor::Yellow, "test msg", fields);

        assert_eq!(event.get_field_cow("lvl").unwrap(), "warning");
        assert_eq!(event.get_field_cow("user_id").unwrap(), "42");
        assert_eq!(event.get_field_cow("level").unwrap(), "warning");
        assert_eq!(event.get_field_cow("timestamp").unwrap(), "1724140800");
        assert_eq!(event.get_field_cow("message").unwrap(), "test msg");

        let plain_event = LogEvent::new("2026-09-11", LogColor::Red, "fatal error", HashMap::new());
        assert_eq!(
            plain_event.get_field_cow("timestamp").unwrap(),
            "2026-09-11"
        );
        assert_eq!(plain_event.get_field_cow("message").unwrap(), "fatal error");
    }

    #[test]
    fn test_log_color_parsing() {
        assert_eq!(LogColor::from_severity_str("10"), LogColor::Gray);
        assert_eq!(LogColor::from_severity_str("20"), LogColor::Gray);
        assert_eq!(LogColor::from_severity_str("30"), LogColor::Green);
        assert_eq!(LogColor::from_severity_str("40"), LogColor::Yellow);
        assert_eq!(LogColor::from_severity_str("50"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("60"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("WARN"), LogColor::Yellow);
        assert_eq!(LogColor::from_severity_str("ERROR"), LogColor::Red);
        assert_eq!(LogColor::from_severity_str("DEBUG"), LogColor::Gray);

        assert_eq!(
            LogColor::from_value(&serde_json::json!(30)),
            LogColor::Green
        );
        assert_eq!(LogColor::from_value(&serde_json::json!(50)), LogColor::Red);
        assert_eq!(
            LogColor::from_value(&serde_json::json!("NOTICE")),
            LogColor::Gray
        );
    }
}
