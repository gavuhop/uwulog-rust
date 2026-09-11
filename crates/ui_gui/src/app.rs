use crate::session_view::GuiSession;
use clap::Parser;
use eframe::egui;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use uwu_core_schema::LogEvent;
pub use uwu_core_workspace::{
    extract_project_name, RemoteConnectionOptions, SourceConfig, SourceType, Workspace,
    WorkspaceLocation, WorkspaceSession, WorkspaceStore,
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

/// Unified Action enum for high-level application & session state mutations (Zed-style Command Pattern)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppAction {
    SwitchSession(usize),
    CloseSession(usize),
    CycleSession(bool),
    OpenWorkspace(Workspace),
    LoadWorkspace(Workspace),
    DeleteWorkspace(uuid::Uuid),
    StartSource,
    StopSource,
    RestartSource,
    OpenLaunchModal,
    CloseLaunchModal,
    ApplyLaunchModal,
    ApplyAndRestartSource(SourceConfig),
    OpenColumnsModal,
    CloseColumnsModal,
    ApplyColumnsModal,
    SelectLog(Option<LogEvent>),
    SwitchTab(ActiveTab),

    // Search Query & Filtering
    ApplyFilterTerm(String),
    ExcludeFilterTerm(String),
    ClearQuery,

    // Highlights
    ToggleRowHighlight(u64),
    ToggleTermHighlight(String),
    ClearAllHighlights,

    // Stream & Latch Controls
    ToggleLatch,
    ToggleUnfilteredLive,
    RefreshUnfilteredSnapshot,
    OpenUnfilteredStream(Option<u64>),
    CloseUnfilteredStream,
    FocusInMainAndClearFilter,

    // Project Picker
    ToggleProjectPicker,
    CloseProjectPicker,

    // Global Dismiss / Stack Pop
    DismissTopLayer,
}

pub struct UwuGuiApp {
    pub sessions: Vec<GuiSession>,
    pub active_index: usize,
    pub store: WorkspaceStore,
    pub rt: Handle,
    pub show_launch_modal: bool,
    pub launch_modal_draft: Option<SourceConfig>,
    pub project_picker_open: bool,
    pub project_search_query: String,
    pub prev_screen_width: f32,
}

impl std::ops::Deref for UwuGuiApp {
    type Target = GuiSession;

    fn deref(&self) -> &Self::Target {
        &self.sessions[self.active_index]
    }
}

impl std::ops::DerefMut for UwuGuiApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let idx = self.active_index;
        &mut self.sessions[idx]
    }
}

/// Tham số dòng lệnh khi khởi chạy uwu-gui
#[derive(Parser, Debug, Clone)]
#[command(
    name = "uwu-gui",
    about = "High-performance log viewer & workspace monitor",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct CliArgs {
    /// Đường dẫn file log hoặc thư mục workspace cần mở
    #[arg(value_name = "PATH", value_hint = clap::ValueHint::AnyPath)]
    pub path: Option<String>,

    /// Lệnh thực thi để thu thập log stdout/stderr (ví dụ: -c "cargo run")
    #[arg(
        short = 'c',
        short_alias = 'r',
        long = "cmd",
        visible_aliases = ["run", "command", "exec", "remote-cmd", "wsl-cmd"]
    )]
    pub cmd: Option<String>,

    /// Đường dẫn file log cần theo dõi (file tailer)
    #[arg(
        short = 'f',
        long = "file",
        visible_aliases = ["remote-file", "wsl-file"],
        value_hint = clap::ValueHint::FilePath
    )]
    pub file: Option<String>,

    /// Thư mục làm việc (working directory)
    #[arg(
        short = 'd',
        long = "dir",
        visible_aliases = ["cwd", "working-dir", "remote-dir", "wsl-dir", "wsl-cwd"],
        value_hint = clap::ValueHint::DirPath
    )]
    pub working_dir: Option<String>,

    /// Kết nối môi trường Remote (ví dụ: "Ubuntu", "wsl:Ubuntu")
    #[arg(
        long = "remote",
        visible_aliases = ["wsl", "remote-wsl", "wsl-distro"]
    )]
    pub remote: Option<String>,

    /// Truy vấn lọc log ban đầu (ví dụ: -q "level:error")
    #[arg(
        short = 'q',
        long = "query",
        visible_aliases = ["filter"]
    )]
    pub query: Option<String>,

    /// Số dòng log hiển thị tối đa trong viewport
    #[arg(short = 'n', long = "limit", default_value_t = 5000)]
    pub display_limit: usize,

    /// Dung lượng bộ đệm RingBuffer (số dòng log tối đa trong RAM)
    #[arg(
        short = 'C',
        long = "capacity",
        visible_alias = "cap",
        default_value_t = 200_000
    )]
    pub capacity: usize,

    /// Command arguments truyền sau `--` (ví dụ: `uwu-gui -- cargo run --bin server`)
    #[arg(last = true)]
    pub trailing_cmd: Vec<String>,
}

