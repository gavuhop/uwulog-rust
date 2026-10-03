use super::keymap::{format_context_path, format_keystroke_zed, humanize_action_name};
use crate::app::overlay_manager::KeymapModalState;
use crate::components::ui::{AppButton, ButtonVariant};
use crate::keymap::KeyContext;
use crate::theme::Theme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2};

/// Render popup modal tùy chỉnh phím tắt (chuẩn phong cách KeybindingEditorModal của Zed)
pub fn render_key_customizer_popup(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut KeymapModalState,
) {
    let Some((ref action, context)) = state.recording.clone() else {
        return;
    };

    let screen_rect = ctx.viewport_rect();

    // 1. Backdrop làm mờ toàn màn hình
    let mut click_outside = false;
    egui::Area::new(egui::Id::new("key_customizer_scrim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(120));
            if resp.clicked() {
                click_outside = true;
            }
        });

    if click_outside {
        state.recording = None;
        state.pending_keystroke = None;
        state.is_recording_keystroke = false;
        state.pending_context = KeyContext::Global;
        return;
    }

    // 2. Card popup nổi ở giữa màn hình (chuẩn thiết kế KeybindingEditorModal của Zed)
    let mut filter_by_shortcut = None;
    let mut close_dialog = false;
    let mut save_dialog = false;
    let is_rec = state.is_recording_keystroke;

    egui::Area::new(egui::Id::new("key_customizer_dialog"))
        .order(egui::Order::Tooltip)
        .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 60.0))
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme.surfaces.surface0)
                .stroke(Stroke::new(1.0, theme.borders.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(22, 18))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 8],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(90),
                })
                .show(ui, |ui| {
                    let dialog_w = 480.0_f32.min((screen_rect.width() - 32.0).max(320.0));
                    ui.set_width(dialog_w);

                    // 1. Header: Humanized Action Name & Description (Chuẩn Zed)
                    ui.label(
                        egui::RichText::new(humanize_action_name(action))
                            .size(14.0)
                            .color(theme.text.primary),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(action.description())
                            .size(11.5)
                            .color(theme.text.muted),
                    );

                    ui.add_space(14.0);

                    // 2. Section: Edit Keystroke
                    ui.label(
                        egui::RichText::new("Edit Keystroke")
                            .size(12.5)
                            .color(theme.text.primary),
                    );
                    ui.add_space(6.0);

                    // Hộp nhập / hiển thị phím trung tâm (Keystroke Box)
                    let box_w = ui.available_width();
                    let box_h = 36.0;
                    let (box_rect, box_resp) =
                        ui.allocate_exact_size(Vec2::new(box_w, box_h), egui::Sense::click());

                    // Nền và viền hộp
                    ui.painter().rect_filled(
                        box_rect,
                        CornerRadius::same(5),
                        theme.surfaces.surface1,
                    );

                    let stroke_color = if is_rec {
                        theme.text.accent
                    } else if box_resp.hovered() {
                        theme.borders.border_focused
                    } else {
                        theme.borders.border
                    };
                    ui.painter().rect_stroke(
                        box_rect,
                        CornerRadius::same(5),
                        Stroke::new(if is_rec { 1.5 } else { 1.0 }, stroke_color),
                        egui::StrokeKind::Inside,
                    );

                    // Text hiển thị ở giữa hộp
                    let (center_text, text_color) = if is_rec {
                        ("Recording keystroke...".to_string(), theme.text.accent)
                    } else if let Some(ref ks) = state.pending_keystroke {
                        (format_keystroke_zed(ks), theme.text.primary)
                    } else {
                        ("<none>".to_string(), theme.text.muted)
                    };

                    ui.painter().text(
                        box_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        center_text,
                        FontId::monospace(13.0),
                        text_color,
                    );

                    // Nút record icon bên phải hộp (▶ / ⏹)
                    let btn_size = Vec2::new(28.0, 24.0);
                    let btn_rect = Rect::from_center_size(
                        Pos2::new(box_rect.max.x - 18.0, box_rect.center().y),
                        btn_size,
                    );

                    let record_btn_resp = ui.interact(
                        btn_rect,
                        ui.id().with("key_customizer_record_btn"),
                        egui::Sense::click(),
                    );

                    if record_btn_resp.clicked()
                        || (box_resp.clicked() && !record_btn_resp.hovered())
                    {
                        state.is_recording_keystroke = !state.is_recording_keystroke;
                    }

                    let btn_bg = if record_btn_resp.is_pointer_button_down_on() {
                        theme.surfaces.surface1
                    } else if record_btn_resp.hovered() {
                        theme.surfaces.surface0
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter()
                        .rect_filled(btn_rect, CornerRadius::same(4), btn_bg);

                    let icon_color = if is_rec {
                        theme.status.warning
                    } else if record_btn_resp.hovered() {
                        theme.text.primary
                    } else {
                        theme.text.muted
                    };
                    let icon_text = if is_rec { "⏹" } else { "▶" };
                    ui.painter().text(
                        btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon_text,
                        FontId::proportional(11.0),
                        icon_color,
                    );

                    // Tooltip chuẩn hình: Left: "Start Recording", Right: "Enter"
                    record_btn_resp.on_hover_ui(|ui| {
                        ui.horizontal(|ui| {
                            let label_text = if is_rec {
                                "Stop Recording"
                            } else {
                                "Start Recording"
                            };
                            ui.label(
                                egui::RichText::new(label_text)
                                    .size(11.0)
                                    .color(theme.text.primary),
                            );
                            ui.add_space(16.0);
                            ui.label(
                                egui::RichText::new("Enter")
                                    .size(11.0)
                                    .color(theme.text.muted),
                            );
                        });
                    });

                    // Thông tin xung đột / bindings cùng phím (Chuẩn câu chữ của Zed)
                    if let Some(ref ks) = state.pending_keystroke {
                        let matching: Vec<_> = state
                            .draft
                            .bindings()
                            .iter()
                            .filter(|b| {
                                b.keystroke == *ks && (&b.action != action || b.context != context)
                            })
                            .collect();

                        let count = matching.len();
                        if count > 0 {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                let info_text = if count == 1 {
                                    "There is 1 binding with the same keystrokes.".to_string()
                                } else {
                                    format!(
                                        "There are {} bindings with the same keystrokes.",
                                        count
                                    )
                                };

                                ui.label(
                                    egui::RichText::new(info_text)
                                        .size(11.0)
                                        .color(theme.text.muted),
                                );

                                let view_btn = ui.add(
                                    egui::Button::new(
                                        egui::RichText::new("View ↗")
                                            .size(11.0)
                                            .color(theme.text.accent),
                                    )
                                    .frame(false),
                                );

                                let mut tip = String::from("Bindings sharing this keystroke:\n");
                                for b in &matching {
                                    tip.push_str(&format!(
                                        " • {} ({})\n",
                                        humanize_action_name(&b.action),
                                        format_context_path(b.context)
                                    ));
                                }
                                tip.push_str("\nClick to filter table by this keystroke.");
                                view_btn.clone().on_hover_text(tip);

                                if view_btn.clicked() {
                                    filter_by_shortcut = Some(format_keystroke_zed(ks));
                                }
                            });
                        }
                    }

                    ui.add_space(14.0);

                    // 3. Section: Edit Context
                    ui.label(
                        egui::RichText::new("Edit Context")
                            .size(12.5)
                            .color(theme.text.primary),
                    );
                    ui.add_space(6.0);

                    ui.scope(|ui| {
                        ui.visuals_mut().widgets.inactive.bg_fill = theme.surfaces.surface1;
                        ui.visuals_mut().widgets.inactive.bg_stroke =
                            Stroke::new(1.0, theme.borders.border);
                        ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(5);
                        ui.visuals_mut().widgets.hovered.bg_fill = theme.surfaces.surface1;
                        ui.visuals_mut().widgets.hovered.bg_stroke =
                            Stroke::new(1.0, theme.borders.border_focused);
                        ui.visuals_mut().widgets.hovered.corner_radius = CornerRadius::same(5);
                        ui.visuals_mut().widgets.active.bg_fill = theme.surfaces.surface1;
                        ui.visuals_mut().widgets.active.bg_stroke =
                            Stroke::new(1.0, theme.borders.border_focused);
                        ui.visuals_mut().widgets.active.corner_radius = CornerRadius::same(5);
                        ui.visuals_mut().widgets.open.bg_fill = theme.surfaces.surface1;
                        ui.visuals_mut().widgets.open.bg_stroke =
                            Stroke::new(1.5, theme.borders.border_focused);
                        ui.visuals_mut().widgets.open.corner_radius = CornerRadius::same(5);

                        egui::ComboBox::from_id_salt("key_customizer_context_combo")
                            .selected_text(
                                egui::RichText::new(format_context_path(state.pending_context))
                                    .size(12.5)
                                    .color(theme.text.primary),
                            )
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for &ctx_variant in KeyContext::all() {
                                    let label = format_context_path(ctx_variant);
                                    ui.selectable_value(
                                        &mut state.pending_context,
                                        ctx_variant,
                                        egui::RichText::new(label)
                                            .size(12.0)
                                            .color(theme.text.primary),
                                    );
                                }
                            });
                    });

                    ui.add_space(18.0);

                    // 4. Modal Footer: Cancel & Save (Aligned Bottom-Right)
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if AppButton::new()
                                .label("Save")
                                .variant(ButtonVariant::Default)
                                .tooltip("Save and apply this keybinding")
                                .show(ui)
                                .clicked()
                            {
                                save_dialog = true;
                            }

                            ui.add_space(8.0);

                            if AppButton::new()
                                .label("Cancel")
                                .variant(ButtonVariant::Ghost)
                                .tooltip("Cancel changes (Esc)")
                                .show(ui)
                                .clicked()
                            {
                                close_dialog = true;
                            }
                        });
                    });
                });
        });

    if save_dialog {
        let target_ctx = state.pending_context;
        if let Some(ks) = state.pending_keystroke {
            if target_ctx != context {
                state.draft.remove_action_binding(action, context);
            }
            state.draft.bind_keystroke(ks, action.clone(), target_ctx);
        }
        state.recording = None;
        state.pending_keystroke = None;
        state.is_recording_keystroke = false;
        state.pending_context = KeyContext::Global;
    } else if close_dialog {
        state.recording = None;
        state.pending_keystroke = None;
        state.is_recording_keystroke = false;
        state.pending_context = KeyContext::Global;
    } else if let Some(shortcut_label) = filter_by_shortcut {
        state.search_query = format!("\"{}\"", shortcut_label);
        state.recording = None;
        state.pending_keystroke = None;
        state.is_recording_keystroke = false;
        state.pending_context = KeyContext::Global;
    }
}
