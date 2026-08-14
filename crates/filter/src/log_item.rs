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
                    if next_c >= '@' && next_c <= '~' {
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
