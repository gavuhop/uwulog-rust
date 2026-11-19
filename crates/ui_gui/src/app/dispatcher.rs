use super::{ActiveTab, AppAction, AppEvent, OverlayLayer, RemoteModalPlacement, UwuGuiApp};
use eframe::egui;
use std::sync::Arc;

impl UwuGuiApp {
    /// Khởi chạy tác vụ tìm kiếm bất đồng bộ trên threadpool (Non-blocking UI search với Cooperative Early Cancellation)
    pub fn spawn_search(&mut self) {
        let active = self.workspaces.active_session_mut();
        let (query_id, cancel_token) = active.view.search.advance_query();
        let session_id = active.session.id;
        let query = active.view.search.query.clone();
        let limit = active.session.display_limit;
        let engine = Arc::clone(&active.session.engine);
        let tx = self.event_tx.clone();
        let egui_ctx = self.egui_ctx.clone();

        self.rt.spawn_blocking(move || {
            let Some((phase1, count_task)) =
                engine.search_phase1(&query, limit, &cancel_token, query_id)
            else {
                return;
            };
            if cancel_token.load(std::sync::atomic::Ordering::Relaxed) != query_id {
                return;
            }
            let _ = tx.send(AppEvent::SearchResultsReady {
                session_id,
                query_id,
                total_matched: phase1.total_matched,
                logs: phase1.logs,
                is_count_exact: phase1.is_count_exact,
            });
            if let Some(ref c) = egui_ctx {
                c.request_repaint();
            }

            if let Some(task) = count_task {
                if let Some(exact_total) = task.execute(&cancel_token, query_id) {
                    if cancel_token.load(std::sync::atomic::Ordering::Relaxed) == query_id {
                        let _ = tx.send(AppEvent::SearchCountUpdated {
                            session_id,
                            query_id,
                            total_matched: exact_total,
                        });
                        if let Some(ref c) = egui_ctx {
                            c.request_repaint();
                        }
                    }
                }
            }
        });
    }

    /// Khởi chạy tác vụ lọc log tăng dần bất đồng bộ trên threadpool (Non-blocking incremental filter)
    pub fn spawn_incremental_filter(&mut self) {
        let active = self.workspaces.active_session_mut();
        let session_id = active.session.id;
        let query = active.view.search.query.clone();
        let prev_processed = active.view.main_tab().last_processed_count();
        let total_processed = active.session.engine.total_processed();
        active
            .view
            .main_tab_mut()
            .set_last_processed_count(total_processed);
        active.view.search.last_search_time = std::time::Instant::now();

        let stream_filtered = active.should_stream_filtered();
        let stream_unfiltered = active.should_stream_unfiltered();
        let engine = Arc::clone(&active.session.engine);
        let tx = self.event_tx.clone();
        let egui_ctx = self.egui_ctx.clone();

        self.rt.spawn_blocking(move || {
            if stream_filtered {
                let (matched, logs) = engine.filter_incremental(&query, prev_processed);
                if matched > 0 {
                    let _ = tx.send(AppEvent::IncrementalLogsReady {
                        session_id,
                        matched,
                        logs,
                        total_processed,
                    });
                }
            }
            if stream_unfiltered {
                let (count, logs) = engine.filter_incremental("", prev_processed);
                if count > 0 {
                    let _ = tx.send(AppEvent::IncrementalUnfilteredReady { session_id, logs });
                }
            }
            if let Some(c) = egui_ctx {
                c.request_repaint();
            }
        });
    }

    /// Khởi chạy tác vụ nạp trang log cũ bất đồng bộ trên threadpool (Non-blocking reverse pagination)
    pub fn spawn_reverse_pagination(&mut self, is_unfiltered: bool, page_size: usize) {
        let active = self.workspaces.active_session_mut();
        let session_id = active.session.id;
        let tab = if is_unfiltered {
            ActiveTab::Unfiltered
        } else {
            ActiveTab::Filtered
        };

        if active.view.stream(tab).reached_oldest || active.view.stream(tab).is_loading_older {
            return;
        }
        active.view.stream_mut(tab).is_loading_older = true;
        active.view.stream_mut(tab).unlatch();
        let query = if is_unfiltered {
            String::new()
        } else {
            active.view.search.query.clone()
        };
        let Some(before_id) = active.view.stream(tab).cached_logs.first().map(|e| e.id) else {
            active.view.stream_mut(tab).is_loading_older = false;
            return;
        };

        let engine = Arc::clone(&active.session.engine);
        let tx = self.event_tx.clone();
        let egui_ctx = self.egui_ctx.clone();

        self.rt.spawn_blocking(move || {
            let logs = engine.search_before(&query, before_id, page_size);
            let reached_oldest = logs.len() < page_size;
            let _ = tx.send(AppEvent::ReversePaginationReady {
                session_id,
                logs,
                is_unfiltered,
                reached_oldest,
            });
            if let Some(c) = egui_ctx {
                c.request_repaint();
            }
        });
    }

