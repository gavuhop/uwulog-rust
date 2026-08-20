use super::utils::parse_numeric_value;
use regex::Regex;

#[derive(Debug, Clone)]
pub enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    FieldContainsAny {
        field: String,
        values: Vec<String>,
    },
    FieldExact {
        field: String,
        value: String,
    },
    FieldRegex {
        field: String,
        pattern: String,
        re: Regex,
    },
    FieldCmp {
        field: String,
        op: NumOp,
        value: f64,
    },
    FieldRange {
        field: String,
        lo: f64,
        hi: f64,
    },
    Text(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum NumOp {
    Gt,
    Lt,
    Gte,
    Lte,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    And,
    Or,
    Not,
    LParen,
    RParen,
    Atom(String),
}

pub fn tokenize(query: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        if chars[i] == '(' {
            tokens.push(Token::LParen);
            i += 1;
            continue;
        }
        if chars[i] == ')' {
            tokens.push(Token::RParen);
            i += 1;
            continue;
        }

        let mut word = String::new();
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' && chars[i] != ')' {
            if chars[i] == '"' || chars[i] == '\'' {
                let quote = chars[i];
                word.push(quote);
                i += 1;
                while i < chars.len() {
                    if chars[i] == quote {
                        let is_eof = i + 1 == chars.len();
                        let next_is_paren = !is_eof && chars[i + 1] == ')';
                        let next_is_space = !is_eof && chars[i + 1].is_whitespace();

                        // Close if followed by space, eof or paren
                        if is_eof || next_is_paren || next_is_space {
                            word.push(quote);
                            i += 1;
                            break;
                        } else {
                            word.push(chars[i]);
                            i += 1;
                        }
                    } else {
                        word.push(chars[i]);
                        i += 1;
                    }
                }
            } else {
                word.push(chars[i]);
                i += 1;
            }
        }
        if word.is_empty() {
            continue;
        }
        match word.to_ascii_uppercase().as_str() {
            "AND" => tokens.push(Token::And),
            "OR" => tokens.push(Token::Or),
            "NOT" => tokens.push(Token::Not),
            _ => tokens.push(Token::Atom(word)),
        }
    }
    tokens
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    now: f64,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, now: f64) -> Self {
        Parser {
            tokens,
            pos: 0,
            now,
        }
    }
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
    fn consume(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }
    pub fn parse(&mut self) -> Option<Expr> {
        self.parse_or()
    }
    fn parse_or(&mut self) -> Option<Expr> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.consume();
            if let Some(right) = self.parse_and() {
                left = Expr::Or(Box::new(left), Box::new(right));
            }
        }
        Some(left)
    }
    fn parse_and(&mut self) -> Option<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Token::And) => {
                    self.consume();
                    if let Some(r) = self.parse_unary() {
                        left = Expr::And(Box::new(left), Box::new(r));
                    }
                }
                Some(Token::Atom(_)) | Some(Token::Not) | Some(Token::LParen) => {
                    if let Some(r) = self.parse_unary() {
                        left = Expr::And(Box::new(left), Box::new(r));
                    }
                }
                _ => break,
            }
        }
        Some(left)
    }
    fn parse_unary(&mut self) -> Option<Expr> {
        if self.peek() == Some(&Token::Not) {
            self.consume();
            return self.parse_unary().map(|e| Expr::Not(Box::new(e)));
        }
        self.parse_primary()
    }
    fn parse_primary(&mut self) -> Option<Expr> {
        match self.peek()? {
            Token::LParen => {
                self.consume();
                let inner = self.parse_or()?;
                if self.peek() == Some(&Token::RParen) {
                    self.consume();
                }
                Some(inner)
            }
            Token::Atom(_) => {
                if let Some(Token::Atom(s)) = self.consume() {
                    Some(parse_atom(&s, self.now))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

fn try_build_cmp(field: &str, op: NumOp, val_str: &str, now: f64) -> Option<Expr> {
    let v = parse_numeric_value(val_str, now)?;
    Some(Expr::FieldCmp {
        field: field.to_string(),
        op,
        value: v,
    })
}

fn parse_atom(s: &str, now: f64) -> Expr {
    let s_trim = s.trim();

    // Nếu toàn bộ token được bọc trong dấu ngoặc kép -> tìm kiếm chuỗi tự do (Text search), không tách field:value
    let is_entirely_quoted = ((s_trim.starts_with('"') && s_trim.ends_with('"'))
        || (s_trim.starts_with('\'') && s_trim.ends_with('\'')))
        && s_trim.len() >= 2;

    if is_entirely_quoted {
        let unquoted = &s_trim[1..s_trim.len() - 1];
        let (negate, rest) = if let Some(stripped) = unquoted.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, unquoted)
        };
        let lower = rest.to_lowercase();
        let expr = Expr::Text(vec![lower]);
        return if negate {
            Expr::Not(Box::new(expr))
        } else {
            expr
        };
    }

    let (negate, rest) = if let Some(stripped) = s_trim.strip_prefix('-') {
        (true, stripped)
    } else {
        (false, s_trim)
    };
    let wrap = |e: Expr| -> Expr {
        if negate {
            Expr::Not(Box::new(e))
        } else {
            e
        }
    };
    let num_sym2: &[(&str, NumOp)] = &[(">=", NumOp::Gte), ("<=", NumOp::Lte)];
    let num_sym1: &[(&str, NumOp)] = &[(">", NumOp::Gt), ("<", NumOp::Lt)];

    if let Some(colon_idx) = rest.find(':') {
        let field = &rest[..colon_idx];
        let mut v_str = &rest[colon_idx + 1..];
        if !field.is_empty() {
            if v_str.is_empty()
                || v_str == "-"
                || v_str == "="
                || v_str == "~"
                || v_str == ">"
                || v_str == "<"
                || v_str == ">="
                || v_str == "<="
                || v_str == ".."
                || v_str == "-~"
                || v_str == "now.."
                || v_str.starts_with("..")
                || v_str.ends_with("..")
            {
                return Expr::Text(vec![]);
            }

            let is_neg_num = v_str.starts_with('-')
                && v_str.len() > 1
                && (v_str.as_bytes()[1].is_ascii_digit() || v_str.as_bytes()[1] == b'.');

            let mut inner_negate = false;
            if v_str.starts_with('-') && !is_neg_num {
                inner_negate = true;
                v_str = &v_str[1..];
            }
            let atom_wrap = |e: Expr| -> Expr {
                let total_negate = negate ^ inner_negate;
                if total_negate {
                    Expr::Not(Box::new(e))
                } else {
                    e
                }
            };
            // Check again if v_str became empty after stripping negate (e.g. "level:-")
            if v_str.is_empty() {
                return Expr::Text(vec![]);
            }
            if let Some(dots) = v_str.find("..") {
                let lo_str = &v_str[..dots];
                let hi_str = &v_str[dots + 2..];
                if let (Some(lo), Some(hi)) = (
                    parse_numeric_value(lo_str, now),
                    parse_numeric_value(hi_str, now),
                ) {
                    return atom_wrap(Expr::FieldRange {
                        field: field.into(),
                        lo,
                        hi,
                    });
                }
            }
            for (sym, op) in num_sym2 {
                if let Some(n) = v_str.strip_prefix(sym) {
                    if let Some(e) = try_build_cmp(field, op.clone(), n, now) {
                        return atom_wrap(e);
                    }
                }
            }
            for (sym, op) in num_sym1 {
                if let Some(n) = v_str.strip_prefix(sym) {
                    if let Some(e) = try_build_cmp(field, op.clone(), n, now) {
                        return atom_wrap(e);
                    }
                }
            }
            if let Some(pattern) = v_str.strip_prefix('~') {
                if !pattern.is_empty() {
                    if let Ok(re) = Regex::new(pattern) {
                        return atom_wrap(Expr::FieldRegex {
                            field: field.into(),
                            pattern: pattern.into(),
                            re,
                        });
                    }
                }
            }
            let values: Vec<String> = v_str
                .split('|')
                .map(|v| {
                    let mut trimmed = v.trim();
                    if ((trimmed.starts_with('"') && trimmed.ends_with('"'))
                        || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
                        && trimmed.len() >= 2
                    {
                        trimmed = &trimmed[1..trimmed.len() - 1];
                    }
                    trimmed.to_lowercase()
                })
                .filter(|v| !v.is_empty())
                .collect();
            if !values.is_empty() {
                return atom_wrap(Expr::FieldContainsAny {
                    field: field.into(),
                    values,
                });
            }
        }
    }
    if let Some(idx) = rest.find('=') {
        let field = &rest[..idx];
        let mut value = &rest[idx + 1..];
        if !field.is_empty() && !value.is_empty() {
            if ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
                && value.len() >= 2
            {
                value = &value[1..value.len() - 1];
            }
            return wrap(Expr::FieldExact {
                field: field.into(),
                value: value.into(),
            });
        }
    }
    for (sym, op) in num_sym2 {
        if let Some(idx) = rest.find(sym) {
            let field = &rest[..idx];
            let val = &rest[idx + sym.len()..];
            if !field.is_empty() && !val.is_empty() {
                if let Some(e) = try_build_cmp(field, op.clone(), val, now) {
                    return wrap(e);
                }
            }
        }
    }
    for (sym, op) in num_sym1 {
        if let Some(idx) = rest.find(sym) {
            let field = &rest[..idx];
            let val = &rest[idx + sym.len()..];
            if !field.is_empty() && !val.is_empty() {
                if let Some(e) = try_build_cmp(field, op.clone(), val, now) {
                    return wrap(e);
                }
            }
        }
    }
    let mut alts: Vec<String> = rest.split('|').map(|v| v.to_lowercase()).collect();
    if alts.len() > 1 {
        alts.retain(|v| !v.is_empty());
    }
    wrap(Expr::Text(alts))
}
