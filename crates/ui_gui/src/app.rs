use crate::session_view::GuiSessionState;
use eframe::egui;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use uwu_core_schema::LogEvent;
pub use uwu_core_workspace::{
    MultiWorkspaceManager, SourceConfig, SourceType, Workspace, WorkspaceLocation,
    WorkspaceSession, WorkspaceStore, WslConfig, WslSubMode,
};

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Filtered,
    Unfiltered,
}

#[derive(Clone, Debug, Default)]
pub struct UnfilteredViewState {
    pub is_open: bool,
    pub target_id: Option<u64>,
    pub cached_unfiltered: Vec<LogEvent>,
    pub target_index: Option<usize>,
    pub request_scroll_to_target: bool,
    pub is_live: bool,
    pub request_scroll_to_bottom: bool,
    pub has_new_data: bool,
    pub snapshot_processed_count: u64,
}

pub struct UwuGuiApp {
    pub workspace_mgr: MultiWorkspaceManager,
    pub view_states: Vec<GuiSessionState>,
    pub rt: Handle,
    pub show_launch_modal: bool,
    pub project_picker_open: bool,
    pub project_search_query: String,
    pub available_wsl_distros: Vec<String>,
    pub wsl_distro_rx: Option<tokio::sync::oneshot::Receiver<Vec<String>>>,
    pub prev_screen_width: f32,
    pub workspace_store: WorkspaceStore,
}

impl std::ops::Deref for UwuGuiApp {
    type Target = GuiSessionState;

    fn deref(&self) -> &Self::Target {
        &self.view_states[self.workspace_mgr.active_index]
    }
}

impl std::ops::DerefMut for UwuGuiApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let idx = self.workspace_mgr.active_index;
        &mut self.view_states[idx]
    }
}

impl UwuGuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle) -> Self {
        crate::ui::theme::apply_theme(&cc.egui_ctx);
        #[cfg(target_os = "windows")]
        crate::ui::theme::apply_windows_titlebar_theme(cc);

        let args: Vec<String> = std::env::args().collect();
        let mut display_limit: usize = 5_000;
        let mut capacity: usize = 200_000;

        let mut cmd_to_run = String::new();
        let mut file_to_read = String::new();
        let mut working_dir = String::new();
        let mut source_type = SourceType::Process;
        let mut wsl_config = WslConfig::default();
        let mut custom_source_specified = false;

        let mut i = 1;
        while i < args.len() {
            if (args[i] == "-n" || args[i] == "--limit") && i + 1 < args.len() {
                if let Ok(val) = args[i + 1].parse::<usize>() {
                    display_limit = val;
                }
                i += 1;
            } else if (args[i] == "-cap" || args[i] == "--capacity") && i + 1 < args.len() {
                if let Ok(val) = args[i + 1].parse::<usize>() {
                    capacity = val;
                }
                i += 1;
            } else if (args[i] == "--wsl-distro") && i + 1 < args.len() {
                wsl_config.distro = args[i + 1].clone();
                i += 1;
            } else if (args[i] == "--wsl-dir" || args[i] == "--wsl-cwd") && i + 1 < args.len() {
                wsl_config.working_dir = args[i + 1].clone();
                i += 1;
            } else if (args[i] == "--wsl-cmd") && i + 1 < args.len() {
                wsl_config.command_str = args[i + 1].clone();
                wsl_config.sub_mode = WslSubMode::Command;
                source_type = SourceType::Wsl;
                custom_source_specified = true;
                i += 1;
            } else if (args[i] == "--wsl-file") && i + 1 < args.len() {
                wsl_config.file_path = args[i + 1].clone();
                wsl_config.sub_mode = WslSubMode::File;
                source_type = SourceType::Wsl;
                custom_source_specified = true;
                i += 1;
            } else if (args[i] == "-r"
                || args[i] == "--run"
                || args[i] == "-c"
                || args[i] == "--cmd")
                && i + 1 < args.len()
            {
                cmd_to_run = args[i + 1].clone();
                source_type = SourceType::Process;
                custom_source_specified = true;
                i += 1;
            } else if (args[i] == "-f" || args[i] == "--file") && i + 1 < args.len() {
                file_to_read = args[i + 1].clone();
                source_type = SourceType::File;
                custom_source_specified = true;
                i += 1;
            } else if !args[i].starts_with('-') {
                file_to_read = args[i].clone();
                source_type = SourceType::File;
                custom_source_specified = true;
            }
            i += 1;
        }

        let workspace_store = WorkspaceStore::load();

        let is_wsl_invoked = args.contains(&"--wsl-distro".to_string())
            || args.contains(&"--wsl-cmd".to_string())
            || args.contains(&"--wsl-file".to_string())
            || args.contains(&"--wsl-dir".to_string())
            || source_type == SourceType::Wsl;

        if is_wsl_invoked && source_type != SourceType::Wsl {
            source_type = SourceType::Wsl;
        }

        let active_workdir = if is_wsl_invoked {
            if !wsl_config.working_dir.is_empty() {
                wsl_config.working_dir.clone()
            } else {
                "/home".to_string()
            }
        } else if !working_dir.is_empty() {
            working_dir.clone()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.to_string_lossy().to_string()
        } else {
            String::new()
        };

