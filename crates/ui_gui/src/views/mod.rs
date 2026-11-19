pub mod detail;
pub mod header;
pub mod modals;
pub mod remote_servers;
pub mod table;
pub mod toasts;
pub mod welcome;

pub use detail::render_detail;
pub use header::render_header;
pub use modals::{
    render_columns_modal, render_launch_modal, render_project_picker_popup, ProjectPickerArgs,
};
pub use table::render_table;
pub use welcome::render_welcome_view;

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
    let is_keymap_open = app.is_overlay_open(crate::overlay::OverlayLayer::KeymapModal);
    let is_remote_open = app.is_overlay_open(crate::overlay::OverlayLayer::RemoteServersModal);
    let is_project_picker_open = app.is_overlay_open(crate::overlay::OverlayLayer::ProjectPicker);
    let is_source_switcher_open = app.is_overlay_open(crate::overlay::OverlayLayer::SourceSwitcher);

    let active_index = app.workspaces.active_index;
    let active_loc = app
        .workspaces
        .sessions
        .get(active_index)
        .map(|s| &s.session.location);

    let session_summaries: Vec<modals::ProjectPickerSessionInfo> = if is_project_picker_open {
        let mut list = Vec::new();
        for (ix, s) in app.workspaces.sessions.iter().enumerate() {
            if matches!(
                s.session.location,
                uwu_core_workspace::WorkspaceLocation::Remote(
                    uwu_core_workspace::RemoteConnectionOptions::Docker(_)
                )
            ) {
                let rep_ix =
                    if active_loc.is_some_and(|al| s.session.location.is_same_docker_project(al)) {
                        active_index
                    } else {
                        app.workspaces
                            .sessions
                            .iter()
                            .enumerate()
                            .find(|(_, other)| {
                                !other.session.is_following_container_logs()
                                    && s.session
                                        .location
                                        .is_same_docker_project(&other.session.location)
                            })
                            .or_else(|| {
                                app.workspaces
                                    .sessions
                                    .iter()
                                    .enumerate()
                                    .find(|(_, other)| {
                                        s.session
                                            .location
                                            .is_same_docker_project(&other.session.location)
                                    })
                            })
                            .map(|(j, _)| j)
                            .unwrap_or(ix)
                    };
                if ix != rep_ix {
                    continue;
                }
            }

            let is_active = ix == active_index
                || active_loc.is_some_and(|al| s.session.location.is_same_docker_project(al));

            let icon = s.session.icon();
            list.push(modals::ProjectPickerSessionInfo {
                session_index: ix,
                id: s.session.id,
                name: s.session.name.clone(),
                server_name: s.session.short_server_name().map(|n| n.to_string()),
                icon,
                target_summary: s.session.target_summary(),
                normalized_dir: s.session.location.normalized_dir(),
                is_active,
            });
        }
        list
    } else {
        Vec::new()
    };

    let container_sessions: Vec<header::devcontainer::ActiveContainerSessionInfo> =
        if is_source_switcher_open {
            if let Some(active_l) = active_loc {
                app.workspaces
                    .sessions
                    .iter()
                    .enumerate()
                    .filter_map(|(ix, s)| {
                        let uwu_core_workspace::WorkspaceLocation::Remote(
                            uwu_core_workspace::RemoteConnectionOptions::Docker(ref d),
                        ) = s.session.location
                        else {
                            return None;
                        };
                        if !s.session.location.is_same_docker_project(active_l) {
                            return None;
                        }
                        let container_name = d.container_name.clone();
                        let short_label = d.short_service_name().to_string();
                        Some(header::devcontainer::ActiveContainerSessionInfo {
                            session_index: ix,
                            container_name,
                            short_label,
                            source_summary: s.session.source_badge_label(),
                            is_active: ix == active_index,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

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
        container_sessions: &container_sessions,
        active_index,
        project_search_query: &mut app.overlays.project_search_query,
        project_shortcut,
        source_switcher_state: &mut app.overlays.source_switcher_state,
        rt: &app.rt,
    };

    let platform = header::WindowControlsPlatform::current();

    if !app.workspaces.has_active_session() {
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
                header::render_empty_header(ui, &mut header_cx, &mut dispatch);
            });

        welcome::render_welcome_view(ui, &mut app.workspaces.store, &mut dispatch);

        modals::render_about_modal(ctx, is_about_open, &app.keymap, &mut dispatch);
        modals::render_keymap_modal(
            ctx,
            is_keymap_open,
            &mut app.overlays.keymap_modal_state,
            &mut dispatch,
        );
        remote_servers::render_remote_servers_modal(
            ctx,
            is_remote_open,
            app.overlays.remote_placement,
            &mut app.overlays.remote_servers,
            &app.rt,
            &app.keymap,
            &mut app.workspaces.store,
            None,
            &mut dispatch,
        );
        header::render_window_resize_borders(ctx);

        for action in pending_actions {
            app.dispatch_action(action);
        }
        return;
    }

    let active_session = &mut app.workspaces.sessions[active_index];

    // Top Panel: Unified 1-Tier Modern Custom Title & Header Bar
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

    let is_any_overlay_open = !app.overlays.stack.is_empty()
        || (active_session.view.search.autocomplete.is_open
            && !active_session
                .view
                .search
                .autocomplete
                .suggestions
                .is_empty())
        || active_session.view.search.history.is_open;

    // Central Panel: Left Table View & Right Inspector Panel
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(theme.surfaces.base)
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ui, |ui| {
            if active_session.view.shared.inspector.is_open() {
                let screen_width = ctx.viewport_rect().width();
                let panel_salt = ("detail_inspector_panel", active_session.session.id);
                let panel_id = egui::Id::new(panel_salt);

                // Nếu tỷ lệ chưa hợp lệ, đặt mặc định 35% chiều rộng màn hình
                if !(0.15..=0.85).contains(&active_session.view.shared.inspector.width_ratio) {
                    active_session.view.shared.inspector.width_ratio = 0.35;
                }

                // Khi kích thước màn hình thay đổi (Resize cửa sổ hoặc Zoom In/Out):
                // Tự động cập nhật lại kích thước panel theo đúng tỷ lệ inspector.width_ratio!
                if app.prev_screen_width > 0.0 && (screen_width - app.prev_screen_width).abs() > 2.0
                {
                    let new_width = (screen_width
                        * active_session.view.shared.inspector.width_ratio)
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
                let target_width = (screen_width
                    * active_session.view.shared.inspector.width_ratio)
                    .clamp(240.0, screen_width * 0.75);
                let min_sidebar_width = 240.0_f32.min(screen_width * 0.4);
                let max_sidebar_width = (screen_width * 0.75).max(min_sidebar_width + 100.0);

                egui::Panel::right(panel_id)
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
                            active_session.view.shared.inspector.width_ratio =
                                (actual_width / screen_width).clamp(0.15, 0.75);
                        }
                        ui.push_id(active_session.session.id, |ui| {
                            detail::render_detail(ui, active_session, &mut dispatch);
                        });
                    });
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::default()
                        .fill(theme.surfaces.base)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show(ui, |ui| {
                    let tab = active_session.view.active_tab;
                    let mut table_dispatch = |action: crate::actions::AppAction| {
                        // Không cho click/cuộn xuyên qua modal/popup làm chọn dòng hoặc tắt Live
                        if is_any_overlay_open
                            && matches!(
                                action,
                                crate::actions::AppAction::Unlatch
                                    | crate::actions::AppAction::Latch
                                    | crate::actions::AppAction::SelectLog(_)
                            )
                        {
                            return;
                        }
                        dispatch(action);
                    };
                    table::render_log_table(ui, active_session, tab, &mut table_dispatch);
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
        &mut active_session.view.shared.columns,
        &app.keymap,
        &mut dispatch,
    );

    // Modal Dialog: About uwulog
    modals::render_about_modal(ctx, is_about_open, &app.keymap, &mut dispatch);

    // Modal Dialog: Keyboard Shortcuts (Key Map Manager)
    modals::render_keymap_modal(
        ctx,
        is_keymap_open,
        &mut app.overlays.keymap_modal_state,
        &mut dispatch,
    );

    // Modal Dialog: Remote Projects
    remote_servers::render_remote_servers_modal(
        ctx,
        is_remote_open,
        app.overlays.remote_placement,
        &mut app.overlays.remote_servers,
        &app.rt,
        &app.keymap,
        &mut app.workspaces.store,
        Some(&active_session.session.location),
        &mut dispatch,
    );

    // Ngăn xếp toast góc phải dưới thống nhất (cũ trên, mới dưới — không bao giờ chồng đè)
    let mut toast_items = Vec::new();
    if let Some(toast) = toasts::build_field_collision_toast(active_session) {
        toast_items.push(toast);
    }
    if let Some(toast) = toasts::build_buffer_overflow_toast(active_session) {
        toast_items.push(toast);
    }
    if let Some(toast) = header::devcontainer::poll_devcontainer_suggest_toast(
        ctx,
        active_session,
        &mut app.overlays.devcontainer_probe_cache,
        &app.rt,
    ) {
        toast_items.push(toast);
    }
    for dismissed in toasts::render_toast_stack(ctx, &toast_items, &mut dispatch) {
        match dismissed {
            toasts::ToastDismiss::Action(action) => dispatch(*action),
            toasts::ToastDismiss::BufferOverflow => {
                active_session.view.buffer_overflow_dismissed = true;
            }
            toasts::ToastDismiss::DevContainerSuggest(dir_key) => {
                app.overlays.devcontainer_probe_cache.dismiss(&dir_key);
            }
        }
    }

    // Window Resize Border Handles (Hỗ trợ kéo dãn / thu nhỏ 4 góc và 4 cạnh cửa sổ)
    header::render_window_resize_borders(ctx);

    // Drain and dispatch all pending actions
    for action in pending_actions {
        app.dispatch_action(action);
    }
}
