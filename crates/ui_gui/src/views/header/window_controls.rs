use crate::theme;
use eframe::egui::{self, Rounding, Stroke};

/// Nút điều khiển cửa sổ vector chuẩn Windows (Ẩn / Thu nhỏ, Phóng to / Khôi phục, Đóng)
pub fn render_window_controls(ui: &mut egui::Ui) {
    let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    let btn_size = egui::vec2(32.0, 24.0);

    // 1. Nút Đóng (✕)
    let (close_rect, close_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if close_resp.hovered() {
        ui.painter().rect_filled(
            close_rect,
            Rounding::same(4.0),
            egui::Color32::from_rgb(0xe8, 0x11, 0x23),
        );
    }
    let close_color = if close_resp.hovered() {
        egui::Color32::WHITE
    } else {
        theme::TEXT_MUTED
    };
    let center = close_rect.center();
    let d = 4.5;
    ui.painter().line_segment(
        [
            egui::pos2(center.x - d, center.y - d),
            egui::pos2(center.x + d, center.y + d),
        ],
        Stroke::new(1.1, close_color),
    );
    ui.painter().line_segment(
        [
            egui::pos2(center.x + d, center.y - d),
            egui::pos2(center.x - d, center.y + d),
        ],
        Stroke::new(1.1, close_color),
    );
    if close_resp.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // 2. Nút Phóng to / Khôi phục (🗖 / 🗗)
    let (max_rect, max_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if max_resp.hovered() {
        ui.painter()
            .rect_filled(max_rect, Rounding::same(4.0), theme::BG_SURFACE1);
    }
    let max_color = if max_resp.hovered() {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    let center = max_rect.center();

    if is_maximized {
        // Biểu tượng Restore (2 ô vuông lồng nhau)
        let s = 4.0;
        let p_top_left = egui::pos2(center.x - 2.0, center.y - s);
        let p_top_right = egui::pos2(center.x + s, center.y - s);
        let p_bottom_right = egui::pos2(center.x + s, center.y + 2.0);
        ui.painter()
            .line_segment([p_top_left, p_top_right], Stroke::new(1.0, max_color));
        ui.painter()
            .line_segment([p_top_right, p_bottom_right], Stroke::new(1.0, max_color));

        // Ô vuông phía trước
        let front_rect = egui::Rect::from_min_max(
            egui::pos2(center.x - s, center.y - 2.0),
            egui::pos2(center.x + 2.0, center.y + s),
        );
        ui.painter()
            .rect_stroke(front_rect, Rounding::ZERO, Stroke::new(1.0, max_color));
    } else {
        // Biểu tượng Maximize (1 ô vuông đơn)
        let s = 4.5;
        let square_rect = egui::Rect::from_min_max(
            egui::pos2(center.x - s, center.y - s),
            egui::pos2(center.x + s, center.y + s),
        );
        ui.painter()
            .rect_stroke(square_rect, Rounding::ZERO, Stroke::new(1.0, max_color));
    }

    if max_resp.clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
    }

    // 3. Nút Thu nhỏ (—)
    let (min_rect, min_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if min_resp.hovered() {
        ui.painter()
            .rect_filled(min_rect, Rounding::same(4.0), theme::BG_SURFACE1);
    }
    let min_color = if min_resp.hovered() {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    let center = min_rect.center();
    let d = 5.0;
    ui.painter().line_segment(
        [
            egui::pos2(center.x - d, center.y + 3.5),
            egui::pos2(center.x + d, center.y + 3.5),
        ],
        Stroke::new(1.1, min_color),
    );
    if min_resp.clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}
