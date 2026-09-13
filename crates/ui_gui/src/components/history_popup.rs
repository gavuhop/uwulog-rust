use crate::components::ui::{AppButton, ButtonVariant, PopoverContainer};
use crate::session::GuiSession;
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect};

pub fn render_history_popup(ctx: &egui::Context, session: &mut GuiSession, input_rect: Rect) {
    if !session.search.history.is_open {
        return;
    }

    let popup_pos = Pos2::new(input_rect.min.x, input_rect.max.y + 8.0);
    let popup_width = input_rect.width();
    let entry_count = session.search.history.entries.len().max(1);
    let approx_height = (entry_count as f32 * 28.0) + 48.0;

    let mut selected_history_item = None;
    let mut clear_all_clicked = false;

    let resp = PopoverContainer::new("search_history_dropdown", input_rect)
        .width(popup_width)
        .custom_pos(popup_pos)
        .max_height(approx_height)
        .show(ctx, |ui| {
            // Header bar của History Popup
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⏱ Search History")
                        .font(FontId::monospace(12.0))
                        .color(theme::TEXT_KEY)
                        .strong(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !session.search.history.entries.is_empty() {
                        let clear_btn = AppButton::new()
                            .label("Clear")
                            .small()
                            .variant(ButtonVariant::Danger)
                            .tooltip("Clear all search history");
                        if clear_btn.show(ui).clicked() {
                            clear_all_clicked = true;
                        }
                    }
                });
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            if session.search.history.entries.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("No search history yet.")
                            .font(FontId::monospace(11.5))
                            .color(theme::TEXT_MUTED)
                            .italics(),
                    );
                });
                ui.add_space(2.0);
            } else {
                for hist_query in &session.search.history.entries {
                    let desired_size = egui::vec2(ui.available_width(), 26.0);
                    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if resp.clicked() {
                        selected_history_item = Some(hist_query.clone());
                    }

                    let row_bg = if resp.hovered() {
                        theme::BG_ROW_SELECTED
                    } else {
                        Color32::TRANSPARENT
                    };

                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(4), row_bg);

                    let center_y = rect.center().y;
                    let left_x = rect.min.x + 8.0;

                    // Icon tìm kiếm nhỏ
                    ui.painter().text(
                        Pos2::new(left_x, center_y),
                        egui::Align2::LEFT_CENTER,
                        "🔍",
                        FontId::monospace(10.5),
                        theme::TEXT_MUTED,
                    );

                    // Nội dung câu query
                    ui.painter().text(
                        Pos2::new(left_x + 20.0, center_y),
                        egui::Align2::LEFT_CENTER,
                        hist_query,
                        FontId::monospace(12.0),
                        theme::TEXT_PRIMARY,
                    );
                }
            }
        });

    if resp.closed {
        session.search.history.is_open = false;
    }

    if clear_all_clicked {
        session.search.history.entries.clear();
        ctx.request_repaint();
    }

    if let Some(chosen_query) = selected_history_item {
        session.search.query = chosen_query.clone();
        session.search.history.apply_history_item(&chosen_query);
        session.search.autocomplete.is_open = false;
        session.search.autocomplete.suggestions.clear();
        session.search.autocomplete.just_applied = true;
        session.trigger_full_search();
        ctx.request_repaint();
    }
}
