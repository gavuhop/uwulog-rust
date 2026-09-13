use crate::components::ui::PopoverContainer;
use crate::session::GuiSession;
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Key, Pos2, Rect};

/// Render Autocomplete Dropdown Popup ngay dưới ô tìm kiếm
pub fn render_autocomplete_popup(ctx: &egui::Context, session: &mut GuiSession, input_rect: Rect) {
    if !session.search.autocomplete.is_open || session.search.autocomplete.suggestions.is_empty() {
        return;
    }

    // Xử lý phím điều hướng khi popup đang mở
    let mut item_to_apply = None;

    if ctx.input(|i| i.key_pressed(Key::ArrowDown))
        && !session.search.autocomplete.suggestions.is_empty()
    {
        session.search.autocomplete.selected_index = (session.search.autocomplete.selected_index
            + 1)
            % session.search.autocomplete.suggestions.len();
    }

    if ctx.input(|i| i.key_pressed(Key::ArrowUp))
        && !session.search.autocomplete.suggestions.is_empty()
    {
        if session.search.autocomplete.selected_index == 0 {
            session.search.autocomplete.selected_index =
                session.search.autocomplete.suggestions.len() - 1;
        } else {
            session.search.autocomplete.selected_index -= 1;
        }
    }

    if ctx.input(|i| i.key_pressed(Key::Tab) || i.key_pressed(Key::Enter)) {
        if let Some(item) = session
            .search
            .autocomplete
            .suggestions
            .get(session.search.autocomplete.selected_index)
        {
            item_to_apply = Some(item.clone());
        }
    }

    // Vẽ Floating Dropdown Panel (cách đáy filter box một khoảng thông thoáng để không bị đè viền)
    let dropdown_pos = Pos2::new(input_rect.min.x, input_rect.max.y + 8.0);
    let dropdown_width = input_rect.width();
    let approx_height = (session.search.autocomplete.suggestions.len() as f32 * 26.0) + 16.0;

    let resp = PopoverContainer::new("search_autocomplete_dropdown", input_rect)
        .width(dropdown_width)
        .custom_pos(dropdown_pos)
        .max_height(approx_height)
        .show(ctx, |ui| {
            let mut hovered_index = None;
            for (idx, item) in session.search.autocomplete.suggestions.iter().enumerate() {
                let is_selected = idx == session.search.autocomplete.selected_index;

                let desired_size = egui::vec2(ui.available_width(), 26.0);
                let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

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
                session.search.autocomplete.selected_index = hover_idx;
            }
        });

    if resp.closed {
        session.search.autocomplete.is_open = false;
    }

    if let Some(item) = item_to_apply {
        session.apply_autocomplete_suggestion(&item);
        ctx.request_repaint();
    }
}
