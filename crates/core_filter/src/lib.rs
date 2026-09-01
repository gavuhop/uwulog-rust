pub mod engine;
pub mod evaluator;
pub mod log_item;
pub mod parser;

pub use engine::LogEngine;
pub use evaluator::{
    eval, eval_event, get_event_field_cow, get_event_field_numeric, get_event_field_str,
};
pub use log_item::{parse_log_line, LogItem};
pub use parser::{parse_query, tokenize, Expr, NumOp, Parser, Token};

#[cfg(test)]
mod tests;
