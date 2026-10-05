use crate::state::{ColumnItem, ColumnState};
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Stroke};
use std::time::Instant;

/// Hiển thị thanh tiêu đề cột của bảng dữ liệu log.
/// Hỗ trợ kéo thả theo cơ chế trực quan:
/// - Khi rê chuột qua cột khác: ô bị chuột đè lên sẽ đổi màu highlight báo hiệu chuẩn bị swap.
/// - Chỉ khi nào thả chuột ra (mouse released) thì mới thực hiện hoán đổi (swap) vị trí cột.
/// - Sau khi swap, tự động nội suy chuyển động mượt mà (position interpolation animation).
pub fn render_table_headers(
    header: &mut egui_extras::TableRow<'_, '_>,
    ctx: &egui::Context,
    visible_cols: &[ColumnItem],
    columns: &mut ColumnState,
) {
    let current_dragged = columns.header_dragged_name.clone();
    let pointer_pos = ctx.input(|i| i.pointer.latest_pos());
    let now = Instant::now();
    let mut detected_drop_target: Option<String> = None;

    for (col_idx, col) in visible_cols.iter().enumerate() {
        header.col(|ui| {
            let is_dragged = current_dragged.as_deref() == Some(&col.name);

            let available_size = ui.available_size();
            let (rect, resp) =
                ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

            if resp.drag_started() {
                columns.header_dragged_name = Some(col.name.clone());
                columns.header_dragged_width = Some(rect.width());
                let off_x = pointer_pos
                    .map(|p| p.x - rect.min.x)
                    .unwrap_or(rect.width() / 2.0);
                columns.header_drag_offset_x = Some(off_x.clamp(0.0, rect.width()));
                ctx.request_repaint();
            }

            // Kiểm tra xem cột này có phải là ô đang bị chuột đè lên (Drop Target) không.
            // Vùng nhận diện tính trên TOÀN BỘ chiều dọc của cột (trục X).
            let is_last = col_idx == visible_cols.len() - 1;
            let is_in_col = pointer_pos.is_some_and(|p| is_pointer_in_col(p.x, rect, is_last));
            let is_drop_target = current_dragged.is_some() && !is_dragged && is_in_col;
            if is_drop_target {
                detected_drop_target = Some(col.name.clone());
            }

            // Đổi cursor icon: Grab khi hover, Grabbing khi đang drag
            if is_dragged {
                columns.header_dragged_width = Some(rect.width());
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }

            // Render ô header với highlight trực quan nếu là drop target
            render_header_cell(
                ui,
                columns,
                &col.name,
                rect,
                is_drop_target,
                resp.hovered(),
                now,
            );
        });
    }

    // Cập nhật trạng thái đang kéo
    if columns.header_dragged_name.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        // Repaint liên tục để phản hồi highlight lập tức theo từng pixel di chuyển của chuột
        ctx.request_repaint();
    }

    // Khi người dùng THẢ CHUỘT (pointer released):
    // Chỉ lúc này mới thực hiện hoán đổi (swap) vị trí cột!
    if ctx.input(|i| i.pointer.any_released()) {
        if let Some(dragged_name) = columns.header_dragged_name.take() {
            if let Some(ref target) = detected_drop_target {
                if &dragged_name != target {
                    columns.swap_columns(&dragged_name, target);
                    ctx.request_repaint();
                }
            }
        }
        columns.clear_header_drag();
    }

    // Tự động dọn dẹp animation kết thúc và repaint mượt mà nếu còn chuyển động
    columns.table_animations.update(ctx, now);
}

/// Kiểm tra tọa độ X của con trỏ chuột có nằm trong phạm vi của một cột hay không
#[inline]
pub fn is_pointer_in_col(pointer_x: f32, rect: Rect, is_last: bool) -> bool {
    if is_last {
        pointer_x >= rect.min.x && pointer_x <= rect.max.x
    } else {
        pointer_x >= rect.min.x && pointer_x < rect.max.x
    }
}

