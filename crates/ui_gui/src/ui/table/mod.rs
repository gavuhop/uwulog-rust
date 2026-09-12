pub mod cell;
pub mod context_menu;
pub mod header;

use crate::app::{AppAction, UwuGuiApp};
use crate::ui::actions::ActionContext;
use crate::ui::columns_modal::ColumnItem;
use crate::ui::theme;
use cell::render_cell;
use eframe::egui::{self, Pos2};
use egui_extras::{Column, TableBuilder};
use header::{render_drag_ghost, render_table_headers};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TableMode {
    Filtered,
    Unfiltered,
}

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    render_log_table(ui, app, TableMode::Filtered);
}

pub fn render_log_table(ui: &mut egui::Ui, app: &mut UwuGuiApp, mode: TableMode) {
    let row_count = match mode {
        TableMode::Filtered => app.viewport.cached_logs.len(),
        TableMode::Unfiltered => app.unfiltered.cached_unfiltered.len(),
    };
    let text_height = egui::TextStyle::Monospace.resolve(ui.style()).size;

    let mut newly_selected_event = None;
    let mut last_row_visible = false;

    // Đọc thao tác cuộn chuột trước khi vẽ TableBuilder
    let scroll_delta_y = ui.input(|i| {
        if i.raw_scroll_delta.y.abs() > 0.0 {
            i.raw_scroll_delta.y
        } else {
            i.smooth_scroll_delta.y
        }
    });
    if scroll_delta_y > 0.0 {
        match mode {
            TableMode::Filtered => app.unlatch(),
            TableMode::Unfiltered => app.unlatch_unfiltered(),
        }
    }

    let mut new_header_drag = None;
    let mut target_header_swap = None;
    let pointer_pos: Option<Pos2> = ui.input(|i| i.pointer.hover_pos());

    let visible_cols: Vec<ColumnItem> = app
        .columns
        .columns
        .iter()
        .filter(|c| c.visible)
        .cloned()
        .collect();

    let salt_prefix = match mode {
        TableMode::Filtered => "main",
        TableMode::Unfiltered => "unfiltered",
    };

    let table_salt = format!(
        "{}_tbl_{}",
        salt_prefix,
        visible_cols
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut action_to_dispatch: Option<AppAction> = None;
    let has_any_highlights = app.has_any_highlights();

    let default_ts = "2026-08-21 23:29:07";
    let logs = match mode {
        TableMode::Filtered => &app.viewport.cached_logs[..],
        TableMode::Unfiltered => &app.unfiltered.cached_unfiltered[..],
    };
    let sample_ts = logs
        .iter()
        .take(50)
        .map(|e| e.timestamp.as_str())
        .max_by_key(|s| s.chars().count())
        .unwrap_or(default_ts);

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

    let hscroll_id = match mode {
        TableMode::Filtered => "main_table_hscroll",
        TableMode::Unfiltered => "unfiltered_table_hscroll",
    };

    egui::ScrollArea::horizontal()
        .id_salt(hscroll_id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut builder = TableBuilder::new(ui)
                .id_salt(format!("{}_{}", salt_prefix, table_salt))
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false]);

            for col in &visible_cols {
                let std_field = uwu_core_schema::StandardField::from_alias(&col.name);
                let (initial_w, min_w) = match std_field {
                    Some(uwu_core_schema::StandardField::Timestamp) => {
                        (ts_needed_width, ts_needed_width)
                    }
                    Some(uwu_core_schema::StandardField::Level) => {
                        (level_needed_width, level_needed_width)
                    }
                    Some(uwu_core_schema::StandardField::Message) => (col.width.max(350.0), 100.0),
                    _ if col.width >= 40.0 => (col.width, 40.0),
                    _ => (120.0, 40.0),
                };

                builder = builder.column(Column::initial(initial_w).at_least(min_w).clip(true));
            }

            match mode {
                TableMode::Filtered => {
                    let has_new_data = app.viewport.has_new_data;
                    let force_scroll = app.viewport.request_scroll_to_bottom;
                    if (force_scroll || (app.viewport.is_auto_scroll && has_new_data))
                        && row_count > 0
                    {
                        builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
                        app.viewport.request_scroll_to_bottom = false;
                    }
                    app.viewport.prev_table_row_count = row_count;
                }
                TableMode::Unfiltered => {
                    if app.unfiltered.request_scroll_to_target && row_count > 0 {
                        if let Some(target_idx) = app.unfiltered.target_index {
                            builder = builder.scroll_to_row(target_idx, Some(egui::Align::Center));
                        }
                        app.unfiltered.request_scroll_to_target = false;
                    } else if app.unfiltered.is_live
                        && (app.unfiltered.request_scroll_to_bottom || app.unfiltered.has_new_data)
                        && row_count > 0
                    {
                        builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
                        app.unfiltered.request_scroll_to_bottom = false;
                    }
                }
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
                    let mut render_ctx = ActionContext {
                        highlighted_terms: &app.inspector.highlighted_terms,
                        has_any_highlights,
                        action: &mut action_to_dispatch,
                    };

                    let target_id = if mode == TableMode::Unfiltered {
                        app.unfiltered.target_id
                    } else {
                        None
                    };

                    body.rows(text_height + 8.0, row_count, |mut row| {
                        let row_index = row.index();

                        if row_count > 0 && row_index == row_count - 1 {
                            last_row_visible = true;
                        }

                        let maybe_event = match mode {
                            TableMode::Filtered => app.viewport.cached_logs.get(row_index),
                            TableMode::Unfiltered => {
                                app.unfiltered.cached_unfiltered.get(row_index)
                            }
                        };

                        if let Some(event) = maybe_event {
                            let is_target = target_id.is_some_and(|id| id == event.id);
                            let is_selected = is_target
                                || app
                                    .inspector
                                    .selected_log
                                    .as_ref()
                                    .is_some_and(|s| s.id == event.id);

                            let is_highlighted = app.is_row_highlighted(&event.id);
                            let row_color = theme::log_color_to_egui(event.color);

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

    if let Some(action) = action_to_dispatch {
        app.dispatch_action(action);
    }

    if let Some(name) = new_header_drag {
        app.columns.header_dragged_name = Some(name);
    }

    if let Some((from_name, to_name)) = target_header_swap {
        let from_idx = app.columns.columns.iter().position(|c| c.name == from_name);
        let to_idx = app.columns.columns.iter().position(|c| c.name == to_name);
        if let (Some(from), Some(to)) = (from_idx, to_idx) {
            app.columns.reorder(from, to);
            ui.ctx().request_repaint();
        }
    }

    if let Some(ref dragged_name) = app.columns.header_dragged_name {
        if let Some(pos) = pointer_pos {
            render_drag_ghost(ui, dragged_name, pos);
        }
    }

    if let Some(event) = newly_selected_event {
        app.dispatch_action(crate::app::AppAction::SelectLog(Some(event)));
        match mode {
            TableMode::Filtered => app.unlatch(),
            TableMode::Unfiltered => app.unlatch_unfiltered(),
        }
    }

    if last_row_visible && scroll_delta_y < 0.0 {
        match mode {
            TableMode::Filtered => app.viewport.is_auto_scroll = true,
            TableMode::Unfiltered => app.unfiltered.is_live = true,
        }
    }
}
