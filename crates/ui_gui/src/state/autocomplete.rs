pub use uwu_core_schema::FieldType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionKind {
    Key,             // Phase 1: Chọn Key -> Tiếp tục mở Phase 2
    OperatorOrValue, // Phase 2: Chọn Operator/Value -> Đóng menu để gõ dữ liệu
}

#[derive(Clone, Debug)]
pub struct SuggestionItem {
    pub kind: SuggestionKind,
    pub op_symbol: &'static str,
    pub action_name: String,
    pub example_syntax: String,
    pub insert_text: String,
}

#[derive(Default)]
pub struct AutocompleteState {
    pub is_open: bool,
    pub selected_index: usize,
    pub suggestions: Vec<SuggestionItem>,
    pub active_token_range: (usize, usize),
    pub just_applied: bool,
}

/// Xác định kiểu dữ liệu của một key (Time, Number, Text) qua SSOT
pub fn classify_field(key: &str) -> FieldType {
    uwu_core_schema::StandardField::from_alias(key)
        .map(|f| f.field_type())
        .unwrap_or(FieldType::Text)
}

fn key_suggestions(available_fields: &[(String, FieldType)]) -> Vec<SuggestionItem> {
    available_fields
        .iter()
        .map(|(k, _)| SuggestionItem {
            kind: SuggestionKind::Key,
            op_symbol: "",
            action_name: k.clone(),
            example_syntax: format!("{k}:"),
            insert_text: format!("{k}:"),
        })
        .collect()
}

