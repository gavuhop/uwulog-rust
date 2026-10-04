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
        state.context_text.clear();
        state.context_autocomplete_open = false;
        state.context_selected_index = 0;
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
                                        format_context_path(&b.context)
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

                    // 3. Section: Edit Context (Zed-style Editable Context Input with Autocomplete)
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Edit Context")
                                .size(12.5)
                                .color(theme.text.primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("Ctrl+Space: Suggestions")
                                    .size(10.5)
                                    .color(theme.text.muted),
                            );
                        });
                    });
                    ui.add_space(6.0);

                    // Thu thập danh sách contexts đã biết (built-in + các context tùy biến hiện có trong keymap)
                    let mut known_contexts: Vec<String> = state
                        .draft
                        .active_contexts()
                        .into_iter()
                        .map(|c| c.display_path().to_string())
                        .collect();
                    known_contexts.sort();
                    known_contexts.dedup();

                    let query = state.context_text.trim().to_lowercase();
                    let candidates: Vec<&String> = if query.is_empty() {
                        known_contexts.iter().collect()
                    } else {
                        known_contexts
                            .iter()
                            .filter(|c| c.to_lowercase().contains(&query))
                            .collect()
                    };

                    let mut toggle_autocomplete = false;
                    let mut select_candidate = None;

                    // Textbox nhập liệu context + nút toggle dropdown
                    let input_w = ui.available_width();
                    let input_h = 30.0;

                    ui.horizontal(|ui| {
                        let text_w = (input_w - 30.0).max(120.0);
                        let edit_resp = ui.add_sized(
                            Vec2::new(text_w, input_h),
                            egui::TextEdit::singleline(&mut state.context_text)
                                .hint_text("e.g. Workspace > Table")
                                .font(FontId::proportional(12.5))
                                .margin(egui::Margin::symmetric(8, 6)),
                        );

                        if edit_resp.changed() {
                            state.context_autocomplete_open = true;
                            state.context_selected_index = 0;
                        }

                        let chevron = if state.context_autocomplete_open { "▴" } else { "▾" };
                        let toggle_btn = ui.add_sized(
                            Vec2::new(24.0, input_h),
                            egui::Button::new(
                                egui::RichText::new(chevron)
                                    .size(11.0)
                                    .color(theme.text.muted),
                            )
                            .corner_radius(CornerRadius::same(5)),
                        );
                        if toggle_btn.clicked() {
                            toggle_autocomplete = true;
                        }
                    });

                    // Kiểm tra cú pháp và ngữ cảnh tức thì (Real-time Context Validation)
                    let trimmed_ctx = state.context_text.trim();
                    let (syntax_valid, syntax_err, parsed_ctx) = if trimmed_ctx.is_empty() {
                        (true, None, KeyContext::Global)
                    } else {
                        match KeyContext::parse_normalized(trimmed_ctx) {
                            Ok(ctx) => (true, None, ctx),
                            Err(err) => (false, Some(err.to_string()), KeyContext::new(trimmed_ctx)),
                        }
                    };

                    // Phản hồi kiểm tra cú pháp và cảnh báo context không xác định (Zed-style Feedback)
                    if !syntax_valid {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("⚠")
                                    .size(11.0)
                                    .color(theme.status.error),
                            );
                            if let Some(ref err) = syntax_err {
                                ui.label(
                                    egui::RichText::new(format!("Syntax error: {}", err))
                                        .size(11.0)
                                        .color(theme.status.error),
                                );
                            }
                        });
                    } else if !trimmed_ctx.is_empty() {
                        let is_known = parsed_ctx.is_builtin()
                            || state.draft.bindings().iter().any(|b| b.context == parsed_ctx);
                        if !is_known {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("ℹ")
                                        .size(11.0)
                                        .color(theme.status.warning),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "Custom context: won't trigger until a view registers this tag",
                                    )
                                    .size(11.0)
                                    .color(theme.status.warning),
                                );
                            });
                        }
                    }

                    // Xử lý phím tắt cho Autocomplete (Ctrl+Space, Arrow Up/Down, Enter, Tab, Esc)
                    let ctrl_space = ctx.input_mut(|i| {
                        i.consume_key(egui::Modifiers::CTRL, egui::Key::Space)
                    });
                    if ctrl_space || toggle_autocomplete {
                        state.context_autocomplete_open = !state.context_autocomplete_open;
                        state.context_selected_index = 0;
                    }

                    if state.context_autocomplete_open && !candidates.is_empty() {
                        let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                        let down = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                        let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                        let tab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
                        let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));

                        if esc {
                            state.context_autocomplete_open = false;
                        } else if down {
                            state.context_selected_index = (state.context_selected_index + 1) % candidates.len();
                        } else if up {
                            state.context_selected_index = if state.context_selected_index == 0 {
                                candidates.len().saturating_sub(1)
                            } else {
                                state.context_selected_index - 1
                            };
                        } else if enter || tab {
                            if let Some(cand) = candidates.get(state.context_selected_index) {
                                select_candidate = Some((*cand).clone());
                            }
                        }
                    }

                    // Render popover gợi ý Autocomplete bên dưới ô nhập
                    if state.context_autocomplete_open {
                        ui.add_space(3.0);
                        egui::Frame::default()
                            .fill(theme.surfaces.surface1)
                            .stroke(Stroke::new(1.0, theme.borders.border_focused))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(4, 4))
                            .show(ui, |ui| {
                                if candidates.is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.add_space(6.0);
                                        ui.label(
                                            egui::RichText::new("No matching contexts (custom will be used)")
                                                .size(11.0)
                                                .italics()
                                                .color(theme.text.muted),
                                        );
                                    });
                                } else {
                                    egui::ScrollArea::vertical()
                                        .max_height(140.0)
                                        .show(ui, |ui| {
                                            for (idx, cand) in candidates.iter().enumerate() {
                                                let is_selected = idx == state.context_selected_index;
                                                let text_color = if is_selected {
                                                    theme.text.accent
                                                } else {
                                                    theme.text.primary
                                                };

                                                let (cand_rect, cand_resp) = ui.allocate_exact_size(
                                                    Vec2::new(ui.available_width(), 22.0),
                                                    egui::Sense::click(),
                                                );

                                                if is_selected || cand_resp.hovered() {
                                                    ui.painter().rect_filled(
                                                        cand_rect,
                                                        CornerRadius::same(4),
                                                        theme.surfaces.surface0,
                                                    );
                                                }

                                                // Context name bên trái
                                                ui.painter().text(
                                                    Pos2::new(cand_rect.min.x + 8.0, cand_rect.center().y),
                                                    egui::Align2::LEFT_CENTER,
                                                    cand.as_str(),
                                                    FontId::proportional(11.5),
                                                    text_color,
                                                );

                                                // Badge [Built-in] / [Custom] bên phải
                                                let is_builtin = KeyContext::all().iter().any(|b| {
                                                    b.display_path() == cand.as_str() || b.as_str() == cand.as_str()
                                                });
                                                let (badge_text, badge_color) = if is_builtin {
                                                    ("Built-in", theme.text.muted)
                                                } else {
                                                    ("Custom", theme.text.accent)
                                                };

                                                ui.painter().text(
                                                    Pos2::new(cand_rect.max.x - 8.0, cand_rect.center().y),
                                                    egui::Align2::RIGHT_CENTER,
                                                    badge_text,
                                                    FontId::proportional(10.0),
                                                    badge_color,
                                                );

                                                if cand_resp.clicked() {
                                                    select_candidate = Some((*cand).clone());
                                                }
                                            }
                                        });
                                }
                            });
                    }

                    if let Some(chosen) = select_candidate {
                        state.context_text = chosen;
                        state.context_autocomplete_open = false;
                    }

                    ui.add_space(18.0);

                    // 4. Modal Footer: Cancel & Save (Aligned Bottom-Right)
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let save_tooltip = if syntax_valid {
                                "Save and apply this keybinding"
                            } else {
                                "Cannot save: context syntax is invalid"
                            };

                            let save_resp = AppButton::new()
                                .label("Save")
                                .variant(if syntax_valid {
                                    ButtonVariant::Default
                                } else {
                                    ButtonVariant::Ghost
                                })
                                .tooltip(save_tooltip)
                                .show(ui);

                            if save_resp.clicked() && syntax_valid {
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
        let trimmed_ctx = state.context_text.trim();
        let target_ctx = if trimmed_ctx.is_empty() {
            KeyContext::Global
        } else {
            KeyContext::parse_normalized(trimmed_ctx)
                .unwrap_or_else(|_| KeyContext::new(trimmed_ctx))
        };

        if let Some(ks) = state.pending_keystroke {
            if target_ctx != context {
                state.draft.remove_action_binding(action, &context);
            }
            state.draft.bind_keystroke(ks, action.clone(), target_ctx);
        }
        state.close_customizer();
    } else if close_dialog {
        state.close_customizer();
    } else if let Some(shortcut_label) = filter_by_shortcut {
        state.search_query = format!("\"{}\"", shortcut_label);
        state.close_customizer();
    }
}