        if is_wsl_invoked && wsl_config.working_dir.is_empty() {
            wsl_config.working_dir = active_workdir.clone();
        } else if working_dir.is_empty() {
            working_dir = active_workdir.clone();
        }

        fn extract_project_name(path_str: &str) -> String {
            let clean = path_str.trim().trim_end_matches(&['/', '\\'][..]);
            if clean.is_empty() {
                return "Workspace".to_string();
            }
            let parts: Vec<&str> = clean
                .split(&['/', '\\'][..])
                .filter(|s| !s.is_empty())
                .collect();
            if let Some(last) = parts.last() {
                last.to_string()
            } else {
                "Workspace".to_string()
            }
        }

        let args_specified_target = custom_source_specified || is_wsl_invoked;

        // Nếu người dùng không chỉ định tham số CLI cụ thể, ưu tiên mở lại active workspace gần nhất (nếu có),
        // hoặc tìm theo active_workdir
        let saved_ws = if !args_specified_target {
            workspace_store
                .get_active()
                .cloned()
                .or_else(|| workspace_store.find_by_workdir(&active_workdir).cloned())
        } else {
            workspace_store.find_by_workdir(&active_workdir).cloned()
        };

        let initial_project_name = if let Some(ref ws) = saved_ws {
            ws.name.clone()
        } else if is_wsl_invoked {
            if !wsl_config.working_dir.is_empty() {
                extract_project_name(&wsl_config.working_dir)
            } else {
                format!("WSL ({})", wsl_config.distro)
            }
        } else if !working_dir.is_empty() {
            extract_project_name(&working_dir)
        } else {
            "Workspace".to_string()
        };

        let initial_location = if let Some(ref ws) = saved_ws {
            ws.location.clone()
        } else if is_wsl_invoked {
            WorkspaceLocation::Wsl {
                distro: wsl_config.distro.clone(),
                working_dir: wsl_config.working_dir.clone(),
            }
        } else {
            WorkspaceLocation::Local {
                working_dir: working_dir.clone(),
            }
        };

        // Kích hoạt background task kiểm tra danh sách WSL Distros
        let (wsl_tx, wsl_rx) = tokio::sync::oneshot::channel();
        rt.spawn(async move {
            let distros = uwu_driver_transport::WslTransport::detect_distros();
            let _ = wsl_tx.send(distros);
        });

        let mut source_config = SourceConfig {
            source_type,
            command_str: cmd_to_run,
            file_path: file_to_read,
            working_dir,
            wsl_config,
            capacity,
            display_limit,
        };

        let mut initial_query = String::new();

        // Nếu có saved_ws và không có cờ custom ghi đè, nạp đầy đủ cấu hình (bao gồm cả WSL)
        if !custom_source_specified {
            if let Some(ref ws) = saved_ws {
                initial_query = ws.last_query.clone();
                match ws.source_type {
                    SourceType::Wsl => {
                        source_config.source_type = SourceType::Wsl;
                        if !ws.command_str.is_empty() {
                            source_config.wsl_config.sub_mode = WslSubMode::Command;
                            source_config.wsl_config.command_str = ws.command_str.clone();
                        } else if !ws.file_path.is_empty() {
                            source_config.wsl_config.sub_mode = WslSubMode::File;
                            source_config.wsl_config.file_path = ws.file_path.clone();
                        }
                        if let WorkspaceLocation::Wsl {
                            distro,
                            working_dir,
                        } = &ws.location
                        {
                            source_config.wsl_config.distro = distro.clone();
                            source_config.wsl_config.working_dir = working_dir.clone();
                        }
                    }
                    SourceType::File => {
                        source_config.source_type = SourceType::File;
                        source_config.file_path = ws.file_path.clone();
                    }
                    SourceType::Process => {
                        source_config.source_type = SourceType::Process;
                        source_config.command_str = ws.command_str.clone();
                    }
                }
            }
        }

        let mut initial_session = if let Some(ref ws) = saved_ws {
            if !custom_source_specified {
                let mut s = WorkspaceSession::from_workspace(ws, capacity, display_limit);
                if !source_config.wsl_config.distro.is_empty() {
                    s.source_config.wsl_config.distro = source_config.wsl_config.distro.clone();
                }
                s
            } else {
                WorkspaceSession::new(
                    initial_project_name.clone(),
                    initial_location,
                    source_config.clone(),
                )
            }
        } else {
            WorkspaceSession::new(
                initial_project_name.clone(),
                initial_location,
                source_config.clone(),
            )
        };

        if let Some(ref ws) = saved_ws {
            if !ws.env_vars.is_empty() {
                initial_session.env_vars = ws.env_vars.clone();
                initial_session
                    .env_watch_tx
                    .send_replace(Some(initial_session.env_vars.clone()));
            }
        }

        let mut view_state = GuiSessionState::new(
            initial_session.engine.clone(),
            initial_session.source_config.clone(),
            initial_session.name.clone(),
        );
        view_state.query = initial_query;

        let workspace_mgr = MultiWorkspaceManager::new(initial_session, workspace_store.clone());

        let mut app = Self {
            workspace_mgr,
            view_states: vec![view_state],
            rt,
            show_launch_modal: false,
            project_picker_open: false,
            project_search_query: String::new(),
            available_wsl_distros: Vec::new(),
            wsl_distro_rx: Some(wsl_rx),
            prev_screen_width: 0.0,
            workspace_store,
        };

