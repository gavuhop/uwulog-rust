use crate::actions::AppAction;
use crate::components::render_card;
use crate::state::ColumnState;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Stroke};

pub fn render_columns_modal(
    ctx: &egui::Context,
    columns: &mut ColumnState,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !columns.is_modal_open {
        return;
    }

    let theme = ctx.app_theme();

    if columns.draft_columns.is_none() {
        columns.draft_columns = Some(columns.columns.clone());
    }

    let mut action_to_dispatch: Option<AppAction> = None;
    let mut should_reset_defaults = false;

    let resp = crate::components::ui::ModalContainer::new(
        "columns_modal_window",
        "Table Columns & Ordering",
    )
    .subtitle("Click & drag items to reorder columns. Toggle checkboxes to show/hide.")
    .width(560.0)
    .min_height(480.0)
    .show(
        ctx,
        |ui| {
            // Filter search box
            ui.horizontal(|ui| {
                let avail_w = ui.available_width();
                let clear_btn_w = if columns.filter_query.is_empty() {
                    0.0
                } else {
                    28.0
                };
                let text_w = (avail_w - clear_btn_w - 6.0).max(100.0);
                let search_id = egui::Id::new("columns_modal_filter_input");
                crate::components::ui::TextInput::new(&mut columns.filter_query)
                    .id(search_id)
                    .auto_focus(true)
                    .hint_text("Filter column keys...")
                    .width(text_w)
                    .show(ui);
                if !columns.filter_query.is_empty()
                    && crate::components::ui::IconButton::new(
                        crate::components::ui::IconName::Close,
                    )
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
                    theme.log.row_hover
                } else {
                    theme.surfaces.surface0
                };
                ui.painter().rect(
                    h_rect,
                    CornerRadius::same(4),
                    h_bg,
                    Stroke::new(1.0, theme.surfaces.surface1),
                    egui::StrokeKind::Inside,
                );

                let h_center_y = h_rect.center().y;

                // Header icon (Grip indicator)
                let grip_x = h_rect.min.x + 8.0;
                let header_grip_rect = Rect::from_center_size(
                    Pos2::new(h_rect.min.x + 12.0, h_center_y),
                    egui::vec2(12.0, 12.0),
                );
                crate::components::ui::IconName::GripVertical.paint(
                    ui.painter(),
                    header_grip_rect,
                    theme.text.muted,
                );

                // Master Checkbox at same X as row checkboxes (grip_x + 20.0 = h_rect.min.x + 28.0)
                let checkbox_x = grip_x + 20.0;
                let check_rect = Rect::from_center_size(
                    Pos2::new(checkbox_x + 8.0, h_center_y),
                    egui::vec2(16.0, 16.0),
                );

                let (chk_bg, chk_stroke) = if all_matching_visible || is_partial {
                    (theme.text.accent, Stroke::new(1.0, theme.text.accent))
                } else {
                    (Color32::TRANSPARENT, Stroke::new(1.0, theme.text.muted))
                };

                ui.painter().rect(
                    check_rect,
                    CornerRadius::same(3),
                    chk_bg,
                    chk_stroke,
                    egui::StrokeKind::Inside,
                );

                if all_matching_visible {
                    crate::components::ui::IconName::Check.paint(
                        ui.painter(),
                        check_rect.shrink(2.5),
                        theme.surfaces.base,
                    );
                } else if is_partial {
                    crate::components::ui::IconName::Dash.paint(
                        ui.painter(),
                        check_rect.shrink(2.5),
                        theme.surfaces.base,
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
                        theme.text.primary
                    } else {
                        theme.text.muted
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
                    theme.status.info,
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

                // Nếu nhả chuột: dọn dẹp drag state
                if pointer_released {
                    columns.clear_modal_drag();
                }

                egui::ScrollArea::vertical()
                    .id_salt("columns_modal_scroll_area")
                    .min_scrolled_height(300.0)
                    .max_height(420.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut new_drag_source = None;
                        let mut toggle_vis = None;
                        let mut row_rects: Vec<(usize, Rect, String, bool)> = Vec::new();

                        let item_h = 32.0;
                        let spacing = 3.0;

                        // Bước 1: Thu thập toạ độ base của từng dòng matching
                        for &idx in &matching_indices {
                            let (col_name, col_visible) = {
                                let item = &columns.draft_columns.as_ref().unwrap()[idx];
                                (item.name.clone(), item.visible)
                            };

                            let desired_size = egui::vec2(ui.available_width(), item_h);
                            let (rect, resp) =
                                ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

                            if resp.drag_started() {
                                new_drag_source = Some(idx);
                            }

                            row_rects.push((idx, rect, col_name, col_visible));
                            ui.add_space(spacing);
                        }

                        // Kiểm tra và hoán đổi vị trí trực tiếp (Live Swap / Thay thế)
                        check_and_apply_modal_swap(
                            columns,
                            &matching_indices,
                            &row_rects,
                            pointer_pos,
                        );

                        let now = std::time::Instant::now();

                        // Bước 2: Render từng dòng (áp dụng position interpolation animation)
                        for (idx, rect, col_name, col_visible) in row_rects {
                            let is_hovered = pointer_pos.is_some_and(|p| rect.contains(p))
                                && columns.dragged_index.is_none();

                            let is_dragged = columns.dragged_index == Some(idx);
                            let draw_rect =
                                columns.modal_animations.track_rect(&col_name, rect, now);

                            let bg_color = if is_dragged {
                                theme.log.row_selected
                            } else if is_hovered {
                                theme.log.row_hover
                            } else {
                                theme.surfaces.base
                            };

                            let border_stroke = if is_dragged {
                                Stroke::new(1.5, theme.text.accent)
                            } else if is_hovered {
                                Stroke::new(1.0, theme.surfaces.surface1)
                            } else {
                                Stroke::new(1.0, theme.surfaces.surface0)
                            };

                            ui.painter().rect(
                                draw_rect,
                                CornerRadius::same(4),
                                bg_color,
                                border_stroke,
                                egui::StrokeKind::Inside,
                            );

                            let center_y = draw_rect.center().y;

                            // 1. Drag Grip Icon (Vector handle)
                            let grip_color = if is_dragged || is_hovered {
                                theme.text.accent
                            } else {
                                theme.text.muted
                            };
                            let grip_rect = Rect::from_center_size(
                                Pos2::new(draw_rect.min.x + 12.0, center_y),
                                egui::vec2(12.0, 12.0),
                            );
                            crate::components::ui::IconName::GripVertical.paint(
                                ui.painter(),
                                grip_rect,
                                grip_color,
                            );

                            // 2. Custom Checkbox
                            let row_grip_x = draw_rect.min.x + 8.0;
                            let checkbox_x = row_grip_x + 20.0;
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
                                theme.text.accent
                            } else {
                                Color32::TRANSPARENT
                            };
                            let check_stroke = Stroke::new(
                                1.0,
                                if col_visible {
                                    theme.text.accent
                                } else {
                                    theme.text.muted
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
                                crate::components::ui::IconName::Check.paint(
                                    ui.painter(),
                                    check_rect.shrink(2.5),
                                    theme.surfaces.base,
                                );
                            }

                            // 3. Raw Key Name
                            let text_x = checkbox_x + 24.0;
                            let text_color = if col_visible {
                                theme.text.primary
                            } else {
                                theme.text.muted
                            };
                            ui.painter().text(
                                Pos2::new(text_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                &col_name,
                                FontId::monospace(12.0),
                                text_color,
                            );

                            // Click row to toggle checkbox
                            let row_resp = ui.interact(
                                draw_rect,
                                ui.make_persistent_id(format!("row_interact_{idx}_{col_name}")),
                                egui::Sense::click(),
                            );
                            if row_resp.clicked() && !check_resp.clicked() {
                                toggle_vis = Some((idx, !col_visible));
                            }

                            if is_dragged {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            } else if is_hovered {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                            }

                            // 4. Status Badge
                            let right_x = draw_rect.max.x - 8.0;
                            if col_visible {
                                let pos_text = format!("Pos #{}", idx + 1);
                                ui.painter().text(
                                    Pos2::new(right_x, center_y),
                                    egui::Align2::RIGHT_CENTER,
                                    pos_text,
                                    FontId::monospace(11.0),
                                    theme.status.info,
                                );
                            } else {
                                ui.painter().text(
                                    Pos2::new(right_x, center_y),
                                    egui::Align2::RIGHT_CENTER,
                                    "Hidden",
                                    FontId::monospace(11.0),
                                    theme.text.muted,
                                );
                            }
                        }

                        // Yêu cầu repaint liên tục khi đang có animation và dọn dẹp sau khi xong
                        columns.modal_animations.update(ui.ctx(), now);

                        if let Some(idx) = new_drag_source {
                            columns.dragged_index = Some(idx);
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
                .icon(crate::components::ui::IconName::Restart)
                .tooltip("Reset draft column order and visibility to default")
                .show(ui)
                .clicked()
            {
                should_reset_defaults = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::components::ui::AppButton::new()
                    .label("Apply & Done")
                    .icon(crate::components::ui::IconName::Check)
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

/// Kiểm tra xem item đang kéo có đang di chuyển qua vị trí của item khác trong modal không.
/// Nếu có, lập tức hoán đổi vị trí trực tiếp (Live Swap) trong `draft_columns`.
fn check_and_apply_modal_swap(
    columns: &mut ColumnState,
    matching_indices: &[usize],
    row_rects: &[(usize, Rect, String, bool)],
    pointer_pos: Option<egui::Pos2>,
) {
    let (Some(drag_idx), Some(pointer)) = (columns.dragged_index, pointer_pos) else {
        return;
    };

    let Some(from_match_pos) = matching_indices.iter().position(|&i| i == drag_idx) else {
        return;
    };

    let threshold = 6.0;

    // 1. Kéo xuống hàng dưới (swap/thay thế hàng bên dưới)
    if from_match_pos + 1 < row_rects.len() {
        let next_rect = row_rects[from_match_pos + 1].1;
        if pointer.y > next_rect.min.y + threshold {
            let to_idx = matching_indices[from_match_pos + 1];
            if let Some(draft) = &mut columns.draft_columns {
                draft.swap(drag_idx, to_idx);
                columns.dragged_index = Some(to_idx);
            }
            return;
        }
    }

    // 2. Kéo lên hàng trên (swap/thay thế hàng bên trên)
    if from_match_pos > 0 {
        let prev_rect = row_rects[from_match_pos - 1].1;
        if pointer.y < prev_rect.max.y - threshold {
            let to_idx = matching_indices[from_match_pos - 1];
            if let Some(draft) = &mut columns.draft_columns {
                draft.swap(drag_idx, to_idx);
                columns.dragged_index = Some(to_idx);
            }
        }
    }
}
