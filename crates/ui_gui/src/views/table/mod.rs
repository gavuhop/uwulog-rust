pub mod autoscroll;
pub mod cell;
pub mod context_menu;
pub mod header;

use crate::actions::{ActionContext, AppAction};
use crate::session::GuiSession;
use crate::state::{ActiveTab, ColumnItem};
use crate::theme;
use cell::{render_cell, CellRenderState};
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
    // Cô lập toàn bộ state bảng (độ rộng cột TableState, vị trí cuộn ngang/dọc, tab trước) theo từng session.id
    // để không ghi đè lẫn nhau khi chuyển qua lại giữa các tab App / Container.
    ui.push_id(session.session.id, |ui| {
        render_log_table_scoped(ui, session, tab, dispatch);
    });
}

fn render_log_table_scoped(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    tab: ActiveTab,
    dispatch: &mut impl FnMut(AppAction),
) {
    let row_count = session.stream(tab).cached_logs.len();
    let scroll_target = session.consume_scroll_request(tab, row_count);
    let stream = session.stream_summary(tab);
    let text_height = crate::theme::TABLE_FONT_SIZE;

    let mut newly_selected_event = None;
    let mut min_visible_row: Option<usize> = None;
    let mut last_row_visible = false;

    let table_rect = ui.available_rect_before_wrap();
    // Chỉ xử lý wheel/drag unlatch khi con trỏ thực sự đang nằm trên bảng (không bị popup/modal che)
    let pointer_over_table = ui.rect_contains_pointer(table_rect);

    // Đọc trạng thái cuộn chuột (sẽ dùng sau khi có kích thước content thực tế từ TableBuilder)
    let is_shift_down = ui.input(|i| i.modifiers.shift);
    let scroll_delta_y = ui.input(|i| i.smooth_scroll_delta.y);

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

    // Dùng chung một table_salt giữa Main và Raw trong cùng session để chia sẻ TableState (độ rộng cột);
    // vẫn cô lập giữa các session khác nhau bằng `session.session.id`.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    session.session.id.hash(&mut hasher);
    session.columns.layout_version.hash(&mut hasher);
    for c in &visible_cols {
        c.name.hash(&mut hasher);
    }
    let table_salt = hasher.finish();

    let mut action_to_dispatch: Option<AppAction> = None;
    let has_any_highlights = session.has_any_highlights();

    let default_ts = "2026-08-21 23:29:07";
    let sample_ts = session
        .stream(tab)
        .cached_logs
        .first()
        .map(|e| e.timestamp.as_str())
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

    const SHARED_HSCROLL_SALT: &str = "log_table_hscroll";
    let scroll_id = ui.id().with(egui::IdSalt::new(SHARED_HSCROLL_SALT));
    let mut hstate = egui::scroll_area::State::load(ui.ctx(), scroll_id).unwrap_or_default();
    let mut hstate_changed = false;

    let prev_tab_id = ui.make_persistent_id("log_table_prev_tab");
    let prev_tab = ui.data(|d| d.get_temp::<ActiveTab>(prev_tab_id));
    let tab_changed = prev_tab.is_some_and(|p| p != tab);
    if let Some(prev) = prev_tab.filter(|&p| p != tab) {
        let prev_hscroll_id = ui.make_persistent_id((prev, "table_hscroll_offset"));
        let next_hscroll_id = ui.make_persistent_id((tab, "table_hscroll_offset"));
        let next_x = ui
            .data(|d| d.get_temp::<f32>(next_hscroll_id))
            .unwrap_or(0.0);
        ui.data_mut(|d| d.insert_temp(prev_hscroll_id, hstate.offset.x));
        hstate.offset.x = next_x;
        hstate_changed = true;
        session.columns.table_animations.reset();
    }
    ui.data_mut(|d| d.insert_temp(prev_tab_id, tab));

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
    let vscroll_id = ui.make_persistent_id((tab, "table_vscroll_offset"));
    let mut current_v_offset = ui.data(|d| d.get_temp::<f32>(vscroll_id)).unwrap_or(0.0);
    let prev_salt_id = ui.make_persistent_id("log_table_prev_salt");
    let prev_salt = ui.data(|d| d.get_temp::<u64>(prev_salt_id));
    let salt_changed = prev_salt.is_some_and(|s| s != table_salt);
    ui.data_mut(|d| d.insert_temp(prev_salt_id, table_salt));
    let mut vstate_changed = tab_changed || (salt_changed && current_v_offset > 0.0);

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
        .id_salt(SHARED_HSCROLL_SALT)
        .auto_shrink([false, false]);

    if hstate_changed {
        scroll_area = scroll_area.horizontal_scroll_offset(hstate.offset.x);
    }

    let hscroll_out = scroll_area.show(ui, |ui| {
        let ctx = ui.ctx().clone();
        let table_clip_rect = ui.clip_rect();
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
                    (col.width.max(ts_needed_width), ts_needed_width)
                }
                Some(uwu_core_schema::StandardField::Level) => {
                    (col.width.max(level_needed_width), level_needed_width)
                }
                Some(uwu_core_schema::StandardField::Message) => (col.width.max(100.0), 100.0),
                _ => (col.width.max(40.0), 40.0),
            };

            builder = builder.column(Column::initial(initial_w).at_least(min_w).clip(true));
        }

        if let Some((target_row, align)) = scroll_target {
            builder = builder.scroll_to_row(target_row, align);
        } else if vstate_changed {
            builder = builder.vertical_scroll_offset(current_v_offset);
        }

        let mut col_offsets = Vec::with_capacity(visible_cols.len());
        let body_output = builder
            .header(26.0, |mut tbl_header| {
                render_table_headers(
                    &mut tbl_header,
                    &ctx,
                    &visible_cols,
                    &mut session.columns,
                    table_clip_rect,
                    &mut col_offsets,
                );
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

                    if let Some(event) = session.stream(tab).cached_logs.get(row_index) {
                        let is_target = target_id.is_some_and(|id| id == event.id);
                        let tab_selected = session.stream(tab).selected_log.as_ref();
                        let is_selected =
                            is_target || tab_selected.is_some_and(|s| s.id == event.id);

                        let is_highlighted = session.is_row_highlighted(&event.id);
                        let row_color = theme::log_color_to_egui(event.color);

                        for (col_idx, col) in visible_cols.iter().enumerate() {
                            let offset_x = col_offsets.get(col_idx).copied().unwrap_or(0.0);
                            row.col(|ui| {
                                let cell_clicked = render_cell(
                                    ui,
                                    event,
                                    &col.name,
                                    row_color,
                                    CellRenderState {
                                        is_selected,
                                        is_row_highlighted: is_highlighted,
                                        offset_x,
                                        table_clip_rect,
                                    },
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

        (
            body_output.state.offset.y,
            body_output.content_size.y,
            body_output.inner_rect.height(),
        )
    });

    let (v_offset, content_h, viewport_h) = hscroll_out.inner;
    ui.data_mut(|d| {
        d.insert_temp(vscroll_id, v_offset);
        d.insert_temp(
            ui.make_persistent_id((tab, "table_hscroll_offset")),
            hscroll_out.state.offset.x,
        );
    });

    // Chỉ chuyển sang Pause (Unlatch) khi cuộn chuột lên nếu bảng thực sự có nội dung tràn màn hình để cuộn;
    // khi mới có vài dòng nằm gọn trong khung nhìn, lăn chuột lên không được ngắt Live.
    let has_vertical_overflow = content_h > viewport_h + 1.0 || v_offset > 0.5;
    if pointer_over_table && scroll_delta_y > 0.0 && !is_shift_down && has_vertical_overflow {
        dispatch(AppAction::Unlatch);
    }

    let stream_mut = session.stream_mut(tab);
    stream_mut.first_visible_row = min_visible_row;

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
    if pointer_over_table
        && ui.input(|i| i.pointer.is_decidedly_dragging())
        && row_count > 0
        && !last_row_visible
    {
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
        && ((pointer_over_table && scroll_delta_y < 0.0)
            || (autoscroll.is_active && autoscroll.scroll_delta.y > 0.0))
        && !is_shift_down
    {
        dispatch(AppAction::Latch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_core_schema::{LogColor, LogEvent, LogFields};
    use uwu_core_workspace::{SourceType, Workspace, WorkspaceLocation};

    fn make_session_with_logs(count: usize) -> GuiSession {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let ws = Workspace::new(
            "demo",
            WorkspaceLocation::local("/tmp"),
            SourceType::Process,
        );
        let mut session = GuiSession::from_workspace(&ws, 1000, 1000);
        for i in 0..count {
            session.viewport.cached_logs.push(
                LogEvent::new(
                    "12:00:00",
                    LogColor::Default,
                    format!("log line {i}"),
                    LogFields::default(),
                )
                .with_id(i as u64 + 1),
            );
        }
        session
    }

    #[test]
    fn popup_occlusion_prevents_click_through_and_scroll_unlatch() {
        let mut session = make_session_with_logs(200);
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let click_pos = egui::pos2(200.0, 200.0);

        // 2 warmup frames để Area foreground hoàn tất sizing_pass và ghi nhận visible_areas_last_frame
        let mut actions = Vec::new();
        for _ in 0..2 {
            let mut out1 = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        render_log_table(ui, &mut session, ActiveTab::Filtered, &mut |a| {
                            actions.push(a);
                        });
                    });
                    egui::Area::new(egui::Id::new("test_foreground_popup"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(egui::pos2(100.0, 100.0))
                        .show(ui.ctx(), |ui| {
                            ui.allocate_exact_size(egui::vec2(300.0, 300.0), egui::Sense::click());
                        });
                },
            );
            out1.textures_delta.clear();
        }

        // Frame 2: click + scroll lên bên trên popup -> KHÔNG được phát AppAction::Unlatch
        actions.clear();
        let mut events = vec![
            egui::Event::PointerMoved(click_pos),
            egui::Event::PointerButton {
                pos: click_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: click_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 40.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ];
        let mut out2 = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events: std::mem::take(&mut events),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    render_log_table(ui, &mut session, ActiveTab::Filtered, &mut |a| {
                        actions.push(a);
                    });
                });
                egui::Area::new(egui::Id::new("test_foreground_popup"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(100.0, 100.0))
                    .show(ui.ctx(), |ui| {
                        ui.allocate_exact_size(egui::vec2(300.0, 300.0), egui::Sense::click());
                    });
            },
        );
        out2.textures_delta.clear();

        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, AppAction::Unlatch | AppAction::SelectLog(_))),
            "expected foreground popup click/scroll not to unlatch or select background table row, got: {actions:?}"
        );

        // Frame 3 & 4: không còn popup, cuộn lên trực tiếp trên bảng -> phải phát Unlatch
        actions.clear();
        let mut out3 = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![egui::Event::PointerMoved(click_pos)],
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    render_log_table(ui, &mut session, ActiveTab::Filtered, &mut |a| {
                        actions.push(a);
                    });
                });
            },
        );
        out3.textures_delta.clear();

        actions.clear();
        let mut out4 = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(click_pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 40.0),
                        modifiers: egui::Modifiers::NONE,
                        phase: egui::TouchPhase::Move,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    render_log_table(ui, &mut session, ActiveTab::Filtered, &mut |a| {
                        actions.push(a);
                    });
                });
            },
        );
        out4.textures_delta.clear();

        assert!(
            actions.iter().any(|a| matches!(a, AppAction::Unlatch)),
            "expected scrolling up over uncovered table to dispatch Unlatch, got: {actions:?}"
        );
    }

    #[test]
    fn wheel_scroll_up_does_not_unlatch_when_logs_fit_without_overflow() {
        let mut session = make_session_with_logs(5);
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let hover_pos = egui::pos2(200.0, 200.0);
        let mut actions = Vec::new();

        for _ in 0..2 {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![egui::Event::PointerMoved(hover_pos)],
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        render_log_table(ui, &mut session, ActiveTab::Filtered, &mut |a| {
                            actions.push(a);
                        });
                    });
                },
            );
            out.textures_delta.clear();
        }

        actions.clear();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![
                    egui::Event::PointerMoved(hover_pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 40.0),
                        modifiers: egui::Modifiers::NONE,
                        phase: egui::TouchPhase::Move,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    render_log_table(ui, &mut session, ActiveTab::Filtered, &mut |a| {
                        actions.push(a);
                    });
                });
            },
        );
        out.textures_delta.clear();

        assert!(
            !actions.iter().any(|a| matches!(a, AppAction::Unlatch)),
            "expected scrolling up with only 5 rows (no overflow) not to dispatch Unlatch, got: {actions:?}"
        );
    }

    #[test]
    fn shared_table_state_preserves_column_widths_across_tabs_and_reorders() {
        let mut session = make_session_with_logs(5);
        session.unfiltered.is_open = true;
        session.unfiltered.cached_logs = session.viewport.cached_logs.clone();
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));

        let run_tab = |tab: ActiveTab, session: &mut GuiSession, events: Vec<egui::Event>| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        render_log_table(ui, session, tab, &mut |_| {});
                    });
                },
            );
            out.textures_delta.clear();
        };

        // Render lần đầu ở Filtered và lấy vị trí mép phải cột đầu tiên ("timestamp")
        run_tab(ActiveTab::Filtered, &mut session, vec![]);
        let initial_w = session.columns.columns[0].width;
        let sep_x = 8.0 + initial_w + 4.0;
        let drag_y = 20.0;

        // Kéo vạch phân cách cột "timestamp" sang phải +60px trên tab Filtered
        run_tab(
            ActiveTab::Filtered,
            &mut session,
            vec![
                egui::Event::PointerMoved(egui::pos2(sep_x, drag_y)),
                egui::Event::PointerButton {
                    pos: egui::pos2(sep_x, drag_y),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        run_tab(
            ActiveTab::Filtered,
            &mut session,
            vec![egui::Event::PointerMoved(egui::pos2(sep_x + 60.0, drag_y))],
        );
        run_tab(
            ActiveTab::Filtered,
            &mut session,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(sep_x + 60.0, drag_y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        run_tab(ActiveTab::Filtered, &mut session, vec![]);
        let resized_w = session.columns.columns[0].width;
        assert!(
            resized_w > initial_w + 30.0,
            "expected timestamp width to grow from {initial_w}, got {resized_w}"
        );

        // Chuyển sang Raw (Unfiltered): dùng chung TableState nên giữ nguyên độ rộng vừa kéo
        run_tab(ActiveTab::Unfiltered, &mut session, vec![]);
        assert!((session.columns.columns[0].width - resized_w).abs() < 0.5);

        // Đổi vị trí cột trên Raw: vẫn giữ nguyên độ rộng đã resize
        session.columns.move_column("timestamp", "message");
        run_tab(ActiveTab::Unfiltered, &mut session, vec![]);
        let moved_w = session
            .columns
            .columns
            .iter()
            .find(|c| c.name == "timestamp")
            .map(|c| c.width)
            .unwrap();
        assert!((moved_w - resized_w).abs() < 0.5);
    }

    #[test]
    fn switching_between_sessions_isolates_column_widths_and_scroll_state() {
        let mut app_session = make_session_with_logs(40);
        let mut container_session = make_session_with_logs(40);
        assert_ne!(app_session.session.id, container_session.session.id);

        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 400.0));
        let render_session = |session: &mut GuiSession, events: Vec<egui::Event>| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        render_log_table(ui, session, ActiveTab::Filtered, &mut |_| {});
                    });
                },
            );
            out.textures_delta.clear();
        };

        // 1. Render App tab và kéo rộng cột "timestamp" thêm +70px
        render_session(&mut app_session, vec![]);
        let initial_app_w = app_session.columns.columns[0].width;
        let sep_x = 8.0 + initial_app_w + 4.0;
        let drag_y = 20.0;

        render_session(
            &mut app_session,
            vec![
                egui::Event::PointerMoved(egui::pos2(sep_x, drag_y)),
                egui::Event::PointerButton {
                    pos: egui::pos2(sep_x, drag_y),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        render_session(
            &mut app_session,
            vec![egui::Event::PointerMoved(egui::pos2(sep_x + 70.0, drag_y))],
        );
        render_session(
            &mut app_session,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(sep_x + 70.0, drag_y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        render_session(&mut app_session, vec![]);
        let resized_app_w = app_session.columns.columns[0].width;
        assert!(
            resized_app_w > initial_app_w + 40.0,
            "expected app timestamp width to grow from {initial_app_w}, got {resized_app_w}"
        );

        // 2. Chuyển sang tab Container (`db`) có cùng layout_version = 0 và cột mặc định:
        //    không được dùng chung TableState với tab App.
        let initial_container_w = container_session.columns.columns[0].width;
        render_session(&mut container_session, vec![]);
        render_session(&mut container_session, vec![]);
        let after_container_w = container_session.columns.columns[0].width;
        assert!(
            (after_container_w - initial_container_w).abs() < 1.0,
            "container tab should keep its own initial width ({initial_container_w}), got {after_container_w}"
        );
        assert!(
            (after_container_w - resized_app_w).abs() > 20.0,
            "container tab must not inherit app tab's resized width ({resized_app_w})"
        );

        // 3. Chuyển ngược về tab App: độ rộng cột đã kéo của App vẫn còn nguyên vẹn.
        render_session(&mut app_session, vec![]);
        render_session(&mut app_session, vec![]);
        let app_w_after_switch_back = app_session.columns.columns[0].width;
        assert!(
            (app_w_after_switch_back - resized_app_w).abs() < 0.5,
            "app tab width must stay {resized_app_w} after switching back, got {app_w_after_switch_back}"
        );
    }
}
