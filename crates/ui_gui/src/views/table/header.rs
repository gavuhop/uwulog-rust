use crate::state::{ColumnItem, ColumnState};
use crate::theme::ActiveTheme;
use eframe::egui::{self, CornerRadius, FontId, Pos2, Rect, Stroke};
use std::time::Instant;

/// Hiển thị thanh tiêu đề cột của bảng dữ liệu log.
/// Hỗ trợ kéo thả theo cơ chế dịch chuyển cột:
/// - Khi rê chuột vào cột khác: chỉ cột đang kéo và cột đích đổi màu, kèm vạch dọc tô đậm
///   ở mép chèn chính xác (mép phải khi kéo sang phải, mép trái khi kéo sang trái).
/// - Các cột nằm giữa giữ nguyên màu bình thường.
/// - Chỉ khi nào thả chuột ra (mouse released) thì mới thực hiện dịch chuyển các cột.
pub fn render_table_headers(
    header: &mut egui_extras::TableRow<'_, '_>,
    ctx: &egui::Context,
    visible_cols: &[ColumnItem],
    columns: &mut ColumnState,
) {
    let current_dragged = columns.header_dragged_name.clone();
    let pointer_pos = ctx.input(|i| i.pointer.latest_pos());
    let now = Instant::now();
    let dragged_idx = current_dragged
        .as_deref()
        .and_then(|d| visible_cols.iter().position(|c| c.name == d));
    let mut col_rects: Vec<Rect> = Vec::with_capacity(visible_cols.len());

    for (col_idx, col) in visible_cols.iter().enumerate() {
        header.col(|ui| {
            let is_dragged = current_dragged.as_deref() == Some(&col.name);

            let available_size = ui.available_size();
            let (rect, resp) =
                ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());
            columns.set_column_width(&col.name, rect.width());
            col_rects.push(rect);

            if resp.drag_started() {
                columns.header_dragged_name = Some(col.name.clone());
                columns.header_dragged_width = Some(rect.width());
                let off_x = pointer_pos
                    .map(|p| p.x - rect.min.x)
                    .unwrap_or(rect.width() / 2.0);
                columns.header_drag_offset_x = Some(off_x.clamp(0.0, rect.width()));
                ctx.request_repaint();
            }

            // Đổi cursor icon: Grab khi hover, Grabbing khi đang drag
            if is_dragged {
                columns.header_dragged_width = Some(rect.width());
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }

            let is_last = col_idx + 1 == visible_cols.len();
            let is_in_col = pointer_pos.is_some_and(|p| {
                p.x >= rect.min.x && (p.x < rect.max.x || (is_last && p.x <= rect.max.x))
            });
            let drop_edge = match dragged_idx {
                Some(d) if !is_dragged && is_in_col => Some(col_idx > d),
                _ => None,
            };

            render_header_cell(
                ui,
                columns,
                &col.name,
                rect,
                is_dragged,
                drop_edge,
                resp.hovered(),
                now,
            );
        });
    }

    let drop_target_idx = current_dragged
        .as_deref()
        .zip(pointer_pos)
        .and_then(|(dragged, pos)| find_drop_target_idx(dragged, visible_cols, &col_rects, pos));

    // Cập nhật trạng thái đang kéo
    if columns.header_dragged_name.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        // Repaint liên tục để phản hồi highlight và animation mượt mà
        ctx.request_repaint();
    }

    // Khi người dùng THẢ CHUỘT (pointer released):
    // Dịch chuyển cột đang kéo tới vị trí cột đích
    if ctx.input(|i| i.pointer.any_released()) {
        if let Some(dragged_name) = columns.header_dragged_name.take() {
            if let Some(target_col) = drop_target_idx.and_then(|idx| visible_cols.get(idx)) {
                columns.move_column(&dragged_name, &target_col.name);
                ctx.request_repaint();
            }
        }
        columns.clear_header_drag();
    }

    // Tự động dọn dẹp animation kết thúc và repaint mượt mà nếu còn chuyển động
    columns.table_animations.update(ctx, now);
}

/// Tìm chỉ số cột mục tiêu khi rê chuột vào bất kỳ phạm vi cột nào khác cột đang kéo.
pub fn find_drop_target_idx(
    dragged_name: &str,
    visible_cols: &[ColumnItem],
    col_rects: &[Rect],
    pointer: Pos2,
) -> Option<usize> {
    if col_rects.is_empty() || col_rects.len() != visible_cols.len() {
        return None;
    }
    let (first, last) = (col_rects.first()?, col_rects.last()?);
    if pointer.x < first.min.x || pointer.x > last.max.x {
        return None;
    }
    let target_idx = (0..col_rects.len() - 1)
        .position(|i| pointer.x < (col_rects[i].max.x + col_rects[i + 1].min.x) * 0.5)
        .unwrap_or(col_rects.len() - 1);
    (visible_cols[target_idx].name != dragged_name).then_some(target_idx)
}