impl CliArgs {
    /// Phân giải vị trí làm việc và cấu hình nguồn chạy (cmd, file, hoặc thư mục) từ các cờ dòng lệnh
    pub fn resolve_target(&self) -> (WorkspaceLocation, SourceType, String, String, bool) {
        let mut cmd = self.cmd.clone().unwrap_or_default();
        if cmd.is_empty() && !self.trailing_cmd.is_empty() {
            cmd = self.trailing_cmd.join(" ");
        }

        let mut file = self.file.clone().unwrap_or_default();
        let mut dir = self.working_dir.clone().unwrap_or_default();
        let mut source_type = SourceType::Process;
        let mut has_custom_source = false;

        if !cmd.is_empty() {
            source_type = SourceType::Process;
            has_custom_source = true;
        } else if !file.is_empty() {
            source_type = SourceType::File;
            has_custom_source = true;
        } else if let Some(ref path_str) = self.path {
            let is_file = if self.remote.is_some() {
                std::path::Path::new(path_str).extension().is_some()
            } else {
                !std::path::Path::new(path_str).is_dir()
            };

            if is_file {
                file = path_str.clone();
                source_type = SourceType::File;
                has_custom_source = true;
            } else {
                dir = path_str.clone();
            }
        }

        // Chuẩn hóa thư mục làm việc nếu ở môi trường local
        if !dir.is_empty() && self.remote.is_none() {
            let p = std::path::Path::new(&dir);
            if let Ok(canon) = p.canonicalize() {
                dir = uwu_core_workspace::clean_path(&canon.to_string_lossy());
            } else {
                dir = uwu_core_workspace::clean_path(&dir);
            }
        }

        let location = if let Some(ref rem) = self.remote {
            let remote_dir = if !dir.is_empty() {
                dir
            } else {
                "/home".to_string()
            };
            WorkspaceLocation::remote(RemoteConnectionOptions::parse(rem, remote_dir))
        } else {
            let local_dir = if !dir.is_empty() {
                dir
            } else {
                std::env::current_dir()
                    .map(|d| uwu_core_workspace::clean_path(&d.to_string_lossy()))
                    .unwrap_or_default()
            };
            WorkspaceLocation::local(local_dir)
        };

        (location, source_type, cmd, file, has_custom_source)
    }
}

