pub mod overlay_manager;
pub mod workspace_manager;

pub use overlay_manager::OverlayManager;
pub use workspace_manager::WorkspaceManager;

pub use crate::actions::AppAction;
pub use crate::cli::CliArgs;
use crate::keymap::KeyActionExt;
pub use crate::overlay::{OverlayLayer, OverlayStack, RemoteModalPlacement};
pub use crate::session::GuiSession;
pub use crate::state::{ActiveTab, GuiViewState, RAW_STREAM_LIMIT};
use clap::Parser;
use eframe::egui;
use std::time::Duration;
use tokio::runtime::Handle;
pub use uwu_core_workspace::SourceType;
use uwu_core_workspace::{
    extract_project_name, SourceConfig, Workspace, WorkspaceSession, WorkspaceStore,
};

/// Zed-style Unified App Shell: Kết nối WorkspaceManager (quản lý sessions/dữ liệu)
/// và OverlayManager (quản lý floating palettes/modals) theo kiến trúc phân tầng sạch.
pub struct UwuGuiApp {
    pub workspaces: WorkspaceManager,
    pub overlays: OverlayManager,
    pub keymap: crate::keymap::KeymapManager,
    pub rt: Handle,
    pub prev_screen_width: f32,
    pub should_quit: bool,
}

impl UwuGuiApp {
    /// Lấy tham chiếu bất biến tới session đang hoạt động
    #[inline]
    pub fn active_session(&self) -> &GuiSession {
        self.workspaces.active_session()
    }

    /// Lấy tham chiếu khả biến tới session đang hoạt động
    #[inline]
    pub fn active_session_mut(&mut self) -> &mut GuiSession {
        self.workspaces.active_session_mut()
    }

    /// Kiểm tra xem một layer có đang mở trên stack hay không
    #[inline]
    pub fn is_overlay_open(&self, layer: OverlayLayer) -> bool {
        self.overlays.is_open(layer)
    }

    /// Lấy ngữ cảnh phím tắt hiện tại dựa trên Overlay Stack và trạng thái session (chuẩn Zed Dispatch Tree)
    pub fn current_key_context(&self) -> crate::keymap::KeyContext {
        match self.overlays.stack.top() {
            Some(OverlayLayer::RemoteServersModal) => crate::keymap::KeyContext::RemoteServers,
            Some(OverlayLayer::LaunchModal)
            | Some(OverlayLayer::AboutModal)
            | Some(OverlayLayer::ColumnsModal)
            | Some(OverlayLayer::ProjectPicker)
            | Some(OverlayLayer::MainMenu)
            | Some(OverlayLayer::ThemeSubmenu) => crate::keymap::KeyContext::Modal,
            None => {
                if self.active_session().view.search.autocomplete.is_open {
                    crate::keymap::KeyContext::Autocomplete
                } else {
                    crate::keymap::KeyContext::Global
                }
            }
        }
    }

    /// Tự động suy luận KeyContext hiện tại dựa trên Overlay Stack, Autocomplete Popup và trạng thái Focus
    pub fn resolve_active_key_context(&self, ctx: &egui::Context) -> crate::keymap::KeyContext {
        let base = self.current_key_context();
        if base == crate::keymap::KeyContext::Global
            && ctx.memory(|m| m.has_focus(egui::Id::new("search_query_input")))
        {
            crate::keymap::KeyContext::SearchInput
        } else {
            base
        }
    }

