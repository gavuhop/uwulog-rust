use crate::app::UwuGuiApp;
use crate::ui::actions::{dispatch_actions, FilterAction, HighlightAction, UnfilteredAction};
use crate::ui::columns_modal::ColumnItem;
use crate::ui::table::actions::TableRenderContext;
use crate::ui::table::cell::render_cell;
use crate::ui::table::header::{render_drag_ghost, render_table_headers};
use crate::ui::theme;
use eframe::egui::{self, Pos2};
use egui_extras::{Column, TableBuilder};
use uwu_core::LogLevel;

pub fn render_unfiltered_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let row_count = app.unfiltered_state.cached_unfiltered.len();
    let text_height = egui::TextStyle::Monospace.resolve(ui.style()).size;

    let mut newly_selected_event = None;
    let mut filter_action: Option<FilterAction> = None;
    let mut highlight_action: Option<HighlightAction> = None;
    let mut unfiltered_action: Option<UnfilteredAction> = None;
    let has_any_highlights = app.has_any_highlights();
    let mut last_row_visible = false;

    let mut new_header_drag = None;
    let mut target_header_swap = None;
    let pointer_pos: Option<Pos2> = ui.input(|i| i.pointer.hover_pos());

    // Đọc thao tác cuộn chuột lên để chuyển sang Freeze Snapshot mode nếu đang Live
    let scroll_delta_y = ui.input(|i| {
        if i.raw_scroll_delta.y.abs() > 0.0 {
            i.raw_scroll_delta.y
        } else {
            i.smooth_scroll_delta.y
        }
    });
    if scroll_delta_y > 0.0 {
        app.unlatch_unfiltered();
    }

    // Table Render Directly (Header controls unified into top navigation bar)
    let visible_cols: Vec<ColumnItem> = app
        .column_state
        .columns
        .iter()
        .filter(|c| c.visible)
        .cloned()
        .collect();

    let table_salt = format!(
        "unfiltered_tbl_{}",
        visible_cols
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );

    let sample_ts = app
        .unfiltered_state
        .cached_unfiltered
        .iter()
        .take(50)
        .map(|e| e.timestamp.as_str())
        .max_by_key(|s| s.chars().count())
        .unwrap_or("2026-08-21 23:29:07");
    let ts_text_width = ui.fonts(|f| {
        let job = egui::text::LayoutJob::simple_singleline(
            sample_ts.to_string(),
            egui::FontId::monospace(11.5),
            egui::Color32::WHITE,
        );
        f.layout_job(job).size().x
    });
    let ts_needed_width = (ts_text_width + 8.0).max(80.0);
    let level_needed_width = 56.0;

    egui::ScrollArea::horizontal()
        .id_salt("unfiltered_table_hscroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut builder = TableBuilder::new(ui)
                .id_salt(format!("unfiltered_{}", table_salt))
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false]);

            for col in &visible_cols {
                let (initial_w, min_w) = if col.name == "timestamp" {
                    (ts_needed_width, ts_needed_width)
                } else if col.name == "level" {
                    (level_needed_width, level_needed_width)
                } else if col.name == "message" {
                    (col.width.max(350.0), 100.0)
                } else if col.width >= 40.0 {
                    (col.width, 40.0)
                } else {
                    (120.0, 40.0)
                };

                builder = builder.column(Column::initial(initial_w).at_least(min_w).clip(true));
            }

            // Tự động cuộn đến vị trí dòng mục tiêu khi vừa mở bảng hoặc bấm Jump
            if app.unfiltered_state.request_scroll_to_target && row_count > 0 {
                if let Some(target_idx) = app.unfiltered_state.target_index {
                    builder = builder.scroll_to_row(target_idx, Some(egui::Align::Center));
                }
                app.unfiltered_state.request_scroll_to_target = false;
            } else if app.unfiltered_state.is_live
                && (app.unfiltered_state.request_scroll_to_bottom
                    || app.unfiltered_state.has_new_data)
                && row_count > 0
            {
                builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
                app.unfiltered_state.request_scroll_to_bottom = false;
            }

            builder
                .header(26.0, |mut tbl_header| {
                    render_table_headers(
                        &mut tbl_header,
                        &visible_cols,
                        app,
                        &mut new_header_drag,
                        &mut target_header_swap,
                    );
                })
                .body(|body| {
                    let mut render_ctx = TableRenderContext {
                        highlighted_terms: &app.highlighted_terms,
                        has_any_highlights,
                        filter_action: &mut filter_action,
                        highlight_action: &mut highlight_action,
                        unfiltered_action: &mut unfiltered_action,
                    };

                    let target_id = app.unfiltered_state.target_id;

                    body.rows(text_height + 8.0, row_count, |mut row| {
                        let row_index = row.index();

                        if row_count > 0 && row_index == row_count - 1 {
                            last_row_visible = true;
                        }

                        if let Some(event) = app.unfiltered_state.cached_unfiltered.get(row_index) {
                            let is_target = target_id.is_some_and(|id| id == event.id);
                            let is_selected = is_target
                                || app.selected_log.as_ref().is_some_and(|s| s.id == event.id);

                            let is_highlighted = app.is_row_highlighted(&event.id);
                            let row_color = match event.level {
                                LogLevel::Error | LogLevel::Fatal => theme::COLOR_ERROR,
                                LogLevel::Warn => theme::COLOR_WARN,
                                LogLevel::Info => theme::COLOR_INFO,
                                _ => theme::TEXT_MUTED,
                            };

                            for col in &visible_cols {
                                row.col(|ui| {
                                    let cell_clicked = render_cell(
                                        ui,
                                        event,
                                        &col.name,
                                        row_color,
                                        is_selected,
                                        is_highlighted,
                                        &mut render_ctx,
                                    );
                                    if cell_clicked {
                                        newly_selected_event = Some(event.clone());
                                    }
                                });
                            }
                        }
                    });
                });
        });

    dispatch_actions(app, filter_action, highlight_action, unfiltered_action);

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

    if let Some(ref dragged_name) = app.column_state.header_dragged_name {
        if let Some(pos) = pointer_pos {
            render_drag_ghost(ui, dragged_name, pos);
        }
    }

    if let Some(event) = newly_selected_event {
        app.selected_log = Some(event);
        app.unlatch_unfiltered();
    }

    if last_row_visible && scroll_delta_y < 0.0 {
        app.unfiltered_state.is_live = true;
    }
}
