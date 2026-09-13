use super::counter::render_log_counter;
use crate::actions::AppAction;
use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme;
use eframe::egui::{self, Rounding, Stroke};

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
    let columns_btn = egui::Button::new(
        egui::RichText::new(format!("📊 ({visible_count})"))
            .size(11.5)
            .strong()
            .color(theme::TEXT_PRIMARY),
    )
    .fill(if session.view.columns.is_modal_open {
        theme::BG_SURFACE1
    } else {
        theme::BG_SURFACE0
    })
    .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
    .rounding(Rounding::same(4.0));

    if ui
        .add(columns_btn)
        .on_hover_text("Configure visible columns and adjust their display order")
        .clicked()
    {
        dispatch(AppAction::OpenColumnsModal);
    }

    ui.add_space(2.0);

    // Source Parameters Modal Button
    let params_btn = egui::Button::new(
        egui::RichText::new("⚙")
            .size(11.5)
            .strong()
            .color(theme::TEXT_PRIMARY),
    )
    .fill(theme::BG_SURFACE0)
    .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
    .rounding(Rounding::same(4.0));

    if ui
        .add(params_btn)
        .on_hover_text("Configure engine buffer, display limits & sources")
        .clicked()
    {
        dispatch(AppAction::OpenLaunchModal);
    }

    ui.add_space(2.0);

    // Stop / Restart Source Button
    if session.session.is_source_running {
        let stop_btn = egui::Button::new(
            egui::RichText::new("⏹")
                .size(11.5)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        )
        .fill(theme::BTN_STOP_BG)
        .stroke(Stroke::new(1.0, theme::BTN_STOP_BORDER))
        .rounding(Rounding::same(4.0));

        if ui
            .add(stop_btn)
            .on_hover_text("Stop running process source")
            .clicked()
        {
            dispatch(AppAction::StopSource);
        }
    } else {
        let restart_btn = egui::Button::new(
            egui::RichText::new("🔄")
                .size(11.5)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        )
        .fill(theme::BTN_RESTART_BG)
        .stroke(Stroke::new(1.0, theme::BTN_RESTART_BORDER))
        .rounding(Rounding::same(4.0));

        if ui
            .add(restart_btn)
            .on_hover_text("Clear logs and restart source")
            .clicked()
        {
            dispatch(AppAction::RestartSource);
        }
    }

    ui.add_space(4.0);

    // Snapshot Button (Chỉ hiển thị khi đang xem Tab Raw Stream)
    if session.view.active_tab == ActiveTab::Unfiltered {
        let snapshot_tooltip = if session.view.unfiltered.is_live {
            "Freeze current Raw Stream into a fixed snapshot at this moment"
        } else {
            "Re-capture the latest surrounding context snapshot from buffer"
        };

        let snapshot_btn = egui::Button::new(
            egui::RichText::new("📸")
                .size(11.0)
                .color(theme::TEXT_PRIMARY)
                .strong(),
        )
        .fill(theme::BG_SURFACE0)
        .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
        .rounding(Rounding::same(4.0));

        if ui
            .add(snapshot_btn)
            .on_hover_text(snapshot_tooltip)
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

    let (latch_text, latch_text_color, latch_bg, latch_border) = if is_live {
        (
            "⚓ Live",
            theme::COLOR_INFO,
            theme::BTN_LATCHED_BG,
            theme::BTN_LATCHED_BORDER,
        )
    } else {
        (
            "⏸ Paused",
            theme::COLOR_WARN,
            theme::BTN_UNLATCHED_BG,
            theme::BTN_UNLATCHED_BORDER,
        )
    };

    let latch_btn = egui::Button::new(
        egui::RichText::new(latch_text)
            .size(11.5)
            .color(latch_text_color)
            .strong(),
    )
    .fill(latch_bg)
    .stroke(Stroke::new(1.0, latch_border))
    .rounding(Rounding::same(4.0));

    if ui.add(latch_btn).on_hover_text(toggle_tooltip).clicked() {
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

            let badge = egui::Button::new(
                egui::RichText::new(format!("{spinner_char} Env loading..."))
                    .size(10.5)
                    .strong()
                    .color(theme::COLOR_INFO),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            ui.add(badge)
                .on_hover_text("Loading system environment variables");
            ui.ctx().request_repaint();
        }
        uwu_core_workspace::EnvLoadStatus::Ready { .. }
        | uwu_core_workspace::EnvLoadStatus::Idle => {}
        uwu_core_workspace::EnvLoadStatus::Failed { error } => {
            ui.add_space(2.0);
            let badge = egui::Button::new(
                egui::RichText::new("⚠️ env error")
                    .size(10.5)
                    .strong()
                    .color(theme::COLOR_WARN),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::COLOR_WARN))
            .rounding(Rounding::same(4.0));

            let resp = ui.add(badge).on_hover_text(format!(
                "Lỗi nạp biến môi trường:\n{error}\n• Click để thử lại (Retry)"
            ));
            if resp.clicked() {
                // Tái nạp môi trường cho session hiện tại
                dispatch(AppAction::StartSource);
            }
        }
    }
}
