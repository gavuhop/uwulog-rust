pub mod detail;
pub mod header;
pub mod modals;
pub mod remote_servers;
pub mod table;
pub mod unfiltered;

pub use detail::render_detail;
pub use header::render_header;
pub use modals::{
    render_columns_modal, render_launch_modal, render_project_picker_popup, ProjectPickerArgs,
};
pub use table::render_table;
pub use unfiltered::render_unfiltered_table;

use crate::app::UwuGuiApp;
use crate::theme::ActiveTheme;
use eframe::egui;

pub fn render_ui(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let theme = ui.app_theme();
    let ctx_handle = ui.ctx().clone();
    let ctx = &ctx_handle;
    let visuals = theme.create_visuals();
    ctx.set_visuals(visuals.clone());
    ui.style_mut().visuals = visuals;

    let mut pending_actions: Vec<crate::actions::AppAction> = Vec::new();
    let mut dispatch = |action: crate::actions::AppAction| {
        pending_actions.push(action);
    };

    // Điều phối phím tắt tập trung qua KeymapManager (Zed-style Frame Dispatch Pipeline)
    app.handle_keybindings(ctx, &mut dispatch);

    let is_launch_open = app.is_overlay_open(crate::overlay::OverlayLayer::LaunchModal);
    let is_about_open = app.is_overlay_open(crate::overlay::OverlayLayer::AboutModal);
    let is_remote_open = app.is_overlay_open(crate::overlay::OverlayLayer::RemoteServersModal);

    let active_index = app.workspaces.active_index;
    let session_summaries: Vec<modals::ProjectPickerSessionInfo> = app
        .workspaces
        .sessions
        .iter()
        .map(|s| {
            let icon = s.session.icon();
            modals::ProjectPickerSessionInfo {
                id: s.session.id,
                name: s.session.name.clone(),
                server_name: s.session.server_name().map(|n| n.to_string()),
                icon,
                target_summary: s.session.target_summary(),
                normalized_dir: s.session.location.normalized_dir(),
            }
        })
        .collect();

    let project_shortcut = app
        .keymap
        .get_label_str(
            &crate::keymap::KeyAction::ToggleProjectPicker,
            crate::keymap::KeyContext::Global,
        )
        .unwrap_or_default();
    let mut header_cx = header::HeaderContext {
        overlay_stack: &mut app.overlays.stack,
        store: &app.workspaces.store,
        sessions: &session_summaries,
        active_index,
        project_search_query: &mut app.overlays.project_search_query,
        project_shortcut,
    };

    let active_session = &mut app.workspaces.sessions[active_index];

    // Top Panel: Unified 1-Tier Modern Custom Title & Header Bar
    let platform = header::WindowControlsPlatform::current();

    egui::Panel::top("header_panel")
        .frame(
            egui::Frame::default()
                .fill(theme.surfaces.mantle)
                .inner_margin(platform.header_panel_margin())
                .stroke(egui::Stroke::new(1.0, theme.borders.border)),
        )
        .exact_size(header::TITLEBAR_HEIGHT)
        .resizable(false)
        .show(ui, |ui| {
            header::render_header(ui, active_session, &mut header_cx, &mut dispatch);
        });

    // Central Panel: Left Table View & Right Inspector Panel
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(theme.surfaces.base)
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ui, |ui| {
            if active_session.view.inspector.is_open() {
                let screen_width = ctx.viewport_rect().width();
                let panel_id = egui::Id::new("detail_inspector_panel");

                // Nếu tỷ lệ chưa hợp lệ, đặt mặc định 35% chiều rộng màn hình
                if !(0.15..=0.85).contains(&active_session.view.inspector.width_ratio) {
                    active_session.view.inspector.width_ratio = 0.35;
                }

                // Khi kích thước màn hình thay đổi (Resize cửa sổ hoặc Zoom In/Out):
                // Tự động cập nhật lại kích thước panel theo đúng tỷ lệ inspector.width_ratio!
                if app.prev_screen_width > 0.0 && (screen_width - app.prev_screen_width).abs() > 2.0
                {
                    let new_width = (screen_width * active_session.view.inspector.width_ratio)
                        .clamp(240.0, screen_width * 0.75);
                    ctx.data_mut(|d| {
                        if let Some(mut state) =
                            d.get_persisted::<egui::containers::panel::PanelState>(panel_id)
                        {
                            state.outer_rect.min.x = state.outer_rect.max.x - new_width;
                            d.insert_persisted(panel_id, state);
                        }
                    });
                }
                app.prev_screen_width = screen_width;

                // Chiều rộng pixel thực tế
                let target_width = (screen_width * active_session.view.inspector.width_ratio)
                    .clamp(240.0, screen_width * 0.75);
                let min_sidebar_width = 240.0_f32.min(screen_width * 0.4);
                let max_sidebar_width = (screen_width * 0.75).max(min_sidebar_width + 100.0);

                egui::Panel::right("detail_inspector_panel")
                    .frame(
                        egui::Frame::default()
                            .fill(theme.surfaces.mantle)
                            .inner_margin(egui::Margin::same(12))
                            .stroke(egui::Stroke::new(1.0, theme.borders.border)),
                    )
                    .resizable(true)
                    .default_size(target_width)
                    .min_size(min_sidebar_width)
                    .max_size(max_sidebar_width)
                    .show(ui, |ui| {
                        let actual_width = ui.available_width();
                        if actual_width > 50.0 && screen_width > 100.0 {
                            // Cập nhật tỷ lệ khi người dùng chủ động kéo dãn thanh Inspector
                            active_session.view.inspector.width_ratio =
                                (actual_width / screen_width).clamp(0.15, 0.75);
                        }
                        detail::render_detail(ui, active_session, &mut dispatch);
                    });
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::default()
                        .fill(theme.surfaces.base)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(ui, |ui| match active_session.view.active_tab {
                    crate::state::ActiveTab::Filtered => {
                        ui.push_id("main_filtered_table_scope", |ui| {
                            table::render_table(ui, active_session, &mut dispatch);
                        });
                    }
                    crate::state::ActiveTab::Unfiltered => {
                        ui.push_id("unfiltered_table_scope", |ui| {
                            unfiltered::render_unfiltered_table(ui, active_session, &mut dispatch);
                        });
                    }
                });
        });

    // Modal Dialog: Launch & Source Parameters
    modals::render_launch_modal(
        ctx,
        is_launch_open,
        &mut app.overlays.launch_modal_draft,
        &active_session.session.source_config,
        &app.keymap,
        &mut dispatch,
    );

    // Modal Dialog: Table Columns & Ordering
    modals::render_columns_modal(
        ctx,
        &mut active_session.view.columns,
        &app.keymap,
        &mut dispatch,
    );

    // Modal Dialog: About uwulog
    modals::render_about_modal(ctx, is_about_open, &app.keymap, &mut dispatch);

    // Modal Dialog: Remote Projects (WSL)
    remote_servers::render_remote_servers_modal(
        ctx,
        is_remote_open,
        app.overlays.remote_placement,
        &app.keymap,
        &mut app.workspaces.store,
        &mut dispatch,
    );

    // Window Resize Border Handles (Hỗ trợ kéo dãn / thu nhỏ 4 góc và 4 cạnh cửa sổ)
    header::render_window_resize_borders(ctx);

    // Drain and dispatch all pending actions
    for action in pending_actions {
        app.dispatch_action(action);
    }
}
