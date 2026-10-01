pub mod cell;
pub mod context_menu;
pub mod header;

use crate::actions::{ActionContext, AppAction};
use crate::session::GuiSession;
use crate::state::ColumnItem;
use crate::theme;
use cell::render_cell;
use eframe::egui::{self, Pos2};
use egui_extras::{Column, TableBuilder};
use header::{render_drag_ghost, render_table_headers};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TableMode {
    Filtered,
    Unfiltered,
}

pub fn render_table(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    render_log_table(ui, session, TableMode::Filtered, dispatch);
}

pub fn render_log_table(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    mode: TableMode,
    dispatch: &mut impl FnMut(AppAction),
) {
    let is_unfiltered = mode == TableMode::Unfiltered;
    let row_count = if is_unfiltered {
        session.unfiltered.cached_unfiltered.len()
    } else {
        session.viewport.cached_logs.len()
    };
    let text_height = egui::TextStyle::Monospace.resolve(ui.style()).size;

    let mut newly_selected_event = None;
    let mut min_visible_row: Option<usize> = None;
    let mut last_row_visible = false;

    let had_forced_scroll = if is_unfiltered {
        session.unfiltered.request_scroll_to_bottom
            || session.unfiltered.request_scroll_to_target
            || session.unfiltered.request_maintain_scroll_offset.is_some()
    } else {
        session.viewport.request_scroll_to_bottom
            || session.viewport.request_maintain_scroll_offset.is_some()
    };

    // Đọc thao tác cuộn chuột trước khi vẽ TableBuilder
    let scroll_delta_y = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        if is_unfiltered {
            session.unlatch_unfiltered();
        } else {
            session.unlatch();
        }
    }

    let pointer_pos: Option<Pos2> = ui.input(|i| i.pointer.latest_pos());

    let visible_cols: Vec<ColumnItem> = session
        .columns
        .columns
        .iter()
        .filter(|c| c.visible)
        .cloned()
        .collect();

    if visible_cols.is_empty() {
        return;
    }

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
    let has_any_highlights = session.has_any_highlights();

    let default_ts = "2026-08-21 23:29:07";
    let logs = match mode {
        TableMode::Filtered => &session.viewport.cached_logs[..],
        TableMode::Unfiltered => &session.unfiltered.cached_unfiltered[..],
    };
    let sample_ts = logs
        .iter()
        .take(50)
        .map(|e| e.timestamp.as_str())
        .max_by_key(|s| s.chars().count())
        .unwrap_or(default_ts);

    let ts_text_width = ui.fonts_mut(|f| {
        let job = egui::text::LayoutJob::simple_singleline(
            sample_ts.to_string(),
            egui::FontId::monospace(12.0),
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
            let ctx = ui.ctx().clone();
            let mut builder = TableBuilder::new(ui)
                .id_salt(format!("{}_{}", salt_prefix, table_salt))
                .striped(false)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
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

            let maintain_offset = if is_unfiltered {
                session.unfiltered.request_maintain_scroll_offset.take()
            } else {
                session.viewport.request_maintain_scroll_offset.take()
            };

            if let Some(offset) = maintain_offset {
                builder = builder.scroll_to_row(offset, Some(egui::Align::Min));
            } else if is_unfiltered && session.unfiltered.request_scroll_to_target && row_count > 0
            {
                if let Some(target_idx) = session.unfiltered.target_index {
                    builder = builder.scroll_to_row(target_idx, Some(egui::Align::Center));
                }
                session.unfiltered.request_scroll_to_target = false;
            } else if row_count > 0 {
                let (force, is_live, has_new) = if is_unfiltered {
                    (
                        session.unfiltered.request_scroll_to_bottom,
                        session.unfiltered.is_live,
                        session.unfiltered.has_new_data,
                    )
                } else {
                    (
                        session.viewport.request_scroll_to_bottom,
                        session.viewport.is_auto_scroll,
                        session.viewport.has_new_data,
                    )
                };
                if force || (is_live && has_new) {
                    builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
                    if is_unfiltered {
                        session.unfiltered.request_scroll_to_bottom = false;
                    } else {
                        session.viewport.request_scroll_to_bottom = false;
                    }
                }
            }
            if !is_unfiltered {
                session.viewport.prev_table_row_count = row_count;
            }

            builder
                .header(26.0, |mut tbl_header| {
                    render_table_headers(
                        &mut tbl_header,
                        &ctx,
                        &visible_cols,
                        &mut session.columns,
                    );
                })
                .body(|body| {
                    let mut render_ctx = ActionContext {
                        highlighted_terms: &session.inspector.highlighted_terms,
                        has_any_highlights,
                        action: &mut action_to_dispatch,
                    };

                    let target_id = if mode == TableMode::Unfiltered {
                        session.unfiltered.target_id
                    } else {
                        None
                    };

                    body.rows(text_height + 8.0, row_count, |mut row| {
                        let row_index = row.index();

                        if min_visible_row.is_none() {
                            min_visible_row = Some(row_index);
                        }
                        if row_count > 0 && row_index == row_count - 1 {
                            last_row_visible = true;
                        }

                        let maybe_event = match mode {
                            TableMode::Filtered => session.viewport.cached_logs.get(row_index),
                            TableMode::Unfiltered => {
                                session.unfiltered.cached_unfiltered.get(row_index)
                            }
                        };

                        if let Some(event) = maybe_event {
                            let is_target = target_id.is_some_and(|id| id == event.id);
                            let is_selected = is_target
                                || session
                                    .inspector
                                    .selected_log
                                    .as_ref()
                                    .is_some_and(|s| s.id == event.id);

                            let is_highlighted = session.is_row_highlighted(&event.id);
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
        dispatch(action);
    }

    if let Some(ref dragged_name) = session.columns.header_dragged_name {
        if let Some(pos) = pointer_pos {
            let width = session
                .columns
                .header_dragged_width
                .or_else(|| {
                    visible_cols
                        .iter()
                        .find(|c| c.name == *dragged_name)
                        .map(|c| c.width)
                })
                .unwrap_or(100.0);
            let offset_x = session.columns.header_drag_offset_x;
            render_drag_ghost(ui, dragged_name, pos, width, offset_x);
        }
    }

    if let Some(event) = newly_selected_event {
        dispatch(AppAction::SelectLog(Some(event)));
        if is_unfiltered {
            session.unlatch_unfiltered();
        } else {
            session.unlatch();
        }
    }

    // Nếu người dùng cuộn/kéo thanh cuộn rời khỏi dòng cuối (không phải do force scroll)
    if row_count > 0 && !last_row_visible && !had_forced_scroll {
        if is_unfiltered {
            session.unlatch_unfiltered();
        } else {
            session.unlatch();
        }
    }

    // Reverse pagination / Infinite scroll up:
    // Khi cuộn gần đỉnh bảng (trong vòng 10 dòng đầu) và không ở đáy stream
    let min_row = min_visible_row.unwrap_or(usize::MAX);
    let is_at_bottom = if is_unfiltered {
        session.unfiltered.is_live && last_row_visible
    } else {
        session.viewport.is_auto_scroll && last_row_visible
    };
    let reached_oldest = if is_unfiltered {
        session.unfiltered.reached_oldest
    } else {
        session.viewport.reached_oldest
    };

    if min_row <= 10 && row_count > 0 && !is_at_bottom && !reached_oldest {
        session.load_older_logs(is_unfiltered, 1000);
        ui.ctx().request_repaint();
    }

    if last_row_visible && scroll_delta_y < 0.0 {
        if is_unfiltered {
            session.unfiltered.is_live = true;
        } else {
            session.viewport.is_auto_scroll = true;
        }
    }
}
