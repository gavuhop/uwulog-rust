//! Linux native caption buttons (GNOME / Libadwaita / KDE Plasma).
//! 24px x 24px circular buttons, 8px spacing, neutral/soft red hover, vector icons.

use super::icons;
use super::CaptionButtonType;
use crate::theme;
use eframe::egui::{self, Color32, Rounding, Sense, Ui};

pub const LINUX_BTN_SIZE: f32 = 24.0;
pub const LINUX_BTN_SPACING: f32 = 8.0;

/// Tổng chiều rộng của cụm 3 nút Linux caption (88px)
#[inline]
pub fn total_width() -> f32 {
    LINUX_BTN_SIZE * 3.0 + LINUX_BTN_SPACING * 2.0
}

fn render_linux_caption_button(ui: &mut Ui, btn_type: CaptionButtonType) -> egui::Response {
    let size = egui::vec2(LINUX_BTN_SIZE, LINUX_BTN_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let is_hovered = response.hovered();
    let is_active = response.is_pointer_button_down_on();

    let rounding = Rounding::same(LINUX_BTN_SIZE / 2.0);

    let (bg_color, fg_color) = match btn_type {
        CaptionButtonType::Close => {
            if is_active {
                (Color32::from_rgb(0xc4, 0x38, 0x44), Color32::WHITE)
            } else if is_hovered {
                (Color32::from_rgb(0xd9, 0x48, 0x54), Color32::WHITE) // Libadwaita soft red
            } else {
                (Color32::TRANSPARENT, theme::TEXT_MUTED)
            }
        }
        _ => {
            if is_active {
                (theme::BG_SURFACE0, theme::TEXT_PRIMARY)
            } else if is_hovered {
                (theme::BG_SURFACE1, theme::TEXT_PRIMARY)
            } else {
                (Color32::TRANSPARENT, theme::TEXT_MUTED)
            }
        }
    };

    if bg_color != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, rounding, bg_color);
    }

    match btn_type {
        CaptionButtonType::Close => icons::draw_close_icon(ui, rect, fg_color),
        CaptionButtonType::Maximize => icons::draw_maximize_icon(ui, rect, fg_color),
        CaptionButtonType::Restore => icons::draw_restore_icon(ui, rect, fg_color),
        CaptionButtonType::Minimize => icons::draw_minimize_icon(ui, rect, fg_color),
    }

    response.on_hover_text(btn_type.tooltip())
}

pub fn render_linux_window_controls(ui: &mut Ui, is_maximized: bool) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(LINUX_BTN_SPACING, 0.0);

        let close_resp = render_linux_caption_button(ui, CaptionButtonType::Close);
        if close_resp.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let max_type = if is_maximized {
            CaptionButtonType::Restore
        } else {
            CaptionButtonType::Maximize
        };
        let max_resp = render_linux_caption_button(ui, max_type);
        if max_resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
        }

        let min_resp = render_linux_caption_button(ui, CaptionButtonType::Minimize);
        if min_resp.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    });
}
