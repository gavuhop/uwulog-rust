use crate::actions::AppAction;
use crate::overlay::{OverlayLayer, OverlayStack};
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, Id, Order, Pos2, Rect, Stroke};

fn restore_preview_theme(ctx: &egui::Context) {
    if let Some(orig_id) =
        ctx.data_mut(|d| d.remove_temp::<String>(Id::new("theme_preview_orig_id")))
    {
        crate::theme::set_active_theme(&orig_id, ctx);
    }
}

pub fn render_main_menu_popup(
    ctx: &egui::Context,
    overlay_stack: &mut OverlayStack,
    dispatch: &mut impl FnMut(AppAction),
    trigger_rect: Rect,
) {
    if !overlay_stack.is_open(OverlayLayer::MainMenu) {
        restore_preview_theme(ctx);
        return;
    }

    let theme = ctx.app_theme();
    let popup_pos = Pos2::new(trigger_rect.min.x, trigger_rect.max.y + 6.0);
    let popup_width = 190.0;
    let submenu_width = 240.0;
    let is_theme_sub_open = overlay_stack.is_open(OverlayLayer::ThemeSubmenu);

    let stored_main_menu_rect: Option<Rect> =
        ctx.data(|d| d.get_temp(Id::new("main_menu_actual_rect")));
    let stored_submenu_rect: Option<Rect> =
        ctx.data(|d| d.get_temp(Id::new("theme_submenu_actual_rect")));

    // Đóng popup nếu click chuột ra ngoài khu vực menu và nút trigger.
    // Sử dụng rect thực tế đã vẽ (kèm buffer an toàn) để đảm bảo click vào bất kỳ item nào cũng không bị đóng nhầm.
    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            let on_trigger = trigger_rect.contains(pos);
            let on_main_menu = stored_main_menu_rect
                .map(|r| r.expand(6.0).contains(pos))
                .unwrap_or(false);
            let on_submenu = is_theme_sub_open
                && stored_submenu_rect
                    .map(|r| r.expand(8.0).contains(pos))
                    .unwrap_or(false);

            // Dự phòng cho frame đầu tiên khi chưa có rect lưu tạm
            let fallback_on_menu = stored_main_menu_rect.is_none()
                && Rect::from_min_size(popup_pos, egui::vec2(popup_width + 10.0, 150.0))
                    .contains(pos);

            if !on_trigger && !on_main_menu && !on_submenu && !fallback_on_menu {
                restore_preview_theme(ctx);
                dispatch(AppAction::CloseMainMenu);
                return;
            }
        }
    }

    let mut action_to_dispatch: Option<AppAction> = None;
    let mut theme_btn_rect = None;

    // 1. Menu chính (About, Theme, Quit)
    let main_menu_area_resp = egui::Area::new(Id::new("main_menu_popup_area"))
        .order(Order::Foreground)
        .fixed_pos(popup_pos)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme.surfaces.mantle)
                .stroke(Stroke::new(1.0, theme.borders.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.set_width(popup_width);

                    let render_menu_item = |ui: &mut egui::Ui,
                                            label: &str,
                                            color: Color32,
                                            is_selected: bool,
                                            trailing_icon: Option<
                        crate::components::ui::IconName,
                    >|
                     -> egui::Response {
                        let row_size = egui::vec2(ui.available_width(), 26.0);
                        let (rect, resp) = ui.allocate_exact_size(row_size, egui::Sense::click());

                        let bg_color = if resp.hovered() || is_selected {
                            theme.surfaces.surface1
                        } else {
                            Color32::TRANSPARENT
                        };

                        if bg_color != Color32::TRANSPARENT {
                            ui.painter()
                                .rect_filled(rect, CornerRadius::same(4), bg_color);
                        }

                        let text_pos = Pos2::new(rect.min.x + 8.0, rect.center().y);
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::monospace(12.0),
                            color,
                        );

                        if let Some(icon) = trailing_icon {
                            let icon_r = Rect::from_center_size(
                                Pos2::new(rect.max.x - 12.0, rect.center().y),
                                egui::vec2(12.0, 12.0),
                            );
                            icon.paint(ui.painter(), icon_r, theme.text.muted);
                        }

                        resp
                    };

                    // --- Item 1: About uwulog ---
                    let about_resp =
                        render_menu_item(ui, "About uwulog", theme.text.primary, false, None);
                    if about_resp.clicked() {
                        action_to_dispatch = Some(AppAction::OpenAboutModal);
                    }
                    if about_resp.hovered() {
                        overlay_stack.close(OverlayLayer::ThemeSubmenu);
                        restore_preview_theme(ctx);
                    }

                    // --- Item 2: Keyboard Shortcuts ---
                    let keymap_resp =
                        render_menu_item(ui, "Keyboard Shortcuts", theme.text.primary, false, None);
                    if keymap_resp.clicked() {
                        action_to_dispatch = Some(AppAction::OpenKeymapModal);
                    }
                    if keymap_resp.hovered() {
                        overlay_stack.close(OverlayLayer::ThemeSubmenu);
                        restore_preview_theme(ctx);
                    }

                    // --- Item 3: Theme ---
                    let is_theme_open = overlay_stack.is_open(OverlayLayer::ThemeSubmenu);
                    let theme_resp = render_menu_item(
                        ui,
                        "Theme",
                        theme.text.primary,
                        is_theme_open,
                        Some(crate::components::ui::IconName::ChevronRight),
                    );

                    theme_btn_rect = Some(theme_resp.rect);

                    // Xử lý mở submenu có độ trễ nhẹ khi hover (120ms) hoặc click ngay lập tức
                    let hover_id = Id::new("theme_menu_item_hover_time");
                    let now = ctx.input(|i| i.time);
                    if theme_resp.hovered() {
                        let hover_start: f64 = ctx.data(|d| d.get_temp(hover_id)).unwrap_or(now);
                        ctx.data_mut(|d| d.insert_temp(hover_id, hover_start));
                        if now - hover_start >= 0.12 || theme_resp.clicked() {
                            overlay_stack.push(OverlayLayer::ThemeSubmenu);
                        } else {
                            ctx.request_repaint();
                        }
                    } else {
                        ctx.data_mut(|d| d.remove_temp::<f64>(hover_id));
                    }

                    // --- Item 3: Quit ---
                    let quit_resp = render_menu_item(ui, "Quit", theme.text.primary, false, None);
                    if quit_resp.clicked() {
                        action_to_dispatch = Some(AppAction::QuitApp);
                    }
                    if quit_resp.hovered() {
                        overlay_stack.close(OverlayLayer::ThemeSubmenu);
                        restore_preview_theme(ctx);
                    }
                });
        });

    ctx.data_mut(|d| {
        d.insert_temp(
            Id::new("main_menu_actual_rect"),
            main_menu_area_resp.response.rect,
        );
    });

    // 2. Submenu bên phải: Danh sách các theme & Tác vụ mở rộng
    if overlay_stack.is_open(OverlayLayer::ThemeSubmenu) {
        let themes = crate::theme::list_themes();
        if let Some(t_rect) = theme_btn_rect {
            let submenu_pos = Pos2::new(t_rect.max.x + 4.0, t_rect.min.y);
            let bridge_rect = Rect::from_min_max(
                Pos2::new(t_rect.min.x, t_rect.min.y - 4.0),
                Pos2::new(submenu_pos.x + 6.0, t_rect.max.y + 4.0),
            );

            // Đóng submenu nếu chuột rời khỏi Theme button, vùng chuyển tiếp và vùng submenu thực tế
            if let Some(pointer_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let in_theme_btn = t_rect.expand(2.0).contains(pointer_pos);
                let in_bridge = bridge_rect.contains(pointer_pos);
                let in_submenu = stored_submenu_rect
                    .map(|r| r.expand(8.0).contains(pointer_pos))
                    .unwrap_or(true);

                if !in_theme_btn && !in_bridge && !in_submenu {
                    overlay_stack.close(OverlayLayer::ThemeSubmenu);
                    restore_preview_theme(ctx);
                }
            }

            if overlay_stack.is_open(OverlayLayer::ThemeSubmenu) {
                let submenu_area_resp = egui::Area::new(Id::new("main_menu_theme_submenu_area"))
                    .order(Order::Foreground)
                    .fixed_pos(submenu_pos)
                    .show(ctx, |ui| {
                        egui::Frame::default()
                            .fill(theme.surfaces.mantle)
                            .stroke(Stroke::new(1.0, theme.borders.border))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(4, 4))
                            .show(ui, |ui| {
                                ui.set_width(submenu_width);

                                let orig_theme_id: String = ctx.data_mut(|d| {
                                    d.get_temp(Id::new("theme_preview_orig_id")).unwrap_or_else(
                                        || {
                                            let id = theme.id.clone();
                                            d.insert_temp(
                                                Id::new("theme_preview_orig_id"),
                                                id.clone(),
                                            );
                                            id
                                        },
                                    )
                                });

                                let render_sub_item =
                                    |ui: &mut egui::Ui,
                                     label: &str,
                                     is_active: bool|
                                     -> egui::Response {
                                        let row_size = egui::vec2(ui.available_width(), 24.0);
                                        let (rect, resp) =
                                            ui.allocate_exact_size(row_size, egui::Sense::click());

                                        let bg_color = if resp.hovered() || is_active {
                                            theme.surfaces.surface1
                                        } else {
                                            Color32::TRANSPARENT
                                        };

                                        if bg_color != Color32::TRANSPARENT {
                                            ui.painter().rect_filled(
                                                rect,
                                                CornerRadius::same(4),
                                                bg_color,
                                            );
                                        }

                                        let text_color = if is_active {
                                            theme.text.accent
                                        } else {
                                            theme.text.primary
                                        };

                                        let text_pos = Pos2::new(rect.min.x + 8.0, rect.center().y);
                                        ui.painter().text(
                                            text_pos,
                                            egui::Align2::LEFT_CENTER,
                                            label,
                                            egui::FontId::monospace(11.0),
                                            text_color,
                                        );

                                        if is_active {
                                            let check_r = Rect::from_center_size(
                                                Pos2::new(rect.max.x - 12.0, rect.center().y),
                                                egui::vec2(12.0, 12.0),
                                            );
                                            crate::components::ui::IconName::Check.paint(
                                                ui.painter(),
                                                check_r,
                                                theme.text.accent,
                                            );
                                        }

                                        resp
                                    };

                                let render_action_sub_item =
                                    |ui: &mut egui::Ui,
                                     label: &str,
                                     icon: Option<crate::components::ui::IconName>|
                                     -> egui::Response {
                                        let row_size = egui::vec2(ui.available_width(), 24.0);
                                        let (rect, resp) =
                                            ui.allocate_exact_size(row_size, egui::Sense::click());

                                        let bg_color = if resp.hovered() {
                                            theme.surfaces.surface1
                                        } else {
                                            Color32::TRANSPARENT
                                        };

                                        if bg_color != Color32::TRANSPARENT {
                                            ui.painter().rect_filled(
                                                rect,
                                                CornerRadius::same(4),
                                                bg_color,
                                            );
                                        }

                                        let text_color = if resp.hovered() {
                                            theme.text.accent
                                        } else {
                                            theme.text.muted
                                        };

                                        let text_pos = Pos2::new(rect.min.x + 8.0, rect.center().y);
                                        ui.painter().text(
                                            text_pos,
                                            egui::Align2::LEFT_CENTER,
                                            label,
                                            egui::FontId::monospace(11.0),
                                            text_color,
                                        );

                                        if let Some(ic) = icon {
                                            let icon_r = Rect::from_center_size(
                                                Pos2::new(rect.max.x - 12.0, rect.center().y),
                                                egui::vec2(12.0, 12.0),
                                            );
                                            ic.paint(ui.painter(), icon_r, text_color);
                                        }

                                        resp
                                    };

                                let render_separator = |ui: &mut egui::Ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 7.0),
                                        egui::Sense::hover(),
                                    );
                                    let y = rect.center().y;
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(rect.min.x + 4.0, y),
                                            Pos2::new(rect.max.x - 4.0, y),
                                        ],
                                        Stroke::new(1.0, theme.borders.border_subtle),
                                    );
                                };

                                // --- Danh sách Themes: Hiển thị liền mạch không cần separator giữa theme gốc và custom ---
                                for t in &themes {
                                    let is_active = t.id == orig_theme_id;
                                    let item_resp = render_sub_item(ui, &t.name, is_active);

                                    if item_resp.hovered() && theme.id != t.id {
                                        crate::theme::set_active_theme(&t.id, ctx);
                                    }

                                    if item_resp.clicked() {
                                        ctx.data_mut(|d| {
                                            d.remove_temp::<String>(Id::new(
                                                "theme_preview_orig_id",
                                            ))
                                        });
                                        crate::theme::set_active_theme(&t.id, ctx);
                                        action_to_dispatch =
                                            Some(AppAction::SwitchTheme(t.id.clone()));
                                        overlay_stack.close_main_menu();
                                    }
                                }

                                // --- Tác vụ Import Theme: Mở file dialog tại thư mục themes, có sẵn template JSON ---
                                render_separator(ui);

                                let import_resp = render_action_sub_item(
                                    ui,
                                    "Import Theme...",
                                    Some(crate::components::ui::IconName::File),
                                );
                                if import_resp.clicked() {
                                    overlay_stack.close_main_menu();

                                    let _ = crate::theme::ensure_template_file();
                                    let themes_dir = crate::theme::get_themes_dir();

                                    if let Some(path) = rfd::FileDialog::new()
                                        .set_directory(&themes_dir)
                                        .add_filter("Theme JSON (*.json)", &["json"])
                                        .set_title("Import Theme")
                                        .pick_file()
                                    {
                                        match crate::theme::import_theme_file(&path) {
                                            Ok(imported) => {
                                                ctx.data_mut(|d| {
                                                    d.remove_temp::<String>(Id::new(
                                                        "theme_preview_orig_id",
                                                    ))
                                                });
                                                crate::theme::set_active_theme(&imported.id, ctx);
                                                action_to_dispatch = Some(AppAction::SwitchTheme(
                                                    imported.id.clone(),
                                                ));
                                            }
                                            Err(err) => {
                                                eprintln!("Failed to import theme: {err}");
                                            }
                                        }
                                    }
                                }
                            });
                    });

                ctx.data_mut(|d| {
                    d.insert_temp(
                        Id::new("theme_submenu_actual_rect"),
                        submenu_area_resp.response.rect,
                    );
                });
            }
        }
    } else {
        ctx.data_mut(|d| d.remove::<Rect>(Id::new("theme_submenu_actual_rect")));
        restore_preview_theme(ctx);
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_raw_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1024.0, 768.0))),
            ..Default::default()
        }
    }

    #[test]
    fn test_theme_hover_real_time_change_and_restore_on_close() {
        let _guard = crate::theme::THEME_TEST_MUTEX.lock().unwrap();
        let ctx = egui::Context::default();
        crate::theme::set_active_theme("nord-dimmed", &ctx);
        assert_eq!(crate::theme::active().id, "nord-dimmed");

        let mut overlay_stack = OverlayStack::new();
        overlay_stack.push(OverlayLayer::MainMenu);
        overlay_stack.push(OverlayLayer::ThemeSubmenu);

        let trigger_rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(30.0, 30.0));
        let mut dispatched = Vec::new();
        let mut dispatch = |action: AppAction| {
            dispatched.push(action);
        };

        let input1 = test_raw_input();
        let mut out1 = ctx.run_ui(input1, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_main_menu_popup(ui.ctx(), &mut overlay_stack, &mut dispatch, trigger_rect);
            });
        });
        out1.textures_delta.clear();

        // Get the rect of the theme submenu
        let submenu_rect: Rect = ctx
            .data(|d| d.get_temp(Id::new("theme_submenu_actual_rect")))
            .expect("Submenu rect should be stored");

        // Item 1 is "one-dark" (min.y + 4 + 24 + 12 = min.y + 40)
        let hover_pos = Pos2::new(submenu_rect.center().x, submenu_rect.min.y + 36.0);

        let mut hover_input = test_raw_input();
        hover_input
            .events
            .push(egui::Event::PointerMoved(hover_pos));

        // Frame 2: Hover over "one-dark"
        let mut out2 = ctx.run_ui(hover_input.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_main_menu_popup(ui.ctx(), &mut overlay_stack, &mut dispatch, trigger_rect);
            });
        });
        out2.textures_delta.clear();

        // Frame 2b: Second frame of hover
        let mut out2b = ctx.run_ui(hover_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_main_menu_popup(ui.ctx(), &mut overlay_stack, &mut dispatch, trigger_rect);
            });
        });
        out2b.textures_delta.clear();

        // Theme should be changed in real time to "one-dark"!
        assert_eq!(crate::theme::active().id, "one-dark");

        // Frame 3: Close ThemeSubmenu without clicking (cancel)
        overlay_stack.close(OverlayLayer::ThemeSubmenu);
        let cancel_input = test_raw_input();
        let mut out3 = ctx.run_ui(cancel_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_main_menu_popup(ui.ctx(), &mut overlay_stack, &mut dispatch, trigger_rect);
            });
        });
        out3.textures_delta.clear();

        // Theme should be restored back to "nord-dimmed"!
        assert_eq!(crate::theme::active().id, "nord-dimmed");
    }

    #[test]
    fn test_theme_hover_and_click_selects_theme() {
        let _guard = crate::theme::THEME_TEST_MUTEX.lock().unwrap();
        let ctx = egui::Context::default();
        crate::theme::set_active_theme("nord-dimmed", &ctx);
        assert_eq!(crate::theme::active().id, "nord-dimmed");

        let mut overlay_stack = OverlayStack::new();
        overlay_stack.push(OverlayLayer::MainMenu);
        overlay_stack.push(OverlayLayer::ThemeSubmenu);

        let trigger_rect = Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(30.0, 30.0));
        let mut dispatched = Vec::new();
        {
            let mut dispatch = |action: AppAction| {
                dispatched.push(action);
            };

            // Frame 1: Initial render
            let input1 = test_raw_input();
            let mut out1 = ctx.run_ui(input1, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_main_menu_popup(
                        ui.ctx(),
                        &mut overlay_stack,
                        &mut dispatch,
                        trigger_rect,
                    );
                });
            });
            out1.textures_delta.clear();

            let submenu_rect: Rect = ctx
                .data(|d| d.get_temp(Id::new("theme_submenu_actual_rect")))
                .expect("Submenu rect should be stored");

            let click_pos = Pos2::new(submenu_rect.center().x, submenu_rect.min.y + 36.0);

            // Frame 2a: Move pointer to "one-dark"
            let mut move_input = test_raw_input();
            move_input.events.push(egui::Event::PointerMoved(click_pos));
            let mut out2a = ctx.run_ui(move_input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_main_menu_popup(
                        ui.ctx(),
                        &mut overlay_stack,
                        &mut dispatch,
                        trigger_rect,
                    );
                });
            });
            out2a.textures_delta.clear();

            // Frame 2b: Mouse down on "one-dark"
            let mut press_input = test_raw_input();
            press_input.events.push(egui::Event::PointerButton {
                pos: click_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            });
            let mut out2 = ctx.run_ui(press_input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_main_menu_popup(
                        ui.ctx(),
                        &mut overlay_stack,
                        &mut dispatch,
                        trigger_rect,
                    );
                });
            });
            out2.textures_delta.clear();

            // Frame 3: Mouse up (release click)
            let mut release_input = test_raw_input();
            release_input.events.push(egui::Event::PointerButton {
                pos: click_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            });
            let mut out3 = ctx.run_ui(release_input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_main_menu_popup(
                        ui.ctx(),
                        &mut overlay_stack,
                        &mut dispatch,
                        trigger_rect,
                    );
                });
            });
            out3.textures_delta.clear();
        }

        // Action SwitchTheme("one-dark") must be dispatched!
        assert!(matches!(
            dispatched.first(),
            Some(AppAction::SwitchTheme(id)) if id == "one-dark"
        ));

        // Theme should remain "one-dark"
        assert_eq!(crate::theme::active().id, "one-dark");

        // Menu should be closed
        assert!(!overlay_stack.is_open(OverlayLayer::MainMenu));
        assert!(!overlay_stack.is_open(OverlayLayer::ThemeSubmenu));

        // Frame 4: Menu closed frame
        let final_input = test_raw_input();
        let mut out4 = ctx.run_ui(final_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_main_menu_popup(ui.ctx(), &mut overlay_stack, &mut |_| {}, trigger_rect);
            });
        });
        out4.textures_delta.clear();

        // Theme still remains "one-dark" (not reverted)
        assert_eq!(crate::theme::active().id, "one-dark");

        // Cleanup
        crate::theme::set_active_theme("nord-dimmed", &ctx);
    }
}
