use crate::app::UwuGuiApp;
use crate::ui::columns_modal::ColumnItem;
use crate::ui::theme;
use eframe::egui::{self, Color32, FontId, Pos2, Rounding, Stroke};

pub fn render_table_headers(
    header: &mut egui_extras::TableRow<'_, '_>,
    visible_cols: &[ColumnItem],
    app: &mut UwuGuiApp,
    new_header_drag: &mut Option<String>,
    target_header_swap: &mut Option<(String, String)>,
) {
    let current_dragged = app.column_state.header_dragged_name.clone();

    for col in visible_cols {
        header.col(|ui| {
            let is_dragged = current_dragged.as_deref() == Some(&col.name);
            let is_drop_target = app.column_state.header_drop_target.as_deref() == Some(&col.name);

            let available_size = ui.available_size();
            let (rect, resp) =
                ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

            if resp.drag_started() {
                *new_header_drag = Some(col.name.clone());
            }

            if resp.hovered() && !is_dragged {
                if let Some(ref dragged_name) = app.column_state.header_dragged_name {
                    if dragged_name != &col.name {
                        app.column_state.header_drop_target = Some(col.name.clone());
                    }
                }
            }

            let header_bg = if is_dragged {
                theme::BG_SURFACE1
            } else if resp.hovered() {
                theme::BG_ROW_HOVER
            } else {
                theme::BG_MANTLE
            };
            ui.painter().rect_filled(rect, Rounding::ZERO, header_bg);

            if is_drop_target && current_dragged.is_some() && !is_dragged {
                ui.painter()
                    .rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0, theme::TEXT_KEY));
            }

            let font_id = FontId::monospace(11.0);
            let text_color = if is_dragged {
                theme::TEXT_KEY
            } else {
                theme::TEXT_MUTED
            };

            let icon_text = "⠿";
            let icon_font = FontId::monospace(11.0);
            let icon_pos = Pos2::new(rect.min.x + 4.0, rect.center().y);
            ui.painter().text(
                icon_pos,
                egui::Align2::LEFT_CENTER,
                icon_text,
                icon_font,
                theme::TEXT_MUTED,
            );

            let text_pos = Pos2::new(rect.min.x + 16.0, rect.center().y);
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_CENTER,
                col.name.to_uppercase(),
                font_id,
                text_color,
            );

            let line_stroke = Stroke::new(1.0, theme::BG_SURFACE0);
            ui.painter().line_segment(
                [
                    Pos2::new(rect.max.x - 1.0, rect.min.y + 3.0),
                    Pos2::new(rect.max.x - 1.0, rect.max.y - 3.0),
                ],
                line_stroke,
            );
        });
    }

    // Khi người dùng thả chuột (drag stopped / pointer released)
    if header.response().ctx.input(|i| i.pointer.any_released()) {
        if let (Some(from_name), Some(to_name)) = (
            app.column_state.header_dragged_name.take(),
            app.column_state.header_drop_target.take(),
        ) {
            if from_name != to_name {
                *target_header_swap = Some((from_name, to_name));
            }
        } else {
            app.column_state.header_dragged_name = None;
            app.column_state.header_drop_target = None;
        }
    }
}

pub fn render_drag_ghost(ui: &egui::Ui, dragged_name: &str, pointer_pos: Pos2) {
    let ghost_layer_id = egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("header_drag_ghost_layer"),
    );
    let painter = ui.ctx().layer_painter(ghost_layer_id);
    let ghost_size = egui::vec2((dragged_name.len() as f32 * 8.0 + 36.0).max(90.0), 26.0);
    let ghost_rect = egui::Rect::from_center_size(pointer_pos, ghost_size);

    painter.rect_filled(
        ghost_rect.expand(2.0),
        Rounding::same(5.0),
        Color32::from_black_alpha(100),
    );
    painter.rect_filled(ghost_rect, Rounding::same(4.0), theme::BG_MANTLE);
    painter.rect_stroke(
        ghost_rect,
        Rounding::same(4.0),
        Stroke::new(1.5, theme::TEXT_KEY),
    );
    painter.text(
        Pos2::new(ghost_rect.min.x + 8.0, ghost_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "⠿",
        FontId::monospace(12.0),
        theme::TEXT_KEY,
    );
    painter.text(
        Pos2::new(ghost_rect.min.x + 22.0, ghost_rect.center().y),
        egui::Align2::LEFT_CENTER,
        dragged_name,
        FontId::monospace(11.5),
        theme::TEXT_PRIMARY,
    );
}