    /// Điều phối và thực thi các hành động cấp ứng dụng (Zed-style Command Dispatcher)
    pub fn dispatch_action(&mut self, action: AppAction) {
        if !self.workspaces.has_active_session() {
            match action {
                AppAction::OpenWorkspace(ws) => {
                    self.open_or_switch_workspace(&ws);
                    self.close_project_picker();
                }
                AppAction::OpenWorkspaceInNewWindow(ws) => {
                    self.open_workspace_in_new_window(&ws);
                    self.close_project_picker();
                }
                AppAction::DeleteWorkspace(id) => self.workspaces.store.remove(id),
                AppAction::ToggleProjectPicker => {
                    if self.is_overlay_open(OverlayLayer::ProjectPicker) {
                        self.close_project_picker();
                    } else {
                        self.close_overlay(OverlayLayer::RemoteServersModal);
                        self.push_overlay(OverlayLayer::ProjectPicker);
                    }
                }
                AppAction::CloseProjectPicker => self.close_project_picker(),
                AppAction::OpenRemoteServersModal => {
                    self.close_project_picker();
                    self.overlays.remote_placement = RemoteModalPlacement::TopCenter;
                    self.push_overlay(OverlayLayer::RemoteServersModal);
                }
                AppAction::CloseRemoteServersModal => {
                    self.close_overlay(OverlayLayer::RemoteServersModal);
                }
                AppAction::ToggleRemoteServersModal => {
                    if self.is_overlay_open(OverlayLayer::RemoteServersModal) {
                        self.close_overlay(OverlayLayer::RemoteServersModal);
                    } else {
                        self.close_project_picker();
                        self.overlays.remote_placement = RemoteModalPlacement::TopLeft;
                        self.push_overlay(OverlayLayer::RemoteServersModal);
                    }
                }
                AppAction::ToggleMainMenu => {
                    if self.is_overlay_open(OverlayLayer::MainMenu) {
                        self.close_main_menu();
                    } else {
                        self.push_overlay(OverlayLayer::MainMenu);
                    }
                }
                AppAction::CloseMainMenu => self.close_main_menu(),
                AppAction::OpenAboutModal => {
                    self.close_main_menu();
                    self.push_overlay(OverlayLayer::AboutModal);
                }
                AppAction::CloseAboutModal => self.close_overlay(OverlayLayer::AboutModal),
                AppAction::OpenKeymapModal => {
                    self.close_main_menu();
                    self.overlays.keymap_modal_state = Some(
                        crate::app::overlay_manager::KeymapModalState::new(&self.keymap),
                    );
                    self.push_overlay(OverlayLayer::KeymapModal);
                }
                AppAction::CloseKeymapModal => {
                    self.close_overlay(OverlayLayer::KeymapModal);
                    self.overlays.keymap_modal_state = None;
                }
                AppAction::ApplyKeymapModal(new_keymap) => {
                    self.keymap = *new_keymap;
                    let _ = crate::keymap::save_user_keymap(&self.keymap, None);
                    self.close_overlay(OverlayLayer::KeymapModal);
                    self.overlays.keymap_modal_state = None;
                }
                AppAction::QuitApp => {
                    self.close_main_menu();
                    self.should_quit = true;
                }
                AppAction::SwitchTheme(theme_id) => {
                    self.workspaces.store.active_theme = Some(theme_id);
                    let _ = self.workspaces.store.save();
                }
                AppAction::DismissTopLayer => {
                    self.dismiss_top_layer();
                }
                _ => {}
            }
            return;
        }

        if let AppAction::LoadOlderLogs(page_size) = action {
            let is_unfiltered = self.active_session().view.active_tab == ActiveTab::Unfiltered;
            self.spawn_reverse_pagination(is_unfiltered, page_size);
            return;
        }

        if self.workspaces.active_session_mut().handle_action(&action) {
            return;
        }

        match action {
            AppAction::SwitchSession(idx) => {
                self.switch_session(idx);
                self.close_project_picker();
            }
            AppAction::CloseSession(idx) => {
                if self.is_overlay_open(OverlayLayer::ProjectPicker) {
                    if let Some(target_loc) = self
                        .workspaces
                        .sessions
                        .get(idx)
                        .map(|s| s.session.location.clone())
                    {
                        if matches!(
                            target_loc,
                            uwu_core_workspace::WorkspaceLocation::Remote(
                                uwu_core_workspace::RemoteConnectionOptions::Docker(_)
                            )
                        ) {
                            let indices: Vec<usize> = self
                                .workspaces
                                .sessions
                                .iter()
                                .enumerate()
                                .filter(|(_, s)| {
                                    s.session.location.is_same_docker_project(&target_loc)
                                })
                                .map(|(i, _)| i)
                                .rev()
                                .collect();
                            for i in indices {
                                self.close_session(i);
                            }
                            return;
                        }
                    }
                }
                self.close_session(idx);
            }
            AppAction::CycleSession(forward) => self.cycle_project(forward),
            AppAction::OpenWorkspace(ws) => {
                self.open_or_switch_workspace(&ws);
                self.close_project_picker();
            }
            AppAction::OpenContainerLogSession(ws) => {
                self.workspaces.open_container_log_session(&ws, &self.rt);
                self.close_overlay(OverlayLayer::SourceSwitcher);
            }
            AppAction::OpenWorkspaceInNewWindow(ws) => {
                self.open_workspace_in_new_window(&ws);
                if let Some(pos) =
                    self.workspaces.sessions.iter().position(|s| {
                        s.session.id == ws.id || s.session.location.is_same(&ws.location)
                    })
                {
                    self.close_session(pos);
                }
                self.close_project_picker();
            }
            AppAction::OpenSessionInNewWindow(idx) => {
                if let Some(session) = self.workspaces.sessions.get(idx) {
                    let ws = session.session.to_workspace();
                    self.dispatch_action(AppAction::OpenWorkspaceInNewWindow(ws));
                }
            }
            AppAction::LoadWorkspace(ws) => {
                self.load_workspace(&ws);
                if self.overlays.launch_modal_draft.is_some() {
                    self.overlays.launch_modal_draft =
                        Some(self.active_session().session.source_config.clone());
                }
            }
            AppAction::DeleteWorkspace(id) => {
                self.workspaces.store.remove(id);
            }
            AppAction::StartSource => self.start_configured_source(),
            AppAction::StopSource => self.stop_current_source(),
            AppAction::RestartSource => self.restart_current_source(),
            AppAction::OpenLaunchModal => {
                self.overlays.launch_modal_draft =
                    Some(self.active_session().session.source_config.clone());
                self.push_overlay(OverlayLayer::LaunchModal);
            }
            AppAction::CloseLaunchModal => {
                self.close_overlay(OverlayLayer::LaunchModal);
                self.overlays.launch_modal_draft = None;
            }
            AppAction::ApplyLaunchModal => {
                if let Some(draft) = self.overlays.launch_modal_draft.take() {
                    let active = &mut self.active_session_mut().session;
                    if let uwu_core_workspace::WorkspaceLocation::Remote(
                        uwu_core_workspace::RemoteConnectionOptions::Docker(ref mut d),
                    ) = active.location
                    {
                        d.follow_container_logs = false;
                    }
                    active.source_config = draft;
                    self.save_current_workspace();
                    self.restart_current_source();
                }
                self.close_overlay(OverlayLayer::LaunchModal);
            }
            AppAction::OpenColumnsModal => {
                self.active_session_mut().view.shared.columns.open_modal();
                self.push_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::CloseColumnsModal => {
                self.active_session_mut().view.shared.columns.close_modal();
                self.close_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::ApplyColumnsModal => {
                self.active_session_mut().view.shared.columns.apply_modal();
                self.close_overlay(OverlayLayer::ColumnsModal);
                self.workspaces.save_current_workspace();
            }
            AppAction::ToggleProjectPicker => {
                if self.is_overlay_open(OverlayLayer::ProjectPicker) {
                    self.close_project_picker();
                } else {
                    self.close_overlay(OverlayLayer::RemoteServersModal);
                    self.push_overlay(OverlayLayer::ProjectPicker);
                }
            }
            AppAction::CloseProjectPicker => self.close_project_picker(),
            AppAction::OpenRemoteServersModal => {
                self.close_project_picker();
                self.overlays.remote_placement = RemoteModalPlacement::TopCenter;
                self.push_overlay(OverlayLayer::RemoteServersModal);
            }
            AppAction::CloseRemoteServersModal => {
                self.close_overlay(OverlayLayer::RemoteServersModal);
            }
            AppAction::ToggleRemoteServersModal => {
                if self.is_overlay_open(OverlayLayer::RemoteServersModal) {
                    self.close_overlay(OverlayLayer::RemoteServersModal);
                } else {
                    self.close_project_picker();
                    self.close_overlay(OverlayLayer::SourceSwitcher);
                    self.overlays.remote_placement = RemoteModalPlacement::TopLeft;
                    self.push_overlay(OverlayLayer::RemoteServersModal);
                }
            }
            AppAction::ToggleSourceSwitcher => {
                if self.is_overlay_open(OverlayLayer::SourceSwitcher) {
                    self.close_overlay(OverlayLayer::SourceSwitcher);
                } else {
                    self.close_project_picker();
                    self.close_overlay(OverlayLayer::RemoteServersModal);
                    self.close_main_menu();
                    self.push_overlay(OverlayLayer::SourceSwitcher);
                }
            }
            AppAction::CloseSourceSwitcher => {
                self.close_overlay(OverlayLayer::SourceSwitcher);
            }
            AppAction::ToggleMainMenu => {
                if self.is_overlay_open(OverlayLayer::MainMenu) {
                    self.close_main_menu();
                } else {
                    self.push_overlay(OverlayLayer::MainMenu);
                }
            }
            AppAction::CloseMainMenu => self.close_main_menu(),
            AppAction::OpenAboutModal => {
                self.close_main_menu();
                self.push_overlay(OverlayLayer::AboutModal);
            }
            AppAction::CloseAboutModal => {
                self.close_overlay(OverlayLayer::AboutModal);
            }
            AppAction::OpenKeymapModal => {
                self.close_main_menu();
                self.overlays.keymap_modal_state = Some(
                    crate::app::overlay_manager::KeymapModalState::new(&self.keymap),
                );
                self.push_overlay(OverlayLayer::KeymapModal);
            }
            AppAction::CloseKeymapModal => {
                self.close_overlay(OverlayLayer::KeymapModal);
                self.overlays.keymap_modal_state = None;
            }
            AppAction::ApplyKeymapModal(new_keymap) => {
                self.keymap = *new_keymap;
                let _ = crate::keymap::save_user_keymap(&self.keymap, None);
                self.close_overlay(OverlayLayer::KeymapModal);
                self.overlays.keymap_modal_state = None;
            }
            AppAction::QuitApp => {
                self.close_main_menu();
                self.should_quit = true;
            }
            AppAction::SwitchTheme(theme_id) => {
                self.workspaces.store.active_theme = Some(theme_id);
                let _ = self.workspaces.store.save();
            }
            AppAction::DismissTopLayer => {
                self.dismiss_top_layer();
            }
            AppAction::FocusSearch => {
                self.active_session_mut().view.search.focus_requested = true;
                let search_id = crate::views::header::search_bar::search_input_id(
                    self.active_session().session.id,
                );
                if let Some(ctx) = &self.egui_ctx {
                    ctx.memory_mut(|m| m.request_focus(search_id));
                    ctx.request_repaint();
                }
            }
            AppAction::CopySelectedLog => {
                let Some(ctx) = self.egui_ctx.clone() else {
                    return;
                };

                // 1. Ưu tiên cao nhất: nếu người dùng đang bôi đen text -> copy đoạn bôi đen
                if let Some(selected_text) = crate::actions::active_text_selection(&ctx) {
                    let copy_id = egui::Id::new("copy_raw_flash");
                    let now = ctx.input(|i| i.time);
                    ctx.data_mut(|d| d.insert_temp(copy_id, now));
                    crate::actions::copy_to_clipboard(&ctx, &selected_text);
                    ctx.request_repaint();
                    return;
                }

                // 2. Không bôi đen: copy toàn bộ dòng log đang pick (hoặc dòng visible đầu tiên)
                let session = self.active_session();
                let picked_event =
                    session
                        .view
                        .shared
                        .inspector
                        .selected_log
                        .as_ref()
                        .or_else(|| {
                            let row_idx = session.first_visible_row();
                            session.active_logs().get(row_idx)
                        });

                if let Some(event) = picked_event {
                    let text = if session.view.shared.inspector.is_beauty_payload
                        && !event.fields.is_empty()
                    {
                        event.beauty_display()
                    } else {
                        event.raw_display()
                    };

                    let copy_id = egui::Id::new("copy_raw_flash");
                    let now = ctx.input(|i| i.time);
                    ctx.data_mut(|d| d.insert_temp(copy_id, now));
                    crate::actions::copy_to_clipboard(&ctx, &text);
                    ctx.request_repaint();
                }
            }
            _ => {}
        }
    }
}