    /// Bộ điều phối phím tắt tập trung tại đầu mỗi Frame (Centralized Key Event Pipeline)
    pub fn handle_keybindings(
        &mut self,
        ctx: &egui::Context,
        dispatch: &mut impl FnMut(AppAction),
    ) {
        let active_context = self.resolve_active_key_context(ctx);

        // Với RemoteServersModal: các subviews tự tiêu thụ Action (SelectNext, SelectPrev, ConfirmSelection, Back, TabComplete)
        // trong lúc render UI qua keymap.consume_input.
        if active_context == crate::keymap::KeyContext::RemoteServers {
            return;
        }

        if let Some(key_action) = self.keymap.consume_input_ctx(ctx, active_context) {
            match key_action {
                crate::keymap::KeyAction::ZoomIn => {
                    let current = ctx.zoom_factor();
                    ctx.set_zoom_factor((current + 0.1).min(2.5));
                }
                crate::keymap::KeyAction::ZoomOut => {
                    let current = ctx.zoom_factor();
                    ctx.set_zoom_factor((current - 0.1).max(0.6));
                }
                crate::keymap::KeyAction::ResetZoom => {
                    ctx.set_zoom_factor(1.0);
                }
                crate::keymap::KeyAction::ConfirmSelection => match active_context {
                    crate::keymap::KeyContext::Modal => match self.overlays.stack.top() {
                        Some(OverlayLayer::LaunchModal) => {
                            dispatch(AppAction::ApplyLaunchModal);
                        }
                        Some(OverlayLayer::ColumnsModal) => {
                            dispatch(AppAction::ApplyColumnsModal);
                        }
                        Some(OverlayLayer::AboutModal) => {
                            dispatch(AppAction::CloseAboutModal);
                        }
                        _ => {}
                    },
                    crate::keymap::KeyContext::Autocomplete => {
                        dispatch(AppAction::AutocompleteConfirm);
                    }
                    _ => {}
                },
                _ => {
                    if let Some(app_action) = key_action.to_app_action() {
                        dispatch(app_action);
                    }
                }
            }
        }
    }

    /// Lấy nhãn phím tắt định dạng thân thiện cho UI (như "Alt+P", "Ctrl+PageUp")
    #[inline]
    pub fn keystroke_text_for(&self, action: &crate::keymap::KeyAction) -> String {
        self.keymap
            .get_label_for_action(action, crate::keymap::KeyContext::Global)
            .unwrap_or_default()
    }

    /// Đẩy một layer mới vào đỉnh ngăn xếp nếu chưa có
    #[inline]
    pub fn push_overlay(&mut self, layer: OverlayLayer) {
        self.overlays.push(layer);
    }

    /// Đóng một layer cụ thể khỏi ngăn xếp
    #[inline]
    pub fn close_overlay(&mut self, layer: OverlayLayer) {
        self.overlays.close(layer);
    }

    /// Đóng toàn bộ hệ thống menu chính và submenu liên quan
    #[inline]
    pub fn close_main_menu(&mut self) {
        self.overlays.close_main_menu();
    }

    /// Đóng popup chuyển dự án và xóa truy vấn tìm kiếm
    #[inline]
    pub fn close_project_picker(&mut self) {
        self.overlays.close_project_picker();
    }

    /// Đóng lớp giao diện trên cùng theo thứ tự ngăn xếp (Navigation Stack LIFO)
    #[inline]
    pub fn dismiss_top_layer(&mut self) -> bool {
        self.overlays
            .dismiss_top_layer(self.workspaces.active_session_mut())
    }

    /// Lưu trạng thái workspace hiện tại vào file lưu trữ cấu hình
    #[inline]
    pub fn save_current_workspace(&mut self) {
        self.workspaces.save_current_workspace();
    }

    /// Tải cấu hình từ một Workspace đã lưu
    #[inline]
    pub fn load_workspace(&mut self, ws: &Workspace) {
        self.workspaces.load_workspace(ws, &self.rt);
    }

    /// Chuyển đổi session đang xem sang chỉ mục tương ứng
    #[inline]
    pub fn switch_session(&mut self, index: usize) {
        self.workspaces.switch_session(index);
    }

    /// Mở hoặc kích hoạt một Workspace từ danh sách lưu trữ
    #[inline]
    pub fn open_or_switch_workspace(&mut self, ws: &Workspace) {
        self.workspaces.open_or_switch_workspace(ws, &self.rt);
    }

    /// Đóng một tab session cụ thể
    #[inline]
    pub fn close_session(&mut self, index: usize) {
        self.workspaces.close_session(index);
    }

    /// Chuyển đổi session theo vòng lặp (Next / Prev)
    #[inline]
    pub fn cycle_project(&mut self, forward: bool) {
        self.workspaces.cycle_project(forward);
    }