impl UwuGuiApp {
    /// Khởi tạo GuiSession ban đầu từ tham số CLI và lịch sử WorkspaceStore đã lưu
    pub(crate) fn build_initial_session(
        cli: &CliArgs,
        store: &WorkspaceStore,
    ) -> (GuiSession, bool, Option<uuid::Uuid>) {
        let (location, source_type, cmd, file, has_custom_source) = cli.resolve_target();

        let saved_ws = store
            .find_by_location(&location)
            .or_else(|| store.find_by_workdir(location.working_dir()));

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
        if let Some(ref q) = cli.query {
            gui_session.view.query = q.clone();
        } else if !has_custom_source {
            if let Some(ws) = saved_ws {
                gui_session.view.query = ws.last_query.clone();
            }
        }

        (gui_session, has_custom_source, saved_id)
    }

    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle) -> Self {
        crate::ui::theme::apply_theme(&cc.egui_ctx);
        #[cfg(target_os = "windows")]
        crate::ui::theme::apply_windows_titlebar_theme(cc);

        let cli = CliArgs::parse();
        let store = WorkspaceStore::load();
        let (initial_gui_session, has_custom_source, saved_id) =
            Self::build_initial_session(&cli, &store);

        let mut app = Self {
            sessions: vec![initial_gui_session],
            active_index: 0,
            store,
            rt,
            show_launch_modal: false,
            launch_modal_draft: None,
            project_picker_open: false,
            project_search_query: String::new(),
            prev_screen_width: 0.0,
        };

        // Background task nạp biến môi trường cho session đầu tiên
        app.spawn_load_environment();

        if let (Some(id), false) = (saved_id, has_custom_source) {
            app.store.active_workspace_id = Some(id);
            let _ = app.store.save();
        } else {
            // Tự động lưu workspace mới hoặc cấu hình nguồn mới vào store
            app.save_current_workspace();
        }

        if has_custom_source {
            app.start_configured_source();
        }
        app.trigger_full_search();

        app
    }

    #[allow(dead_code)]
    pub fn active_session(&self) -> &WorkspaceSession {
        &self.sessions[self.active_index].session
    }

    #[allow(dead_code)]
    pub fn active_session_mut(&mut self) -> &mut WorkspaceSession {
        &mut self.sessions[self.active_index].session
    }

    pub fn switch_session(&mut self, index: usize) {
        if index < self.sessions.len() {
            if self.active_index == index {
                return;
            }
            self.active_index = index;
            self.store.active_workspace_id = Some(self.sessions[index].session.id);
            let _ = self.store.save();
            self.trigger_full_search();
        }
    }

    pub fn open_or_switch_workspace(&mut self, ws: &Workspace) {
        if let Some(pos) = self
            .sessions
            .iter()
            .position(|s| s.session.id == ws.id || s.session.location.is_same(&ws.location))
        {
            self.switch_session(pos);
            return;
        }

        let mut gui_session = GuiSession::from_workspace(
            ws,
            self.session.source_config.capacity,
            self.session.display_limit,
        );
        gui_session.session.spawn_load_environment(&self.rt);
        self.sessions.push(gui_session);
        self.switch_session(self.sessions.len() - 1);
    }

    pub fn close_session(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }

        let mut removed = self.sessions.remove(index);
        removed.session.stop_source();

        if self.sessions.is_empty() {
            let s = WorkspaceSession::new_default(
                removed.session.source_config.capacity,
                removed.session.display_limit,
            );
            self.sessions.push(GuiSession::new(s));
            self.active_index = 0;
        } else if self.active_index > index {
            self.active_index -= 1;
        } else if self.active_index >= self.sessions.len() {
            self.active_index = self.sessions.len() - 1;
        }

        self.store.active_workspace_id = Some(self.sessions[self.active_index].session.id);
        let _ = self.store.save();
        self.trigger_full_search();
    }

    pub fn cycle_project(&mut self, forward: bool) {
        if self.sessions.is_empty() {
            return;
        }
        let n = self.sessions.len();
        let new_idx = if forward {
            (self.active_index + 1) % n
        } else {
            (self.active_index + n - 1) % n
        };
        self.switch_session(new_idx);
    }

    /// Điều phối và thực thi các hành động cấp ứng dụng (Zed-style Command Dispatcher)
    pub fn dispatch_action(&mut self, action: AppAction) {
        match action {
            AppAction::SwitchSession(idx) => {
                self.switch_session(idx);
                self.close_project_picker();
            }
            AppAction::CloseSession(idx) => self.close_session(idx),
            AppAction::CycleSession(forward) => self.cycle_project(forward),
            AppAction::OpenWorkspace(ws) => {
                self.open_or_switch_workspace(&ws);
                self.close_project_picker();
            }
            AppAction::LoadWorkspace(ws) => {
                self.load_workspace(&ws);
                if self.launch_modal_draft.is_some() {
                    self.launch_modal_draft = Some(self.session.source_config.clone());
                }
            }
            AppAction::DeleteWorkspace(id) => {
                self.store.remove(id);
            }
            AppAction::StartSource => self.start_configured_source(),
            AppAction::StopSource => self.stop_current_source(),
            AppAction::RestartSource => self.restart_current_source(),
            AppAction::OpenLaunchModal => {
                self.launch_modal_draft = Some(self.session.source_config.clone());
                self.show_launch_modal = true;
            }
            AppAction::CloseLaunchModal => {
                self.show_launch_modal = false;
                self.launch_modal_draft = None;
            }
            AppAction::ApplyLaunchModal => {
                if let Some(draft) = self.launch_modal_draft.take() {
                    self.session.source_config = draft;
                    self.save_current_workspace();
                    self.restart_current_source();
                }
                self.show_launch_modal = false;
            }
            AppAction::ApplyAndRestartSource(new_config) => {
                self.session.source_config = new_config;
                self.save_current_workspace();
                self.restart_current_source();
                self.show_launch_modal = false;
                self.launch_modal_draft = None;
            }
            AppAction::OpenColumnsModal => self.column_state.open_modal(),
            AppAction::CloseColumnsModal => self.column_state.close_modal(),
            AppAction::ApplyColumnsModal => self.column_state.apply_modal(),
            AppAction::SelectLog(log) => self.selected_log = log,
            AppAction::SwitchTab(tab) => {
                if tab == ActiveTab::Unfiltered && !self.unfiltered_state.is_open {
                    self.open_unfiltered_stream(None);
                }
                self.active_tab = tab;
            }
            AppAction::ApplyFilterTerm(term) => self.apply_filter_term(&term),
            AppAction::ExcludeFilterTerm(term) => self.exclude_filter_term(&term),
            AppAction::ClearQuery => {
                self.query.clear();
                self.autocomplete_state.is_open = false;
                self.history_state.close_popup();
                self.trigger_full_search();
            }
            AppAction::ToggleRowHighlight(id) => self.toggle_row_highlight(id),
            AppAction::ToggleTermHighlight(term) => self.toggle_term_highlight(&term),
            AppAction::ClearAllHighlights => self.clear_all_highlights(),
            AppAction::ToggleLatch => self.toggle_latch(),
            AppAction::ToggleUnfilteredLive => self.toggle_unfiltered_live(),
            AppAction::RefreshUnfilteredSnapshot => self.refresh_unfiltered_snapshot(),
            AppAction::OpenUnfilteredStream(id) => self.open_unfiltered_stream(id),
            AppAction::CloseUnfilteredStream => self.close_unfiltered_stream(),
            AppAction::FocusInMainAndClearFilter => self.focus_in_main_and_clear_filter(),
            AppAction::ToggleProjectPicker => {
                self.project_picker_open = !self.project_picker_open;
                if !self.project_picker_open {
                    self.project_search_query.clear();
                }
            }
            AppAction::CloseProjectPicker => self.close_project_picker(),
            AppAction::DismissTopLayer => {
                self.dismiss_top_layer();
            }
        }
    }

    /// Đóng lớp giao diện trên cùng theo thứ tự ngăn xếp (Chain of Responsibility / Pop Stack)
    pub fn dismiss_top_layer(&mut self) -> bool {
        if self.autocomplete_state.is_open {
            self.autocomplete_state.is_open = false;
            true
        } else if self.history_state.is_open {
            self.history_state.close_popup();
            true
        } else if self.project_picker_open {
            self.close_project_picker();
            true
        } else if self.column_state.is_modal_open {
            self.column_state.close_modal();
            true
        } else if self.show_launch_modal {
            self.show_launch_modal = false;
            self.launch_modal_draft = None;
            true
        } else if self.active_tab == ActiveTab::Unfiltered {
            self.close_unfiltered_stream();
            true
        } else if self.selected_log.is_some() {
            self.selected_log = None;
            true
        } else {
            false
        }
    }

    pub fn close_project_picker(&mut self) {
        self.project_picker_open = false;
        self.project_search_query.clear();
    }

    pub fn save_current_workspace(&mut self) {
        let active_idx = self.active_index;
        let gui_session = &mut self.sessions[active_idx];
        if gui_session.session.name.trim().is_empty() {
            gui_session.session.name = "Workspace".to_string();
        }

        gui_session.session.sync_location();

        let mut ws = gui_session.session.to_workspace();
        ws.last_query = gui_session.view.query.clone();
        self.store.add_or_update(ws);
    }

    pub fn load_workspace(&mut self, ws: &Workspace) {
        let active_idx = self.active_index;
        let gui_session = &mut self.sessions[active_idx];
        gui_session.session.apply_workspace(ws);
        gui_session.view.query = ws.last_query.clone();
        gui_session.session.spawn_load_environment(&self.rt);
        self.save_current_workspace();
    }

    pub fn start_configured_source(&mut self) {
        self.save_current_workspace();
        let active_idx = self.active_index;
        self.sessions[active_idx].session.start_source(&self.rt);
    }

    pub fn stop_current_source(&mut self) {
        let active_idx = self.active_index;
        self.sessions[active_idx].session.stop_source();
    }

    pub fn restart_current_source(&mut self) {
        self.save_current_workspace();
        let active_idx = self.active_index;
        self.sessions[active_idx].session.restart_source(&self.rt);

        let view = &mut self.sessions[active_idx].view;
        view.cached_logs.clear();
        view.total_matched = 0;
        view.selected_log = None;
        view.last_processed_count = 0;
        view.unfiltered_state.cached_unfiltered.clear();

        self.trigger_full_search();
    }

    pub fn spawn_load_environment(&mut self) {
        let active_idx = self.active_index;
        self.sessions[active_idx]
            .session
            .spawn_load_environment(&self.rt);
    }

    pub fn tick(&mut self) {
        // Chạy tick trên tất cả các runtime sessions (phát hiện process hoàn tất, nhận env vars)
        for s in &mut self.sessions {
            s.session.tick();
        }

        let total_processed = self.session.engine.total_processed();
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
            let (new_matched_count, new_matching_logs) = self
                .session
                .engine
                .filter_incremental(&self.query, prev_processed);

            if new_matched_count > 0 {
                self.total_matched += new_matched_count;
                self.sync_discovered_fields(&new_matching_logs);
                self.cached_logs.extend(new_matching_logs);

                if self.cached_logs.len() > self.session.display_limit {
                    let overflow = self.cached_logs.len() - self.session.display_limit;
                    self.cached_logs.drain(0..overflow);
                }
                self.has_new_data = true;
            }
        }

        if !self.is_auto_scroll && !self.query.trim().is_empty() && new_logs_arrived {
            let (new_matched, _) = self
                .session
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
            let (new_count, new_logs) = self.session.engine.filter_incremental("", prev_processed);
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
            .session
            .engine
            .search_with_count(&self.query, self.session.display_limit);
        self.total_matched = matched;
        self.sync_discovered_fields(&logs);
        self.cached_logs = logs;
        self.last_query = self.query.clone();
        self.last_processed_count = self.session.engine.total_processed();
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
        let total = self.session.engine.total_processed();
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
        self.discovered_fields_cache = self.session.engine.get_schema_map().into_iter().collect();
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
        } else {
            format!("{}:{}", field, Self::format_selection_term(clean_val))
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
        self.apply_filter_term(&exclude_term);
    }

    pub fn open_unfiltered_stream(&mut self, target_id: Option<u64>) {
        self.unfiltered_state.is_open = true;
        self.unfiltered_state.target_id = target_id;
        self.unfiltered_state.is_live = target_id.is_none();
        self.refresh_unfiltered_snapshot();
        self.unfiltered_state.has_new_data = false;
        self.active_tab = ActiveTab::Unfiltered;
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        self.unfiltered_state.snapshot_processed_count = self.session.engine.total_processed();
        let (target_idx, unfiltered) = self
            .session
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
            self.unfiltered_state.snapshot_processed_count = self.session.engine.total_processed();
        }
    }

    pub fn unlatch_unfiltered(&mut self) {
        if !self.unfiltered_state.is_live {
            return;
        }
        self.unfiltered_state.is_live = false;
        self.unfiltered_state.snapshot_processed_count = self.session.engine.total_processed();
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
        self.save_current_workspace();
        for s in &mut self.sessions {
            s.session.stop_source();
        }
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
            capacity: 100,
            display_limit: 50,
        };

        let session = WorkspaceSession::new(
            "Test Project".to_string(),
            WorkspaceLocation::local(""),
            source_config,
        );

        let gui_session = GuiSession::new(session);
        let store = WorkspaceStore::default();

        UwuGuiApp {
            sessions: vec![gui_session],
            active_index: 0,
            store,
            rt,
            show_launch_modal: false,
            launch_modal_draft: None,
            project_picker_open: false,
            project_search_query: String::new(),
            prev_screen_width: 0.0,
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
        let tx = app.session.engine.get_channel();

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
        let tx = app.session.engine.get_channel();

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
        let tx = app.session.engine.get_channel();

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
            app.session.env_status,
            uwu_core_workspace::EnvLoadStatus::Ready { .. }
        ));
    }

    #[tokio::test]
    async fn test_source_running_state_transitions_to_stopped() {
        let mut app = create_test_app();
        assert!(!app.session.is_source_running);

        #[cfg(target_os = "windows")]
        {
            app.session.source_config.command_str = "cmd /c echo test".to_string();
        }
        #[cfg(not(target_os = "windows"))]
        {
            app.session.source_config.command_str = "echo test".to_string();
        }

        app.start_configured_source();
        assert!(app.session.is_source_running);

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            app.tick();
            if !app.session.is_source_running {
                break;
            }
        }

        assert!(!app.session.is_source_running);
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
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.active_index, 0);

        app.query = "level:error".to_string();

        let ws2 = Workspace::new(
            "Project B",
            WorkspaceLocation::local("D:\\test\\proj_b"),
            SourceType::Process,
        );
        app.open_or_switch_workspace(&ws2);

        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_index, 1);
        assert_eq!(app.session.name, "Project B");
        assert_eq!(app.query, "");

        app.query = "tag:Audio".to_string();

        app.switch_session(0);
        assert_eq!(app.active_index, 0);
        assert_eq!(app.session.name, "Test Project");
        assert_eq!(app.query, "level:error");

        app.switch_session(1);
        assert_eq!(app.active_index, 1);
        assert_eq!(app.session.name, "Project B");
        assert_eq!(app.query, "tag:Audio");

        app.close_session(1);
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.active_index, 0);
        assert_eq!(app.session.name, "Test Project");
        assert_eq!(app.query, "level:error");
    }

    #[tokio::test]
    async fn test_remote_workspace_save_and_reload() {
        let mut app = create_test_app();

        // 1. Cấu hình Remote workspace trong session hiện tại
        app.session.location = WorkspaceLocation::remote(
            uwu_core_workspace::WslConnectionOptions::new("Ubuntu", "/home/user/backend"),
        );
        app.session.source_config.source_type = SourceType::Process;
        app.session.source_config.command_str = "python3 app.py".to_string();
        app.session.name = "Remote-Backend".to_string();

        // 2. Lưu workspace hiện tại
        app.save_current_workspace();

        // 3. Kiểm tra xem workspace được lưu vào store đúng chưa
        let ws = app
            .store
            .recent_workspaces
            .iter()
            .find(|w| w.name == "Remote-Backend")
            .cloned()
            .expect("Remote-Backend must be saved in store");

        assert_eq!(ws.source_type, SourceType::Process);
        assert_eq!(ws.command_str, "python3 app.py");
        let remote = ws.location.as_remote().expect("Expected remote location");
        assert_eq!(remote.display_name(), "Ubuntu");
        assert_eq!(remote.working_dir(), "/home/user/backend");

        // 4. Mở lại workspace Remote qua open_or_switch_workspace
        app.open_or_switch_workspace(&ws);
        assert_eq!(app.session.source_config.source_type, SourceType::Process);
        assert_eq!(app.session.source_config.command_str, "python3 app.py");
        let remote = app
            .session
            .location
            .as_remote()
            .expect("Expected remote location");
        assert_eq!(remote.display_name(), "Ubuntu");
        assert_eq!(remote.working_dir(), "/home/user/backend");
    }

    #[tokio::test]
    async fn test_open_or_switch_workspace_slash_normalization() {
        let mut app = create_test_app();
        let ws_forward = Workspace::new(
            "test-slash",
            WorkspaceLocation::local("D:/Learn/Go/uwulog-rust"),
            SourceType::Process,
        );
        app.open_or_switch_workspace(&ws_forward);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_index, 1);

        // Try opening the same folder with backslashes
        let ws_backward = Workspace::new(
            "test-slash-alt",
            WorkspaceLocation::local("D:\\Learn\\Go\\uwulog-rust\\"),
            SourceType::Process,
        );
        app.open_or_switch_workspace(&ws_backward);

        // Should NOT create a duplicate session, should switch to session index 1
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_index, 1);
    }

    #[test]
    fn test_extract_project_name() {
        assert_eq!(
            extract_project_name("D:\\Learn\\Go\\uwulog-rust"),
            "uwulog-rust"
        );
        assert_eq!(
            extract_project_name("D:/Learn/Go/uwulog-rust/"),
            "uwulog-rust"
        );
        assert_eq!(extract_project_name("/home/user/backend"), "backend");
        assert_eq!(extract_project_name(""), "Workspace");
    }

    #[tokio::test]
    async fn test_zed_style_draft_isolation_and_cancel() {
        let mut app = create_test_app();
        app.session.source_config.source_type = SourceType::Process;
        app.session.source_config.command_str = "cargo run".to_string();

        // Open launch modal creates draft from live session
        app.dispatch_action(AppAction::OpenLaunchModal);
        assert!(app.show_launch_modal);
        assert!(app.launch_modal_draft.is_some());

        // Modify draft in form (e.g. user toggles to File source and types a path)
        if let Some(ref mut draft) = app.launch_modal_draft {
            draft.source_type = SourceType::File;
            draft.file_path = "/var/log/test.log".to_string();
        }

        // Live session must remain completely untouched while modal is uncommitted
        assert_eq!(app.session.source_config.source_type, SourceType::Process);
        assert_eq!(app.session.source_config.command_str, "cargo run");
        assert_eq!(app.session.source_config.file_path, "");

        // User cancels modal
        app.dispatch_action(AppAction::CloseLaunchModal);
        assert!(!app.show_launch_modal);
        assert!(app.launch_modal_draft.is_none());

        // Live session remains intact
        assert_eq!(app.session.source_config.source_type, SourceType::Process);
        assert_eq!(app.session.source_config.command_str, "cargo run");
    }

    #[tokio::test]
    async fn test_zed_style_draft_apply() {
        let mut app = create_test_app();
        app.session.source_config.source_type = SourceType::Process;
        app.session.source_config.command_str = "cargo run".to_string();

        // Open modal
        app.dispatch_action(AppAction::OpenLaunchModal);
        if let Some(ref mut draft) = app.launch_modal_draft {
            draft.source_type = SourceType::File;
            draft.file_path = "C:\\logs\\app.log".to_string();
            draft.capacity = 50_000;
        }

        // Apply
        app.dispatch_action(AppAction::ApplyLaunchModal);
        assert!(!app.show_launch_modal);
        assert!(app.launch_modal_draft.is_none());

        // Live session has received the new config
        assert_eq!(app.session.source_config.source_type, SourceType::File);
        assert_eq!(app.session.source_config.file_path, "C:\\logs\\app.log");
        assert_eq!(app.session.source_config.capacity, 50_000);
    }

    #[tokio::test]
    async fn test_zed_style_app_action_dispatch() {
        let mut app = create_test_app();
        let ws1 = Workspace::new(
            "Service A",
            WorkspaceLocation::local("C:\\projects\\service_a"),
            SourceType::Process,
        );
        let ws2 = Workspace::new(
            "Service B",
            WorkspaceLocation::local("C:\\projects\\service_b"),
            SourceType::Process,
        );

        app.dispatch_action(AppAction::OpenWorkspace(ws1));
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_index, 1);

        app.dispatch_action(AppAction::OpenWorkspace(ws2));
        assert_eq!(app.sessions.len(), 3);
        assert_eq!(app.active_index, 2);

        // Cycle backwards
        app.dispatch_action(AppAction::CycleSession(false));
        assert_eq!(app.active_index, 1);

        // Cycle forwards
        app.dispatch_action(AppAction::CycleSession(true));
        assert_eq!(app.active_index, 2);

        // Switch directly
        app.dispatch_action(AppAction::SwitchSession(0));
        assert_eq!(app.active_index, 0);

        // Close session
        app.dispatch_action(AppAction::CloseSession(1));
        assert_eq!(app.sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_app_action_columns_modal_flow() {
        let mut app = create_test_app();
        let initial_cols = app.column_state.columns.clone();

        // 1. Open columns modal
        app.dispatch_action(AppAction::OpenColumnsModal);
        assert!(app.column_state.is_modal_open);
        assert!(app.column_state.draft_columns.is_some());

        // 2. Modify draft
        if let Some(ref mut draft) = app.column_state.draft_columns {
            draft[0].visible = false;
        }

        // Live columns should still be unchanged (Live vs Draft separation)
        assert_eq!(app.column_state.columns, initial_cols);
        assert!(app.column_state.columns[0].visible);

        // 3. Cancel / Close modal
        app.dispatch_action(AppAction::CloseColumnsModal);
        assert!(!app.column_state.is_modal_open);
        assert!(app.column_state.draft_columns.is_none());
        assert_eq!(app.column_state.columns, initial_cols);

        // 4. Open again, modify and Apply
        app.dispatch_action(AppAction::OpenColumnsModal);
        if let Some(ref mut draft) = app.column_state.draft_columns {
            draft[0].visible = false;
        }
        app.dispatch_action(AppAction::ApplyColumnsModal);
        assert!(!app.column_state.is_modal_open);
        assert!(!app.column_state.columns[0].visible);
    }

    #[tokio::test]
    async fn test_app_action_select_log_and_switch_tab() {
        let mut app = create_test_app();

        let event = LogEvent::new(
            "2026-08-20T10:00:00Z",
            uwu_core_schema::LogLevel::Info,
            "test message",
            std::collections::HashMap::new(),
        );

        app.dispatch_action(AppAction::SelectLog(Some(event.clone())));
        assert!(app.selected_log.is_some());
        assert_eq!(app.selected_log.as_ref().unwrap().message, "test message");

        app.dispatch_action(AppAction::SelectLog(None));
        assert!(app.selected_log.is_none());

        app.dispatch_action(AppAction::SwitchTab(ActiveTab::Unfiltered));
        assert_eq!(app.active_tab, ActiveTab::Unfiltered);

        app.dispatch_action(AppAction::SwitchTab(ActiveTab::Filtered));
        assert_eq!(app.active_tab, ActiveTab::Filtered);
    }

    #[tokio::test]
    async fn test_app_action_query_filter_and_clear() {
        let mut app = create_test_app();

        app.dispatch_action(AppAction::ApplyFilterTerm("level:error".to_string()));
        assert_eq!(app.query, "level:error");

        app.dispatch_action(AppAction::ExcludeFilterTerm("user_id:42".to_string()));
        assert_eq!(app.query, "level:error -user_id:42");

        app.dispatch_action(AppAction::ClearQuery);
        assert!(app.query.is_empty());
    }

    #[tokio::test]
    async fn test_app_action_highlights() {
        let mut app = create_test_app();

        app.dispatch_action(AppAction::ToggleRowHighlight(100));
        assert!(app.is_row_highlighted(&100));

        app.dispatch_action(AppAction::ToggleTermHighlight("error".to_string()));
        assert!(app.highlighted_terms.contains("error"));

        app.dispatch_action(AppAction::ClearAllHighlights);
        assert!(!app.is_row_highlighted(&100));
        assert!(app.highlighted_terms.is_empty());
    }

    #[tokio::test]
    async fn test_app_action_latch_and_unfiltered() {
        let mut app = create_test_app();

        // Latch toggle
        assert!(app.is_auto_scroll);
        app.dispatch_action(AppAction::ToggleLatch);
        assert!(!app.is_auto_scroll);
        app.dispatch_action(AppAction::ToggleLatch);
        assert!(app.is_auto_scroll);

        // Open & close unfiltered stream
        app.dispatch_action(AppAction::OpenUnfilteredStream(Some(42)));
        assert!(app.unfiltered_state.is_open);
        assert_eq!(app.active_tab, ActiveTab::Unfiltered);

        // Unfiltered live toggle
        assert!(!app.unfiltered_state.is_live);
        app.dispatch_action(AppAction::ToggleUnfilteredLive);
        assert!(app.unfiltered_state.is_live);

        app.dispatch_action(AppAction::RefreshUnfilteredSnapshot);

        app.dispatch_action(AppAction::FocusInMainAndClearFilter);
        assert!(!app.unfiltered_state.is_open);
        assert_eq!(app.active_tab, ActiveTab::Filtered);

        app.dispatch_action(AppAction::OpenUnfilteredStream(None));
        assert!(app.unfiltered_state.is_open);
        app.dispatch_action(AppAction::CloseUnfilteredStream);
        assert!(!app.unfiltered_state.is_open);
    }

    #[tokio::test]
    async fn test_app_action_project_picker() {
        let mut app = create_test_app();

        assert!(!app.project_picker_open);
        app.dispatch_action(AppAction::ToggleProjectPicker);
        assert!(app.project_picker_open);

        app.project_search_query = "search_test".to_string();
        app.dispatch_action(AppAction::CloseProjectPicker);
        assert!(!app.project_picker_open);
        assert!(app.project_search_query.is_empty());
    }

    #[tokio::test]
    async fn test_app_action_dismiss_top_layer() {
        let mut app = create_test_app();

        // 1. When selected_log is present
        let test_log = LogEvent::new("2026-09-10 12:00:00", LogLevel::Info, "msg", HashMap::new())
            .with_id(123);
        app.selected_log = Some(test_log);
        assert!(app.selected_log.is_some());

        // 2. Add unfiltered stream on top
        app.open_unfiltered_stream(None);
        assert_eq!(app.active_tab, ActiveTab::Unfiltered);

        // 3. Add launch modal on top
        app.show_launch_modal = true;

        // 4. Add columns modal on top
        app.column_state.is_modal_open = true;

        // 5. Add project picker on top
        app.project_picker_open = true;

        // Popping order verification:
        // Pop 1: Project picker
        app.dispatch_action(AppAction::DismissTopLayer);
        assert!(!app.project_picker_open);
        assert!(app.column_state.is_modal_open);

        // Pop 2: Columns modal
        app.dispatch_action(AppAction::DismissTopLayer);
        assert!(!app.column_state.is_modal_open);
        assert!(app.show_launch_modal);

        // Pop 3: Launch modal
        app.dispatch_action(AppAction::DismissTopLayer);
        assert!(!app.show_launch_modal);
        assert_eq!(app.active_tab, ActiveTab::Unfiltered);

        // Pop 4: Unfiltered stream tab
        app.dispatch_action(AppAction::DismissTopLayer);
        assert_eq!(app.active_tab, ActiveTab::Filtered);
        assert!(app.selected_log.is_some());

        // Pop 5: Selected log inspector
        app.dispatch_action(AppAction::DismissTopLayer);
        assert!(app.selected_log.is_none());

        // Pop 6: Nothing left to pop
        assert!(!app.dismiss_top_layer());
    }

    #[test]
    fn test_cli_args_parsing() {
        // 1. Test default arguments
        let defaults = CliArgs::try_parse_from(["uwu-gui"]).unwrap();
        assert_eq!(defaults.display_limit, 5000);
        assert_eq!(defaults.capacity, 200_000);
        assert_eq!(defaults.cmd, None);
        assert_eq!(defaults.file, None);
        assert_eq!(defaults.working_dir, None);
        assert_eq!(defaults.remote, None);
        assert_eq!(defaults.query, None);
        assert_eq!(defaults.path, None);
        assert!(defaults.trailing_cmd.is_empty());

        // 2. Test standard short flags, query and positional path
        let parsed = CliArgs::try_parse_from([
            "uwu-gui",
            "-n",
            "1234",
            "-C",
            "50000",
            "-c",
            "cargo run",
            "-d",
            "C:/my_project",
            "-q",
            "level:error",
            "server.log",
        ])
        .unwrap();
        assert_eq!(parsed.display_limit, 1234);
        assert_eq!(parsed.capacity, 50000);
        assert_eq!(parsed.cmd.as_deref(), Some("cargo run"));
        assert_eq!(parsed.working_dir.as_deref(), Some("C:/my_project"));
        assert_eq!(parsed.query.as_deref(), Some("level:error"));
        assert_eq!(parsed.path.as_deref(), Some("server.log"));

        // 3. Test short_alias -r for command
        let r_flag = CliArgs::try_parse_from(["uwu-gui", "-r", "python app.py"]).unwrap();
        assert_eq!(r_flag.cmd.as_deref(), Some("python app.py"));

        // 4. Test trailing command after `--`
        let trailing = CliArgs::try_parse_from([
            "uwu-gui",
            "-d",
            "/repo",
            "--",
            "cargo",
            "test",
            "--workspace",
        ])
        .unwrap();
        assert_eq!(trailing.working_dir.as_deref(), Some("/repo"));
        assert_eq!(
            trailing.trailing_cmd,
            vec![
                "cargo".to_string(),
                "test".to_string(),
                "--workspace".to_string()
            ]
        );

        // 5. Test standard remote flag
        let remote_clean = CliArgs::try_parse_from([
            "uwu-gui",
            "--cmd",
            "htop",
            "--remote",
            "Debian",
            "--dir",
            "/home/user",
        ])
        .unwrap();
        assert_eq!(remote_clean.cmd.as_deref(), Some("htop"));
        assert_eq!(remote_clean.remote.as_deref(), Some("Debian"));
        assert_eq!(remote_clean.working_dir.as_deref(), Some("/home/user"));

        // 6. Test backward compatibility aliases (wsl legacy)
        let wsl_legacy = CliArgs::try_parse_from([
            "uwu-gui",
            "--wsl-cmd",
            "tail -f log",
            "--wsl-distro",
            "Ubuntu-22.04",
            "--wsl-cwd",
            "/var/log",
        ])
        .unwrap();
        assert_eq!(wsl_legacy.cmd.as_deref(), Some("tail -f log"));
        assert_eq!(wsl_legacy.remote.as_deref(), Some("Ubuntu-22.04"));
        assert_eq!(wsl_legacy.working_dir.as_deref(), Some("/var/log"));
    }

    #[tokio::test]
    async fn test_resolve_target_and_build_initial_session() {
        let store = WorkspaceStore::default();

        // Case 1: Remote CLI with command
        let cli_remote = CliArgs::try_parse_from([
            "uwu-gui",
            "--remote",
            "Ubuntu",
            "-d",
            "/var/log",
            "-c",
            "journalctl -f",
            "-q",
            "error",
        ])
        .unwrap();
        let (loc, src_type, cmd, file, has_custom) = cli_remote.resolve_target();
        assert!(loc.is_remote());
        assert_eq!(loc.working_dir(), "/var/log");
        assert_eq!(src_type, SourceType::Process);
        assert_eq!(cmd, "journalctl -f");
        assert_eq!(file, "");
        assert!(has_custom);

        let (session, custom_src, _) = UwuGuiApp::build_initial_session(&cli_remote, &store);
        assert!(custom_src);
        assert_eq!(session.session.location, loc);
        assert_eq!(session.view.query, "error");

        // Case 2: Positional log file on local
        let cli_file = CliArgs::try_parse_from(["uwu-gui", "production.log"]).unwrap();
        let (loc, src_type, cmd, file, has_custom) = cli_file.resolve_target();
        assert!(!loc.is_remote());
        assert_eq!(src_type, SourceType::File);
        assert_eq!(cmd, "");
        assert_eq!(file, "production.log");
        assert!(has_custom);

        let (session, custom_src, _) = UwuGuiApp::build_initial_session(&cli_file, &store);
        assert!(custom_src);
        assert_eq!(session.session.source_config.file_path, "production.log");
    }

    #[tokio::test]
    async fn test_build_initial_session_preserves_saved_workspace_id_and_envs() {
        let mut store = WorkspaceStore::default();
        let mut saved_ws = Workspace::new(
            "SavedProj",
            WorkspaceLocation::local("D:\\my\\repo"),
            SourceType::Process,
        );
        let saved_uuid = saved_ws.id;
        saved_ws
            .env_vars
            .insert("ENV_KEY".to_string(), "ENV_VAL".to_string());
        store.recent_workspaces.push(saved_ws);

        let cli =
            CliArgs::try_parse_from(["uwu-gui", "-d", "D:\\my\\repo", "-c", "custom-cmd"]).unwrap();
        let (session, has_custom, saved_id) = UwuGuiApp::build_initial_session(&cli, &store);

        assert!(has_custom);
        assert_eq!(saved_id, Some(saved_uuid));
        assert_eq!(session.session.id, saved_uuid);
        assert_eq!(session.session.source_config.command_str, "custom-cmd");
        assert_eq!(
            session.session.env_vars.get("ENV_KEY").map(String::as_str),
            Some("ENV_VAL")
        );
    }

    #[tokio::test]
    async fn test_close_and_switch_session_behaviors() {
        let mut app = create_test_app();
        let ws1 = Workspace::new(
            "P1",
            WorkspaceLocation::local("D:\\test\\p1"),
            SourceType::Process,
        );
        let ws2 = Workspace::new(
            "P2",
            WorkspaceLocation::local("D:\\test\\p2"),
            SourceType::Process,
        );

        app.open_or_switch_workspace(&ws1);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_index, 1);
        // Kiểm tra kế thừa capacity và display_limit từ active session
        assert_eq!(app.session.source_config.capacity, 100);
        assert_eq!(app.session.display_limit, 50);

        app.open_or_switch_workspace(&ws2);
        assert_eq!(app.sessions.len(), 3);
        assert_eq!(app.active_index, 2);

        // 1. switch_session vào chính index hiện tại là no-op
        app.switch_session(2);
        assert_eq!(app.active_index, 2);

        // 2. close_session tab trước active_index sẽ giảm active_index đi 1
        app.close_session(0);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.active_index, 1);
        assert_eq!(app.session.name, "P2");

        // 3. Đóng hết các session -> tạo session mặc định và kế thừa capacity
        app.close_session(1);
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.active_index, 0);

        app.close_session(0);
        assert_eq!(app.sessions.len(), 1);
        assert_eq!(app.active_index, 0);
        assert_eq!(app.session.source_config.capacity, 100);
        assert_eq!(app.session.display_limit, 50);
    }

    #[tokio::test]
    async fn test_on_exit_saves_current_workspace() {
        use eframe::App;
        let mut app = create_test_app();
        app.sessions[0].session.name = "ExitTestProj".to_string();
        app.sessions[0].view.query = "error_query".to_string();

        app.on_exit(None);

        let ws = app
            .store
            .recent_workspaces
            .iter()
            .find(|w| w.name == "ExitTestProj");
        assert!(ws.is_some());
        assert_eq!(ws.unwrap().last_query, "error_query");
    }
}
