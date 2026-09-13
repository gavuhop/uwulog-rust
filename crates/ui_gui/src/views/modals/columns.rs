use crate::actions::AppAction;
use crate::components::render_card;
use crate::state::ColumnState;
use crate::theme;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Rounding, Stroke};

pub fn render_columns_modal(
    ctx: &egui::Context,
    columns: &mut ColumnState,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !columns.is_modal_open {
        return;
    }

    if columns.draft_columns.is_none() {
        columns.draft_columns = Some(columns.columns.clone());
    }

    let mut action_to_dispatch: Option<AppAction> = None;
    let mut should_reset_defaults = false;

    let resp = crate::components::ui::ModalContainer::new(
        "columns_modal_window",
        "📊 Table Columns & Ordering",
    )
    .subtitle("Click & drag ⠿ items up or down to reorder columns. Toggle checkboxes to show/hide.")
    .width(520.0)
    .show(
        ctx,
        |ui| {
            // Filter search box
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("🔍")
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
                let avail_w = ui.available_width();
                let clear_btn_w = if columns.filter_query.is_empty() {
                    0.0
                } else {
                    28.0
                };
                let text_w = (avail_w - clear_btn_w - 6.0).max(100.0);

                ui.add_sized(
                    [text_w, 22.0],
                    egui::TextEdit::singleline(&mut columns.filter_query)
                        .hint_text("Filter column keys...")
                        .font(egui::TextStyle::Monospace)
                        .margin(egui::Margin::symmetric(8.0, 4.0)),
                );
                if !columns.filter_query.is_empty()
                    && crate::components::ui::IconButton::new("✖")
                        .size(20.0)
                        .tooltip("Clear filter")
                        .show(ui)
                        .clicked()
                {
                    columns.filter_query.clear();
                }
            });

            ui.add_space(8.0);

            // Columns Drag & Drop List Card
            render_card(ui, "Columns List (Drag to Reorder)", |ui| {
                let filter_lower = columns.filter_query.trim().to_lowercase();
                let total_cols = columns.draft_columns.as_ref().map_or(0, |c| c.len());

                let pointer_pos = ui.ctx().pointer_latest_pos();
                let pointer_released = ui.input(|i| i.pointer.any_released());

                if pointer_released {
                    columns.dragged_index = None;
                }

                egui::ScrollArea::vertical()
                    .id_salt("columns_modal_scroll_area")
                    .max_height(340.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut new_drag_source = None;
                        let mut target_drop = None;
                        let mut toggle_vis = None;

                        for idx in 0..total_cols {
                            let (col_name, col_visible) = {
                                let item = &columns.draft_columns.as_ref().unwrap()[idx];
                                (item.name.clone(), item.visible)
                            };

                            if !filter_lower.is_empty()
                                && !col_name.to_lowercase().contains(&filter_lower)
                            {
                                continue;
                            }

                            let is_dragging_this = columns.dragged_index == Some(idx);
                            let desired_size = egui::vec2(ui.available_width(), 30.0);
                            let (rect, resp) =
                                ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

                            // Detect drag started
                            if resp.drag_started() {
                                new_drag_source = Some(idx);
                            }

                            // Detect drop target while dragging
                            if let Some(dragged_idx) = columns.dragged_index {
                                if dragged_idx != idx {
                                    if let Some(pos) = pointer_pos {
                                        if rect.contains(pos) {
                                            target_drop = Some((dragged_idx, idx));
                                        }
                                    }
                                }
                            }

                            // Cursor icon
                            if resp.hovered() || is_dragging_this {
                                if is_dragging_this {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                } else {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                }
                            }

                            // Background and border styling
                            let bg_color = if is_dragging_this {
                                theme::BG_ROW_SELECTED
                            } else if resp.hovered() {
                                theme::BG_ROW_HOVER
                            } else {
                                theme::BG_BASE
                            };

                            let border_stroke = if is_dragging_this {
                                Stroke::new(1.5, theme::TEXT_KEY)
                            } else if resp.hovered() {
                                Stroke::new(1.0, theme::BG_SURFACE1)
                            } else {
                                Stroke::new(1.0, theme::BG_SURFACE0)
                            };

                            ui.painter()
                                .rect(rect, Rounding::same(4.0), bg_color, border_stroke);

                            let center_y = rect.center().y;

                            // 1. Drag Grip Icon (⠿)
                            let grip_x = rect.min.x + 10.0;
                            ui.painter().text(
                                Pos2::new(grip_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                "⠿",
                                FontId::monospace(14.0),
                                if is_dragging_this || resp.hovered() {
                                    theme::TEXT_KEY
                                } else {
                                    theme::TEXT_MUTED
                                },
                            );

                            // 2. Custom Checkbox
                            let checkbox_x = grip_x + 22.0;
                            let check_rect = Rect::from_center_size(
                                Pos2::new(checkbox_x + 8.0, center_y),
                                egui::vec2(16.0, 16.0),
                            );

                            let check_resp = ui.interact(
                                check_rect,
                                ui.make_persistent_id(format!("chk_{idx}_{col_name}")),
                                egui::Sense::click(),
                            );

                            if check_resp.clicked() {
                                toggle_vis = Some((idx, !col_visible));
                            }

                            let check_bg = if col_visible {
                                theme::TEXT_KEY
                            } else {
                                Color32::TRANSPARENT
                            };
                            let check_stroke = Stroke::new(
                                1.0,
                                if col_visible {
                                    theme::TEXT_KEY
                                } else {
                                    theme::TEXT_MUTED
                                },
                            );

                            ui.painter().rect(
                                check_rect,
                                Rounding::same(3.0),
                                check_bg,
                                check_stroke,
                            );

                            if col_visible {
                                ui.painter().text(
                                    check_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "✓",
                                    FontId::monospace(11.0),
                                    theme::BG_BASE,
                                );
                            }

                            // 3. Raw Key Name
                            let text_x = checkbox_x + 24.0;
                            let text_color = if col_visible {
                                theme::TEXT_PRIMARY
                            } else {
                                theme::TEXT_MUTED
                            };
                            ui.painter().text(
                                Pos2::new(text_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                &col_name,
                                FontId::monospace(12.0),
                                text_color,
                            );

                            // Also toggle visibility on clicking name area if not dragging
                            if resp.clicked() && !check_resp.clicked() {
                                toggle_vis = Some((idx, !col_visible));
                            }

                            // 4. Position / Status badge on the right
                            let right_x = rect.max.x - 10.0;
                            if col_visible {
                                let pos_text = format!("Pos #{}", idx + 1);
                                ui.painter().text(
                                    Pos2::new(right_x, center_y),
                                    egui::Align2::RIGHT_CENTER,
                                    pos_text,
                                    FontId::monospace(11.0),
                                    theme::COLOR_INFO,
                                );
                            } else {
                                ui.painter().text(
                                    Pos2::new(right_x, center_y),
                                    egui::Align2::RIGHT_CENTER,
                                    "Hidden",
                                    FontId::monospace(11.0),
                                    theme::TEXT_MUTED,
                                );
                            }
                        }

                        if let Some(idx) = new_drag_source {
                            columns.dragged_index = Some(idx);
                        }

                        if let Some((from, to)) = target_drop {
                            columns.reorder_draft(from, to);
                            columns.dragged_index = Some(to);
                            ui.ctx().request_repaint();
                        }

                        if let Some((idx, new_vis)) = toggle_vis {
                            if let Some(draft) = &mut columns.draft_columns {
                                draft[idx].visible = new_vis;
                            }
                        }
                    });
            });
        },
        Some(|ui: &mut egui::Ui, close_req: &mut bool| {
            if crate::components::ui::AppButton::new()
                .label("Reset Defaults")
                .icon("🔄")
                .tooltip("Reset draft column order and visibility to default")
                .show(ui)
                .clicked()
            {
                should_reset_defaults = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::components::ui::AppButton::new()
                    .label("Apply & Done")
                    .icon("✔")
                    .variant(crate::components::ui::ButtonVariant::Success)
                    .show(ui)
                    .clicked()
                {
                    action_to_dispatch = Some(AppAction::ApplyColumnsModal);
                }

                ui.add_space(6.0);

                if crate::components::ui::AppButton::new()
                    .label("Cancel")
                    .show(ui)
                    .clicked()
                {
                    *close_req = true;
                }
            });
        }),
    );

    if should_reset_defaults {
        columns.reset_draft_to_defaults();
    }

    if resp.closed {
        action_to_dispatch = Some(AppAction::CloseColumnsModal);
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}
