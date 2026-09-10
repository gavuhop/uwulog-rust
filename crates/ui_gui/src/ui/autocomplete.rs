use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Color32, FontId, Id, Key, Order, Pos2, Rect, Rounding, Stroke};

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
                        action_name: "Numeric Range".to_string(),
                        example_syntax: format!("{key}:200..500"),
                        insert_text: format!("{key}:"),
                    },
                    SuggestionItem {
                        kind: SuggestionKind::OperatorOrValue,
                        op_symbol: "=",
                        action_name: "Exact Value".to_string(),
                        example_syntax: format!("{key}:500"),
                        insert_text: format!("{key}:"),
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

/// Render Autocomplete Dropdown Popup ngay dưới ô tìm kiếm
pub fn render_autocomplete_popup(ctx: &egui::Context, app: &mut UwuGuiApp, input_rect: Rect) {
    if !app.autocomplete_state.is_open || app.autocomplete_state.suggestions.is_empty() {
        return;
    }

    // Xử lý phím điều hướng khi popup đang mở
    let mut item_to_apply = None;

    if ctx.input(|i| i.key_pressed(Key::ArrowDown))
        && !app.autocomplete_state.suggestions.is_empty()
    {
        app.autocomplete_state.selected_index =
            (app.autocomplete_state.selected_index + 1) % app.autocomplete_state.suggestions.len();
    }

    if ctx.input(|i| i.key_pressed(Key::ArrowUp)) && !app.autocomplete_state.suggestions.is_empty()
    {
        if app.autocomplete_state.selected_index == 0 {
            app.autocomplete_state.selected_index = app.autocomplete_state.suggestions.len() - 1;
        } else {
            app.autocomplete_state.selected_index -= 1;
        }
    }

    if ctx.input(|i| i.key_pressed(Key::Tab) || i.key_pressed(Key::Enter)) {
        if let Some(item) = app
            .autocomplete_state
            .suggestions
            .get(app.autocomplete_state.selected_index)
        {
            item_to_apply = Some(item.clone());
        }
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.autocomplete_state.is_open = false;
        return;
    }

    // Vẽ Floating Dropdown Panel (cách đáy filter box một khoảng thông thoáng để không bị đè viền)
    let dropdown_pos = Pos2::new(input_rect.min.x, input_rect.max.y + 8.0);
    let dropdown_width = input_rect.width().max(420.0);
    let approx_height = (app.autocomplete_state.suggestions.len() as f32 * 26.0) + 16.0;
    let popup_rect = Rect::from_min_size(dropdown_pos, egui::vec2(dropdown_width, approx_height));

    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !input_rect.contains(pos) && !popup_rect.contains(pos) {
                app.autocomplete_state.is_open = false;
                return;
            }
        }
    }

    egui::Area::new(Id::new("search_autocomplete_dropdown_area"))
        .order(Order::Foreground)
        .fixed_pos(dropdown_pos)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::same(4.0))
                .show(ui, |ui| {
                    ui.set_width(dropdown_width);

                    let mut hovered_index = None;
                    for (idx, item) in app.autocomplete_state.suggestions.iter().enumerate() {
                        let is_selected = idx == app.autocomplete_state.selected_index;

                        let desired_size = egui::vec2(ui.available_width(), 26.0);
                        let (rect, resp) =
                            ui.allocate_exact_size(desired_size, egui::Sense::click());

                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            hovered_index = Some(idx);
                        }

                        if resp.clicked() {
                            item_to_apply = Some(item.clone());
                        }

                        let row_bg = if is_selected || resp.hovered() {
                            theme::BG_ROW_SELECTED
                        } else {
                            Color32::TRANSPARENT
                        };

                        ui.painter().rect_filled(rect, Rounding::same(4.0), row_bg);

                        // Vẽ trực tiếp bằng Painter: hoàn toàn như 1 Button thuần túy, không có widget con cướp click hay bôi đen chữ
                        let center_y = rect.center().y;
                        let mut left_x = rect.min.x + 8.0;

                        // Cột trái 1: Ký hiệu toán tử (~, -, <=, ...)
                        if !item.op_symbol.is_empty() {
                            ui.painter().text(
                                Pos2::new(left_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                item.op_symbol,
                                FontId::monospace(11.5),
                                theme::TEXT_KEY,
                            );
                            left_x += 24.0;
                        }

                        // Cột trái 2: Tên hành động / Tên key
                        ui.painter().text(
                            Pos2::new(left_x, center_y),
                            egui::Align2::LEFT_CENTER,
                            &item.action_name,
                            FontId::monospace(12.0),
                            theme::TEXT_PRIMARY,
                        );

                        // Cột phải: Cú pháp ví dụ in nghiêng
                        let right_x = rect.max.x - 8.0;
                        ui.painter().text(
                            Pos2::new(right_x, center_y),
                            egui::Align2::RIGHT_CENTER,
                            &item.example_syntax,
                            FontId::monospace(11.5),
                            theme::TEXT_MUTED,
                        );
                    }
                    if let Some(hover_idx) = hovered_index {
                        app.autocomplete_state.selected_index = hover_idx;
                    }
                });
        });

    if let Some(item) = item_to_apply {
        app.apply_autocomplete_suggestion(&item);
        ctx.request_repaint();
    }
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
