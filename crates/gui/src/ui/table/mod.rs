pub mod actions;
pub mod cell;
pub mod context_menu;
pub mod header;

use crate::app::UwuGuiApp;
use crate::ui::columns_modal::ColumnItem;
use crate::ui::theme;
use actions::{dispatch_actions, FilterAction, HighlightAction, TableRenderContext};
use cell::render_cell;
use eframe::egui::{self, Pos2};
use egui_extras::{Column, TableBuilder};
use header::{render_drag_ghost, render_table_headers};
use uwu_core::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let row_count = app.cached_logs.len();
    let text_height = egui::TextStyle::Monospace.resolve(ui.style()).size;

    let mut newly_selected_event = None;
    let mut last_row_visible = false;

    // Đọc thao tác cuộn chuột trước khi vẽ TableBuilder
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        app.unlatch();
    }

    let mut new_header_drag = None;
    let mut target_header_swap = None;
    let pointer_pos: Option<Pos2> = ui.input(|i| i.pointer.hover_pos());

    let visible_cols: Vec<ColumnItem> = app
        .column_state
        .columns
        .iter()
        .filter(|c| c.visible)
        .cloned()
        .collect();

    let table_salt = format!(
        "log_tbl_{}",
        visible_cols
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut filter_action: Option<FilterAction> = None;
    let mut highlight_action: Option<HighlightAction> = None;
    let has_any_highlights = app.has_any_highlights();

    let sample_ts = app
        .cached_logs
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
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut builder = TableBuilder::new(ui)
                .id_salt(table_salt)
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

            let has_new_data = row_count > app.prev_table_row_count;
            let force_scroll = app.request_scroll_to_bottom;
            if (force_scroll || (app.is_auto_scroll && has_new_data)) && row_count > 0 {
                builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
                app.request_scroll_to_bottom = false;
            }
            app.prev_table_row_count = row_count;

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
                    };

                    body.rows(text_height + 8.0, row_count, |mut row| {
                        let row_index = row.index();

                        if row_count > 0 && row_index == row_count - 1 {
                            last_row_visible = true;
                        }

                        if let Some(event) = app.cached_logs.get(row_index) {
                            let is_selected =
                                app.selected_log.as_ref().is_some_and(|s| s.id == event.id);

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

    dispatch_actions(app, filter_action, highlight_action);

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
        app.unlatch();
    }

    if last_row_visible && scroll_delta_y < 0.0 {
        app.is_auto_scroll = true;
    }
}
