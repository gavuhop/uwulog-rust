pub mod evaluator;
pub mod parser;

pub use evaluator::{
    eval_event, get_event_field_cow, get_event_field_numeric, get_event_field_str, value_to_cow,
};
pub use parser::{parse_query, tokenize, Expr, NumOp, Parser, Token};

#[cfg(test)]
mod tests;
