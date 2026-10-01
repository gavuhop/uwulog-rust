use regex::Regex;
use uwu_core_util::parse_numeric_value;

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
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        word.push(chars[i]);
                        word.push(chars[i + 1]);
                        i += 2;
                    } else if chars[i] == quote {
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

pub fn parse_query(query: &str, now: f64) -> Expr {
    let tokens = tokenize(query);
    let mut parser = Parser::new(tokens, now);
    parser.parse().unwrap_or(Expr::Text(vec![]))
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

fn is_valid_field_name(s: &str) -> bool {
    if s.is_empty() || s.contains('/') || s.contains('\\') || s.contains(':') {
        return false;
    }
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-' || c == '@')
}

fn unescape_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == '"' || next == '\'' || next == '\\' {
                    out.push(next);
                    chars.next();
                    continue;
                }
            }
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

#[inline]
fn strip_quotes(s: &str) -> Option<&str> {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        Some(&s[1..s.len() - 1])
    } else {
        None
    }
}

fn parse_atom(s: &str, now: f64) -> Expr {
    let s_trim = s.trim();

    // Nếu toàn bộ token được bọc trong dấu ngoặc kép -> tìm kiếm chuỗi tự do (Text search), bảo toàn nguyên vẹn dấu '-' bên trong
    if let Some(unquoted) = strip_quotes(s_trim) {
        let unescaped = unescape_value(unquoted);
        let lower = unescaped.to_lowercase();
        return Expr::Text(vec![lower]);
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

    // Nếu sau khi strip '-' là chuỗi ngoặc kép (ví dụ `-"error"`)
    if let Some(unquoted) = strip_quotes(rest) {
        let unescaped = unescape_value(unquoted);
        let lower = unescaped.to_lowercase();
        return wrap(Expr::Text(vec![lower]));
    }

    let num_sym2: &[(&str, NumOp)] = &[(">=", NumOp::Gte), ("<=", NumOp::Lte)];
    let num_sym1: &[(&str, NumOp)] = &[(">", NumOp::Gt), ("<", NumOp::Lt)];

    if let Some(colon_idx) = rest.find(':') {
        let field = &rest[..colon_idx];
        let mut v_str = &rest[colon_idx + 1..];
        // Phân biệt URL (http://, https://), IPv6 (fe80::1) hoặc field không hợp lệ
        if !field.is_empty()
            && is_valid_field_name(field)
            && !v_str.starts_with("//")
            && !v_str.starts_with(':')
        {
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

            let mut inner_negate = false;
            if v_str.starts_with('-') && !v_str.starts_with("now-") {
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
            if let Some(raw_pattern) = v_str.strip_prefix('~') {
                let pattern = strip_quotes(raw_pattern).unwrap_or(raw_pattern);
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
            let values: Vec<String> = if let Some(unquoted) = strip_quotes(v_str) {
                let unescaped = unescape_value(unquoted);
                let lower = unescaped.to_lowercase();
                if lower.is_empty() {
                    vec![]
                } else {
                    vec![lower]
                }
            } else {
                v_str
                    .split('|')
                    .map(|v| {
                        let trimmed = v.trim();
                        let unquoted = strip_quotes(trimmed).unwrap_or(trimmed);
                        let unescaped = unescape_value(unquoted);
                        unescaped.to_lowercase()
                    })
                    .filter(|v| !v.is_empty())
                    .collect()
            };
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
        let value = &rest[idx + 1..];
        if !field.is_empty() && is_valid_field_name(field) && !value.is_empty() {
            let unquoted = strip_quotes(value).unwrap_or(value);
            let unescaped = unescape_value(unquoted);
            return wrap(Expr::FieldExact {
                field: field.into(),
                value: unescaped,
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
    let mut alts: Vec<String> = rest
        .split('|')
        .map(|v| unescape_value(v.trim()).to_lowercase())
        .collect();
    if alts.len() > 1 {
        alts.retain(|v| !v.is_empty());
    }
    wrap(Expr::Text(alts))
}