/// Áp dụng position interpolation animation, background highlight,
/// grip icon, tên cột, separator line và vạch dọc mép chèn cho một ô header.
#[allow(clippy::too_many_arguments)]
fn render_header_cell(
    ui: &mut egui::Ui,
    columns: &mut ColumnState,
    col_name: &str,
    rect: Rect,
    is_dragged: bool,
    drop_edge: Option<bool>,
    is_hovered: bool,
    now: Instant,
) {
    let theme = ui.app_theme();
    // Tự động phát hiện thay đổi vị trí logic và lấy visual_rect nội suy chuyển động
    let visual_rect = columns.table_animations.track_rect(col_name, rect, now);
    let is_drop_target = drop_edge.is_some();

    let drop_t = ui.ctx().animate_bool_with_time(
        egui::Id::new(("header_col_drop", col_name)),
        is_drop_target,
        0.08,
    );

    // 1. Vẽ nền tiêu đề tại vị trí trực quan visual_rect
    let header_bg = if is_dragged {
        theme.surfaces.surface1
    } else if is_drop_target {
        theme.surfaces.surface0
    } else if is_hovered {
        theme.log.row_hover
    } else {
        theme.surfaces.mantle
    };
    ui.painter()
        .rect_filled(visual_rect, CornerRadius::ZERO, header_bg);

    // 2. Giữ màu nhấn trên cột đang kéo và tô đậm nhẹ cột đích
    if is_dragged {
        ui.painter().rect_filled(
            visual_rect,
            CornerRadius::ZERO,
            theme.text.accent.gamma_multiply(0.14),
        );
    } else if drop_t > 0.01 {
        ui.painter().rect_filled(
            visual_rect,
            CornerRadius::ZERO,
            theme.text.primary.gamma_multiply(0.05 * drop_t),
        );
    }

    // 3. Vẽ grip handle
    let grip_color = if is_dragged || is_drop_target || is_hovered {
        theme.text.accent
    } else {
        theme.text.muted
    };
    let grip_rect = Rect::from_center_size(
        Pos2::new(visual_rect.min.x + 8.0, visual_rect.center().y),
        egui::vec2(10.0, 10.0),
    );
    crate::components::ui::IconName::GripVertical.paint(ui.painter(), grip_rect, grip_color);

    // 4. Vẽ tên nhãn cột
    let font_id = FontId::monospace(crate::theme::TABLE_FONT_SIZE);
    let text_color = if is_dragged || is_drop_target {
        theme.text.primary
    } else {
        theme.text.muted
    };
    let text_pos = Pos2::new(visual_rect.min.x + 16.0, visual_rect.center().y);
    let full_text = col_name.to_uppercase();
    let max_w = visual_rect.max.x - text_pos.x - 4.0;
    let label = if ui
        .painter()
        .layout_no_wrap(full_text.clone(), font_id.clone(), text_color)
        .size()
        .x
        > max_w
    {
        let char_w = ui
            .painter()
            .layout_no_wrap("A".into(), font_id.clone(), text_color)
            .size()
            .x;
        let max_chars = if char_w > 0.0 {
            (max_w / char_w).floor() as usize
        } else {
            0
        };
        let count = full_text.chars().count();
        if count > max_chars && max_chars > 3 {
            let suffix: String = full_text.chars().skip(count - (max_chars - 3)).collect();
            let suffix = suffix.trim_start_matches(['.', '/']);
            format!("...{suffix}")
        } else {
            full_text
        }
    } else {
        full_text
    };
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        label,
        font_id,
        text_color,
    );

    // 5. Vẽ vạch phân cách cột
    let line_stroke = Stroke::new(1.0, theme.borders.border);
    ui.painter().line_segment(
        [
            Pos2::new(visual_rect.max.x - 1.0, visual_rect.min.y + 3.0),
            Pos2::new(visual_rect.max.x - 1.0, visual_rect.max.y - 3.0),
        ],
        line_stroke,
    );

    // 6. Vạch dọc tô đậm ở mép chèn chính xác của cột đích (phải khi kéo sang phải, trái khi kéo sang trái)
    let edge_id = egui::Id::new(("header_col_drop_edge", col_name));
    let on_right = match drop_edge {
        Some(right) => {
            ui.ctx().data_mut(|d| d.insert_temp(edge_id, right));
            right
        }
        None => ui
            .ctx()
            .data(|d| d.get_temp::<bool>(edge_id).unwrap_or(true)),
    };
    if drop_t > 0.01 {
        let edge_rect = if on_right {
            Rect::from_min_max(
                Pos2::new(visual_rect.max.x - 3.0, visual_rect.min.y),
                visual_rect.max,
            )
        } else {
            Rect::from_min_max(
                visual_rect.min,
                Pos2::new(visual_rect.min.x + 3.0, visual_rect.max.y),
            )
        };
        ui.painter().rect_filled(
            edge_rect,
            CornerRadius::ZERO,
            theme.text.primary.gamma_multiply(0.85 * drop_t),
        );
    }
}

