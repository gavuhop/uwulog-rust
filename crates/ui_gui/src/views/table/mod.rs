pub mod autoscroll;
pub mod cell;
pub mod context_menu;
pub mod header;

use crate::actions::{ActionContext, AppAction};
use crate::session::GuiSession;
use crate::state::{ActiveTab, ColumnItem};
use crate::theme;
use cell::render_cell;
use eframe::egui::{self, Pos2};
use egui_extras::{Column, TableBuilder};
use header::{render_drag_ghost, render_table_headers};
use std::hash::{Hash, Hasher};

pub fn render_table(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    render_log_table(ui, session, ActiveTab::Filtered, dispatch);
}

pub fn render_log_table(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    tab: ActiveTab,
    dispatch: &mut impl FnMut(AppAction),
) {
    let row_count = match tab {
        ActiveTab::Filtered => session.viewport.cached_logs.len(),
        ActiveTab::Unfiltered => session.unfiltered.cached_unfiltered.len(),
    };
    let scroll_target = session.consume_scroll_request(tab, row_count);
    let stream = session.stream_summary(tab);
    let text_height = crate::theme::TABLE_FONT_SIZE;

    let mut newly_selected_event = None;
    let mut min_visible_row: Option<usize> = None;
    let mut last_row_visible = false;

    // Đọc thao tác cuộn chuột trước khi vẽ TableBuilder
    let is_shift_down = ui.input(|i| i.modifiers.shift);
    let scroll_delta_y = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta_y > 0.0 && !is_shift_down {
        dispatch(AppAction::Unlatch);
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

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tab.hash(&mut hasher);
    for c in &visible_cols {
        c.name.hash(&mut hasher);
    }
    let table_salt = hasher.finish();

    let mut action_to_dispatch: Option<AppAction> = None;
    let has_any_highlights = session.has_any_highlights();

    let default_ts = "2026-08-21 23:29:07";
    let sample_ts = match tab {
        ActiveTab::Filtered => session
            .viewport
            .cached_logs
            .first()
            .map(|e| e.timestamp.as_str()),
        ActiveTab::Unfiltered => session
            .unfiltered
            .cached_unfiltered
            .first()
            .map(|e| e.timestamp.as_str()),
    }
    .unwrap_or(default_ts);

    let ts_text_width = ui.fonts_mut(|f| {
        let job = egui::text::LayoutJob::simple_singleline(
            sample_ts.to_owned(),
            egui::FontId::monospace(crate::theme::TABLE_FONT_SIZE),
            egui::Color32::WHITE,
        );
        f.layout_job(job).size().x
    });
    let ts_needed_width = (ts_text_width + 8.0).max(80.0);
    let level_text_width = ui.fonts_mut(|f| {
        let job = egui::text::LayoutJob::simple_singleline(
            "EMERGENCY".to_owned(),
            egui::FontId::monospace(crate::theme::TABLE_FONT_SIZE),
            egui::Color32::WHITE,
        );
        f.layout_job(job).size().x
    });
    let level_needed_width = (level_text_width + 8.0).max(65.0);

    let hscroll_id = match tab {
        ActiveTab::Filtered => "main_table_hscroll",
        ActiveTab::Unfiltered => "unfiltered_table_hscroll",
    };
    let scroll_id = ui.id().with(egui::IdSalt::new(hscroll_id));
    let mut hstate = egui::scroll_area::State::load(ui.ctx(), scroll_id).unwrap_or_default();
    let mut hstate_changed = false;

    // 1. Phím tắt cuộn ngang (ArrowLeft / ArrowRight) từ session
    if let Some(delta) = session.consume_horizontal_scroll(tab) {
        hstate.offset.x = (hstate.offset.x + delta).max(0.0);
        hstate_changed = true;
    }

    // 2. Lăn chuột ngang: Shift + Con lăn chuột (cuộn dọc smooth_y khi giữ Shift)
    let smooth_y = ui.input(|i| i.smooth_scroll_delta.y);
    if is_shift_down && smooth_y != 0.0 {
        hstate.offset.x = (hstate.offset.x - smooth_y).max(0.0);
        hstate_changed = true;
    }

    // 3. Cuộn tự động theo tâm bằng chuột giữa (Web-style Center-Anchor Autoscroll)
    let vscroll_key = format!("{table_salt}_vscroll_offset");
    let vscroll_id = ui.make_persistent_id(vscroll_key);
    let mut current_v_offset = ui.data(|d| d.get_temp::<f32>(vscroll_id)).unwrap_or(0.0);
    let mut vstate_changed = false;

    let table_rect = ui.available_rect_before_wrap();
    let autoscroll = autoscroll::handle_middle_autoscroll(ui, table_rect);
    if autoscroll.is_active {
        if autoscroll.scroll_delta.x != 0.0 {
            hstate.offset.x = (hstate.offset.x + autoscroll.scroll_delta.x).max(0.0);
            hstate_changed = true;
        }
        if autoscroll.scroll_delta.y != 0.0 {
            current_v_offset = (current_v_offset + autoscroll.scroll_delta.y).max(0.0);
            vstate_changed = true;
            if autoscroll.scroll_delta.y < 0.0 {
                dispatch(AppAction::Unlatch);
            }
        }
    }

    if hstate_changed {
        hstate.store(ui.ctx(), scroll_id);
    }

    let mut scroll_area = egui::ScrollArea::horizontal()
        .id_salt(hscroll_id)
        .auto_shrink([false, false]);

    if hstate_changed {
        scroll_area = scroll_area.horizontal_scroll_offset(hstate.offset.x);
    }

    let hscroll_out = scroll_area.show(ui, |ui| {
        let ctx = ui.ctx().clone();
        let mut builder = TableBuilder::new(ui)
            .id_salt(table_salt)
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

        if let Some((target_row, align)) = scroll_target {
            builder = builder.scroll_to_row(target_row, align);
        } else if vstate_changed {
            builder = builder.vertical_scroll_offset(current_v_offset);
        }

        let body_output = builder
            .header(26.0, |mut tbl_header| {
                render_table_headers(&mut tbl_header, &ctx, &visible_cols, &mut session.columns);
            })
            .body(|body| {
                let mut render_ctx = ActionContext {
                    highlighted_terms: &session.inspector.highlighted_terms,
                    has_any_highlights,
                    is_filtering: !session.view.search.query.trim().is_empty(),
                    is_unfiltered_tab: tab == ActiveTab::Unfiltered,
                    action: &mut action_to_dispatch,
                };

                let target_id = if tab == ActiveTab::Unfiltered {
                    session.unfiltered.target_id
                } else {
                    None
                };

                body.rows(text_height + 4.0, row_count, |mut row| {
                    let row_index = row.index();

                    if min_visible_row.is_none() {
                        min_visible_row = Some(row_index);
                    }
                    if row_count > 0 && row_index == row_count - 1 {
                        last_row_visible = true;
                    }

                    let maybe_event = match tab {
                        ActiveTab::Filtered => session.viewport.cached_logs.get(row_index),
                        ActiveTab::Unfiltered => {
                            session.unfiltered.cached_unfiltered.get(row_index)
                        }
                    };

                    if let Some(event) = maybe_event {
                        let is_target = target_id.is_some_and(|id| id == event.id);
                        let tab_selected = match tab {
                            ActiveTab::Filtered => session.viewport.selected_log.as_ref(),
                            ActiveTab::Unfiltered => session.unfiltered.selected_log.as_ref(),
                        };
                        let is_selected =
                            is_target || tab_selected.is_some_and(|s| s.id == event.id);

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

        body_output.state.offset.y
    });

    ui.data_mut(|d| d.insert_temp(vscroll_id, hscroll_out.inner));

    match tab {
        ActiveTab::Filtered => {
            session.viewport.first_visible_row = min_visible_row;
        }
        ActiveTab::Unfiltered => {
            session.unfiltered.first_visible_row = min_visible_row;
        }
    }

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
        dispatch(AppAction::Unlatch);
    }

    // Nếu người dùng cuộn/kéo thanh cuộn rời khỏi dòng cuối
    if ui.input(|i| i.pointer.is_decidedly_dragging()) && row_count > 0 && !last_row_visible {
        dispatch(AppAction::Unlatch);
    }

    // Reverse pagination / Infinite scroll up:
    // Khi cuộn gần đỉnh bảng (trong vòng 10 dòng đầu) khi đang dừng xem log cũ
    let min_row = min_visible_row.unwrap_or(usize::MAX);

    if !stream.is_live
        && min_row <= 10
        && row_count > 0
        && !stream.reached_oldest
        && !stream.is_loading_older
    {
        dispatch(AppAction::LoadOlderLogs(1000));
    }

    if last_row_visible
        && (scroll_delta_y < 0.0 || (autoscroll.is_active && autoscroll.scroll_delta.y > 0.0))
        && !is_shift_down
    {
        dispatch(AppAction::Latch);
    }
}
