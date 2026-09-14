use crate::actions::AppAction;
use crate::components::render_card;
use crate::state::ColumnState;
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Stroke};

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
    .subtitle("Click & drag ⠿ items to reorder columns. Toggle checkboxes to show/hide.")
    .width(560.0)
    .min_height(480.0)
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
                crate::components::ui::TextInput::new(&mut columns.filter_query)
                    .hint_text("Filter column keys...")
                    .width(text_w)
                    .show(ui);
                if !columns.filter_query.is_empty()
                    && crate::components::ui::IconButton::new("✖")
                        .size(24.0)
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

                // Identify matching columns and their visibility
                let matching_indices: Vec<usize> = if let Some(draft) = &columns.draft_columns {
                    draft
                        .iter()
                        .enumerate()
                        .filter(|(_, col)| {
                            filter_lower.is_empty()
                                || col.name.to_lowercase().contains(&filter_lower)
                        })
                        .map(|(i, _)| i)
                        .collect()
                } else {
                    Vec::new()
                };

                let matching_count = matching_indices.len();
                let visible_matching = if let Some(draft) = &columns.draft_columns {
                    matching_indices
                        .iter()
                        .filter(|&&i| draft[i].visible)
                        .count()
                } else {
                    0
                };

                let total_visible = columns
                    .draft_columns
                    .as_ref()
                    .map_or(0, |draft| draft.iter().filter(|c| c.visible).count());

                let all_matching_visible = matching_count > 0 && visible_matching == matching_count;
                let is_partial = !all_matching_visible && visible_matching > 0;

                // --- Master Header Row (Tick All / Untick All) ---
                let header_h = 30.0;
                let header_size = egui::vec2(ui.available_width(), header_h);
                let (h_rect, h_resp) = ui.allocate_exact_size(header_size, egui::Sense::click());

                if h_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                let h_bg = if h_resp.hovered() {
                    theme::BG_ROW_HOVER
                } else {
                    theme::BG_SURFACE0
                };
                ui.painter().rect(
                    h_rect,
                    CornerRadius::same(4),
                    h_bg,
                    Stroke::new(1.0, theme::BG_SURFACE1),
                    egui::StrokeKind::Inside,
                );

                let h_center_y = h_rect.center().y;

                // Header icon (⇅)
                let grip_x = h_rect.min.x + 8.0;
                ui.painter().text(
                    Pos2::new(grip_x, h_center_y),
                    egui::Align2::LEFT_CENTER,
                    "⇅",
                    FontId::monospace(13.0),
                    theme::TEXT_MUTED,
                );

                // Master Checkbox at same X as row checkboxes (grip_x + 20.0 = h_rect.min.x + 28.0)
                let checkbox_x = grip_x + 20.0;
                let check_rect = Rect::from_center_size(
                    Pos2::new(checkbox_x + 8.0, h_center_y),
                    egui::vec2(16.0, 16.0),
                );

                let (chk_bg, chk_stroke) = if all_matching_visible || is_partial {
                    (theme::TEXT_KEY, Stroke::new(1.0, theme::TEXT_KEY))
                } else {
                    (Color32::TRANSPARENT, Stroke::new(1.0, theme::TEXT_MUTED))
                };

                ui.painter().rect(
                    check_rect,
                    CornerRadius::same(3),
                    chk_bg,
                    chk_stroke,
                    egui::StrokeKind::Inside,
                );

                if all_matching_visible {
                    ui.painter().text(
                        check_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "✔",
                        FontId::monospace(11.0),
                        theme::BG_BASE,
                    );
                } else if is_partial {
                    ui.painter().text(
                        check_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "-",
                        FontId::monospace(13.0),
                        theme::BG_BASE,
                    );
                }

                // Label next to checkbox
                let text_x = checkbox_x + 24.0;
                let label_text = if !filter_lower.is_empty() {
                    if all_matching_visible {
                        "Untick all filtered"
                    } else {
                        "Tick all filtered"
                    }
                } else if all_matching_visible {
                    "Untick all columns"
                } else {
                    "Tick all columns"
                };

                ui.painter().text(
                    Pos2::new(text_x, h_center_y),
                    egui::Align2::LEFT_CENTER,
                    label_text,
                    FontId::monospace(12.0),
                    if all_matching_visible || is_partial {
                        theme::TEXT_PRIMARY
                    } else {
                        theme::TEXT_MUTED
                    },
                );

                // Right side: status count
                let right_x = h_rect.max.x - 8.0;
                let counter_text = if !filter_lower.is_empty() {
                    format!("{visible_matching}/{matching_count} filtered ({total_visible} total)")
                } else {
                    format!("{total_visible}/{total_cols} visible")
                };
                ui.painter().text(
                    Pos2::new(right_x, h_center_y),
                    egui::Align2::RIGHT_CENTER,
                    counter_text,
                    FontId::monospace(11.0),
                    theme::COLOR_INFO,
                );

                if h_resp.clicked() {
                    let target_state = !all_matching_visible;
                    if let Some(draft) = &mut columns.draft_columns {
                        for &idx in &matching_indices {
                            draft[idx].visible = target_state;
                        }
                    }
                }

                h_resp.on_hover_text(if all_matching_visible {
                    "Click to untick all"
                } else {
                    "Click to tick all"
                });

                ui.add_space(4.0);

                let pointer_pos = ui.ctx().pointer_latest_pos();
                let pointer_released = ui.input(|i| i.pointer.any_released());

                if pointer_released {
                    columns.dragged_index = None;
                }

                egui::ScrollArea::vertical()
                    .id_salt("columns_modal_scroll_area")
                    .min_scrolled_height(300.0)
                    .max_height(420.0)
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
                            let desired_size = egui::vec2(ui.available_width(), 32.0);
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

                            // Cursor icon: PointingHand on hover, Grabbing while dragging
                            if resp.hovered() || is_dragging_this {
                                if is_dragging_this {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                } else {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
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

                            ui.painter().rect(
                                rect,
                                CornerRadius::same(4),
                                bg_color,
                                border_stroke,
                                egui::StrokeKind::Inside,
                            );

                            let center_y = rect.center().y;

                            // 1. Drag Grip Icon (Vector 6-dot handle)
                            let grip_color = if is_dragging_this || resp.hovered() {
                                theme::TEXT_KEY
                            } else {
                                theme::TEXT_MUTED
                            };
                            theme::draw_drag_handle(
                                ui.painter(),
                                Pos2::new(rect.min.x + 12.0, center_y),
                                grip_color,
                            );

                            // 2. Custom Checkbox
                            let checkbox_x = grip_x + 20.0;
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
                                CornerRadius::same(3),
                                check_bg,
                                check_stroke,
                                egui::StrokeKind::Inside,
                            );

                            if col_visible {
                                ui.painter().text(
                                    check_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "✔",
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

                            // Toggle visibility on clicking row (excluding checkbox which handles its own click)
                            if resp.clicked() && !check_resp.clicked() {
                                toggle_vis = Some((idx, !col_visible));
                            }

                            // 4. Position / Status badge on the right
                            let right_x = rect.max.x - 8.0;
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

                            ui.add_space(3.0);
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
                    .tooltip("Apply column visibility and ordering changes")
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