/// Vẽ chip tiêu đề nổi bám theo con trỏ chuột (Floating Drag Ghost Chip)
/// Có độ rộng bằng chính xác với độ rộng của cột được kéo đi (dragged_width).
pub fn render_drag_ghost(
    ui: &egui::Ui,
    dragged_name: &str,
    pointer_pos: Pos2,
    width: f32,
    offset_x: Option<f32>,
) {
    let ghost_layer_id = egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("header_drag_ghost_layer"),
    );
    let painter = ui.ctx().layer_painter(ghost_layer_id);
    let theme = ui.app_theme();

    let ghost_h = 26.0;
    let ghost_w = width.max(30.0);
    let off_x = offset_x.unwrap_or(ghost_w / 2.0).clamp(0.0, ghost_w);
    let ghost_rect = Rect::from_min_size(
        Pos2::new(pointer_pos.x - off_x, pointer_pos.y - ghost_h / 2.0),
        egui::vec2(ghost_w, ghost_h),
    );

    painter.rect_filled(
        ghost_rect,
        CornerRadius::same(4),
        theme.surfaces.surface0.gamma_multiply(0.95),
    );
    let grip_rect = Rect::from_center_size(
        Pos2::new(ghost_rect.min.x + 8.0, ghost_rect.center().y),
        egui::vec2(10.0, 10.0),
    );
    crate::components::ui::IconName::GripVertical.paint(&painter, grip_rect, theme.text.accent);
    painter.text(
        Pos2::new(ghost_rect.min.x + 16.0, ghost_rect.center().y),
        egui::Align2::LEFT_CENTER,
        dragged_name.to_uppercase(),
        FontId::monospace(crate::theme::TABLE_FONT_SIZE),
        theme.text.primary,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_columns() -> (ColumnState, Vec<ColumnItem>, Vec<Rect>) {
        let mut columns = ColumnState::default();
        columns.columns.push(ColumnItem {
            name: "extra".into(),
            visible: true,
            width: 100.0,
        });
        let visible_cols = columns.columns.clone();
        // ["timestamp" (0..150), "level" (150..206), "message" (206..556), "extra" (556..656)]
        let mut x = 0.0;
        let col_rects = visible_cols
            .iter()
            .map(|c| {
                let r = Rect::from_min_size(Pos2::new(x, 0.0), egui::vec2(c.width, 26.0));
                x += c.width;
                r
            })
            .collect();
        (columns, visible_cols, col_rects)
    }

    #[test]
    fn test_find_drop_target_idx_enters_any_other_column() {
        let (_columns, visible_cols, col_rects) = setup_test_columns();

        // Trong chính cột đang kéo ("level": 150..206) -> None
        assert_eq!(
            find_drop_target_idx("level", &visible_cols, &col_rects, Pos2::new(178.0, 13.0)),
            None
        );

        // Vừa rê vào cột kế bên ("timestamp": 0..150 hoặc "message": 206..556) là nhận ngay
        assert_eq!(
            find_drop_target_idx("level", &visible_cols, &col_rects, Pos2::new(148.0, 13.0)),
            Some(0)
        );
        assert_eq!(
            find_drop_target_idx("level", &visible_cols, &col_rects, Pos2::new(210.0, 13.0)),
            Some(2)
        );

        // Nhận trên toàn bộ chiều dọc cột (kể cả Y ở dưới bảng)
        assert_eq!(
            find_drop_target_idx("level", &visible_cols, &col_rects, Pos2::new(620.0, 600.0)),
            Some(3)
        );

        // Ngoài phạm vi bảng -> None
        assert_eq!(
            find_drop_target_idx("level", &visible_cols, &col_rects, Pos2::new(700.0, 13.0)),
            None
        );
    }

    #[test]
    fn test_shift_only_executed_on_release_both_directions() {
        let (mut columns, visible_cols, col_rects) = setup_test_columns();
        columns.header_dragged_name = Some("timestamp".into());

        // Kéo "timestamp" (0) vào "extra" (3) -> trước khi nhả chuột thứ tự giữ nguyên
        let target = find_drop_target_idx(
            "timestamp",
            &visible_cols,
            &col_rects,
            Pos2::new(580.0, 13.0),
        );
        assert_eq!(target, Some(3));

        let names =
            |c: &ColumnState| -> Vec<String> { c.columns.iter().map(|i| i.name.clone()).collect() };
        assert_eq!(names(&columns), ["timestamp", "level", "message", "extra"]);

        // Nhả chuột -> các cột bị ảnh hưởng ("level", "message", "extra") dịch sang trái
        if let (Some(dragged), Some(idx)) = (columns.header_dragged_name.take(), target) {
            columns.move_column(&dragged, &visible_cols[idx].name);
        }
        assert_eq!(names(&columns), ["level", "message", "extra", "timestamp"]);

        // Kéo ngược sang trái vào cột kế bên ("extra" vào "message") -> đổi chỗ
        let vis_after = columns.columns.clone();
        let target_left =
            find_drop_target_idx("extra", &vis_after, &col_rects, Pos2::new(180.0, 13.0));
        assert_eq!(target_left, Some(1));
        if let Some(idx) = target_left {
            columns.move_column("extra", &vis_after[idx].name);
        }
        assert_eq!(names(&columns), ["level", "extra", "message", "timestamp"]);
    }
}
