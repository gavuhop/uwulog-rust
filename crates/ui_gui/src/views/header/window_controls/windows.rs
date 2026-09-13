//! Windows native caption buttons (Minimize, Maximize / Restore, Close).
//! 46px x 32px rectangular, contiguous (0px spacing), Segoe glyphs, red hover.
//! Matches Windows 11 / Windows 10 default window controls behavior.

use super::icons;
use super::CaptionButtonType;
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Sense, Ui};

pub const WINDOW_CONTROL_BTN_WIDTH: f32 = 46.0;
pub const WINDOW_CONTROL_BTN_HEIGHT: f32 = 32.0;

/// Tổng chiều rộng của 3 nút Windows caption (138px chuẩn Windows 11/10)
#[inline]
pub fn total_width() -> f32 {
    WINDOW_CONTROL_BTN_WIDTH * 3.0
}

/// Render 1 nút Windows caption đơn lẻ
fn render_windows_caption_button(
    ui: &mut Ui,
    btn_type: CaptionButtonType,
    has_segoe_font: bool,
    _is_maximized: bool,
) -> egui::Response {
    let size = egui::vec2(WINDOW_CONTROL_BTN_WIDTH, WINDOW_CONTROL_BTN_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let is_hovered = response.hovered();
    let is_active = response.is_pointer_button_down_on();

    // 1. Màu nền (Background)
    let bg_color = match btn_type {
        CaptionButtonType::Close => {
            if is_active {
                Color32::from_rgb(0xc4, 0x10, 0x1f) // Active / Pressed Windows Red
            } else if is_hovered {
                Color32::from_rgb(0xe8, 0x11, 0x23) // Hover Windows Red (#e81123)
            } else {
                Color32::TRANSPARENT
            }
        }
        _ => {
            if is_active {
                theme::BG_SURFACE0 // Pressed state
            } else if is_hovered {
                theme::BG_SURFACE1 // Hover state (#343a4e)
            } else {
                Color32::TRANSPARENT
            }
        }
    };

    // Bo góc chuẩn Windows 11 / Windows 10:
    // Khi ở chế độ Windowed (chưa phóng to), góc trên-phải (NorthEast) của nút Close bo nhẹ (8px)
    // khớp với viền cong cửa sổ của Windows 11.
    // Khi đã phóng to (Maximized) hoặc là các nút Minimize/Maximize, phẳng tuyệt đối (Rounding::ZERO).
    let rounding = CornerRadius::ZERO;

    if bg_color != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, rounding, bg_color);
    }

    // 2. Màu biểu tượng (Foreground)
    let fg_color = match btn_type {
        CaptionButtonType::Close => {
            if is_hovered || is_active {
                Color32::WHITE
            } else {
                theme::TEXT_MUTED
            }
        }
        _ => {
            if is_hovered || is_active {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_MUTED
            }
        }
    };

    // 3. Biểu tượng: Segoe glyph chuẩn hoặc Vector fallback
    if has_segoe_font {
        icons::draw_segoe_glyph(ui, rect, btn_type.segoe_glyph(), fg_color);
    } else {
        match btn_type {
            CaptionButtonType::Close => icons::draw_close_icon(ui, rect, fg_color),
            CaptionButtonType::Maximize => icons::draw_maximize_icon(ui, rect, fg_color),
            CaptionButtonType::Restore => icons::draw_restore_icon(ui, rect, fg_color),
            CaptionButtonType::Minimize => icons::draw_minimize_icon(ui, rect, fg_color),
        }
    }

    response.on_hover_text(btn_type.tooltip())
}

/// Render cụm 3 nút Windows caption (dính liền nhau, flush vào góc trên phải)
pub fn render_windows_window_controls(ui: &mut Ui, is_maximized: bool) {
    let test_font = FontId::new(10.0, egui::FontFamily::Name(theme::FONT_SEGOE_ICONS.into()));
    let has_segoe_font = ui.ctx().fonts_mut(|f| f.has_glyph(&test_font, '\u{e8bb}'));

    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        // Layout right_to_left: Close trước, sau đó Maximize, sau đó Minimize
        let close_resp = render_windows_caption_button(
            ui,
            CaptionButtonType::Close,
            has_segoe_font,
            is_maximized,
        );
        if close_resp.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let max_type = if is_maximized {
            CaptionButtonType::Restore
        } else {
            CaptionButtonType::Maximize
        };
        let max_resp = render_windows_caption_button(ui, max_type, has_segoe_font, is_maximized);
        if max_resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
        }

        let min_resp = render_windows_caption_button(
            ui,
            CaptionButtonType::Minimize,
            has_segoe_font,
            is_maximized,
        );
        if min_resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    });
}