/// Tìm cột mục tiêu (drop target) dựa trên tọa độ con trỏ chuột.
/// Phạm vi nhận diện tính trên TOÀN BỘ chiều dọc của cột (trục X).
/// Trả về None nếu con trỏ chuột nằm trong chính cột đang được kéo, hoặc nằm ngoài các cột.
pub fn find_drop_target(
    dragged_name: &str,
    col_rects: &[(usize, Rect, String)],
    pointer: Pos2,
) -> Option<String> {
    for (i, (_col_idx, rect, name)) in col_rects.iter().enumerate() {
        let is_last = i == col_rects.len() - 1;
        if is_pointer_in_col(pointer.x, *rect, is_last) {
            return if name == dragged_name {
                None
            } else {
                Some(name.clone())
            };
        }
    }
    None
}

/// Áp dụng position interpolation animation, background highlight,
/// grip icon, tên cột và separator line cho một ô header.
fn render_header_cell(
    ui: &mut egui::Ui,
    columns: &mut ColumnState,
    col_name: &str,
    rect: Rect,
    is_drop_target: bool,
    is_hovered: bool,
    now: Instant,
) {
    let theme = ui.app_theme();
    // Tự động phát hiện thay đổi vị trí logic (swap/reorder) và lấy visual_rect nội suy chuyển động
    let visual_rect = columns.table_animations.track_rect(col_name, rect, now);

    // 1. Vẽ nền tiêu đề tại vị trí trực quan visual_rect
    let header_bg = if is_drop_target {
        // Ô bị swap đè lên: đổi màu sáng rõ rệt báo hiệu chuẩn bị swap
        theme.surfaces.surface1
    } else if is_hovered {
        theme.log.row_hover
    } else {
        theme.surfaces.mantle
    };
    ui.painter()
        .rect_filled(visual_rect, CornerRadius::ZERO, header_bg);

    // 2. Báo hiệu trực quan khi ô là drop target (ô bị swap đè lên)
    if is_drop_target {
        // Phủ lớp phát sáng màu accent xanh dương pastel dịu mắt
        ui.painter().rect_filled(
            visual_rect,
            CornerRadius::ZERO,
            theme.text.accent.gamma_multiply(0.22),
        );
        // Viền nổi bật báo hiệu vị trí sẽ tiếp đất
        ui.painter().rect_stroke(
            visual_rect,
            CornerRadius::ZERO,
            Stroke::new(2.0, theme.text.accent),
            egui::StrokeKind::Inside,
        );
    }

    // 3. Vẽ grip handle
    let grip_color = if is_drop_target || is_hovered {
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
    let text_color = if is_drop_target {
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

    // Deep smooth drop shadow
    painter.rect_filled(
        ghost_rect.expand(3.0),
        CornerRadius::same(5),
        Color32::from_black_alpha(130),
    );
    painter.rect_filled(ghost_rect, CornerRadius::same(4), theme.surfaces.surface0);
    painter.rect_stroke(
        ghost_rect,
        CornerRadius::same(4),
        Stroke::new(1.5, theme.text.accent),
        egui::StrokeKind::Inside,
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

    fn setup_test_columns() -> (ColumnState, Vec<ColumnItem>, Vec<(usize, Rect, String)>) {
        let mut columns = ColumnState::default();
        let visible_cols = vec![
            ColumnItem {
                name: "timestamp".to_string(),
                visible: true,
                width: 150.0,
            },
            ColumnItem {
                name: "level".to_string(),
                visible: true,
                width: 80.0,
            },
            ColumnItem {
                name: "message".to_string(),
                visible: true,
                width: 300.0,
            },
        ];
        columns.columns = visible_cols.clone();

        // Giả lập tọa độ X trên màn hình:
        // Cột 0 ("timestamp"): [0.0 .. 150.0]
        // Cột 1 ("level"):     [150.0 .. 230.0]
        // Cột 2 ("message"):   [230.0 .. 530.0]
        let col_rects = vec![
            (
                0,
                Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(150.0, 26.0)),
                "timestamp".to_string(),
            ),
            (
                1,
                Rect::from_min_size(Pos2::new(150.0, 0.0), egui::vec2(80.0, 26.0)),
                "level".to_string(),
            ),
            (
                2,
                Rect::from_min_size(Pos2::new(230.0, 0.0), egui::vec2(300.0, 26.0)),
                "message".to_string(),
            ),
        ];

        (columns, visible_cols, col_rects)
    }

    #[test]
    fn test_no_drop_target_when_pointer_inside_dragged_column() {
        let (_columns, _visible_cols, col_rects) = setup_test_columns();

        // Đang kéo cột "level" [150.0 .. 230.0]
        // Chuột nằm ở X = 190.0 (vẫn trong cột "level")
        let pointer = Pos2::new(190.0, 13.0);
        let drop_target = find_drop_target("level", &col_rects, pointer);

        assert_eq!(
            drop_target, None,
            "Khi chuột vẫn ở trong cột đang kéo thì không có drop target"
        );
    }

    #[test]
    fn test_detect_drop_target_in_other_column_right() {
        let (_columns, _visible_cols, col_rects) = setup_test_columns();

        // Đang kéo cột "level", chuột di chuyển sang X = 250.0 (nằm trong cột "message" [230.0 .. 530.0])
        let pointer = Pos2::new(250.0, 13.0);
        let drop_target = find_drop_target("level", &col_rects, pointer);

        assert_eq!(
            drop_target,
            Some("message".to_string()),
            "Phải nhận diện cột 'message' là drop target để đổi màu highlight"
        );
    }

    #[test]
    fn test_detect_drop_target_in_other_column_left() {
        let (_columns, _visible_cols, col_rects) = setup_test_columns();

        // Đang kéo cột "level", chuột di chuyển sang X = 100.0 (nằm trong cột "timestamp" [0.0 .. 150.0])
        let pointer = Pos2::new(100.0, 13.0);
        let drop_target = find_drop_target("level", &col_rects, pointer);

        assert_eq!(
            drop_target,
            Some("timestamp".to_string()),
            "Phải nhận diện cột 'timestamp' là drop target để đổi màu highlight"
        );
    }

    #[test]
    fn test_detect_drop_target_works_anywhere_vertically() {
        let (_columns, _visible_cols, col_rects) = setup_test_columns();

        // Chuột nằm ở tít bên dưới bảng log (Y = 600.0), nhưng hoành độ X = 280.0
        let pointer_down_in_logs = Pos2::new(280.0, 600.0);
        let drop_target = find_drop_target("level", &col_rects, pointer_down_in_logs);

        assert_eq!(
            drop_target,
            Some("message".to_string()),
            "Phải nhận diện drop target trên toàn bộ chiều dọc của cột"
        );
    }

    #[test]
    fn test_swap_only_executed_on_release() {
        let (mut columns, _visible_cols, col_rects) = setup_test_columns();
        columns.header_dragged_name = Some("level".to_string());

        // 1. Trong lúc đang kéo: chuột ở X = 260.0 (cột "message")
        let pointer = Pos2::new(260.0, 13.0);
        let drop_target = find_drop_target("level", &col_rects, pointer);
        assert_eq!(drop_target, Some("message".to_string()));

        // Trong lúc kéo, danh sách columns VẪN GIỮ NGUYÊN (KHÔNG live-swap)
        assert_eq!(columns.columns[0].name, "timestamp");
        assert_eq!(columns.columns[1].name, "level");
        assert_eq!(columns.columns[2].name, "message");

        // 2. Người dùng thả chuột ra (release):
        if let (Some(dragged), Some(target)) = (columns.header_dragged_name.take(), drop_target) {
            if dragged != target {
                columns.swap_columns(&dragged, &target);
            }
        }

        // Sau khi thả chuột, vị trí mới chính thức được swap!
        assert_eq!(columns.columns[0].name, "timestamp");
        assert_eq!(columns.columns[1].name, "message");
        assert_eq!(columns.columns[2].name, "level");
    }

    #[test]
    fn test_no_swap_when_released_in_same_column() {
        let (mut columns, _visible_cols, col_rects) = setup_test_columns();
        columns.header_dragged_name = Some("level".to_string());

        // Chuột được thả ở trong chính cột "level" (X = 180.0)
        let pointer = Pos2::new(180.0, 13.0);
        let drop_target = find_drop_target("level", &col_rects, pointer);
        assert_eq!(drop_target, None);

        // Thả chuột nhưng không có drop target -> không swap
        if let (Some(dragged), Some(target)) = (columns.header_dragged_name.take(), drop_target) {
            if dragged != target {
                columns.swap_columns(&dragged, &target);
            }
        }

        assert_eq!(columns.columns[0].name, "timestamp");
        assert_eq!(columns.columns[1].name, "level");
        assert_eq!(columns.columns[2].name, "message");
    }
}
