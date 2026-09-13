use crate::actions::AppAction;
use crate::overlay::{OverlayLayer, OverlayStack};
use crate::theme;
use eframe::egui::{self, Color32, Id, Key, Order, Pos2, Rect, Rounding, Stroke};

pub fn render_main_menu_popup(
    ctx: &egui::Context,
    overlay_stack: &mut OverlayStack,
    dispatch: &mut impl FnMut(AppAction),
    trigger_rect: Rect,
) {
    if !overlay_stack.is_open(OverlayLayer::MainMenu) {
        return;
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        dispatch(AppAction::CloseMainMenu);
        return;
    }

    let popup_pos = Pos2::new(trigger_rect.min.x, trigger_rect.max.y + 6.0);
    let popup_width = 170.0;
    let submenu_width = 230.0;
    let is_theme_sub_open = overlay_stack.is_open(OverlayLayer::ThemeSubmenu);

    // Kích thước ước lượng của toàn bộ menu và submenu
    let total_bounds = Rect::from_min_size(
        popup_pos,
        egui::vec2(
            if is_theme_sub_open {
                popup_width + submenu_width + 10.0
            } else {
                popup_width
            },
            240.0,
        ),
    );

    // Đóng popup nếu click chuột ra ngoài khu vực menu và nút trigger
    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !trigger_rect.contains(pos) && !total_bounds.contains(pos) {
                dispatch(AppAction::CloseMainMenu);
                return;
            }
        }
    }

    let mut action_to_dispatch: Option<AppAction> = None;
    let mut theme_btn_rect = None;

    // 1. Menu chính (About, Theme, Quit)
    egui::Area::new(Id::new("main_menu_popup_area"))
        .order(Order::Foreground)
        .fixed_pos(popup_pos)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(4.0, 4.0))
                .show(ui, |ui| {
                    ui.set_width(popup_width);

                    let render_menu_item = |ui: &mut egui::Ui,
                                            label: &str,
                                            color: Color32,
                                            is_selected: bool,
                                            trailing_text: Option<(&str, Color32)>|
                     -> egui::Response {
                        let row_size = egui::vec2(ui.available_width(), 26.0);
                        let (rect, resp) = ui.allocate_exact_size(row_size, egui::Sense::click());

                        let bg_color = if resp.hovered() || is_selected {
                            theme::BG_SURFACE1
                        } else {
                            Color32::TRANSPARENT
                        };

                        if bg_color != Color32::TRANSPARENT {
                            ui.painter()
                                .rect_filled(rect, Rounding::same(4.0), bg_color);
                        }

                        // Căn chữ bên trái, thụt vào 8.0px
                        let text_pos = Pos2::new(rect.min.x + 8.0, rect.center().y);
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::monospace(12.0),
                            color,
                        );

                        // Icon hoặc text phụ bên phải (như ▶ hoặc ✓)
                        if let Some((trail, trail_color)) = trailing_text {
                            let trail_pos = Pos2::new(rect.max.x - 8.0, rect.center().y);
                            ui.painter().text(
                                trail_pos,
                                egui::Align2::RIGHT_CENTER,
                                trail,
                                egui::FontId::monospace(11.0),
                                trail_color,
                            );
                        }

                        resp
                    };

                    // --- Item 1: About uwulog ---
                    let about_resp =
                        render_menu_item(ui, "About uwulog", theme::TEXT_PRIMARY, false, None);
                    if about_resp.clicked() {
                        action_to_dispatch = Some(AppAction::OpenAboutModal);
                    }
                    if about_resp.hovered() {
                        overlay_stack.close(OverlayLayer::ThemeSubmenu);
                    }

                    // --- Item 2: Theme (Hover có độ trễ nhẹ hoặc click để mở menu bên phải) ---
                    let is_theme_open = overlay_stack.is_open(OverlayLayer::ThemeSubmenu);
                    let theme_resp = render_menu_item(
                        ui,
                        "Theme",
                        theme::TEXT_PRIMARY,
                        is_theme_open,
                        Some(("▶", theme::TEXT_MUTED)),
                    );

                    theme_btn_rect = Some(theme_resp.rect);

                    // Xử lý mở submenu có độ trễ khi hover (150ms) hoặc click ngay lập tức
                    let hover_id = Id::new("theme_menu_item_hover_time");
                    let now = ctx.input(|i| i.time);
                    if theme_resp.hovered() {
                        let hover_start: f64 = ctx.data(|d| d.get_temp(hover_id)).unwrap_or(now);
                        ctx.data_mut(|d| d.insert_temp(hover_id, hover_start));
                        if now - hover_start >= 0.15 || theme_resp.clicked() {
                            overlay_stack.push(OverlayLayer::ThemeSubmenu);
                        } else {
                            ctx.request_repaint();
                        }
                    } else {
                        ctx.data_mut(|d| d.remove_temp::<f64>(hover_id));
                    }

                    // --- Item 3: Quit ---
                    let quit_resp = render_menu_item(ui, "Quit", theme::TEXT_PRIMARY, false, None);
                    if quit_resp.clicked() {
                        action_to_dispatch = Some(AppAction::QuitApp);
                    }
                    if quit_resp.hovered() {
                        overlay_stack.close(OverlayLayer::ThemeSubmenu);
                    }
                });
        });

    // 2. Submenu bên phải: Danh sách các theme (Mock UI)
    const THEME_OPTIONS: &[&str] = &[
        "Tokyo Night (Dark)",
        "Catppuccin Mocha (Dark)",
        "Nord (Dark)",
        "Gruvbox (Dark)",
        "One Light (Light)",
        "Catppuccin Latte (Light)",
    ];

    if overlay_stack.is_open(OverlayLayer::ThemeSubmenu) {
        if let Some(t_rect) = theme_btn_rect {
            let submenu_pos = Pos2::new(t_rect.max.x + 4.0, t_rect.min.y);
            let submenu_height = (THEME_OPTIONS.len() as f32) * 24.0 + 8.0;

            let submenu_rect =
                Rect::from_min_size(submenu_pos, egui::vec2(submenu_width, submenu_height));
            let bridge_rect = Rect::from_min_max(
                Pos2::new(t_rect.max.x - 2.0, t_rect.min.y),
                Pos2::new(submenu_pos.x + 2.0, t_rect.max.y),
            );

            // Đóng submenu nếu chuột không ở trên nút Theme, không ở vùng chuyển tiếp và không ở trên submenu
            if let Some(pointer_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let in_theme_btn = t_rect.expand(1.0).contains(pointer_pos);
                let in_bridge = bridge_rect.contains(pointer_pos);
                let in_submenu = submenu_rect.expand(2.0).contains(pointer_pos);

                if !in_theme_btn && !in_bridge && !in_submenu {
                    overlay_stack.close(OverlayLayer::ThemeSubmenu);
                }
            }

            if overlay_stack.is_open(OverlayLayer::ThemeSubmenu) {
                egui::Area::new(Id::new("main_menu_theme_submenu_area"))
                    .order(Order::Foreground)
                    .fixed_pos(submenu_pos)
                    .show(ctx, |ui| {
                        egui::Frame::default()
                            .fill(theme::BG_MANTLE)
                            .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                            .rounding(Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(4.0, 4.0))
                            .show(ui, |ui| {
                                ui.set_width(submenu_width);

                                let selected_theme_id = Id::new("mock_selected_theme_name");
                                let current_selected = ctx
                                    .data(|d| d.get_temp::<String>(selected_theme_id))
                                    .unwrap_or_else(|| "Tokyo Night (Dark)".to_string());

                                let render_sub_item =
                                    |ui: &mut egui::Ui,
                                     label: &str,
                                     is_active: bool|
                                     -> egui::Response {
                                        let row_size = egui::vec2(ui.available_width(), 24.0);
                                        let (rect, resp) =
                                            ui.allocate_exact_size(row_size, egui::Sense::click());

                                        let bg_color = if resp.hovered() || is_active {
                                            theme::BG_SURFACE1
                                        } else {
                                            Color32::TRANSPARENT
                                        };

                                        if bg_color != Color32::TRANSPARENT {
                                            ui.painter().rect_filled(
                                                rect,
                                                Rounding::same(4.0),
                                                bg_color,
                                            );
                                        }

                                        let text_color = if is_active {
                                            theme::TEXT_KEY
                                        } else {
                                            theme::TEXT_PRIMARY
                                        };

                                        // Text căn lề trái thụt vào 8.0px
                                        let text_pos = Pos2::new(rect.min.x + 8.0, rect.center().y);
                                        ui.painter().text(
                                            text_pos,
                                            egui::Align2::LEFT_CENTER,
                                            label,
                                            egui::FontId::monospace(11.5),
                                            text_color,
                                        );

                                        if is_active {
                                            let trail_pos =
                                                Pos2::new(rect.max.x - 8.0, rect.center().y);
                                            ui.painter().text(
                                                trail_pos,
                                                egui::Align2::RIGHT_CENTER,
                                                "✓",
                                                egui::FontId::monospace(12.0),
                                                theme::TEXT_KEY,
                                            );
                                        }

                                        resp
                                    };

                                for name in THEME_OPTIONS {
                                    let is_active = current_selected == *name;
                                    let item_resp = render_sub_item(ui, name, is_active);

                                    if item_resp.clicked() {
                                        ctx.data_mut(|d| {
                                            d.insert_temp(selected_theme_id, name.to_string());
                                        });
                                        overlay_stack.close_main_menu();
                                    }
                                }
                            });
                    });
            }
        }
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}
