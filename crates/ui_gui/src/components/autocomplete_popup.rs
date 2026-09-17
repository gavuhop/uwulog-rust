use crate::components::ui::PopoverContainer;
use crate::session::GuiSession;
use crate::state::SuggestionKind;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect};

/// Render Autocomplete Dropdown Popup ngay dưới ô tìm kiếm
pub fn render_autocomplete_popup(ctx: &egui::Context, session: &mut GuiSession, input_rect: Rect) {
    if !session.search.autocomplete.is_open || session.search.autocomplete.suggestions.is_empty() {
        return;
    }

    let mut item_to_apply = None;
    let prev_selected_idx: Option<usize> =
        ctx.data(|d| d.get_temp(egui::Id::new("ac_last_selected")));
    let selection_changed = prev_selected_idx != Some(session.search.autocomplete.selected_index);
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new("ac_last_selected"),
            session.search.autocomplete.selected_index,
        )
    });

    let is_syntax = session
        .search
        .autocomplete
        .suggestions
        .first()
        .is_some_and(|s| s.kind == SuggestionKind::OperatorOrValue);

    // Vẽ Floating Dropdown Panel (cách đáy filter box một khoảng thông thoáng để không bị đè viền)
    let dropdown_pos = Pos2::new(input_rect.min.x, input_rect.max.y + 8.0);
    let dropdown_width = input_rect.width();
    let count = session.search.autocomplete.suggestions.len();
    let approx_height = if is_syntax {
        (count as f32 * 28.0) + 16.0
    } else {
        ((count as f32 * 28.0) + 16.0).min(280.0)
    };

    let resp = PopoverContainer::new("search_autocomplete_dropdown", input_rect)
        .width(dropdown_width)
        .custom_pos(dropdown_pos)
        .max_height(approx_height)
        .show(ctx, |ui| {
            let theme = ui.app_theme();
            ui.spacing_mut().item_spacing.y = 2.0;
            let mut hovered_index = None;

            let mut render_list = |ui: &mut egui::Ui| {
                for (idx, item) in session.search.autocomplete.suggestions.iter().enumerate() {
                    let is_selected = idx == session.search.autocomplete.selected_index;

                    let desired_size = egui::vec2(ui.available_width(), 26.0);
                    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());
                    if selection_changed && is_selected {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }

                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        hovered_index = Some(idx);
                    }

                    if resp.clicked() {
                        item_to_apply = Some(item.clone());
                    }

                    let row_bg = if is_selected || resp.hovered() {
                        theme.log.row_selected
                    } else {
                        Color32::TRANSPARENT
                    };

                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(4), row_bg);

                    // Vẽ trực tiếp bằng Painter: hoàn toàn như 1 Button thuần túy, không có widget con cướp click hay bôi đen chữ
                    let center_y = rect.center().y;
                    let mut left_x = rect.min.x + 8.0;

                    // Cột trái 1: Ký hiệu toán tử (~, -, <=, ...)
                    if !item.op_symbol.is_empty() {
                        ui.painter().text(
                            Pos2::new(left_x, center_y),
                            egui::Align2::LEFT_CENTER,
                            item.op_symbol,
                            FontId::monospace(11.0),
                            theme.text.accent,
                        );
                        left_x += 24.0;
                    }

                    // Cột trái 2: Tên hành động / Tên key
                    ui.painter().text(
                        Pos2::new(left_x, center_y),
                        egui::Align2::LEFT_CENTER,
                        &item.action_name,
                        FontId::monospace(12.0),
                        theme.text.primary,
                    );

                    // Cột phải: Cú pháp ví dụ in nghiêng
                    let right_x = rect.max.x - 8.0;
                    ui.painter().text(
                        Pos2::new(right_x, center_y),
                        egui::Align2::RIGHT_CENTER,
                        &item.example_syntax,
                        FontId::monospace(11.0),
                        theme.text.muted,
                    );
                }
            };

            if is_syntax {
                render_list(ui);
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    render_list(ui);
                });
            }

            if let Some(hover_idx) = hovered_index {
                session.search.autocomplete.selected_index = hover_idx;
            }
        });

    if resp.closed {
        session.search.autocomplete.is_open = false;
    }

    let popup_rect = Rect::from_min_size(dropdown_pos, egui::vec2(dropdown_width, approx_height));
    if ctx.input(|i| {
        i.pointer.hover_pos().is_some_and(|pos| {
            popup_rect.expand(4.0).contains(pos) || input_rect.expand(2.0).contains(pos)
        })
    }) {
        ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }

    if let Some(item) = item_to_apply {
        session.apply_autocomplete_suggestion(&item);
        ctx.request_repaint();
    }
}
