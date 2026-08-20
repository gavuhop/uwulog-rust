use serde_json::Value;

pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            continue;
        }
        if c == '\x1b' {
            if let Some(&'[') = chars.peek() {
                let _ = chars.next(); // consume '['
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
