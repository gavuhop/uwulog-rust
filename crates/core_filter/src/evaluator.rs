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
    if field == "timestamp" || field == "time" || field == "ts" || field == "date" {
        if let Some(ts_sec) = log.timestamp_secs {
            return Some(ts_sec);
        }
    }
    // Direct numeric evaluation (0 heap allocation, 0 string formatting/parsing overhead)
    if let Some(v) = log.fields.get(field) {
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
            return parse_numeric_value(s, now);
        }
    }
    get_event_field_cow(log, field).and_then(|s| parse_numeric_value(&s, now))
}

pub fn get_event_field_cow<'a>(log: &'a LogEvent, field: &str) -> Option<Cow<'a, str>> {
    if let Some(v) = log.fields.get(field) {
        return value_to_cow(v);
    }
    match field {
        "level" => Some(Cow::Borrowed(log.level.as_str())),
        "timestamp" => Some(Cow::Borrowed(&log.timestamp)),
        "message" => Some(Cow::Borrowed(&log.message)),
        "raw" => Some(Cow::Borrowed(&log.raw)),
        "id" => Some(Cow::Owned(log.id.to_string())),
        _ => None,
    }
}

pub fn get_event_field_str(log: &LogEvent, field: &str) -> Option<String> {
    get_event_field_cow(log, field).map(|c| c.into_owned())
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
