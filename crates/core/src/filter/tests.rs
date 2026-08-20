use crate::filter::LogEngine;
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
fn test_timestamp_free_text_and_field_search() {
    let ts = "2026-08-15T16:28:35.089Z";
    let logs = vec![
        log("INFO", ts, "User logged in", "auth"),
        log("ERROR", "2026-08-15T16:29:00.000Z", "DB error", "db"),
    ];

    // 1. Quoted timestamp search
    assert_eq!(filter_logs(logs.clone(), format!("\"{}\"", ts)), vec![0]);

    // 2. Explicit field search
    assert_eq!(
        filter_logs(logs.clone(), format!("timestamp:\"{}\"", ts)),
        vec![0]
    );

    // 3. Quoted text containing colon
    let logs2 = vec![
        log("INFO", "T1", "Error: failed to connect to db", "auth"),
        log("INFO", "T2", "Success", "auth"),
    ];
    assert_eq!(
        filter_logs(logs2, "\"Error: failed to connect\"".into()),
        vec![0]
    );
}

#[test]
fn test_deeply_nested_boolean_logic() {
    let logs = vec![
        log("ERROR", "T1", "connection refused", "auth"),
        log("WARN", "T2", "slow query", "db"),
        log("INFO", "T3", "health check ok", "api"),
    ];

    // (level:error OR level:warn) AND (source:auth OR source:db)
    assert_eq!(
        filter_logs(
            logs.clone(),
            "(level:error OR level:warn) AND (source:auth OR source:db)".into()
        ),
        vec![0, 1]
    );

    // level:info OR (level:error AND source:db)
    assert_eq!(
        filter_logs(
            logs.clone(),
            "level:info OR (level:error AND source:db)".into()
        ),
        vec![2]
    );
}

#[test]
fn test_pipe_separated_multi_values() {
    let logs = vec![
        log("ERROR", "T1", "msg1", "auth"),
        log("WARN", "T2", "msg2", "db"),
        log("INFO", "T3", "msg3", "api"),
    ];

    // level:error|warn
    assert_eq!(
        filter_logs(logs.clone(), "level:error|warn".into()),
        vec![0, 1]
    );

    // source:auth|api
    assert_eq!(
        filter_logs(logs.clone(), "source:auth|api".into()),
        vec![0, 2]
    );
}

#[test]
fn test_malformed_queries_safety() {
    let logs = vec![
        log("ERROR", "T1", "failed connection to db", "auth"),
        log("INFO", "T2", "user logged in", "web"),
    ];

    // Unclosed quotes should not panic
    let res1 = filter_logs(logs.clone(), "message:\"failed connection".into());
    assert!(!res1.is_empty() || res1.is_empty()); // No panic

    // Unmatched parenthesis
    let res2 = filter_logs(logs.clone(), "(level:error OR".into());
    assert_eq!(res2, vec![0]);

    // Dangling colon / empty operator
    let res3 = filter_logs(logs.clone(), "level:".into());
    assert_eq!(res3, vec![0, 1]);

    // Invalid regex syntax falls back to literal substring search -> matches nothing if pattern not found
    let res4 = filter_logs(logs.clone(), "message:~[invalid(".into());
    assert_eq!(res4, Vec::<u32>::new());

    // Just boolean operators
    let res5 = filter_logs(logs.clone(), "AND OR NOT".into());
    assert_eq!(res5, vec![0, 1]);
}

#[test]
fn test_dotted_nested_field_filtering() {
    let logs = vec![
        json!({
            "level": "ERROR",
            "http.status": 500,
            "http.method": "POST",
            "user.id": "usr_99",
            "message": "Internal Server Error"
        }),
        json!({
            "level": "INFO",
            "http.status": 200,
            "http.method": "GET",
            "user.id": "usr_100",
            "message": "OK"
        }),
    ];

    assert_eq!(filter_logs(logs.clone(), "http.status:500".into()), vec![0]);
    assert_eq!(filter_logs(logs.clone(), "http.method:GET".into()), vec![1]);
    assert_eq!(filter_logs(logs.clone(), "user.id:usr_99".into()), vec![0]);
    assert_eq!(
        filter_logs(
            logs.clone(),
            "http.status:>=200 AND http.status:<400".into()
        ),
        vec![1]
    );
}

