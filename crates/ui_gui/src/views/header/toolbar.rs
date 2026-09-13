use super::counter::render_log_counter;
use crate::actions::AppAction;
use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme;
use eframe::egui::{self, Stroke};

pub fn render_toolbar(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    // Table Columns & Ordering Modal Button
    let visible_count = session
        .view
        .columns
        .columns
        .iter()
        .filter(|c| c.visible)
        .count();
    let col_label = format!("📊 ({visible_count})");
    if crate::components::ui::AppButton::new()
        .label(&col_label)
        .selected(session.view.columns.is_modal_open)
        .tooltip("Configure visible columns and adjust their display order")
        .show(ui)
        .clicked()
    {
        dispatch(AppAction::OpenColumnsModal);
    }

    ui.add_space(2.0);

    // Source Parameters Modal Button
    if crate::components::ui::IconButton::new("⚙")
        .tooltip("Configure engine buffer, display limits & sources")
        .show(ui)
        .clicked()
    {
        dispatch(AppAction::OpenLaunchModal);
    }

    ui.add_space(2.0);

    // Stop / Restart Source Button
    if session.session.is_source_running {
        if crate::components::ui::IconButton::new("⏹")
            .fill(theme::BTN_STOP_BG)
            .stroke(Stroke::new(1.0, theme::BTN_STOP_BORDER))
            .tooltip("Stop running process source")
            .show(ui)
            .clicked()
        {
            dispatch(AppAction::StopSource);
        }
    } else if crate::components::ui::IconButton::new("🔄")
        .fill(theme::BTN_RESTART_BG)
        .stroke(Stroke::new(1.0, theme::BTN_RESTART_BORDER))
        .tooltip("Clear logs and restart source")
        .show(ui)
        .clicked()
    {
        dispatch(AppAction::RestartSource);
    }

    ui.add_space(4.0);

    // Snapshot Button (Chỉ hiển thị khi đang xem Tab Raw Stream)
    if session.view.active_tab == ActiveTab::Unfiltered {
        let snapshot_tooltip = if session.view.unfiltered.is_live {
            "Freeze current Raw Stream into a fixed snapshot at this moment"
        } else {
            "Re-capture the latest surrounding context snapshot from buffer"
        };

        if crate::components::ui::IconButton::new("📸")
            .tooltip(snapshot_tooltip)
            .show(ui)
            .clicked()
        {
            if session.view.unfiltered.is_live {
                dispatch(AppAction::ToggleUnfilteredLive);
            } else {
                dispatch(AppAction::RefreshUnfilteredSnapshot);
            }
        }

        ui.add_space(4.0);
    }

    // Latch / Live / Paused State Toggle Button
    let (is_live, toggle_tooltip) = match session.view.active_tab {
        ActiveTab::Filtered => (
            session.view.viewport.is_auto_scroll,
            if session.view.viewport.is_auto_scroll {
                "Main Stream: LIVE (Following tail)\n• Click to pause (Unlatch)\n• Scroll up or select a log to unlatch"
            } else {
                "Main Stream: PAUSED (View frozen)\n• Click to live stream & scroll to bottom"
            },
        ),
        ActiveTab::Unfiltered => (
            session.view.unfiltered.is_live,
            if session.view.unfiltered.is_live {
                "Raw Stream: LIVE (Following real-time stream)\n• Click to pause / freeze snapshot"
            } else {
                "Raw Stream: PAUSED (Snapshot frozen)\n• Click to follow live real-time stream"
            },
        ),
    };

    let (latch_text, latch_bg, latch_border) = if is_live {
        ("⚓ Live", theme::BTN_LATCHED_BG, theme::BTN_LATCHED_BORDER)
    } else {
        (
            "⏸ Paused",
            theme::BTN_UNLATCHED_BG,
            theme::BTN_UNLATCHED_BORDER,
        )
    };

    if crate::components::ui::AppButton::new()
        .label(latch_text)
        .fill(latch_bg)
        .stroke(Stroke::new(1.0, latch_border))
        .tooltip(toggle_tooltip)
        .show(ui)
        .clicked()
    {
        match session.view.active_tab {
            ActiveTab::Filtered => {
                dispatch(AppAction::ToggleLatch);
            }
            ActiveTab::Unfiltered => {
                dispatch(AppAction::ToggleUnfilteredLive);
            }
        }
    }

    ui.add_space(4.0);

    // Log Count / Filter Matched Indicator
    render_log_counter(ui, session);
}

/// Hiển thị chỉ báo trạng thái nạp biến môi trường của Workspace
pub fn render_environment_status(
    ui: &mut egui::Ui,
    session: &GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    match &session.session.env_status {
        uwu_core_workspace::EnvLoadStatus::Loading { .. } => {
            ui.add_space(2.0);
            let spinner_frames = ['◐', '◓', '◑', '◒'];
            let frame_idx = (ui.input(|i| i.time) * 6.0) as usize % spinner_frames.len();
            let spinner_char = spinner_frames[frame_idx];
            let label = format!("{spinner_char} Env loading...");

            crate::components::ui::AppButton::new()
                .label(&label)
                .small()
                .tooltip("Loading system environment variables")
                .show(ui);
            ui.ctx().request_repaint();
        }
        uwu_core_workspace::EnvLoadStatus::Ready { .. }
        | uwu_core_workspace::EnvLoadStatus::Idle => {}
        uwu_core_workspace::EnvLoadStatus::Failed { error } => {
            ui.add_space(2.0);
            let tooltip = format!("Lỗi nạp biến môi trường:\n{error}\n• Click để thử lại (Retry)");
            if crate::components::ui::AppButton::new()
                .label("⚠️ env error")
                .small()
                .stroke(Stroke::new(1.0, theme::COLOR_WARN))
                .tooltip(&tooltip)
                .show(ui)
                .clicked()
            {
                // Tái nạp môi trường cho session hiện tại
                dispatch(AppAction::StartSource);
            }
        }
    }
}
