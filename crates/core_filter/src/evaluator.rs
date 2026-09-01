use super::parser::{Expr, NumOp};
use serde_json::Value;
use std::borrow::Cow;
use uwu_core_schema::LogEvent;
use uwu_core_util::{contains_ignore_case, parse_numeric_value};

pub fn eval_event(expr: &Expr, log: &LogEvent, now: f64) -> bool {
    match expr {
        Expr::And(a, b) => eval_event(a, log, now) && eval_event(b, log, now),
        Expr::Or(a, b) => eval_event(a, log, now) || eval_event(b, log, now),
        Expr::Not(i) => !eval_event(i, log, now),

        Expr::Text(alts) => {
            if alts.is_empty() {
                return true;
            }
            alts.iter().any(|a| {
                if a.is_empty() {
                    return true;
                }
                contains_ignore_case(&log.message, a)
                    || contains_ignore_case(log.level.as_str(), a)
                    || contains_ignore_case(&log.source_id, a)
                    || contains_ignore_case(&log.timestamp, a)
                    || contains_ignore_case(&log.raw, a)
                    || log.fields.values().any(|v| match v {
                        Value::String(s) => contains_ignore_case(s, a),
                        Value::Number(n) => contains_ignore_case(&n.to_string(), a),
                        Value::Bool(b) => {
                            if *b {
                                contains_ignore_case("true", a)
                            } else {
                                contains_ignore_case("false", a)
                            }
                        }
                        Value::Null => false,
                        other => contains_ignore_case(&other.to_string(), a),
                    })
            })
        }

        Expr::FieldContainsAny { field, values } => {
            if let Some(actual) = get_event_field_cow(log, field) {
                values.iter().any(|v| contains_ignore_case(&actual, v))
            } else {
                false
            }
        }

        Expr::FieldExact { field, value } => {
            if let Some(actual) = get_event_field_cow(log, field) {
                actual.eq_ignore_ascii_case(value)
            } else {
                false
            }
        }

        Expr::FieldRegex { field, re, .. } => {
            if let Some(actual) = get_event_field_cow(log, field) {
                re.is_match(&actual)
            } else {
                false
            }
        }

        Expr::FieldCmp { field, op, value } => {
            let actual_f = get_event_field_numeric(log, field, now);
            match actual_f {
                Some(av) => match op {
                    NumOp::Gt => av > *value,
                    NumOp::Lt => av < *value,
                    NumOp::Gte => av >= *value,
                    NumOp::Lte => av <= *value,
                },
                None => false,
            }
        }

        Expr::FieldRange { field, lo, hi } => {
            get_event_field_numeric(log, field, now).is_some_and(|av| {
                let min = lo.min(*hi);
                let max = lo.max(*hi);
                av >= min && av <= max
            })
        }
    }
}

pub fn get_event_field_numeric(log: &LogEvent, field: &str, now: f64) -> Option<f64> {
    if field.eq_ignore_ascii_case("timestamp")
        || field.eq_ignore_ascii_case("time")
        || field.eq_ignore_ascii_case("ts")
        || field.eq_ignore_ascii_case("date")
    {
        if let Some(ts_sec) = log.timestamp_secs {
            return Some(ts_sec);
        }
    }
    get_event_field_cow(log, field).and_then(|s| parse_numeric_value(&s, now))
}

pub fn get_event_field_cow<'a>(log: &'a LogEvent, field: &str) -> Option<Cow<'a, str>> {
    let lower_field = field.to_ascii_lowercase();
    match lower_field.as_str() {
        "level" => Some(Cow::Borrowed(log.level.as_str())),
        "timestamp" | "time" | "ts" | "date" => Some(Cow::Borrowed(&log.timestamp)),
        "message" | "msg" => Some(Cow::Borrowed(&log.message)),
        "source" | "source_id" => Some(Cow::Borrowed(&log.source_id)),
        "id" => Some(Cow::Owned(log.id.to_string())),
        "raw" => Some(Cow::Borrowed(&log.raw)),
        _ => {
            // Direct match in fields
            if let Some(v) = log.fields.get(field) {
                return value_to_cow(v);
            }
            // Case-insensitive match in fields
            for (k, v) in &log.fields {
                if k.eq_ignore_ascii_case(field) {
                    return value_to_cow(v);
                }
            }
            None
        }
    }
}

pub fn get_event_field_str(log: &LogEvent, field: &str) -> Option<String> {
    get_event_field_cow(log, field).map(|c| c.into_owned())
}

pub fn eval(expr: &Expr, log: &Value, now: f64) -> bool {
    match expr {
        Expr::And(a, b) => eval(a, log, now) && eval(b, log, now),
        Expr::Or(a, b) => eval(a, log, now) || eval(b, log, now),
        Expr::Not(i) => !eval(i, log, now),

        Expr::Text(alts) => {
            if alts.is_empty() {
                return true;
            }
            if let Some(obj) = log.as_object() {
                alts.iter().any(|a| {
                    if a.is_empty() {
                        return true;
                    }
                    obj.values().any(|v| match v {
                        Value::String(s) => contains_ignore_case(s, a),
                        Value::Number(n) => contains_ignore_case(&n.to_string(), a),
                        Value::Bool(b) => {
                            if *b {
                                contains_ignore_case("true", a)
                            } else {
                                contains_ignore_case("false", a)
                            }
                        }
                        Value::Null => false,
                        other => contains_ignore_case(&other.to_string(), a),
                    })
                })
            } else {
                let s = value_to_string(log);
                alts.iter().any(|a| contains_ignore_case(&s, a))
            }
        }

        Expr::FieldContainsAny { field, values } => get_field_str(log, field)
            .is_some_and(|actual| values.iter().any(|v| contains_ignore_case(&actual, v))),

        Expr::FieldExact { field, value } => {
            get_field_str(log, field).is_some_and(|actual| actual.eq_ignore_ascii_case(value))
        }

        Expr::FieldRegex { field, re, .. } => {
            get_field_str(log, field).is_some_and(|actual| re.is_match(&actual))
        }

        Expr::FieldCmp { field, op, value } => {
            let actual_f = get_field_str(log, field).and_then(|s| parse_numeric_value(&s, now));
            match actual_f {
                Some(av) => match op {
                    NumOp::Gt => av > *value,
                    NumOp::Lt => av < *value,
                    NumOp::Gte => av >= *value,
                    NumOp::Lte => av <= *value,
                },
                None => false,
            }
        }

        Expr::FieldRange { field, lo, hi } => get_field_str(log, field)
            .and_then(|s| parse_numeric_value(&s, now))
            .is_some_and(|av| {
                let min = lo.min(*hi);
                let max = lo.max(*hi);
                av >= min && av <= max
            }),
    }
}

pub fn get_field_str(log: &Value, field: &str) -> Option<String> {
    log.get(field).map(value_to_string)
}

pub fn value_to_cow(v: &Value) -> Option<Cow<'_, str>> {
    match v {
        Value::String(s) => Some(Cow::Borrowed(s)),
        Value::Number(n) => Some(Cow::Owned(n.to_string())),
        Value::Bool(b) => Some(Cow::Borrowed(if *b { "true" } else { "false" })),
        Value::Null => Some(Cow::Borrowed("")),
        other => Some(Cow::Owned(other.to_string())),
    }
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "".into(),
        other => other.to_string(),
    }
}
