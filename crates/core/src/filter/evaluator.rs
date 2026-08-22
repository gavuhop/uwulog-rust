use super::parser::{Expr, NumOp};
use super::utils::parse_numeric_value;
use crate::schema::LogEvent;
use serde_json::Value;

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
                log.message.to_lowercase().contains(a)
                    || log.level.to_string().to_lowercase().contains(a)
                    || log.source_id.to_lowercase().contains(a)
                    || log.timestamp.to_lowercase().contains(a)
                    || log.raw.to_lowercase().contains(a)
                    || log.fields.values().any(|v| {
                        let s = value_to_string(v).to_lowercase();
                        s.contains(a)
                    })
            })
        }

        Expr::FieldContainsAny { field, values } => {
            get_event_field_str(log, field).is_some_and(|actual| {
                let lower = actual.to_lowercase();
                values.iter().any(|v| lower.contains(v))
            })
        }

        Expr::FieldExact { field, value } => get_event_field_str(log, field)
            .is_some_and(|actual| actual.to_lowercase() == value.to_lowercase()),

        Expr::FieldRegex { field, re, .. } => {
            get_event_field_str(log, field).is_some_and(|actual| re.is_match(&actual))
        }

        Expr::FieldCmp { field, op, value } => {
            let actual_f =
                get_event_field_str(log, field).and_then(|s| parse_numeric_value(&s, now));
            match actual_f {
                Some(av) => {
                    let eps = 0.0001;
                    match op {
                        NumOp::Gt => av > (*value + eps),
                        NumOp::Lt => av < (*value - eps),
                        NumOp::Gte => av >= (*value - eps),
                        NumOp::Lte => av <= (*value + eps),
                    }
                }
                None => false,
            }
        }

        Expr::FieldRange { field, lo, hi } => get_event_field_str(log, field)
            .and_then(|s| parse_numeric_value(&s, now))
            .is_some_and(|av| {
                let min = lo.min(*hi);
                let max = lo.max(*hi);
                let eps = 0.0001;
                av >= (min - eps) && av <= (max + eps)
            }),
    }
}

pub fn get_event_field_str(log: &LogEvent, field: &str) -> Option<String> {
    let lower_field = field.to_lowercase();
    match lower_field.as_str() {
        "level" => Some(log.level.to_string()),
        "timestamp" | "time" | "ts" | "date" => Some(log.timestamp.clone()),
        "message" | "msg" => Some(log.message.clone()),
        "source" | "source_id" => Some(log.source_id.clone()),
        "id" => Some(log.id.to_string()),
        "raw" => Some(log.raw.clone()),
        _ => {
            // Check direct match in fields
            if let Some(v) = log.fields.get(field) {
                return Some(value_to_string(v));
            }
            // Check case-insensitive match in fields
            for (k, v) in &log.fields {
                if k.eq_ignore_ascii_case(field) {
                    return Some(value_to_string(v));
                }
            }
            None
        }
    }
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
            // Thay vì build full text, ta kiểm tra từng field
            if let Some(obj) = log.as_object() {
                alts.iter().any(|a| {
                    if a.is_empty() {
                        return true;
                    }
                    obj.values().any(|v| {
                        let s = value_to_string(v).to_lowercase();
                        s.contains(a)
                    })
                })
            } else {
                let s = value_to_string(log).to_lowercase();
                alts.iter().any(|a| s.contains(a))
            }
        }

        Expr::FieldContainsAny { field, values } => {
            get_field_str(log, field).is_some_and(|actual| {
                let lower = actual.to_lowercase();
                values.iter().any(|v| lower.contains(v))
            })
        }

        Expr::FieldExact { field, value } => get_field_str(log, field)
            .is_some_and(|actual| actual.to_lowercase() == value.to_lowercase()),

        Expr::FieldRegex { field, re, .. } => {
            get_field_str(log, field).is_some_and(|actual| re.is_match(&actual))
        }

        Expr::FieldCmp { field, op, value } => {
            let actual_f = get_field_str(log, field).and_then(|s| parse_numeric_value(&s, now));
            match actual_f {
                Some(av) => {
                    let eps = 0.0001;
                    match op {
                        NumOp::Gt => av > (*value + eps),
                        NumOp::Lt => av < (*value - eps),
                        NumOp::Gte => av >= (*value - eps),
                        NumOp::Lte => av <= (*value + eps),
                    }
                }
                None => false,
            }
        }

        Expr::FieldRange { field, lo, hi } => get_field_str(log, field)
            .and_then(|s| parse_numeric_value(&s, now))
            .is_some_and(|av| {
                let min = lo.min(*hi);
                let max = lo.max(*hi);
                let eps = 0.0001;
                av >= (min - eps) && av <= (max + eps)
            }),
    }
}

pub fn get_field_str(log: &Value, field: &str) -> Option<String> {
    log.get(field).map(value_to_string)
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
