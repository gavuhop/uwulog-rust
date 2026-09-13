//! macOS native Traffic Lights (Close: Red, Minimize: Yellow, Zoom: Green).
//! Placed at the top-left corner before the Menu button, group-hover inner glyphs, inactive dimming.

use eframe::egui::{self, Color32, Rect, Sense, Stroke, Ui};

pub const TRAFFIC_LIGHT_DIAMETER: f32 = 12.0;
pub const TRAFFIC_LIGHT_SPACING: f32 = 8.0;

/// Vùng an toàn của cụm Traffic Lights ở góc trên-trái (bao gồm cả lề đệm)
#[inline]
pub fn safe_width() -> f32 {
    TRAFFIC_LIGHT_DIAMETER * 3.0 + TRAFFIC_LIGHT_SPACING * 2.0 + 20.0 // 72.0px
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrafficLightColor {
    Close,
    Minimize,
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrafficLightGlyph {
    Close,
    Minimize,
    Zoom,
}

fn render_traffic_light_circle(
    ui: &mut Ui,
    center: egui::Pos2,
    btn_color: TrafficLightColor,
    is_group_hovered: bool,
    is_focused: bool,
    glyph: TrafficLightGlyph,
    tooltip: &str,
) -> egui::Response {
    let radius = TRAFFIC_LIGHT_DIAMETER / 2.0;
    let hit_rect = Rect::from_center_size(
        center,
        egui::vec2(TRAFFIC_LIGHT_DIAMETER, TRAFFIC_LIGHT_DIAMETER),
    );
    let response = ui.interact(hit_rect, ui.make_persistent_id(tooltip), Sense::click());

    let is_active = response.is_pointer_button_down_on();

    // Màu sắc chuẩn macOS Human Interface Guidelines
    let (fill_color, border_color, glyph_color) = if !is_focused && !is_group_hovered {
        // Cửa sổ inactive: xám mờ đồng bộ
        (
            Color32::from_rgb(0x4a, 0x4c, 0x54),
            Color32::from_rgb(0x3e, 0x40, 0x46),
            Color32::TRANSPARENT,
        )
    } else {
        match btn_color {
            TrafficLightColor::Close => {
                let fill = if is_active {
                    Color32::from_rgb(0xbf, 0x49, 0x42)
                } else {
                    Color32::from_rgb(0xff, 0x5f, 0x56)
                };
                (
                    fill,
                    Color32::from_rgb(0xe0, 0x44, 0x3e),
                    Color32::from_rgb(0x4a, 0x12, 0x14),
                )
            }
            TrafficLightColor::Minimize => {
                let fill = if is_active {
                    Color32::from_rgb(0xc1, 0x8e, 0x22)
                } else {
                    Color32::from_rgb(0xff, 0xbd, 0x2e)
                };
                (
                    fill,
                    Color32::from_rgb(0xde, 0xa1, 0x23),
                    Color32::from_rgb(0x7a, 0x49, 0x00),
                )
            }
            TrafficLightColor::Zoom => {
                let fill = if is_active {
                    Color32::from_rgb(0x1d, 0x97, 0x30)
                } else {
                    Color32::from_rgb(0x27, 0xc9, 0x3f)
                };
                (
                    fill,
                    Color32::from_rgb(0x1a, 0xab, 0x29),
                    Color32::from_rgb(0x0a, 0x4d, 0x14),
                )
            }
        }
    };

    // Vẽ hình tròn
    ui.painter()
        .circle(center, radius, fill_color, Stroke::new(1.0, border_color));

    // Vẽ biểu tượng nhỏ bên trong khi hover vào cụm Traffic Lights
    if is_group_hovered {
        let stroke = Stroke::new(1.1, glyph_color);
        match glyph {
            TrafficLightGlyph::Close => {
                let d = 2.4;
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
            TrafficLightGlyph::Minimize => {
                let d = 3.0;
                ui.painter().line_segment(
                    [
                        egui::pos2(center.x - d, center.y),
                        egui::pos2(center.x + d, center.y),
                    ],
                    stroke,
                );
            }
            TrafficLightGlyph::Zoom => {
                // Biểu tượng full-screen chuẩn Apple (2 tam giác đối xứng)
                let tri1 = [
                    egui::pos2(center.x + 2.5, center.y - 2.5),
                    egui::pos2(center.x - 0.2, center.y - 2.5),
                    egui::pos2(center.x + 2.5, center.y + 0.2),
                ];
                let tri2 = [
                    egui::pos2(center.x - 2.5, center.y + 2.5),
                    egui::pos2(center.x + 0.2, center.y + 2.5),
                    egui::pos2(center.x - 2.5, center.y - 0.2),
                ];
                ui.painter().add(egui::Shape::convex_polygon(
                    tri1.to_vec(),
                    glyph_color,
                    Stroke::NONE,
                ));
                ui.painter().add(egui::Shape::convex_polygon(
                    tri2.to_vec(),
                    glyph_color,
                    Stroke::NONE,
                ));
            }
        }
    }

    response.on_hover_text(tooltip)
}

/// Render cụm Traffic Lights ở góc trên cùng bên trái của Header
pub fn render_macos_traffic_lights(ui: &mut Ui, bar_height: f32) {
    let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    let is_focused = ui.ctx().input(|i| i.viewport().focused.unwrap_or(true));

    let group_width = TRAFFIC_LIGHT_DIAMETER * 3.0 + TRAFFIC_LIGHT_SPACING * 2.0; // 52px
    let (group_rect, group_response) =
        ui.allocate_exact_size(egui::vec2(group_width, bar_height), Sense::hover());

    let is_group_hovered = group_response.hovered();
    let center_y = group_rect.center().y;

    let close_center = egui::pos2(group_rect.min.x + TRAFFIC_LIGHT_DIAMETER / 2.0, center_y);
    let min_center = egui::pos2(
        close_center.x + TRAFFIC_LIGHT_DIAMETER + TRAFFIC_LIGHT_SPACING,
        center_y,
    );
    let zoom_center = egui::pos2(
        min_center.x + TRAFFIC_LIGHT_DIAMETER + TRAFFIC_LIGHT_SPACING,
        center_y,
    );

    // 1. Close
    let close_resp = render_traffic_light_circle(
        ui,
        close_center,
        TrafficLightColor::Close,
        is_group_hovered,
        is_focused,
        TrafficLightGlyph::Close,
        "Close",
    );
    if close_resp.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // 2. Minimize
    let min_resp = render_traffic_light_circle(
        ui,
        min_center,
        TrafficLightColor::Minimize,
        is_group_hovered,
        is_focused,
        TrafficLightGlyph::Minimize,
        "Minimize",
    );
    if min_resp.clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    // 3. Zoom / Maximize
    let zoom_tooltip = if is_maximized {
        "Restore"
    } else {
        "Zoom / Full Screen"
    };
    let zoom_resp = render_traffic_light_circle(
        ui,
        zoom_center,
        TrafficLightColor::Zoom,
        is_group_hovered,
        is_focused,
        TrafficLightGlyph::Zoom,
        zoom_tooltip,
    );
    if zoom_resp.clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
    }

    ui.add_space(8.0);
}
