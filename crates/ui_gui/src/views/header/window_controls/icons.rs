//! Reusable vector drawing routines for window caption icons (Close, Maximize, Restore, Minimize).

use eframe::egui::{self, Color32, CornerRadius, Rect, Stroke, Ui};

/// Vẽ icon đóng (✕)
pub fn draw_close_icon(ui: &mut Ui, rect: Rect, color: Color32) {
    let center = rect.center();
    let d = 4.5;
    let stroke = Stroke::new(1.0, color);
    ui.painter().line_segment(
        [
            egui::pos2(center.x - d, center.y - d),
            egui::pos2(center.x + d, center.y + d),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(center.x + d, center.y - d),
            egui::pos2(center.x - d, center.y + d),
        ],
        stroke,
    );
}

/// Vẽ icon phóng to (ô vuông đơn)
pub fn draw_maximize_icon(ui: &mut Ui, rect: Rect, color: Color32) {
    let center = rect.center();
    let s = 4.5;
    let stroke = Stroke::new(1.0, color);
    let square = Rect::from_min_max(
        egui::pos2(center.x - s, center.y - s),
        egui::pos2(center.x + s, center.y + s),
    );
    ui.painter()
        .rect_stroke(square, CornerRadius::ZERO, stroke, egui::StrokeKind::Inside);
}

/// Vẽ icon khôi phục (2 ô vuông lồng nhau)
pub fn draw_restore_icon(ui: &mut Ui, rect: Rect, color: Color32) {
    let center = rect.center();
    let stroke = Stroke::new(1.0, color);
    let s = 3.5;
    let offset = 2.0;

    // Hộp phía sau (lệch lên trên sang phải)
    let p_top_left = egui::pos2(center.x - s + offset, center.y - s);
    let p_top_right = egui::pos2(center.x + s + offset, center.y - s);
    let p_bottom_right = egui::pos2(center.x + s + offset, center.y + s);
    let p_bottom_left = egui::pos2(center.x + s, center.y + s);
    let p_top_left2 = egui::pos2(center.x - s + offset, center.y - s + offset);

    ui.painter().line_segment([p_top_left, p_top_right], stroke);
    ui.painter()
        .line_segment([p_top_right, p_bottom_right], stroke);
    ui.painter()
        .line_segment([p_bottom_right, p_bottom_left], stroke);
    ui.painter().line_segment([p_top_left, p_top_left2], stroke);

    // Hộp phía trước (lệch xuống dưới sang trái)
    let front_rect = Rect::from_min_max(
        egui::pos2(center.x - s, center.y - s + offset),
        egui::pos2(center.x + s, center.y + s + offset),
    );
    ui.painter().rect_stroke(
        front_rect,
        CornerRadius::ZERO,
        stroke,
        egui::StrokeKind::Inside,
    );
}

/// Vẽ icon thu nhỏ (dấu gạch ngang —)
pub fn draw_minimize_icon(ui: &mut Ui, rect: Rect, color: Color32) {
    let center = rect.center();
    let d = 5.0;
    let stroke = Stroke::new(1.0, color);
    ui.painter().line_segment(
        [
            egui::pos2(center.x - d, center.y + 4.0),
            egui::pos2(center.x + d, center.y + 4.0),
        ],
        stroke,
    );
}

/// Vẽ glyph từ font Segoe Fluent Icons / Segoe MDL2 Assets
pub fn draw_segoe_glyph(ui: &mut Ui, rect: Rect, glyph: &str, color: Color32) {
    let font_id = egui::FontId::new(
        10.0,
        egui::FontFamily::Name(crate::theme::FONT_SEGOE_ICONS.into()),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        font_id,
        color,
    );
}
