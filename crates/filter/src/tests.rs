#[cfg(test)]
mod tests {
    use crate::engine::LogEngine;
    use serde_json::{json, Value};

    fn filter_logs(logs_val: Vec<Value>, query: String) -> Vec<u32> {
        let mut engine = LogEngine::new(100000);
        for l in logs_val {
            engine.push(&serde_json::to_string(&l).unwrap());
        }
        engine.filter(query)
    }

    fn log(level: &str, ts: &str, msg: &str, src: &str) -> Value {
        json!({
            "level": level,
            "timestamp": ts,
            "message": msg,
            "source": src,
            "latency": 500
        })
    }

    #[test]
    fn test_negation_variants() {
        let logs = vec![log("ERROR", "T1", "failed", "src")];
        // Cả 2 dạng -level:error và level:-error đều phải mang nghĩa phủ định
        assert_eq!(
            filter_logs(logs.clone(), "-level:error".into()),
            Vec::<u32>::new()
        );
        assert_eq!(filter_logs(logs, "level:-error".into()), Vec::<u32>::new());
    }

    #[test]
    fn test_complex_heuristic_quotes() {
        let logs = vec![log("INFO", "T1", "db connection failed", "auth")];
        // Test "Smart Closer" logic
        assert_eq!(
            filter_logs(logs.clone(), "message:\"db\" connection failed".into()),
            vec![0]
        );
        assert_eq!(
            filter_logs(logs.clone(), "message:\"db connection\" failed".into()),
            vec![0]
        );
    }

    #[test]
    fn test_advanced_logic() {
        let logs = vec![
            log("ERROR", "T1", "auth fail", "sys"),
            log("INFO", "T2", "login ok", "web"),
        ];
        // Sửa tag -> source để khớp với log generator
        assert_eq!(
            filter_logs(logs.clone(), "level:error OR source:web".into()),
            vec![0, 1]
        );
        assert_eq!(
            filter_logs(
                logs.clone(),
                "level:error AND (source:sys OR message:login)".into()
            ),
            vec![0]
        );
    }

    #[test]
    fn test_regex_variants() {
        let logs = vec![log("INFO", "T1", "user_123 logged in", "auth")];
        assert_eq!(
            filter_logs(logs.clone(), "message:~user_\\d+".into()),
            vec![0]
        );
        // Negative regex
        assert_eq!(filter_logs(logs, "-message:~admin".into()), vec![0]);
    }

    #[test]
    fn test_numeric_edge_cases() {
        let logs = vec![log("INFO", "T1", "msg", "src")];
        // latency:500 -> vec![0], latency:>600 -> empty
        assert_eq!(
            filter_logs(logs.clone(), "latency:200..600".into()),
            vec![0]
        );
        assert_eq!(filter_logs(logs, "latency:>600".into()), Vec::<u32>::new());
    }

    #[test]
    fn test_empty_and_unknown() {
        let logs = vec![log("INFO", "T1", "msg", "src")];
        assert_eq!(
            filter_logs(logs.clone(), "non_existent:val".into()),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn test_precedence_and_nesting() {
        let logs = vec![log("DEBUG", "T1", "test", "src")];
        assert_eq!(
            filter_logs(
                logs.clone(),
                "level:debug OR (level:info AND message:none)".into()
            ),
            vec![0]
        );
    }

    #[test]
    fn test_unicode_and_emojis() {
        let logs = vec![log("INFO", "T1", "Lỗi rỗng 🚀", "src")];
        assert_eq!(filter_logs(logs, "message:rỗng".into()), vec![0]);
    }

    #[test]
    fn test_strict_equality() {
        let logs = vec![log("ERROR", "T1", "msg", "src")];
        // level=error (exact) vs level:rr (contains)
        assert_eq!(filter_logs(logs.clone(), "level=ERROR".into()), vec![0]);
    }

    #[test]
    fn test_all_time_units() {
        let logs = vec![
            log("INFO", "2026-04-13T10:00:00Z", "L0", "src"),
            log("INFO", "2026-04-13T09:59:50Z", "L1", "src"), // -10s
            log("INFO", "2026-04-13T09:00:00Z", "L2", "src"), // -1h
        ];
        assert_eq!(
            filter_logs(logs.clone(), "timestamp:now..10s".into()),
            vec![0, 1]
        );
        assert_eq!(
            filter_logs(logs.clone(), "timestamp:now..2h".into()),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn test_empty_matches() {
        let logs = vec![
            log("INFO", "T1", "content", "auth"),
            log("INFO", "T2", "other", "db"),
        ];
        assert_eq!(filter_logs(logs.clone(), "".into()), vec![0, 1]);
        assert_eq!(filter_logs(logs.clone(), "\"\"".into()), vec![0, 1]);
    }

    #[test]
    fn test_unknown_field_safety() {
        let logs = vec![log("INFO", "T1", "ok", "src")];
        let empty: Vec<u32> = vec![];
        assert_eq!(filter_logs(logs, "random_xyz:123".into()), empty);
    }

    #[test]
    fn test_ansi_stripping() {
        let mut engine = LogEngine::new(100);

        // 1. Test parsing line with ANSI codes
        let line_with_ansi = "\x1b[32m{\"level\":\"info\",\"message\":\"hello\"}\x1b[0m";
        let res = engine.push(line_with_ansi);
        assert!(res.is_some());
        assert_eq!(res.unwrap()["message"], "hello");
    }
}
