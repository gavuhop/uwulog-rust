use super::counter::render_log_counter;
use crate::actions::AppAction;
use crate::components::ui::IconName;
use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme::ActiveTheme;
use eframe::egui;

pub fn render_toolbar(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    let theme = ui.app_theme();

    // Table Columns & Ordering Modal Button
    let col_variant = if session.view.columns.is_modal_open {
        crate::components::ui::ButtonVariant::Selected
    } else {
        crate::components::ui::ButtonVariant::Default
    };
    if crate::components::ui::IconButton::new(IconName::TableColumns)
        .size(24.0)
        .variant(col_variant)
        .selected(session.view.columns.is_modal_open)
        .tooltip("Configure visible columns and adjust their display order (Alt+C)")
        .show(ui)
        .clicked()
    {
        dispatch(AppAction::OpenColumnsModal);
    }

    ui.add_space(2.0);

    // Source Parameters Modal Button (Run Command Settings)
    if crate::components::ui::IconButton::new(IconName::Settings)
        .size(24.0)
        .variant(crate::components::ui::ButtonVariant::Default)
        .tooltip("Configure run command, engine buffer & sources (Alt+R)")
        .show(ui)
        .clicked()
    {
        dispatch(AppAction::OpenLaunchModal);
    }

    ui.add_space(2.0);

    // Stop / Restart Source Button
    if session.session.is_source_running {
        if crate::components::ui::IconButton::new(IconName::Stop)
            .size(24.0)
            .fill(theme.controls.stop_bg)
            .text_color(theme.status.error)
            .tooltip("Stop running process source")
            .show(ui)
            .clicked()
        {
            dispatch(AppAction::StopSource);
        }
    } else if crate::components::ui::IconButton::new(IconName::Restart)
        .size(24.0)
        .fill(theme.controls.restart_bg)
        .text_color(theme.status.info)
        .tooltip("Clear logs and restart command source (Ctrl+R, F5)")
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

        if crate::components::ui::IconButton::new(IconName::Camera)
            .size(24.0)
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

    let (latch_icon, latch_text, latch_bg, latch_fg) = if is_live {
        (
            IconName::Anchor,
            "Live",
            theme.controls.latched_bg,
            theme.status.info,
        )
    } else {
        (
            IconName::Pause,
            "Paused",
            theme.controls.unlatched_bg,
            theme.status.warning,
        )
    };

    let pause_measure = ui.painter().layout_no_wrap(
        "Paused".to_string(),
        egui::FontId::proportional(12.0),
        egui::Color32::PLACEHOLDER,
    );
    let paused_width = (pause_measure.size().x + 14.0 + 5.0 + 16.0).ceil();

    if crate::components::ui::AppButton::new()
        .icon(latch_icon)
        .label(latch_text)
        .min_width(paused_width)
        .fill(latch_bg)
        .text_color(latch_fg)
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
                .icon(IconName::AlertTriangle)
                .label("env error")
                .small()
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
