use super::{CliArgs, GuiSession, OverlayManager, UwuGuiApp, WorkspaceManager};
use clap::Parser;
use tokio::runtime::Handle;
use uwu_core_workspace::{
    extract_project_name, SourceConfig, Workspace, WorkspaceSession, WorkspaceStore,
};

impl UwuGuiApp {
    /// Mở một Workspace trong một cửa sổ OS độc lập mới (tiến trình mới)
    pub fn open_workspace_in_new_window(&mut self, ws: &Workspace) {
        // Đảm bảo thông tin workspace đã được lưu vào persistent store
        self.workspaces.store.add_or_update(ws.clone());
        let _ = self.workspaces.store.save();

        let current_exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                eprintln!("Failed to get current executable path: {e}");
                return;
            }
        };

        let mut cmd = std::process::Command::new(current_exe);
        cmd.arg("--workspace-id").arg(ws.id.to_string());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
        }

        if let Err(e) = cmd.spawn() {
            eprintln!("Failed to spawn new window for workspace {}: {e}", ws.name);
        }
    }

    /// Khởi tạo GuiSession ban đầu từ tham số CLI và lịch sử WorkspaceStore đã lưu
    pub(crate) fn build_initial_session(
        cli: &CliArgs,
        store: &WorkspaceStore,
    ) -> (GuiSession, bool, Option<uuid::Uuid>) {
        let saved_ws = if let Some(id) = cli.workspace_id {
            store.recent_workspaces.iter().find(|ws| ws.id == id)
        } else {
            None
        };

        let (location, source_type, cmd, file, has_custom_source) = if let Some(ws) = saved_ws {
            (
                ws.location.clone(),
                ws.source_type,
                ws.command_str.clone(),
                ws.file_path.clone(),
                false,
            )
        } else {
            cli.resolve_target()
        };

        let saved_ws = saved_ws.or_else(|| {
            store
                .find_by_location(&location)
                .or_else(|| store.find_by_workdir(location.working_dir()))
        });

        let saved_id = saved_ws.map(|ws| ws.id);

        let initial_session = if let (Some(ws), false) = (saved_ws, has_custom_source) {
            WorkspaceSession::from_workspace(ws, cli.capacity, cli.display_limit)
        } else {
            let project_name = saved_ws.map(|ws| ws.name.clone()).unwrap_or_else(|| {
                let dir = location.working_dir();
                if !dir.is_empty() {
                    extract_project_name(dir)
                } else {
                    location.display_name().to_string()
                }
            });

            let source_config = SourceConfig {
                source_type,
                command_str: cmd,
                file_path: file,
                working_dir: location.working_dir().to_string(),
                capacity: cli.capacity,
                display_limit: cli.display_limit,
            };

            let mut s = WorkspaceSession::new(project_name, location, source_config);
            if let Some(ws) = saved_ws {
                s.id = ws.id;
                if !ws.env_vars.is_empty() {
                    s.env_vars = ws.env_vars.clone();
                    s.env_watch_tx.send_replace(Some(s.env_vars.clone()));
                }
            }
            s
        };

        let mut gui_session = GuiSession::new(initial_session);
        if let Some(prefs) = store.get_viewer_prefs(&gui_session.viewer_key()) {
            gui_session.apply_viewer_prefs(prefs);
        }
        if let Some(ref q) = cli.query {
            gui_session.view.search.query = q.clone();
        } else if has_custom_source {
            gui_session.view.search.query.clear();
        }

        (gui_session, has_custom_source, saved_id)
    }

    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle) -> Self {
        // Dọn sạch các thư mục spill mồ côi trên đĩa không sử dụng quá 7 ngày
        rt.spawn_blocking(|| {
            let root = uwu_core_util::app_spill_dir();
            let _ = uwu_core_util::cleanup_stale_spill_dirs(
                &root,
                std::time::Duration::from_secs(7 * 24 * 3600),
            );
        });

        let cli = CliArgs::parse();
        let mut store = WorkspaceStore::load();
        store.recent_workspaces.retain(|w| {
            !matches!(
                &w.location,
                uwu_core_workspace::WorkspaceLocation::Remote(
                    uwu_core_workspace::RemoteConnectionOptions::Docker(d)
                ) if d.follow_container_logs
            )
        });

        // Đảm bảo file template mẫu luôn tồn tại sẵn trong thư mục themes
        let _ = crate::theme::ensure_template_file();

        if let Some(ref theme_id) = store.active_theme {
            if !crate::theme::is_builtin_theme(theme_id) {
                let _ = crate::theme::load_theme_by_id(theme_id);
            }
            let _ = crate::theme::set_active_theme(theme_id, &cc.egui_ctx);
        }
        crate::theme::apply_theme(&cc.egui_ctx);
        #[cfg(target_os = "windows")]
        crate::theme::apply_windows_titlebar_theme(cc);

        let mut sessions: Vec<GuiSession> = Vec::new();
        let mut has_custom_source = false;
        let mut active_index = 0;
        let mut saved_id = None;

        if cli.has_target() {
            let (initial_gui_session, custom_source, sid) =
                Self::build_initial_session(&cli, &store);
            has_custom_source = custom_source;
            saved_id = sid;
            sessions.push(initial_gui_session);
            active_index = 0;
        } else {
            // Khôi phục các project đã mở lúc shutdown (start at shutdown)
            let open_ids = store.open_workspace_ids.clone();
            for id in open_ids {
                if let Some(ws) = store.recent_workspaces.iter().find(|w| w.id == id).cloned() {
                    if ws.is_valid_to_open()
                        && !sessions
                            .iter()
                            .any(|s| s.session.location.is_same_docker_project(&ws.location))
                    {
                        let mut gui_session =
                            GuiSession::from_workspace(&ws, cli.capacity, cli.display_limit);
                        if let Some(prefs) = store.get_viewer_prefs(&gui_session.viewer_key()) {
                            gui_session.apply_viewer_prefs(prefs);
                        }
                        gui_session.session.spawn_load_environment(&rt);
                        sessions.push(gui_session);
                    }
                    // Nếu mở lại fail thì thôi, nó vẫn là recent project (không xóa khỏi store.recent_workspaces)
                }
            }

            if !sessions.is_empty() {
                if let Some(active_id) = store.active_workspace_id {
                    if let Some(pos) = sessions.iter().position(|s| s.session.id == active_id) {
                        active_index = pos;
                    }
                }
            }
        }

        let workspaces = WorkspaceManager::with_sessions(sessions, active_index, store);
        let overlays = OverlayManager::new();

        let mut keymap = crate::keymap::KeymapManager::new();
        // Đảm bảo file cấu hình mẫu và nạp cấu hình phím tắt của người dùng nếu có
        let _ = crate::keymap::ensure_sample_config_file(None);
        let _ = crate::keymap::load_user_keymap(&mut keymap, None);

        let (event_tx, event_rx) = std::sync::mpsc::channel();

        let mut app = Self {
            workspaces,
            overlays,
            keymap,
            rt,
            prev_screen_width: 0.0,
            should_quit: false,
            event_tx,
            event_rx,
            egui_ctx: None,
        };

        if app.workspaces.has_active_session() {
            // Background task nạp biến môi trường cho session đầu tiên
            app.spawn_load_environment();

            if (saved_id.is_none() || has_custom_source) && cli.has_target() {
                // Tự động lưu workspace mới hoặc cấu hình nguồn mới vào store
                app.save_current_workspace();
            }

            if has_custom_source {
                app.start_configured_source();
            } else {
                app.workspaces.auto_start_container_log_if_needed(&app.rt);
            }
            app.active_session_mut().view.search.mark_needs_search();
        }

        app
    }
}