    /// Bắt đầu stream nguồn cho session đang hoạt động
    #[inline]
    pub fn start_configured_source(&mut self) {
        self.workspaces.start_configured_source(&self.rt);
    }

    /// Dừng stream nguồn hiện tại
    #[inline]
    pub fn stop_current_source(&mut self) {
        self.workspaces.stop_current_source();
    }

    /// Khởi động lại stream nguồn hiện tại
    #[inline]
    pub fn restart_current_source(&mut self) {
        self.workspaces.restart_current_source(&self.rt);
    }

    /// Chạy tác vụ tải biến môi trường trong nền
    #[inline]
    pub fn spawn_load_environment(&mut self) {
        self.workspaces.spawn_load_environment(&self.rt);
    }

    /// Cập nhật trạng thái định kỳ cho các sessions
    #[inline]
    pub fn tick(&mut self) {
        self.workspaces.tick(std::time::Instant::now());
    }

    #[inline]
    pub fn format_field_term(field: &str, val: &str) -> String {
        GuiSession::format_field_term(field, val)
    }

    #[inline]
    pub fn format_selection_term(text: &str) -> String {
        GuiSession::format_selection_term(text)
    }

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
        let cli = CliArgs::parse();
        let mut store = WorkspaceStore::load();

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

        let (initial_gui_session, has_custom_source, saved_id) =
            Self::build_initial_session(&cli, &store);

        if let (Some(id), false) = (saved_id, has_custom_source) {
            store.active_workspace_id = Some(id);
            let _ = store.save();
        }

        let workspaces = WorkspaceManager::new(initial_gui_session, store);
        let overlays = OverlayManager::new();

        let mut keymap = crate::keymap::KeymapManager::new();
        // Đảm bảo file cấu hình mẫu và nạp cấu hình phím tắt của người dùng nếu có
        let _ = crate::keymap::ensure_sample_config_file(None);
        let _ = crate::keymap::load_user_keymap(&mut keymap, None);

        let mut app = Self {
            workspaces,
            overlays,
            keymap,
            rt,
            prev_screen_width: 0.0,
            should_quit: false,
        };

        // Background task nạp biến môi trường cho session đầu tiên
        app.spawn_load_environment();

        if saved_id.is_none() || has_custom_source {
            // Tự động lưu workspace mới hoặc cấu hình nguồn mới vào store
            app.save_current_workspace();
        }

        if has_custom_source {
            app.start_configured_source();
        }
        app.active_session_mut().trigger_full_search();

        app
    }

    /// Điều phối và thực thi các hành động cấp ứng dụng (Zed-style Command Dispatcher)
    pub fn dispatch_action(&mut self, action: AppAction) {
        if self.workspaces.active_session_mut().handle_action(&action) {
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
                    self.active_session_mut().session.source_config = draft;
                    self.save_current_workspace();
                    self.restart_current_source();
                }
                self.close_overlay(OverlayLayer::LaunchModal);
            }
            AppAction::ApplyAndRestartSource(new_config) => {
                self.active_session_mut().session.source_config = new_config;
                self.save_current_workspace();
                self.restart_current_source();
                self.close_overlay(OverlayLayer::LaunchModal);
                self.overlays.launch_modal_draft = None;
            }
            AppAction::OpenColumnsModal => {
                self.active_session_mut().view.columns.open_modal();
                self.push_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::CloseColumnsModal => {
                self.active_session_mut().view.columns.close_modal();
                self.close_overlay(OverlayLayer::ColumnsModal);
            }
            AppAction::ApplyColumnsModal => {
                self.active_session_mut().view.columns.apply_modal();
                self.close_overlay(OverlayLayer::ColumnsModal);
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
            AppAction::CloseAboutModal => {
                self.close_overlay(OverlayLayer::AboutModal);
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
    }
}

impl eframe::App for UwuGuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.tick();
        ctx.request_repaint_after(Duration::from_millis(100));
        crate::views::render_ui(ui, self);
    }

    fn on_exit(&mut self) {
        self.save_current_workspace();
        for s in &mut self.workspaces.sessions {
            s.session.stop_source();
        }
    }
}

#[cfg(test)]
mod tests;
