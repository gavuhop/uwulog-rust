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
    let col_variant = if session.view.shared.columns.is_modal_open {
        crate::components::ui::ButtonVariant::Selected
    } else {
        crate::components::ui::ButtonVariant::Default
    };
    if crate::components::ui::IconButton::new(IconName::TableColumns)
        .size(24.0)
        .variant(col_variant)
        .selected(session.view.shared.columns.is_modal_open)
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

    // Latch / Live / Paused State Toggle Button
    let is_live = session.view.stream(session.view.active_tab).is_auto_scroll;
    let toggle_tooltip = match (session.view.active_tab, is_live) {
        (ActiveTab::Filtered, true) => {
            "Main Stream: LIVE (Following tail)\n• Click to pause (Unlatch)\n• Scroll up or select a log to unlatch"
        }
        (ActiveTab::Filtered, false) => {
            "Main Stream: PAUSED (View frozen)\n• Click to live stream & scroll to bottom"
        }
        (ActiveTab::Unfiltered, true) => {
            "Raw Stream: LIVE (Following real-time stream)\n• Click to pause / freeze snapshot"
        }
        (ActiveTab::Unfiltered, false) => {
            "Raw Stream: PAUSED (Snapshot frozen)\n• Click to follow live real-time stream"
        }
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
        dispatch(AppAction::ToggleLatch);
    }

    ui.add_space(8.0);

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

/// Hiển thị toast popup góc dưới phải khi log vượt quá `max_capacity` của bộ đệm trong RAM
pub fn render_buffer_overflow_popup(ctx: &egui::Context, session: &mut GuiSession) {
    let spilled = session.session.engine.total_spilled();
    if spilled == 0 || session.view.buffer_overflow_dismissed {
        return;
    }

    let theme = ctx.app_theme();
    let max_cap = session.session.engine.max_capacity();
    let mut dismiss_clicked = false;

    egui::Area::new(egui::Id::new(("buffer_overflow_toast", session.session.id)))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(ctx, |ui| {
            let frame = egui::Frame::default()
                .fill(theme.surfaces.mantle)
                .stroke(egui::Stroke::new(1.0, theme.borders.border))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 10))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 6],
                    blur: 20,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(55),
                });

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon_r, _) =
                        ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
                    IconName::AlertTriangle.paint(ui.painter(), icon_r, theme.status.warning);
                    ui.add_space(6.0);

                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Buffer limit exceeded")
                                .size(12.0)
                                .strong()
                                .color(theme.text.primary),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Buffer exceeded {} logs — spilling older logs to disk ({} spilled)",
                                crate::theme::format_number(max_cap),
                                crate::theme::format_number(spilled as usize),
                            ))
                            .size(11.0)
                            .color(theme.text.muted),
                        );
                    });

                    ui.add_space(8.0);

                    if crate::components::ui::IconButton::new(IconName::Close)
                        .size(22.0)
                        .tooltip("Dismiss")
                        .show(ui)
                        .clicked()
                    {
                        dismiss_clicked = true;
                    }
                });
            });
        });

    if dismiss_clicked {
        session.view.buffer_overflow_dismissed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_core_schema::{RawLogEntry, RawPayload};
    use uwu_core_workspace::{SourceConfig, SourceType, WorkspaceLocation, WorkspaceSession};

    #[tokio::test]
    async fn test_buffer_overflow_popup_visibility_and_reset() {
        let ws = WorkspaceSession::new(
            "OverflowTest".to_string(),
            WorkspaceLocation::local("/tmp"),
            SourceConfig {
                source_type: SourceType::Process,
                command_str: String::new(),
                file_path: String::new(),
                working_dir: String::new(),
                capacity: 5,
                display_limit: 5,
            },
        );
        let mut session = GuiSession::new(ws);
        let ctx = egui::Context::default();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };

        // 1. Chưa tràn buffer -> không hiển thị popup
        let mut out = ctx.run_ui(raw_input.clone(), |ui| {
            render_buffer_overflow_popup(ui.ctx(), &mut session);
        });
        out.textures_delta.clear();
        assert!(out.shapes.is_empty());

        // 2. Nạp vượt capacity (5) -> tổng spilled > 0 -> hiển thị popup
        let tx = session.session.engine.get_channel();
        for i in 0..8 {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(format!("msg {i}")),
            })
            .await
            .unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        session.session.engine.flush_spill();
        assert_eq!(session.session.engine.total_spilled(), 3);

        let mut out = ctx.run_ui(raw_input.clone(), |ui| {
            render_buffer_overflow_popup(ui.ctx(), &mut session);
        });
        out.textures_delta.clear();
        assert!(!out.shapes.is_empty());

        // 3. Đã dismiss -> không hiển thị lại nữa; reset_stream_data -> cho phép thông báo lại
        session.view.buffer_overflow_dismissed = true;
        let mut out = ctx.run_ui(raw_input, |ui| {
            render_buffer_overflow_popup(ui.ctx(), &mut session);
        });
        out.textures_delta.clear();
        assert!(out.shapes.is_empty());

        session.view.reset_stream_data();
        assert!(!session.view.buffer_overflow_dismissed);
    }
}