        // Background task nạp biến môi trường cho session đầu tiên
        app.spawn_load_environment();

        if custom_source_specified {
            app.save_current_workspace();
            app.start_configured_source();
        }
        app.trigger_full_search();

        app
    }

    #[allow(dead_code)]
    pub fn active_session(&self) -> &WorkspaceSession {
        self.workspace_mgr.active_session()
    }

    #[allow(dead_code)]
    pub fn active_session_mut(&mut self) -> &mut WorkspaceSession {
        self.workspace_mgr.active_session_mut()
    }

    pub fn switch_session(&mut self, index: usize) {
        if index < self.workspace_mgr.sessions.len() {
            self.workspace_mgr.switch_session(index);
            while self.view_states.len() <= self.workspace_mgr.active_index {
                let s = &self.workspace_mgr.sessions[self.view_states.len()];
                self.view_states.push(GuiSessionState::new(
                    s.engine.clone(),
                    s.source_config.clone(),
                    s.name.clone(),
                ));
            }

            // Đồng bộ trạng thái từ active session sang view state
            let idx = self.workspace_mgr.active_index;
            let session = &self.workspace_mgr.sessions[idx];
            self.view_states[idx].engine = session.engine.clone();
            self.view_states[idx].source_config = session.source_config.clone();
            self.view_states[idx].is_source_running = session.is_source_running;
            self.view_states[idx].project_name_input = session.name.clone();
            self.view_states[idx].env_status = session.env_status.clone();
            self.view_states[idx].env_vars = session.env_vars.clone();

            self.workspace_store = self.workspace_mgr.store.clone();
            self.trigger_full_search();
        }
    }

    pub fn open_or_switch_workspace(&mut self, ws: &Workspace) {
        let idx = self
            .workspace_mgr
            .open_or_switch_workspace(ws, 200_000, 5_000, &self.rt, false);

        while self.view_states.len() < self.workspace_mgr.sessions.len() {
            let s = &self.workspace_mgr.sessions[self.view_states.len()];
            let mut vs =
                GuiSessionState::new(s.engine.clone(), s.source_config.clone(), s.name.clone());
            vs.query = ws.last_query.clone();
            self.view_states.push(vs);
        }

        self.switch_session(idx);
    }

    pub fn close_session(&mut self, index: usize) {
        if index < self.view_states.len() {
            self.view_states.remove(index);
        }

        self.workspace_mgr.close_session(index, 200_000, 5_000);

        if self.view_states.is_empty() {
            let s = &self.workspace_mgr.sessions[0];
            self.view_states.push(GuiSessionState::new(
                s.engine.clone(),
                s.source_config.clone(),
                s.name.clone(),
            ));
        }

        if self.workspace_mgr.active_index >= self.view_states.len() {
            self.workspace_mgr.active_index = self.view_states.len() - 1;
        }

        self.workspace_store = self.workspace_mgr.store.clone();
        self.switch_session(self.workspace_mgr.active_index);
    }

    #[allow(dead_code)]
    pub fn cycle_project(&mut self, forward: bool) {
        self.workspace_mgr.cycle_session(forward);
        self.switch_session(self.workspace_mgr.active_index);
    }

    pub fn save_current_workspace(&mut self) {
        let active_idx = self.workspace_mgr.active_index;
        let view = &self.view_states[active_idx];
        let name = if view.project_name_input.trim().is_empty() {
            "Workspace".to_string()
        } else {
            view.project_name_input.trim().to_string()
        };

        let session = &mut self.workspace_mgr.sessions[active_idx];
        session.name = name;
        session.source_config = view.source_config.clone();

        match view.source_config.source_type {
            SourceType::Wsl => {
                session.location = WorkspaceLocation::Wsl {
                    distro: view.source_config.wsl_config.distro.clone(),
                    working_dir: view.source_config.wsl_config.working_dir.clone(),
                };
            }
            _ => {
                let dir = if !view.source_config.working_dir.trim().is_empty() {
                    view.source_config.working_dir.clone()
                } else {
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                session.location = WorkspaceLocation::Local { working_dir: dir };
            }
        }

        let mut ws = session.to_workspace();
        ws.last_query = view.query.clone();
        self.workspace_mgr.store.add_or_update(ws.clone());
        self.workspace_store = self.workspace_mgr.store.clone();
    }

    pub fn load_workspace(&mut self, ws: &Workspace) {
        let active_idx = self.workspace_mgr.active_index;
        self.workspace_mgr.sessions[active_idx].apply_workspace(ws);

        let view = &mut self.view_states[active_idx];
        view.project_name_input = ws.name.clone();
        view.query = ws.last_query.clone();
        view.source_config = self.workspace_mgr.sessions[active_idx]
            .source_config
            .clone();
        view.env_vars = self.workspace_mgr.sessions[active_idx].env_vars.clone();

        self.workspace_mgr.sessions[active_idx].spawn_load_environment(&self.rt);
        self.save_current_workspace();
    }

    pub fn start_configured_source(&mut self) {
        self.save_current_workspace();
        let active_idx = self.workspace_mgr.active_index;
        self.workspace_mgr.sessions[active_idx].source_config = self.source_config.clone();
        self.workspace_mgr.sessions[active_idx].start_source(&self.rt);
        self.is_source_running = true;
    }

    pub fn stop_current_source(&mut self) {
        let active_idx = self.workspace_mgr.active_index;
        self.workspace_mgr.sessions[active_idx].stop_source();
        self.is_source_running = false;
    }

    pub fn restart_current_source(&mut self) {
        self.stop_current_source();
        self.save_current_workspace();
        let active_idx = self.workspace_mgr.active_index;
        self.workspace_mgr.sessions[active_idx].source_config = self.source_config.clone();
        self.workspace_mgr.sessions[active_idx].restart_source(&self.rt);

        let view = &mut self.view_states[active_idx];
        view.cached_logs.clear();
        view.total_matched = 0;
        view.selected_log = None;
        view.last_processed_count = 0;
        view.unfiltered_state.cached_unfiltered.clear();
        view.is_source_running = true;

        self.trigger_full_search();
    }

    pub fn spawn_load_environment(&mut self) {
        let active_idx = self.workspace_mgr.active_index;
        self.workspace_mgr.sessions[active_idx].source_config = self.source_config.clone();
        self.workspace_mgr.sessions[active_idx].spawn_load_environment(&self.rt);
    }

    pub fn tick(&mut self) {
        // 1. Nhận kết quả phát hiện WSL distros
        if let Some(mut rx) = self.wsl_distro_rx.take() {
            match rx.try_recv() {
                Ok(distros) => {
                    self.available_wsl_distros = distros;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    self.wsl_distro_rx = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {}
            }
        }

        // 2. Chạy tick trên tất cả các runtime sessions (phát hiện process hoàn tất, nhận env vars)
        self.workspace_mgr.tick_all();

        // 3. Đồng bộ trạng thái từ active session sang active view state
        let active_idx = self.workspace_mgr.active_index;
        let session = &self.workspace_mgr.sessions[active_idx];
        self.view_states[active_idx].is_source_running = session.is_source_running;
        self.view_states[active_idx].env_status = session.env_status.clone();
        self.view_states[active_idx].env_vars = session.env_vars.clone();

        let total_processed = session.engine.total_processed();
        let now = Instant::now();

        let query_changed = self.query != self.last_query;
        if query_changed {
            self.trigger_full_search();
            if self.query.trim().is_empty() && self.active_tab == ActiveTab::Unfiltered {
                self.close_unfiltered_stream();
            }
        }

        let q = self.query.clone();
        self.history_state.try_debounced_record(&q, now);

        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(150);

        self.has_new_data = false;
        let prev_processed = self.last_processed_count;

        if self.is_auto_scroll && new_logs_arrived && !query_changed {
            let (new_matched_count, new_matching_logs) =
                self.engine.filter_incremental(&self.query, prev_processed);

            if new_matched_count > 0 {
                self.total_matched += new_matched_count;
                self.sync_discovered_fields(&new_matching_logs);
                self.cached_logs.extend(new_matching_logs);

                if self.cached_logs.len() > self.display_limit {
                    let overflow = self.cached_logs.len() - self.display_limit;
                    self.cached_logs.drain(0..overflow);
                }
                self.has_new_data = true;
            }
        }

        if !self.is_auto_scroll && !self.query.trim().is_empty() && new_logs_arrived {
            let (new_matched, _) = self
                .engine
                .filter_incremental(&self.query, self.filtered_processed_at_pause);
            self.paused_new_matched_count = new_matched;
        }

        if self.is_auto_scroll {
            self.global_seen_at_pause = total_processed;
            self.filtered_seen_at_pause = self.total_matched;
            self.filtered_processed_at_pause = total_processed;
            self.paused_new_matched_count = 0;
        }

        self.unfiltered_state.has_new_data = false;
        if self.unfiltered_state.is_open && self.unfiltered_state.is_live && new_logs_arrived {
            let (new_count, new_logs) = self.engine.filter_incremental("", prev_processed);
            if new_count > 0 {
                self.unfiltered_state.cached_unfiltered.extend(new_logs);
                if self.unfiltered_state.cached_unfiltered.len() > RAW_STREAM_LIMIT {
                    let overflow = self.unfiltered_state.cached_unfiltered.len() - RAW_STREAM_LIMIT;
                    self.unfiltered_state.cached_unfiltered.drain(0..overflow);
                }
                self.unfiltered_state.has_new_data = true;
            }
        }

        if new_logs_arrived {
            self.last_processed_count = total_processed;
            self.last_search_time = now;
        }
    }

    pub fn trigger_full_search(&mut self) {
        let (matched, logs) = self
            .engine
            .search_with_count(&self.query, self.display_limit);
        self.total_matched = matched;
        self.sync_discovered_fields(&logs);
        self.cached_logs = logs;
        self.last_query = self.query.clone();
        self.last_processed_count = self.engine.total_processed();
        self.last_search_time = Instant::now();
        self.global_seen_at_pause = self.last_processed_count;
        self.filtered_seen_at_pause = self.total_matched;
        self.filtered_processed_at_pause = self.last_processed_count;
        self.paused_new_matched_count = 0;
    }

    pub fn latch(&mut self) {
        let was_unlatched = !self.is_auto_scroll;
        self.is_auto_scroll = true;
        self.request_scroll_to_bottom = true;
        if was_unlatched {
            self.trigger_full_search();
        }
    }

    pub fn unlatch(&mut self) {
        if !self.is_auto_scroll {
            return;
        }
        self.is_auto_scroll = false;
        let total = self.engine.total_processed();
        self.global_seen_at_pause = total;
        self.filtered_seen_at_pause = self.total_matched;
        self.filtered_processed_at_pause = total;
        self.paused_new_matched_count = 0;
    }

    pub fn toggle_latch(&mut self) {
        if self.is_auto_scroll {
            self.unlatch();
        } else {
            self.latch();
        }
    }

    pub fn sync_discovered_fields(&mut self, logs: &[LogEvent]) {
        self.column_state.sync_discovered_keys(logs);
        self.discovered_fields_cache = self.engine.get_schema_map().into_iter().collect();
    }

    pub fn get_available_log_fields(&self) -> Vec<(String, crate::ui::autocomplete::FieldType)> {
        self.discovered_fields_cache
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    pub fn apply_autocomplete_suggestion(
        &mut self,
        item: &crate::ui::autocomplete::SuggestionItem,
    ) {
        let (start, end) = self.autocomplete_state.active_token_range;
        if start <= end && end <= self.query.len() {
            let mut new_query = String::new();
            new_query.push_str(&self.query[..start]);
            new_query.push_str(&item.insert_text);
            new_query.push_str(&self.query[end..]);
            self.query = new_query;
        } else {
            self.query = item.insert_text.clone();
        }

        self.autocomplete_state.just_applied = true;

        match item.kind {
            crate::ui::autocomplete::SuggestionKind::Key => {
                let available_fields = self.get_available_log_fields();
                let (suggestions, token_range) =
                    crate::ui::autocomplete::generate_suggestions(&self.query, &available_fields);
                if !suggestions.is_empty() {
                    self.autocomplete_state.suggestions = suggestions;
                    self.autocomplete_state.active_token_range = token_range;
                    self.autocomplete_state.selected_index = 0;
                    self.autocomplete_state.is_open = true;
                } else {
                    self.autocomplete_state.is_open = false;
                }
            }
            crate::ui::autocomplete::SuggestionKind::OperatorOrValue => {
                self.autocomplete_state.is_open = false;
                self.trigger_full_search();
            }
        }
    }

    pub fn toggle_row_highlight(&mut self, id: u64) {
        if self.highlighted_row_ids.contains(&id) {
            self.highlighted_row_ids.remove(&id);
        } else {
            self.highlighted_row_ids.insert(id);
        }
    }

    pub fn is_row_highlighted(&self, id: &u64) -> bool {
        self.highlighted_row_ids.contains(id)
    }

    pub fn toggle_term_highlight(&mut self, term: &str) {
        let clean = term.trim().to_lowercase();
        if clean.is_empty() {
            return;
        }
        if self.highlighted_terms.contains(&clean) {
            self.highlighted_terms.remove(&clean);
        } else {
            self.highlighted_terms.insert(clean);
        }
    }

    #[allow(dead_code)]
    pub fn is_term_highlighted(&self, term: &str) -> bool {
        let clean = term.trim().to_lowercase();
        if clean.is_empty() {
            false
        } else {
            self.highlighted_terms.contains(&clean)
        }
    }

    pub fn has_any_highlights(&self) -> bool {
        !self.highlighted_row_ids.is_empty() || !self.highlighted_terms.is_empty()
    }

    pub fn clear_all_highlights(&mut self) {
        self.highlighted_row_ids.clear();
        self.highlighted_terms.clear();
    }

    pub fn format_field_term(field: &str, val: &str) -> String {
        let clean_val = val.trim();
        if field.eq_ignore_ascii_case("level") {
            format!("level:{}", clean_val.to_lowercase())
        } else if clean_val.contains(' ') || clean_val.contains('"') || clean_val.contains(':') {
            format!("{}:\"{}\"", field, clean_val.replace('"', "\\\""))
        } else {
            format!("{}:{}", field, clean_val)
        }
    }

    pub fn format_selection_term(text: &str) -> String {
        let clean = text
            .replace(" ↵ ", " ")
            .replace('\n', " ")
            .replace('\r', "");
        let clean = clean.trim();
        if clean.contains(' ') || clean.contains('"') || clean.contains(':') {
            format!("\"{}\"", clean.replace('"', "\\\""))
        } else {
            clean.to_string()
        }
    }

    pub fn apply_filter_term(&mut self, term: &str) {
        let current = self.query.trim();
        if current.is_empty() {
            self.query = term.to_string();
        } else {
            let tokens: Vec<&str> = current.split_whitespace().collect();
            if !tokens.contains(&term) {
                self.query = format!("{current} {term}");
            }
        }
        self.trigger_full_search();
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        let exclude_term = if term.starts_with('-') {
            term.to_string()
        } else {
            format!("-{term}")
        };

        let current = self.query.trim();
        if current.is_empty() {
            self.query = exclude_term;
        } else {
            let tokens: Vec<&str> = current.split_whitespace().collect();
            if !tokens.contains(&exclude_term.as_str()) {
                self.query = format!("{current} {exclude_term}");
            }
        }
        self.trigger_full_search();
    }

    pub fn open_unfiltered_stream(&mut self, target_id: Option<u64>) {
        self.unfiltered_state.is_open = true;
        self.unfiltered_state.target_id = target_id;
        self.unfiltered_state.is_live = target_id.is_none();
        self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
        let (target_idx, unfiltered) = self
            .engine
            .get_unfiltered_events(target_id, RAW_STREAM_LIMIT);
        self.unfiltered_state.cached_unfiltered = unfiltered;
        self.unfiltered_state.target_index = target_idx;
        self.unfiltered_state.request_scroll_to_target = true;
        self.unfiltered_state.has_new_data = false;
        self.active_tab = ActiveTab::Unfiltered;
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
        let (target_idx, unfiltered) = self
            .engine
            .get_unfiltered_events(self.unfiltered_state.target_id, RAW_STREAM_LIMIT);
        self.unfiltered_state.cached_unfiltered = unfiltered;
        self.unfiltered_state.target_index = target_idx;
        self.unfiltered_state.request_scroll_to_target = true;
    }

    pub fn toggle_unfiltered_live(&mut self) {
        self.unfiltered_state.is_live = !self.unfiltered_state.is_live;
        if self.unfiltered_state.is_live {
            self.refresh_unfiltered_snapshot();
            self.unfiltered_state.request_scroll_to_bottom = true;
        } else {
            self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
        }
    }

    pub fn unlatch_unfiltered(&mut self) {
        if !self.unfiltered_state.is_live {
            return;
        }
        self.unfiltered_state.is_live = false;
        self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
    }

    pub fn close_unfiltered_stream(&mut self) {
        self.unfiltered_state.is_open = false;
        self.unfiltered_state.target_id = None;
        self.unfiltered_state.target_index = None;
        self.unfiltered_state.cached_unfiltered.clear();
        self.active_tab = ActiveTab::Filtered;
    }

    pub fn focus_in_main_and_clear_filter(&mut self) {
        if let Some(target_id) = self.unfiltered_state.target_id {
            if let Some(target_event) = self
                .unfiltered_state
                .cached_unfiltered
                .iter()
                .find(|e| e.id == target_id)
                .cloned()
            {
                self.selected_log = Some(target_event);
            }
        }
        self.query.clear();
        self.trigger_full_search();
        self.close_unfiltered_stream();
    }
}

impl eframe::App for UwuGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick();
        ctx.request_repaint_after(Duration::from_millis(100));
        crate::ui::render_ui(ctx, self);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.workspace_mgr.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::autocomplete::{SuggestionItem, SuggestionKind};
    use std::collections::HashMap;
    use uwu_core_schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};

    fn create_test_app() -> UwuGuiApp {
        let rt = tokio::runtime::Handle::current();

        let source_config = SourceConfig {
            source_type: SourceType::Process,
            command_str: String::new(),
            file_path: String::new(),
            working_dir: String::new(),
            wsl_config: WslConfig::default(),
            capacity: 100,
            display_limit: 50,
        };

        let session = WorkspaceSession::new(
            "Test Project".to_string(),
            WorkspaceLocation::Local {
                working_dir: String::new(),
            },
            source_config.clone(),
        );

        let view_state = GuiSessionState::new(
            session.engine.clone(),
            source_config,
            "Test Project".to_string(),
        );

        let workspace_store = WorkspaceStore::default();
        let workspace_mgr = MultiWorkspaceManager::new(session, workspace_store.clone());

        UwuGuiApp {
            workspace_mgr,
            view_states: vec![view_state],
            rt,
            show_launch_modal: false,
            project_picker_open: false,
            project_search_query: String::new(),
            available_wsl_distros: Vec::new(),
            wsl_distro_rx: None,
            prev_screen_width: 0.0,
            workspace_store,
        }
    }

    #[tokio::test]
    async fn test_latch_toggle() {
        let mut app = create_test_app();
        assert!(app.is_auto_scroll);

        app.unlatch();
        assert!(!app.is_auto_scroll);

        app.latch();
        assert!(app.is_auto_scroll);
        assert!(app.request_scroll_to_bottom);

        app.toggle_latch();
        assert!(!app.is_auto_scroll);

        app.toggle_latch();
        assert!(app.is_auto_scroll);
    }

    #[tokio::test]
    async fn test_highlight_toggle_and_clear() {
        let mut app = create_test_app();
        let id1 = 1001u64;
        let id2 = 1002u64;

        assert!(!app.is_row_highlighted(&id1));
        assert!(!app.has_any_highlights());

        app.toggle_row_highlight(id1);
        assert!(app.is_row_highlighted(&id1));
        assert!(app.has_any_highlights());

        app.toggle_row_highlight(id2);
        assert!(app.is_row_highlighted(&id2));

        // Term highlight
        assert!(!app.is_term_highlighted("timeout"));
        app.toggle_term_highlight("timeout");
        assert!(app.is_term_highlighted("timeout"));
        assert!(app.is_term_highlighted("TIMEOUT")); // Case-insensitive
        assert!(app.has_any_highlights());

        app.toggle_term_highlight("timeout");
        assert!(!app.is_term_highlighted("timeout"));

        // Clear all
        app.toggle_term_highlight("error");
        app.clear_all_highlights();
        assert!(!app.has_any_highlights());
        assert!(!app.is_row_highlighted(&id1));
        assert!(!app.is_term_highlighted("error"));
    }

    #[tokio::test]
    async fn test_format_field_and_selection_term() {
        assert_eq!(
            UwuGuiApp::format_field_term("level", "ERROR"),
            "level:error"
        );
        assert_eq!(
            UwuGuiApp::format_field_term("source", "auth-service"),
            "source:auth-service"
        );
        assert_eq!(
            UwuGuiApp::format_field_term("message", "connection refused"),
            "message:\"connection refused\""
        );

        assert_eq!(
            UwuGuiApp::format_selection_term("connection refused"),
            "\"connection refused\""
        );
        assert_eq!(UwuGuiApp::format_selection_term("simple"), "simple");
    }

    #[tokio::test]
    async fn test_filter_and_exclude_term() {
        let mut app = create_test_app();

        app.apply_filter_term("level:error");
        assert_eq!(app.query, "level:error");

        app.apply_filter_term("tag:Auth");
        assert_eq!(app.query, "level:error tag:Auth");

        app.exclude_filter_term("healthcheck");
        assert_eq!(app.query, "level:error tag:Auth -healthcheck");

        app.exclude_filter_term("-already_negated");
        assert_eq!(
            app.query,
            "level:error tag:Auth -healthcheck -already_negated"
        );
    }

    #[tokio::test]
    async fn test_apply_autocomplete_suggestion() {
        let mut app = create_test_app();

        let item_key = SuggestionItem {
            kind: SuggestionKind::Key,
            op_symbol: "",
            action_name: "level".to_string(),
            example_syntax: "level:".to_string(),
            insert_text: "level:".to_string(),
        };
        app.autocomplete_state.active_token_range = (0, 0);
        app.apply_autocomplete_suggestion(&item_key);
        assert_eq!(app.query, "level:");

        let item_val = SuggestionItem {
            kind: SuggestionKind::OperatorOrValue,
            op_symbol: "",
            action_name: "error".to_string(),
            example_syntax: "error".to_string(),
            insert_text: "error".to_string(),
        };
        app.autocomplete_state.active_token_range = (6, 6);
        app.apply_autocomplete_suggestion(&item_val);
        assert_eq!(app.query, "level:error");
        assert!(!app.autocomplete_state.is_open);
    }

    #[tokio::test]
    async fn test_get_available_log_fields_inference() {
        let app = create_test_app();
        let fields = app.get_available_log_fields();
        assert!(!fields.is_empty());
        assert!(fields.iter().any(|(k, _)| k == "level"));
        assert!(fields.iter().any(|(k, _)| k == "timestamp"));
    }

    #[tokio::test]
    async fn test_unfiltered_stream_open_close_and_focus() {
        let mut app = create_test_app();
        assert_eq!(app.active_tab, ActiveTab::Filtered);

        app.open_unfiltered_stream(Some(12345));
        assert_eq!(app.active_tab, ActiveTab::Unfiltered);
        assert!(app.unfiltered_state.is_open);
        assert_eq!(app.unfiltered_state.target_id, Some(12345));
        assert!(!app.unfiltered_state.is_live);

        app.close_unfiltered_stream();
        assert_eq!(app.active_tab, ActiveTab::Filtered);
        assert!(!app.unfiltered_state.is_open);
        assert!(app.unfiltered_state.target_id.is_none());
    }

    #[tokio::test]
    async fn test_sync_discovered_fields_replaces_aliases() {
        let mut app = create_test_app();
        let log = LogEvent::new(
            "2026-08-20T10:00:00Z",
            LogLevel::Info,
            "msg",
            HashMap::from([("custom_field".to_string(), serde_json::json!("val"))]),
        );

        app.sync_discovered_fields(&[log]);
        let fields = app.get_available_log_fields();
        assert!(fields.iter().any(|(k, _)| k == "level"));
    }

    #[tokio::test]
    async fn test_unlatch_unfiltered_idempotency() {
        let mut app = create_test_app();
        app.open_unfiltered_stream(None);
        assert!(app.unfiltered_state.is_live);

        app.unlatch_unfiltered();
        assert!(!app.unfiltered_state.is_live);

        app.unlatch_unfiltered();
        assert!(!app.unfiltered_state.is_live);
    }

    #[tokio::test]
    async fn test_unfiltered_live_tick_sync_both_branches() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        // 1. Initial 5 logs
        for i in 0..5 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        app.trigger_full_search();
        assert_eq!(app.cached_logs.len(), 5);

        // 2. Open unfiltered stream in LIVE mode
        app.open_unfiltered_stream(None);
        assert!(app.unfiltered_state.is_live);
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 5);

        // 3. Ingest 5 more logs
        for i in 5..10 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Force last_search_time backward to satisfy 150ms debounce in tick()
        app.last_search_time = Instant::now() - Duration::from_millis(200);
        app.tick();

        // Both filtered and unfiltered live buffers must have received all 10 logs
        assert_eq!(app.cached_logs.len(), 10);
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 10);
    }

    #[tokio::test]
    async fn test_unfiltered_frozen_snapshot_no_drift() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        for i in 0..5 {
            let entry = RawLogEntry {
                payload: RawPayload::Text(format!(
                    "{{\"level\":\"INFO\",\"message\":\"msg {}\"}}",
                    i
                )),
            };
            let _ = tx.send(entry).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        app.open_unfiltered_stream(Some(2));
        assert!(!app.unfiltered_state.is_live);
        let initial_count = app.unfiltered_state.cached_unfiltered.len();

        let new_entry = RawLogEntry {
            payload: RawPayload::Text("{\"level\":\"INFO\",\"message\":\"msg 99\"}".to_string()),
        };
        let _ = tx.send(new_entry).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        app.tick();
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), initial_count);
    }

    #[tokio::test]
    async fn test_repeated_unlatch_idempotency_preserves_pause_state() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        for i in 0..10 {
            let entry = RawLogEntry {
                payload: RawPayload::Text(format!(
                    "{{\"level\":\"INFO\",\"message\":\"msg {}\"}}",
                    i
                )),
            };
            let _ = tx.send(entry).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        app.tick();

        app.unlatch();
        assert!(!app.is_auto_scroll);
        let paused_count = app.filtered_seen_at_pause;

        app.unlatch();
        assert_eq!(app.filtered_seen_at_pause, paused_count);
    }

    #[tokio::test]
    async fn test_spawn_load_environment_and_tick() {
        let mut app = create_test_app();
        app.spawn_load_environment();

        tokio::time::sleep(Duration::from_millis(350)).await;
        app.tick();

        assert!(matches!(
            app.env_status,
            uwu_core_workspace::EnvLoadStatus::Ready { .. }
        ));
    }

    #[tokio::test]
    async fn test_source_running_state_transitions_to_stopped() {
        let mut app = create_test_app();
        assert!(!app.is_source_running);

        #[cfg(target_os = "windows")]
        {
            app.source_config.command_str = "cmd /c echo test".to_string();
        }
        #[cfg(not(target_os = "windows"))]
        {
            app.source_config.command_str = "echo test".to_string();
        }

        app.start_configured_source();
        assert!(app.is_source_running);

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            app.tick();
            if !app.is_source_running {
                break;
            }
        }

        assert!(!app.is_source_running);
    }

    #[tokio::test]
    async fn test_close_unfiltered_stream_frees_ram() {
        let mut app = create_test_app();
        app.open_unfiltered_stream(None);
        app.close_unfiltered_stream();
        assert!(app.unfiltered_state.cached_unfiltered.is_empty());
    }

    #[tokio::test]
    async fn test_multi_project_switch_and_close() {
        let mut app = create_test_app();
        assert_eq!(app.workspace_mgr.sessions.len(), 1);
        assert_eq!(app.workspace_mgr.active_index, 0);

        app.query = "level:error".to_string();

        let ws2 = Workspace::new(
            "Project B",
            WorkspaceLocation::Local {
                working_dir: "D:\\test\\proj_b".to_string(),
            },
            SourceType::Process,
        );
        app.open_or_switch_workspace(&ws2);

        assert_eq!(app.workspace_mgr.sessions.len(), 2);
        assert_eq!(app.workspace_mgr.active_index, 1);
        assert_eq!(app.project_name_input, "Project B");
        assert_eq!(app.query, "");

        app.query = "tag:Audio".to_string();

        app.switch_session(0);
        assert_eq!(app.workspace_mgr.active_index, 0);
        assert_eq!(app.project_name_input, "Test Project");
        assert_eq!(app.query, "level:error");

        app.switch_session(1);
        assert_eq!(app.workspace_mgr.active_index, 1);
        assert_eq!(app.project_name_input, "Project B");
        assert_eq!(app.query, "tag:Audio");

        app.close_session(1);
        assert_eq!(app.workspace_mgr.sessions.len(), 1);
        assert_eq!(app.workspace_mgr.active_index, 0);
        assert_eq!(app.project_name_input, "Test Project");
        assert_eq!(app.query, "level:error");
    }

    #[tokio::test]
    async fn test_wsl_workspace_save_and_reload() {
        let mut app = create_test_app();

        // 1. Cấu hình WSL source trong session hiện tại
        app.source_config.source_type = SourceType::Wsl;
        app.source_config.wsl_config.distro = "Ubuntu".to_string();
        app.source_config.wsl_config.working_dir = "/home/user/backend".to_string();
        app.source_config.wsl_config.sub_mode = WslSubMode::Command;
        app.source_config.wsl_config.command_str = "python3 app.py".to_string();
        app.project_name_input = "WSL-Backend".to_string();

        // 2. Lưu workspace hiện tại
        app.save_current_workspace();

        // 3. Kiểm tra xem workspace được lưu vào store đúng chưa
        let ws = app
            .workspace_store
            .recent_workspaces
            .iter()
            .find(|w| w.name == "WSL-Backend")
            .cloned()
            .expect("WSL-Backend must be saved in store");

        assert_eq!(ws.source_type, SourceType::Wsl);
        assert_eq!(ws.command_str, "python3 app.py");
        match &ws.location {
            WorkspaceLocation::Wsl {
                distro,
                working_dir,
            } => {
                assert_eq!(distro, "Ubuntu");
                assert_eq!(working_dir, "/home/user/backend");
            }
            _ => panic!("Expected WSL location"),
        }

        // 4. Mở lại workspace WSL qua open_or_switch_workspace
        app.open_or_switch_workspace(&ws);
        assert_eq!(app.source_config.source_type, SourceType::Wsl);
        assert_eq!(app.source_config.wsl_config.distro, "Ubuntu");
        assert_eq!(
            app.source_config.wsl_config.working_dir,
            "/home/user/backend"
        );
        assert_eq!(app.source_config.wsl_config.sub_mode, WslSubMode::Command);
        assert_eq!(app.source_config.wsl_config.command_str, "python3 app.py");
    }
}
