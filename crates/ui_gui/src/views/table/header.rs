use crate::state::{ColumnItem, ColumnState};
use crate::theme::{self, ActiveTheme};
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Stroke};

pub fn render_table_headers(
    header: &mut egui_extras::TableRow<'_, '_>,
    visible_cols: &[ColumnItem],
    columns: &mut ColumnState,
    new_header_drag: &mut Option<String>,
    target_header_swap: &mut Option<(String, String)>,
) {
    let current_dragged = columns.header_dragged_name.clone();

    for col in visible_cols {
        header.col(|ui| {
            let theme = ui.app_theme();
            let is_dragged = current_dragged.as_deref() == Some(&col.name);
            let is_drop_target = columns.header_drop_target.as_deref() == Some(&col.name);

            let available_size = ui.available_size();
            let (rect, resp) =
                ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

            if resp.drag_started() {
                *new_header_drag = Some(col.name.clone());
            }

            if resp.hovered() && !is_dragged {
                if let Some(ref dragged_name) = columns.header_dragged_name {
                    if dragged_name != &col.name {
                        columns.header_drop_target = Some(col.name.clone());
                    }
                }
            }

            let header_bg = if is_dragged {
                theme.surfaces.surface1
            } else if resp.hovered() {
                theme.log.row_hover
            } else {
                theme.surfaces.mantle
            };
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, header_bg);

            if is_drop_target && current_dragged.is_some() && !is_dragged {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::ZERO,
                    Stroke::new(2.0, theme.text.accent),
                    egui::StrokeKind::Inside,
                );
            }

            let font_id = FontId::monospace(11.0);
            let text_color = if is_dragged {
                theme.text.accent
            } else {
                theme.text.muted
            };

            let grip_rect = Rect::from_center_size(
                Pos2::new(rect.min.x + 8.0, rect.center().y),
                egui::vec2(10.0, 10.0),
            );
            crate::components::ui::IconName::GripVertical.paint(
                ui.painter(),
                grip_rect,
                theme.text.muted,
            );

            let text_pos = Pos2::new(rect.min.x + 16.0, rect.center().y);
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_CENTER,
                col.name.to_uppercase(),
                font_id,
                text_color,
            );

            let line_stroke = Stroke::new(1.0, theme.borders.border);
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
            columns.header_dragged_name.take(),
            columns.header_drop_target.take(),
        ) {
            if from_name != to_name {
                *target_header_swap = Some((from_name, to_name));
            }
        } else {
            columns.header_dragged_name = None;
            columns.header_drop_target = None;
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

    let theme = ui.app_theme();
    painter.rect_filled(
        ghost_rect.expand(2.0),
        CornerRadius::same(5),
        Color32::from_black_alpha(100),
    );
    painter.rect_filled(ghost_rect, CornerRadius::same(4), theme.surfaces.mantle);
    painter.rect_stroke(
        ghost_rect,
        CornerRadius::same(4),
        Stroke::new(1.5, theme.text.accent),
        egui::StrokeKind::Inside,
    );
    theme::draw_drag_handle(
        &painter,
        Pos2::new(ghost_rect.min.x + 12.0, ghost_rect.center().y),
        theme.text.accent,
    );
    painter.text(
        Pos2::new(ghost_rect.min.x + 22.0, ghost_rect.center().y),
        egui::Align2::LEFT_CENTER,
        dragged_name,
        FontId::monospace(11.5),
        theme.text.primary,
    );
}