#[test]
fn test_floating_point_and_negative_number_comparisons() {
    let logs = vec![
        json!({ "latency": 0.05, "temp": -5.5, "level": "INFO" }),
        json!({ "latency": 1.25, "temp": 15.0, "level": "WARN" }),
        json!({ "latency": 150.0, "temp": 40.2, "level": "ERROR" }),
    ];

    // Floating point comparison
    assert_eq!(filter_logs(logs.clone(), "latency:<1.0".into()), vec![0]);
    assert_eq!(
        filter_logs(logs.clone(), "latency:>=1.25".into()),
        vec![1, 2]
    );

    // Negative numbers with comparison (<, >) and exact match (=)
    assert_eq!(filter_logs(logs.clone(), "temp:<-1.0".into()), vec![0]);
    assert_eq!(filter_logs(logs.clone(), "temp=-5.5".into()), vec![0]);
    assert_eq!(filter_logs(logs.clone(), "temp:>0".into()), vec![1, 2]);

    // Field negation: temp:-15 means temp NOT containing "15"
    assert_eq!(filter_logs(logs.clone(), "temp:-15".into()), vec![0, 2]);
}

#[test]
fn test_strict_field_casing_and_case_insensitive_values() {
    let logs = vec![
        log("ERROR", "T1", "DATABASE TIMEOUT", "PostgresDB"),
        log("INFO", "T2", "Redis Connected", "RedisCache"),
    ];

    // Field names are strict (case-sensitive as defined in schema)
    assert_eq!(filter_logs(logs.clone(), "level:error".into()), vec![0]);
    assert_eq!(
        filter_logs(logs.clone(), "LEVEL:ERROR".into()),
        Vec::<u32>::new()
    );

    assert_eq!(
        filter_logs(logs.clone(), "source:postgresdb".into()),
        vec![0]
    );
    assert_eq!(
        filter_logs(logs.clone(), "SOURCE:REDISCACHE".into()),
        Vec::<u32>::new()
    );

    // Field values are matched case-insensitively
    assert_eq!(
        filter_logs(logs.clone(), "message:database".into()),
        vec![0]
    );
    assert_eq!(
        filter_logs(logs.clone(), "message:DATABASE".into()),
        vec![0]
    );
}

#[test]
fn test_complex_not_negation_trees() {
    let logs = vec![
        log("ERROR", "T1", "msg1", "auth"),
        log("WARN", "T2", "msg2", "api"),
        log("INFO", "T3", "msg3", "cron"),
        log("DEBUG", "T4", "msg4", "auth"),
    ];

    // NOT (level:info OR level:debug) -> should match 0 (ERROR) and 1 (WARN)
    assert_eq!(
        filter_logs(logs.clone(), "NOT (level:info OR level:debug)".into()),
        vec![0, 1]
    );

    // -source:auth AND NOT level:info -> should match 1 (WARN on api)
    assert_eq!(
        filter_logs(logs.clone(), "-source:auth AND NOT level:info".into()),
        vec![1]
    );
}

#[test]
fn test_quotes_with_special_characters() {
    let logs = vec![
        log(
            "INFO",
            "T1",
            "Connected to https://api.service.io:8443/v1/auth",
            "client",
        ),
        log(
            "ERROR",
            "T2",
            "Host 192.168.1.50:9092 unreachable (err: ETIMEDOUT)",
            "kafka",
        ),
    ];

    assert_eq!(
        filter_logs(
            logs.clone(),
            "\"https://api.service.io:8443/v1/auth\"".into()
        ),
        vec![0]
    );
    assert_eq!(
        filter_logs(logs.clone(), "\"192.168.1.50:9092\"".into()),
        vec![1]
    );
    assert_eq!(
        filter_logs(logs.clone(), "message:\"ETIMEDOUT\"".into()),
        vec![1]
    );
}
