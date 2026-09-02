use super::parser::{Expr, NumOp};
use std::borrow::Cow;
use uwu_core_schema::LogEvent;
use uwu_core_util::contains_ignore_case;

pub fn eval_event(expr: &Expr, log: &LogEvent, now: f64) -> bool {
    match expr {
        Expr::And(a, b) => eval_event(a, log, now) && eval_event(b, log, now),
        Expr::Or(a, b) => eval_event(a, log, now) || eval_event(b, log, now),
        Expr::Not(i) => !eval_event(i, log, now),

        Expr::Text(alts) => {
            if alts.is_empty() {
                return true;
            }
            alts.iter().any(|a| a.is_empty() || log.matches_text(a))
        }

        Expr::FieldContainsAny { field, values } => {
            if let Some(actual) = log.get_field_cow(field) {
                values.iter().any(|v| contains_ignore_case(&actual, v))
            } else {
                false
            }
        }

        Expr::FieldExact { field, value } => {
            if let Some(actual) = log.get_field_cow(field) {
                actual.eq_ignore_ascii_case(value)
            } else {
                false
            }
        }

        Expr::FieldRegex { field, re, .. } => {
            if let Some(actual) = log.get_field_cow(field) {
                re.is_match(&actual)
            } else {
                false
            }
        }

        Expr::FieldCmp { field, op, value } => {
            let actual_f = log.get_field_numeric(field, now);
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

        Expr::FieldRange { field, lo, hi } => log.get_field_numeric(field, now).is_some_and(|av| {
            let min = lo.min(*hi);
            let max = lo.max(*hi);
            av >= min && av <= max
        }),
    }
}

pub fn get_event_field_cow<'a>(log: &'a LogEvent, field: &str) -> Option<Cow<'a, str>> {
    log.get_field_cow(field)
}

pub fn get_event_field_numeric(log: &LogEvent, field: &str, now: f64) -> Option<f64> {
    log.get_field_numeric(field, now)
}

pub fn get_event_field_str(log: &LogEvent, field: &str) -> Option<String> {
    log.get_field_cow(field).map(|c| c.into_owned())
}

pub fn value_to_cow(v: &serde_json::Value) -> Option<Cow<'_, str>> {
    uwu_core_schema::value_to_cow(v)
}