/// Phân tích query và sinh danh sách gợi ý CHỈ DỰA TRÊN các trường thực sự có trong log
pub fn generate_suggestions(
    query: &str,
    available_fields: &[(String, FieldType)],
) -> (Vec<SuggestionItem>, (usize, usize)) {
    if query.trim().is_empty() {
        // Query rỗng: Phase 1 - gợi ý các key thực sự xuất hiện trong log
        return (key_suggestions(available_fields), (0, 0));
    }

    // Tìm token cuối cùng đang được gõ (tính từ khoảng trắng cuối cùng)
    let last_space_idx = query.rfind(' ').map(|idx| idx + 1).unwrap_or(0);
    let active_token = &query[last_space_idx..];
    let token_range = (last_space_idx, query.len());

    if active_token.is_empty() {
        // Sau khoảng trắng: Phase 1 - gợi ý danh sách các key thực sự có trong log
        return (key_suggestions(available_fields), token_range);
    }

    // 1. Phase 2: Trường hợp token đã có dấu ':' hoặc '=' (ví dụ `msg:`, `level:`, `latency:`, `timestamp:`)
    if let Some(colon_pos) = active_token.find(':').or_else(|| active_token.find('=')) {
        let key = &active_token[..colon_pos];
        let after_colon = &active_token[colon_pos + 1..];

        // Nếu người dùng đã bắt đầu gõ giá trị tìm kiếm (ví dụ `msg:auth`, `level:error`, `msg:~^sys`),
        // KHÔNG hiển thị menu gợi ý nữa để người dùng gõ tự nhiên không bị che khuất.
        if !after_colon.is_empty() {
            return (Vec::new(), token_range);
        }

        if !key.is_empty() {
            let field_type = available_fields
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, f)| *f)
                .unwrap_or_else(|| classify_field(key));

            let suggestions = match field_type {
                FieldType::Text => vec![
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "~",
                        action_name: "Regex Match".to_string(),
                        example_syntax: format!("{key}:~^sys"),
                        insert_text: format!("{key}:~"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "-",
                        action_name: "Negate".to_string(),
                        example_syntax: format!("{key}:-debug"),
                        insert_text: format!("{key}:-"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "-~",
                        action_name: "Negated Regex".to_string(),
                        example_syntax: format!("{key}:-~^sys"),
                        insert_text: format!("{key}:-~"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "=",
                        action_name: "Exact Match".to_string(),
                        example_syntax: format!("{key}=ERROR"),
                        insert_text: format!("{key}="),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: " ",
                        action_name: "Contains".to_string(),
                        example_syntax: format!("{key}:audio"),
                        insert_text: format!("{key}:"),
                    },
                ],
                FieldType::Number => vec![
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "<=",
                        action_name: "Less than or equal".to_string(),
                        example_syntax: format!("{key}:<=500"),
                        insert_text: format!("{key}:<="),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: ">=",
                        action_name: "Greater than or equal".to_string(),
                        example_syntax: format!("{key}:>=1000"),
                        insert_text: format!("{key}:>="),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "<",
                        action_name: "Less than".to_string(),
                        example_syntax: format!("{key}:<200"),
                        insert_text: format!("{key}:<"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: ">",
                        action_name: "Greater than".to_string(),
                        example_syntax: format!("{key}:>100"),
                        insert_text: format!("{key}:>"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "..",
                        action_name: "Number Range".to_string(),
                        example_syntax: format!("{key}:200..500"),
                        insert_text: format!("{key}:"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "=",
                        action_name: "Exact Match".to_string(),
                        example_syntax: format!("{key}=200"),
                        insert_text: format!("{key}="),
                    },
                ],
                FieldType::Time => vec![
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "now..",
                        action_name: "Last 10 minutes".to_string(),
                        example_syntax: format!("{key}:now..10m"),
                        insert_text: format!("{key}:now..10m"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "now..",
                        action_name: "Last 1 hour".to_string(),
                        example_syntax: format!("{key}:now..1h"),
                        insert_text: format!("{key}:now..1h"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "now..",
                        action_name: "Last 24 hours".to_string(),
                        example_syntax: format!("{key}:now..24h"),
                        insert_text: format!("{key}:now..24h"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: ">",
                        action_name: "Newer than".to_string(),
                        example_syntax: format!("{key}:>15m"),
                        insert_text: format!("{key}:>15m"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "<",
                        action_name: "Older than".to_string(),
                        example_syntax: format!("{key}:<1h"),
                        insert_text: format!("{key}:<1h"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "..",
                        action_name: "Specific Window".to_string(),
                        example_syntax: format!("{key}:1h..30m"),
                        insert_text: format!("{key}:1h..30m"),
                    },
                ],
            };
            return (suggestions, token_range);
        }
    }

    // 2. Phase 1: Gợi ý tên key khi gõ prefix
    let mut suggestions = Vec::new();
    let lower_token = active_token.to_lowercase();

    for (k, _) in available_fields {
        if k.to_lowercase().starts_with(&lower_token) {
            suggestions.push(SuggestionItem {
                kind: SuggestionKind::Key,
                op_symbol: "",
                action_name: k.clone(),
                example_syntax: format!("{k}:"),
                insert_text: format!("{k}:"),
            });
        }
    }

    (suggestions, token_range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_field() {
        assert_eq!(classify_field("timestamp"), FieldType::Time);
        assert_eq!(classify_field("time"), FieldType::Time);
        assert_eq!(classify_field("msg"), FieldType::Text);
        assert_eq!(classify_field("level"), FieldType::Text);
        assert_eq!(classify_field("tag"), FieldType::Text);
        assert_eq!(classify_field("latency"), FieldType::Text);
    }

    #[test]
    fn test_text_field_suggestions_with_active_key() {
        let (suggestions, _) = generate_suggestions("msg:", &[]);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].kind, SuggestionKind::OperatorOrValue);

        let examples: Vec<&str> = suggestions
            .iter()
            .map(|s| s.example_syntax.as_str())
            .collect();
        assert!(examples.contains(&"msg:~^sys"));
        assert!(examples.contains(&"msg:-debug"));
        assert!(examples.contains(&"msg:-~^sys"));
        assert!(examples.contains(&"msg=ERROR"));
        assert!(examples.contains(&"msg:audio"));
    }

    #[test]
    fn test_number_field_suggestions_with_active_key() {
        let available = vec![("latency".to_string(), FieldType::Number)];
        let (suggestions, _) = generate_suggestions("latency:", &available);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].kind, SuggestionKind::OperatorOrValue);

        let examples: Vec<&str> = suggestions
            .iter()
            .map(|s| s.example_syntax.as_str())
            .collect();
        assert!(examples.contains(&"latency:<=500"));
        assert!(examples.contains(&"latency:>=1000"));
        assert!(examples.contains(&"latency:<200"));
        assert!(examples.contains(&"latency:>100"));
        assert!(examples.contains(&"latency:200..500"));
    }

    #[test]
    fn test_time_field_suggestions_with_active_key() {
        let (suggestions, _) = generate_suggestions("timestamp:", &[]);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].kind, SuggestionKind::OperatorOrValue);

        let examples: Vec<&str> = suggestions
            .iter()
            .map(|s| s.example_syntax.as_str())
            .collect();
        assert!(examples.contains(&"timestamp:now..10m"));
        assert!(examples.contains(&"timestamp:now..1h"));
        assert!(examples.contains(&"timestamp:now..24h"));
        assert!(examples.contains(&"timestamp:>15m"));
    }

    #[test]
    fn test_prefix_key_suggestions_only_available_fields() {
        let available = vec![
            ("level".to_string(), FieldType::Text),
            ("timestamp".to_string(), FieldType::Time),
            ("message".to_string(), FieldType::Text),
            ("source".to_string(), FieldType::Text),
            ("userId".to_string(), FieldType::Text),
        ];

        let (suggestions, _) = generate_suggestions("user", &available);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].kind, SuggestionKind::Key);
        assert_eq!(suggestions[0].insert_text, "userId:");

        // Không gợi ý latency vì không có trong available
        let (suggestions, _) = generate_suggestions("lat", &available);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_no_suggestions_when_typing_value() {
        let (suggestions, _) = generate_suggestions("msg:auth", &[]);
        assert!(suggestions.is_empty());

        let (suggestions, _) = generate_suggestions("level:error", &[]);
        assert!(suggestions.is_empty());

        let (suggestions, _) = generate_suggestions("timestamp:>15m", &[]);
        assert!(suggestions.is_empty());
    }
}
