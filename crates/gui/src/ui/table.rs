use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Color32, FontId, Pos2, Rounding, Stroke};
use egui_extras::{Column, TableBuilder};
use uwu_core::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
    let row_count = app.cached_logs.len();

    let not_focusing_text = !ui.memory(|m| m.focused().is_some());

    // Phím End: Latch auto-scroll và cuộn ngay xuống dòng mới nhất (khi không focus vào ô nhập text)
    let end_key_pressed = ui.input(|i| i.key_pressed(egui::Key::End)) && not_focusing_text;
    if end_key_pressed {
        app.latch();
    }

    // Các phím điều hướng cuộn lên (PageUp, Home, ArrowUp) khi không gõ text -> tự động Unlatch
    if not_focusing_text {
        let scroll_up_keys = ui.input(|i| {
            i.key_pressed(egui::Key::PageUp)
                || i.key_pressed(egui::Key::Home)
                || i.key_pressed(egui::Key::ArrowUp)
        });
        if scroll_up_keys {
            app.unlatch();
        }
    }

    // Phát hiện cuộn chuột LÊN → tắt auto-scroll (Unlatch)
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        app.unlatch();
    }

    ui.visuals_mut().selection.bg_fill = theme::BG_ROW_SELECTED;
    ui.visuals_mut().selection.stroke = egui::Stroke::NONE;

    let pointer_pos = ui.ctx().pointer_latest_pos();
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    let pointer_released = ui.input(|i| i.pointer.any_released());

    if pointer_released || !pointer_down {
        app.column_state.header_dragged_name = None;
    }

    let mut new_header_drag = None;
    let mut target_header_swap = None;
    let mut last_row_visible = false;
    let mut newly_selected_event = None;

    let visible_cols: Vec<crate::ui::columns_modal::ColumnItem> = app
        .column_state
        .columns
        .iter()
        .filter(|c| c.visible)
        .cloned()
        .collect();

    // Định danh table salt theo thứ tự cột hiện tại để TableBuilder không áp dụng nhầm kích thước theo vị trí cũ
    let table_salt = format!(
        "log_tbl_{}",
        visible_cols
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );

    enum FilterAction {
        Apply(String),
        Exclude(String),
    }

    enum HighlightAction {
        ToggleRow(uuid::Uuid),
        ToggleTerm(String),
        ClearAll,
    }

    let mut filter_action: Option<FilterAction> = None;
    let mut highlight_action: Option<HighlightAction> = None;
    let has_any_highlights = app.has_any_highlights();

    // Cho phép cuộn ngang (Horizontal Scrolling) khi có nhiều cột hoặc tổng độ rộng các cột lớn hơn màn hình
    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut builder = TableBuilder::new(ui)
                .id_salt(table_salt)
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false]);

            for col in &visible_cols {
                let initial_w = if col.width >= 40.0 {
                    col.width
                } else if col.name == "message" {
                    350.0
                } else {
                    120.0
                };
                builder = builder.column(Column::initial(initial_w).at_least(40.0).clip(true));
            }

            // Cuộn xuống dòng cuối khi:
            // 1. Có request cuộn ngay (bấm nút Latch hoặc phím End) HOẶC
            // 2. Auto-scroll đang bật VÀ có log mới đến (row_count tăng)
            let has_new_data = row_count > app.prev_table_row_count;
            let force_scroll = app.request_scroll_to_bottom;
            if (force_scroll || (app.is_auto_scroll && has_new_data)) && row_count > 0 {
                builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
                app.request_scroll_to_bottom = false;
            }
            app.prev_table_row_count = row_count;

            builder
                .header(26.0, |mut header| {
                    for col in &visible_cols {
                        header.col(|ui| {
                            let is_dragged =
                                app.column_state.header_dragged_name.as_deref() == Some(&col.name);

                            let available_size = ui.available_size();
                            let (rect, resp) = ui
                                .allocate_exact_size(available_size, egui::Sense::click_and_drag());

                            // Cập nhật chiều rộng thực tế khi người dùng kéo thay đổi kích thước cột
                            let actual_width = rect.width();
                            if actual_width >= 40.0 && !is_dragged {
                                if let Some(col_item) = app
                                    .column_state
                                    .columns
                                    .iter_mut()
                                    .find(|c| c.name == col.name)
                                {
                                    col_item.width = actual_width;
                                }
                            }

                            if resp.drag_started() {
                                new_header_drag = Some(col.name.clone());
                            }

                            if let Some(ref dragged_name) = app.column_state.header_dragged_name {
                                if dragged_name != &col.name {
                                    if let Some(pos) = pointer_pos {
                                        let dragged_vis_idx = visible_cols
                                            .iter()
                                            .position(|c| c.name == *dragged_name);
                                        let current_vis_idx =
                                            visible_cols.iter().position(|c| c.name == col.name);

                                        if let (Some(drag_i), Some(curr_i)) =
                                            (dragged_vis_idx, current_vis_idx)
                                        {
                                            let cell_mid_x = rect.center().x;
                                            // Kéo sang PHẢI hoặc TRÁI: chỉ hoán đổi khi con trỏ đã vượt qua tâm cột đích
                                            let should_swap = (curr_i > drag_i
                                                && pos.x >= cell_mid_x)
                                                || (curr_i < drag_i && pos.x <= cell_mid_x);
                                            if should_swap {
                                                target_header_swap =
                                                    Some((dragged_name.clone(), col.name.clone()));
                                            }
                                        }
                                    }
                                }
                            }

                            if resp.hovered() || is_dragged {
                                if is_dragged {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                } else {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                }
                            }

                            // Background & border
                            let bg_color = if is_dragged {
                                theme::BG_ROW_SELECTED
                            } else if resp.hovered() {
                                theme::BG_ROW_HOVER
                            } else {
                                Color32::TRANSPARENT
                            };

                            let border_stroke = if is_dragged {
                                Stroke::new(1.5, theme::TEXT_KEY)
                            } else if resp.hovered() {
                                Stroke::new(1.0, theme::BG_SURFACE1)
                            } else {
                                Stroke::NONE
                            };

                            ui.painter()
                                .rect(rect, Rounding::same(3.0), bg_color, border_stroke);

                            let center_y = rect.center().y;
                            let grip_x = rect.min.x + 4.0;

                            // Biểu tượng tay nắm ⠿ khi hover hoặc kéo
                            if resp.hovered() || is_dragged {
                                ui.painter().text(
                                    Pos2::new(grip_x, center_y),
                                    egui::Align2::LEFT_CENTER,
                                    "⠿",
                                    FontId::monospace(10.5),
                                    theme::TEXT_KEY,
                                );
                            }

                            let text_x = if resp.hovered() || is_dragged {
                                grip_x + 12.0
                            } else {
                                rect.min.x + 4.0
                            };

                            let text_color = if is_dragged {
                                theme::TEXT_KEY
                            } else if resp.hovered() {
                                theme::TEXT_PRIMARY
                            } else {
                                theme::TEXT_MUTED
                            };

                            ui.painter().text(
                                Pos2::new(text_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                &col.name,
                                FontId::monospace(11.0),
                                text_color,
                            );
                        });
                    }
                })
                .body(|body| {
                    body.rows(text_height + 8.0, row_count, |mut row| {
                        let row_index = row.index();

                        if row_count > 0 && row_index == row_count - 1 {
                            last_row_visible = true;
                        }

                        if let Some(event) = app.cached_logs.get(row_index) {
                            let is_selected =
                                app.selected_log.as_ref().is_some_and(|s| s.id == event.id);
                            if is_selected {
                                row.set_selected(true);
                            }

                            let is_highlighted = app.is_row_highlighted(&event.id);
                            let row_color = match event.level {
                                LogLevel::Error | LogLevel::Fatal => theme::COLOR_ERROR,
                                LogLevel::Warn => theme::COLOR_WARN,
                                LogLevel::Info => theme::COLOR_INFO,
                                _ => theme::TEXT_MUTED,
                            };

                            for col in &visible_cols {
                                row.col(|ui| {
                                    if is_highlighted {
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            egui::Rounding::ZERO,
                                            theme::BG_ROW_HIGHLIGHT,
                                        );
                                    }

                                    let (cell_text, _is_bold) = match col.name.as_str() {
                                        "timestamp" => (event.timestamp.clone(), false),
                                        "level" => (event.level.to_string(), true),
                                        "message" => (event.message.replace('\n', " ↵ "), false),
                                        custom_key => {
                                            let val_str =
                                                if let Some(val) = event.fields.get(custom_key) {
                                                    match val {
                                                        serde_json::Value::String(s) => s.clone(),
                                                        _ => val.to_string(),
                                                    }
                                                } else {
                                                    "-".to_string()
                                                };
                                            (val_str, false)
                                        }
                                    };

                                    let cell_id = ui.make_persistent_id((event.id, &col.name));
                                    let mut text_val = cell_text.clone();
                                    let highlighted_terms_ref = &app.highlighted_terms;
                                    let mut layouter =
                                        |ui: &egui::Ui, _text: &str, _wrap_width: f32| {
                                            let mut job = theme::create_highlighted_layout_job(
                                                &cell_text,
                                                row_color,
                                                egui::FontId::monospace(11.5),
                                                highlighted_terms_ref,
                                            );
                                            job.wrap.max_width = f32::INFINITY;
                                            ui.fonts(|f| f.layout_job(job))
                                        };
                                    let edit = egui::TextEdit::singleline(&mut text_val)
                                        .id(cell_id)
                                        .font(egui::FontId::monospace(11.5))
                                        .text_color(row_color)
                                        .frame(false)
                                        .clip_text(true)
                                        .desired_width(f32::INFINITY)
                                        .layouter(&mut layouter);

                                    let resp = ui.add(edit);
                                    if resp.clicked()
                                        && !ui.input(|i| {
                                            i.pointer.button_down(egui::PointerButton::Secondary)
                                        })
                                    {
                                        newly_selected_event = Some(event.clone());
                                    }

                                    // Kiểm tra từ bôi đen trong ô nếu có
                                    let mut selected_text = None;
                                    if let Some(state) =
                                        egui::text_edit::TextEditState::load(ui.ctx(), cell_id)
                                    {
                                        if let Some(range) = state.cursor.char_range() {
                                            let [min_c, max_c] = range.sorted();
                                            if min_c.index < max_c.index {
                                                let s = min_c.index;
                                                let e = max_c.index;
                                                let txt: String = cell_text
                                                    .chars()
                                                    .skip(s)
                                                    .take(e.saturating_sub(s))
                                                    .collect();
                                                let clean_txt = txt.replace(" ↵ ", " ");
                                                let trimmed = clean_txt.trim().to_string();
                                                if !trimmed.is_empty() {
                                                    selected_text = Some(trimmed.clone());
                                                    ui.ctx().data_mut(|d| {
                                                        d.insert_temp(cell_id, trimmed)
                                                    });
                                                }
                                            } else if resp.clicked()
                                                && !ui.input(|i| {
                                                    i.pointer
                                                        .button_down(egui::PointerButton::Secondary)
                                                })
                                            {
                                                ui.ctx()
                                                    .data_mut(|d| d.remove_temp::<String>(cell_id));
                                            }
                                        }
                                    }

                                    // Nếu right-click làm reset selection của TextEdit, khôi phục từ temp storage
                                    if selected_text.is_none() {
                                        selected_text =
                                            ui.ctx().data(|d| d.get_temp::<String>(cell_id));
                                    }

                                    let col_name = col.name.clone();
                                    let raw_cell_val = match col.name.as_str() {
                                        "level" => event.level.to_string(),
                                        "message" => event.message.clone(),
                                        "timestamp" => event.timestamp.clone(),
                                        custom_key => {
                                            if let Some(val) = event.fields.get(custom_key) {
                                                match val {
                                                    serde_json::Value::String(s) => s.clone(),
                                                    _ => val.to_string(),
                                                }
                                            } else {
                                                cell_text.clone()
                                            }
                                        }
                                    };

                                    resp.context_menu(|ui| {
                                        ui.set_min_width(180.0);

                                        // 1. Lọc và Highlight theo từ đã bôi đen trong ô này (nếu có)
                                        if let Some(ref sel) = selected_text {
                                            let display_sel = if sel.chars().count() > 25 {
                                                format!(
                                                    "{}...",
                                                    sel.chars().take(25).collect::<String>()
                                                )
                                            } else {
                                                sel.clone()
                                            };

                                            if ui
                                                .button(format!("Filter \"{}\"", display_sel))
                                                .clicked()
                                            {
                                                let term = UwuGuiApp::format_selection_term(sel);
                                                filter_action = Some(FilterAction::Apply(term));
                                                ui.close_menu();
                                            }

                                            if ui
                                                .button(format!("Exclude \"{}\"", display_sel))
                                                .clicked()
                                            {
                                                let term = UwuGuiApp::format_selection_term(sel);
                                                filter_action = Some(FilterAction::Exclude(term));
                                                ui.close_menu();
                                            }

                                            let is_term_hl = app.is_term_highlighted(sel);
                                            let hl_term_text = if is_term_hl {
                                                format!("Unhighlight \"{}\"", display_sel)
                                            } else {
                                                format!("Highlight \"{}\"", display_sel)
                                            };
                                            if ui.button(hl_term_text).clicked() {
                                                highlight_action =
                                                    Some(HighlightAction::ToggleTerm(sel.clone()));
                                                ui.close_menu();
                                            }

                                            ui.separator();
                                        }

                                        // 2. Lọc và Highlight theo giá trị ô
                                        let display_val = if raw_cell_val.chars().count() > 25 {
                                            format!(
                                                "{}...",
                                                raw_cell_val.chars().take(25).collect::<String>()
                                            )
                                        } else {
                                            raw_cell_val.clone()
                                        };

                                        if ui
                                            .button(format!("Filter \"{}\"", display_val))
                                            .clicked()
                                        {
                                            let term = UwuGuiApp::format_field_term(
                                                &col_name,
                                                &raw_cell_val,
                                            );
                                            filter_action = Some(FilterAction::Apply(term));
                                            ui.close_menu();
                                        }

                                        if ui
                                            .button(format!("Exclude \"{}\"", display_val))
                                            .clicked()
                                        {
                                            let term = UwuGuiApp::format_field_term(
                                                &col_name,
                                                &raw_cell_val,
                                            );
                                            filter_action = Some(FilterAction::Exclude(term));
                                            ui.close_menu();
                                        }

                                        let is_cell_val_hl = app.is_term_highlighted(&raw_cell_val);
                                        let hl_cell_text = if is_cell_val_hl {
                                            format!("Unhighlight \"{}\"", display_val)
                                        } else {
                                            format!("Highlight \"{}\"", display_val)
                                        };
                                        if ui.button(hl_cell_text).clicked() {
                                            highlight_action = Some(HighlightAction::ToggleTerm(
                                                raw_cell_val.clone(),
                                            ));
                                            ui.close_menu();
                                        }

                                        ui.separator();

                                        // 3. Highlight / Bỏ highlight dòng
                                        if is_highlighted {
                                            if ui.button("Unhighlight row").clicked() {
                                                highlight_action =
                                                    Some(HighlightAction::ToggleRow(event.id));
                                                ui.close_menu();
                                            }
                                        } else if ui.button("Highlight row").clicked() {
                                            highlight_action =
                                                Some(HighlightAction::ToggleRow(event.id));
                                            ui.close_menu();
                                        }

                                        if has_any_highlights
                                            && ui.button("Unhighlight all").clicked()
                                        {
                                            highlight_action = Some(HighlightAction::ClearAll);
                                            ui.close_menu();
                                        }

                                        ui.separator();

                                        // 4. Copy giá trị
                                        if ui.button("Copy value").clicked() {
                                            ui.ctx().output_mut(|o| {
                                                o.copied_text = raw_cell_val.clone()
                                            });
                                            ui.close_menu();
                                        }
                                    });
                                });
                            }
                        }
                    });
                });
        });

    if let Some(action) = filter_action {
        match action {
            FilterAction::Apply(term) => app.apply_filter_term(&term),
            FilterAction::Exclude(term) => app.exclude_filter_term(&term),
        }
    }

    if let Some(action) = highlight_action {
        match action {
            HighlightAction::ToggleRow(id) => app.toggle_row_highlight(id),
            HighlightAction::ToggleTerm(term) => app.toggle_term_highlight(&term),
            HighlightAction::ClearAll => app.clear_all_highlights(),
        }
    }

    if let Some(name) = new_header_drag {
        app.column_state.header_dragged_name = Some(name);
    }

    if let Some((from_name, to_name)) = target_header_swap {
        let from_idx = app
            .column_state
            .columns
            .iter()
            .position(|c| c.name == from_name);
        let to_idx = app
            .column_state
            .columns
            .iter()
            .position(|c| c.name == to_name);
        if let (Some(from), Some(to)) = (from_idx, to_idx) {
            app.column_state.reorder(from, to);
            ui.ctx().request_repaint();
        }
    }

    // Ghost preview badge nổi theo chuột khi kéo theo chiều ngang của bảng
    if let Some(ref dragged_name) = app.column_state.header_dragged_name {
        if let Some(pos) = pointer_pos {
            let ghost_layer_id = egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("header_drag_ghost_layer"),
            );
            let painter = ui.ctx().layer_painter(ghost_layer_id);
            let ghost_size = egui::vec2((dragged_name.len() as f32 * 8.0 + 36.0).max(90.0), 26.0);
            let ghost_rect = egui::Rect::from_center_size(pos, ghost_size);

            painter.rect_filled(
                ghost_rect.expand(2.0),
                Rounding::same(5.0),
                Color32::from_black_alpha(100),
            );
            painter.rect_filled(ghost_rect, Rounding::same(4.0), theme::BG_MANTLE);
            painter.rect_stroke(
                ghost_rect,
                Rounding::same(4.0),
                Stroke::new(1.5, theme::TEXT_KEY),
            );
            painter.text(
                Pos2::new(ghost_rect.min.x + 8.0, ghost_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "⠿",
                FontId::monospace(12.0),
                theme::TEXT_KEY,
            );
            painter.text(
                Pos2::new(ghost_rect.min.x + 22.0, ghost_rect.center().y),
                egui::Align2::LEFT_CENTER,
                dragged_name,
                FontId::monospace(11.5),
                theme::TEXT_PRIMARY,
            );
        }
    }

    if let Some(event) = newly_selected_event {
        app.selected_log = Some(event);
        // Khi user chọn xem một dòng log, unlatch để màn hình đứng yên giúp đọc chi tiết
        app.unlatch();
    }

    // Bật lại auto-scroll CHỈ KHI: user chủ động cuộn XUỐNG (scroll_delta_y < 0) VÀ đã chạm đáy
    if last_row_visible && scroll_delta_y < 0.0 {
        app.is_auto_scroll = true;
    }
}
