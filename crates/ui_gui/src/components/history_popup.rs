use crate::app::UwuGuiApp;
use crate::theme;
use eframe::egui::{self, Color32, FontId, Id, Key, Order, Pos2, Rect, Rounding, Stroke};

pub fn render_history_popup(ctx: &egui::Context, app: &mut UwuGuiApp, input_rect: Rect) {
    if !app.search.history.is_open {
        return;
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.search.history.is_open = false;
        return;
    }

    let popup_pos = Pos2::new(input_rect.min.x, input_rect.max.y + 8.0);
    let popup_width = input_rect.width().max(420.0);
    let entry_count = app.search.history.entries.len().max(1);
    let approx_height = (entry_count as f32 * 28.0) + 48.0;
    let popup_rect = Rect::from_min_size(popup_pos, egui::vec2(popup_width, approx_height));

    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !input_rect.contains(pos) && !popup_rect.contains(pos) {
                app.search.history.is_open = false;
                return;
            }
        }
    }

    let mut selected_history_item = None;
    let mut clear_all_clicked = false;

    egui::Area::new(Id::new("search_history_dropdown_area"))
        .order(Order::Foreground)
        .fixed_pos(popup_pos)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::same(6.0))
                .show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header bar của History Popup
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("⏱ Search History")
                                .font(FontId::monospace(12.0))
                                .color(theme::TEXT_KEY)
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !app.search.history.entries.is_empty()
                                && ui
                                    .button(
                                        egui::RichText::new("Clear")
                                            .font(FontId::monospace(11.0))
                                            .color(theme::COLOR_WARN),
                                    )
                                    .on_hover_text("Clear all search history")
                                    .clicked()
                            {
                                clear_all_clicked = true;
                            }
                        });
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if app.search.history.entries.is_empty() {
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
                        for hist_query in &app.search.history.entries {
                            let desired_size = egui::vec2(ui.available_width(), 26.0);
                            let (rect, resp) =
                                ui.allocate_exact_size(desired_size, egui::Sense::click());

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

                            ui.painter().rect_filled(rect, Rounding::same(4.0), row_bg);

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
        });

    if clear_all_clicked {
        app.search.history.entries.clear();
        ctx.request_repaint();
    }

    if let Some(chosen_query) = selected_history_item {
        app.search.query = chosen_query.clone();
        app.search.history.apply_history_item(&chosen_query);
        app.search.autocomplete.is_open = false;
        app.search.autocomplete.suggestions.clear();
        app.search.autocomplete.just_applied = true;
        app.trigger_full_search();
        ctx.request_repaint();
    }
}
