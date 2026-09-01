use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use uwu_core_util::strip_ansi;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogItem {
    pub id: Uuid,
    pub fields: HashMap<String, Value>,
}

pub fn parse_log_line(line: &str) -> Option<Value> {
    let clean = strip_ansi(line);
    serde_json::from_str::<Value>(&clean).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_line() {
        let valid_json = "{\"level\":\"info\",\"message\":\"test message\"}";
        let parsed = parse_log_line(valid_json);
        assert!(parsed.is_some());
        assert_eq!(parsed.unwrap().get("level").unwrap(), "info");

        let invalid = "plain text log";
        assert_eq!(parse_log_line(invalid), None);
    }

    #[test]
    fn test_strip_ansi_in_log_item() {
        let color_log = "\x1b[33mWarning message\x1b[0m";
        assert_eq!(strip_ansi(color_log), "Warning message");
    }
}
