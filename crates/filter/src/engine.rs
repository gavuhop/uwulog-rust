use crate::evaluator;
use crate::log_item;
use crate::parser;
use crate::utils;
use rayon::prelude::*;
use serde_json::Value;
use std::collections::VecDeque;

pub struct LogEngine {
    logs: VecDeque<Value>,
    max_history: usize,
    max_timestamp: f64,
}

impl LogEngine {
    pub fn new(max_history: usize) -> Self {
        LogEngine {
            logs: VecDeque::with_capacity(1000),
            max_history,
            max_timestamp: 0.0,
        }
    }

    pub fn push(&mut self, line: &str) -> Option<Value> {
        let clean_trimmed = log_item::strip_ansi(line);
        let trimmed = clean_trimmed.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(parsed) = log_item::parse_log_line(trimmed) {
            return self.add_parsed_log(parsed);
        }
        None
    }

    pub fn push_value(&mut self, parsed: Value) -> Option<Value> {
        self.add_parsed_log(parsed)
    }

    fn add_parsed_log(&mut self, parsed: Value) -> Option<Value> {
        // Cập nhật max_timestamp từ log mới một cách nhanh chóng
        if let Some(obj) = parsed.as_object() {
            for v in obj.values() {
                if let Some(s) = v.as_str() {
                    // Kiểm tra nhanh xem có vẻ là timestamp không trước khi parse nặng
                    if s.len() >= 10 && (s.contains('-') || s.contains('T')) {
                        if let Some(ts) = utils::parse_iso_to_secs(s) {
                            if ts > self.max_timestamp {
                                self.max_timestamp = ts;
                            }
                            break; // Thường chỉ có 1 field timestamp chính
                        }
                    }
                }
            }
        }

        let log_to_return = parsed.clone();
        self.logs.push_back(parsed);
        if self.logs.len() > self.max_history {
            self.logs.pop_front();
        }
        Some(log_to_return)
    }

    pub fn filter(&self, query: String) -> Vec<u32> {
        if query.trim().is_empty() {
            return (0..self.logs.len() as u32).collect();
        }

        let data_now = if self.max_timestamp > 0.0 {
            self.max_timestamp
        } else {
            utils::now_secs()
        };

        let tokens = parser::tokenize(&query);
        let mut parser = parser::Parser::new(tokens, data_now);
        let ast = match parser.parse() {
            Some(e) => e,
            None => return (0u32..self.logs.len() as u32).collect(),
        };

        // Lọc song song dùng Rayon
        self.logs
            .par_iter()
            .enumerate()
            .filter_map(|(i, log)| {
                if evaluator::eval(&ast, log, data_now) {
                    Some(i as u32)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn filter_slice(&self, slice: &[Value], query: &str) -> Vec<usize> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return (0..slice.len()).collect();
        }

        let data_now = if self.max_timestamp > 0.0 {
            self.max_timestamp
        } else {
            utils::now_secs()
        };

        let tokens = parser::tokenize(trimmed);
        let mut parser = parser::Parser::new(tokens, data_now);
        let ast = match parser.parse() {
            Some(e) => e,
            None => return (0..slice.len()).collect(),
        };

        slice
            .iter()
            .enumerate()
            .filter_map(|(i, log)| {
                if evaluator::eval(&ast, log, data_now) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_logs(&self, indices: Vec<u32>) -> Vec<Value> {
        indices
            .into_iter()
            .filter_map(|i| self.logs.get(i as usize).cloned())
            .collect()
    }

    pub fn get_history(&self) -> Vec<Value> {
        self.logs.iter().cloned().collect()
    }

    pub fn stats(&self) -> usize {
        self.logs.len()
    }

    pub fn clear(&mut self) {
        self.logs.clear();
        self.max_timestamp = 0.0;
    }
}
