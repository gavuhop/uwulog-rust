pub use crate::cli::CliArgs;
pub use crate::overlay::{OverlayLayer, OverlayStack};
#[allow(unused_imports)]
pub use crate::session_view::{
    ActiveTab, GuiSession, GuiViewState, UnfilteredViewState, RAW_STREAM_LIMIT,
};
use clap::Parser;
use eframe::egui;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use uwu_core_schema::LogEvent;
pub use uwu_core_workspace::SourceType;
use uwu_core_workspace::{
    extract_project_name, SourceConfig, Workspace, WorkspaceSession, WorkspaceStore,
};

/// Unified Action enum for high-level application & session state mutations (Zed-style Command Pattern)
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

    // Main Menu & About
    ToggleMainMenu,
    CloseMainMenu,
    OpenAboutModal,
    CloseAboutModal,
    QuitApp,

    // Global Dismiss / Stack Pop
    DismissTopLayer,
}

pub struct UwuGuiApp {
    pub sessions: Vec<GuiSession>,
    pub active_index: usize,
    pub store: WorkspaceStore,
    pub rt: Handle,
    pub launch_modal_draft: Option<SourceConfig>,
    pub project_search_query: String,
    pub prev_screen_width: f32,
    pub overlay_stack: OverlayStack,
    pub should_quit: bool,
}

impl UwuGuiApp {
    /// Kiểm tra xem một layer có đang mở trên stack hay không
    #[inline]
    pub fn is_overlay_open(&self, layer: OverlayLayer) -> bool {
        self.overlay_stack.is_open(layer)
    }

    /// Đẩy một layer mới vào đỉnh ngăn xếp nếu chưa có
    #[inline]
    pub fn push_overlay(&mut self, layer: OverlayLayer) {
        self.overlay_stack.push(layer);
    }

    /// Đóng một layer cụ thể khỏi ngăn xếp
    #[inline]
    pub fn close_overlay(&mut self, layer: OverlayLayer) {
        self.overlay_stack.close(layer);
    }
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
            gui_session.view.search.query = q.clone();
        } else if !has_custom_source {
            if let Some(ws) = saved_ws {
                gui_session.view.search.query = ws.last_query.clone();
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
            launch_modal_draft: None,
            project_search_query: String::new(),
            prev_screen_width: 0.0,
            overlay_stack: OverlayStack::new(),
            should_quit: false,
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
        if self.sessions[self.active_index].handle_action(&action) {
            return;
        }

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
                self.push_overlay(OverlayLayer::LaunchModal);
            }
            AppAction::CloseLaunchModal => {
                self.close_overlay(OverlayLayer::LaunchModal);
                self.launch_modal_draft = None;
            }
            AppAction::ApplyLaunchModal => {
                if let Some(draft) = self.launch_modal_draft.take() {
                    self.session.source_config = draft;
                    self.save_current_workspace();
                    self.restart_current_source();
                }
                self.close_overlay(OverlayLayer::LaunchModal);
            }
            AppAction::ApplyAndRestartSource(new_config) => {
                self.session.source_config = new_config;
                self.save_current_workspace();
                self.restart_current_source();
                self.close_overlay(OverlayLayer::LaunchModal);
                self.launch_modal_draft = None;
            }
            AppAction::OpenColumnsModal => {
                self.columns.open_modal();
                self.push_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::CloseColumnsModal => {
                self.columns.close_modal();
                self.close_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::ApplyColumnsModal => {
                self.columns.apply_modal();
                self.close_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::ToggleProjectPicker => {
                if self.is_overlay_open(OverlayLayer::ProjectPicker) {
                    self.close_project_picker();
                } else {
                    self.push_overlay(OverlayLayer::ProjectPicker);
                }
            }
            AppAction::CloseProjectPicker => self.close_project_picker(),
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
            AppAction::QuitApp => {
                self.close_main_menu();
                self.should_quit = true;
            }
            AppAction::DismissTopLayer => {
                self.dismiss_top_layer();
            }
            _ => {}
        }
    }

    /// Đóng toàn bộ hệ thống menu chính và submenu liên quan
    #[inline]
    pub fn close_main_menu(&mut self) {
        self.overlay_stack.close_main_menu();
    }

    /// Đóng lớp giao diện trên cùng theo thứ tự ngăn xếp (Navigation Stack LIFO)
    pub fn dismiss_top_layer(&mut self) -> bool {
        if let Some(top) = self.overlay_stack.pop() {
            match top {
                OverlayLayer::MainMenu => {
                    self.close_overlay(OverlayLayer::ThemeSubmenu);
                }
                OverlayLayer::ThemeSubmenu => {}
                OverlayLayer::AboutModal => {}
                OverlayLayer::LaunchModal => {
                    self.launch_modal_draft = None;
                }
                OverlayLayer::ProjectPicker => {
                    self.project_search_query.clear();
                }
                OverlayLayer::ColumnsModal => {
                    self.columns.close_modal();
                }
            }
            true
        } else if self.search.autocomplete.is_open {
            self.search.autocomplete.is_open = false;
            true
        } else if self.search.history.is_open {
            self.search.history.close_popup();
            true
        } else if self.active_tab == ActiveTab::Unfiltered {
            self.close_unfiltered_stream();
            true
        } else if self.inspector.selected_log.is_some() {
            self.inspector.selected_log = None;
            true
        } else {
            false
        }
    }

    pub fn close_project_picker(&mut self) {
        self.close_overlay(OverlayLayer::ProjectPicker);
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
        ws.last_query = gui_session.view.search.query.clone();
        self.store.add_or_update(ws);
    }

    pub fn load_workspace(&mut self, ws: &Workspace) {
        let active_idx = self.active_index;
        let gui_session = &mut self.sessions[active_idx];
        gui_session.session.apply_workspace(ws);
        gui_session.view.search.query = ws.last_query.clone();
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
        view.viewport.cached_logs.clear();
        view.viewport.total_matched = 0;
        view.inspector.selected_log = None;
        view.viewport.last_processed_count = 0;
        view.unfiltered.cached_unfiltered.clear();

        self.trigger_full_search();
    }

    pub fn spawn_load_environment(&mut self) {
        let active_idx = self.active_index;
        self.sessions[active_idx]
            .session
            .spawn_load_environment(&self.rt);
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        for (idx, s) in self.sessions.iter_mut().enumerate() {
            if idx == self.active_index {
                s.tick(now);
            } else {
                s.session.tick();
            }
        }
    }

    #[inline]
    pub fn format_field_term(field: &str, val: &str) -> String {
        GuiSession::format_field_term(field, val)
    }

    #[inline]
    pub fn format_selection_term(text: &str) -> String {
        GuiSession::format_selection_term(text)
    }
}

impl eframe::App for UwuGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

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
#[path = "app_tests.rs"]
mod tests;
