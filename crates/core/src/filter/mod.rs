pub mod engine;
pub mod evaluator;
pub mod log_item;
pub mod parser;
pub mod utils;

pub use engine::LogEngine;
pub use evaluator::eval;
pub use parser::{tokenize, Expr, NumOp, Parser, Token};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogItem {
    pub id: Uuid,
    pub fields: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests;
