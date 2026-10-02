use super::evaluator::eval_event;
use super::parser::parse_query;
use serde_json::{json, Value};
use std::collections::HashMap;
use uwu_core_schema::{LogColor, LogEvent};
use uwu_core_util::parse_numeric_value;

fn val_to_log_event(v: &Value) -> LogEvent {
    let mut fields = HashMap::new();
    let mut color = LogColor::Default;
    let mut timestamp = String::new();
    let mut message = String::new();

    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            fields.insert(k.clone(), val.clone());
            if let Some(std_field) = uwu_core_schema::StandardField::from_alias(k) {
                match std_field {
                    uwu_core_schema::StandardField::Level => {
                        color = LogColor::from_value(val);
                    }
                    uwu_core_schema::StandardField::Timestamp => {
                        if let Some(s) = val.as_str() {
                            timestamp = s.to_string();
                        } else {
                            timestamp = val.to_string();
                        }
                    }
                    uwu_core_schema::StandardField::Message => {
                        if let Some(s) = val.as_str() {
                            message = s.to_string();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    LogEvent::new(timestamp, color, message, fields)
}

fn filter_logs(logs_val: Vec<Value>, query: String) -> Vec<u32> {
    let events: Vec<LogEvent> = logs_val.iter().map(val_to_log_event).collect();
    let max_ts = events
        .iter()
        .filter_map(|e| e.timestamp_secs)
        .fold(0.0f64, |acc, ts| acc.max(ts));
    let now = if max_ts > 0.0 {
        max_ts
    } else {
        uwu_core_util::now_secs()
    };

    let expr = parse_query(&query, now);
    events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            if eval_event(&expr, e, now) {
                Some(i as u32)
            } else {
                None
            }
        })
        .collect()
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
    assert_eq!(
        filter_logs(logs.clone(), "message:~\"user_\\d+\"".into()),
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

    // 4. Partial unquoted time substring search (e.g. 16:28, 16:28:35)
    assert_eq!(filter_logs(logs.clone(), "16:28".into()), vec![0]);
    assert_eq!(filter_logs(logs.clone(), "16:28:35".into()), vec![0]);
    assert_eq!(filter_logs(logs.clone(), "timestamp:16:28".into()), vec![0]);
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

#[test]
fn test_relative_now_and_sub_millisecond_floats() {
    let now = 1700000000.0;
    // 1. Test now-5m, now-1h, now+30s
    let parsed_5m = parse_numeric_value("now-5m", now).unwrap();
    assert_eq!(parsed_5m, now - 300.0);

    let parsed_1h = parse_numeric_value("now-1h", now).unwrap();
    assert_eq!(parsed_1h, now - 3600.0);

    let parsed_30s = parse_numeric_value("now+30s", now).unwrap();
    assert_eq!(parsed_30s, now + 30.0);

    // 2. Test sub-millisecond float precision without epsilon 0.0001
    let mut log_item = LogEvent::new(
        "2026-08-23T10:00:00Z",
        LogColor::Green,
        "Sub-ms latency test",
        HashMap::new(),
    );
    log_item
        .fields
        .insert("latency".to_string(), serde_json::json!(0.00005)); // 50 microseconds

    let expr_gt_zero = parse_query("latency>0", now);
    assert!(eval_event(&expr_gt_zero, &log_item, now));

    let expr_lt_100us = parse_query("latency<0.0001", now);
    assert!(eval_event(&expr_lt_100us, &log_item, now));

    // 3. Test quoted minus literal preservation vs negated quoted string
    let log_neg_val = LogEvent::new(
        "2026-08-23T10:00:00Z",
        LogColor::Green,
        "Offset is -500ms between clocks",
        HashMap::new(),
    );
    let expr_quoted_minus = parse_query("\"-500ms\"", now);
    assert!(eval_event(&expr_quoted_minus, &log_neg_val, now));

    let expr_negated_quote = parse_query("-\"clocks\"", now);
    assert!(!eval_event(&expr_negated_quote, &log_neg_val, now));

    // 4. Test URL matching without quotes
    let log_url = LogEvent::new(
        "2026-08-23T10:00:00Z",
        LogColor::Green,
        "Request to https://api.service.io/v1/health status=200",
        HashMap::new(),
    );
    let expr_url = parse_query("https://api.service.io/v1/health", now);
    assert!(eval_event(&expr_url, &log_url, now));
}

#[test]
fn test_escaped_quotes_and_nested_quotes_query() {
    let now = 1755940000.0;
    let log_event = LogEvent::new(
        "2026-08-23T10:00:00Z",
        LogColor::Red,
        "Failed to \"validate\" payment token",
        HashMap::new(),
    );

    // 1. Field contains with escaped quotes: message:"Failed to \"validate\" payment token"
    let expr1 = parse_query("message:\"Failed to \\\"validate\\\" payment token\"", now);
    assert!(eval_event(&expr1, &log_event, now));

    // 2. Free-text with escaped quotes: "Failed to \"validate\" payment token"
    let expr2 = parse_query("\"Failed to \\\"validate\\\" payment token\"", now);
    assert!(eval_event(&expr2, &log_event, now));

    // 3. Exact field match with escaped quotes: message="Failed to \"validate\" payment token"
    let expr3 = parse_query("message=\"Failed to \\\"validate\\\" payment token\"", now);
    assert!(eval_event(&expr3, &log_event, now));

    // 4. Negated search with escaped quotes: -message:"Failed to \"validate\" payment token"
    let expr4 = parse_query("-message:\"Failed to \\\"validate\\\" payment token\"", now);
    assert!(!eval_event(&expr4, &log_event, now));
}
