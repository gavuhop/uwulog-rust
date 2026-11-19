mod dispatcher;
mod keybindings;
pub mod overlay_manager;
mod startup;
pub mod workspace_manager;

pub use overlay_manager::OverlayManager;
pub use workspace_manager::WorkspaceManager;

pub use crate::actions::{AppAction, AppEvent};
pub use crate::cli::CliArgs;
pub use crate::overlay::{OverlayLayer, OverlayStack, RemoteModalPlacement};
pub use crate::session::GuiSession;
pub use crate::state::{ActiveTab, GuiViewState, RAW_STREAM_LIMIT};
use eframe::egui;
use std::time::Duration;
use tokio::runtime::Handle;
pub use uwu_core_workspace::SourceType;
use uwu_core_workspace::Workspace;

/// Zed-style Unified App Shell: Kết nối WorkspaceManager (quản lý sessions/dữ liệu)
/// và OverlayManager (quản lý floating palettes/modals) theo kiến trúc phân tầng sạch.
pub struct UwuGuiApp {
    pub workspaces: WorkspaceManager,
    pub overlays: OverlayManager,
    pub keymap: crate::keymap::KeymapManager,
    pub rt: Handle,
    pub prev_screen_width: f32,
    pub should_quit: bool,
    pub event_tx: std::sync::mpsc::Sender<AppEvent>,
    pub event_rx: std::sync::mpsc::Receiver<AppEvent>,
    pub egui_ctx: Option<egui::Context>,
}

impl UwuGuiApp {
    /// Lấy tham chiếu bất biến tùy chọn tới session đang hoạt động
    #[inline]
    pub fn active_session_opt(&self) -> Option<&GuiSession> {
        self.workspaces.active_session_opt()
    }

    /// Lấy tham chiếu khả biến tùy chọn tới session đang hoạt động
    #[inline]
    pub fn active_session_opt_mut(&mut self) -> Option<&mut GuiSession> {
        self.workspaces.active_session_opt_mut()
    }

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
        if let Some(active) = self.workspaces.active_session_opt_mut() {
            self.overlays.dismiss_top_layer(active)
        } else {
            self.overlays.stack.pop().is_some()
        }
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
        let prev_index = self.workspaces.active_index;
        self.workspaces.switch_session(index);
        if self.workspaces.active_index != prev_index {
            self.workspaces.auto_start_container_log_if_needed(&self.rt);
        }
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
        let prev_index = self.workspaces.active_index;
        self.workspaces.cycle_project(forward);
        if self.workspaces.active_index != prev_index {
            self.workspaces.auto_start_container_log_if_needed(&self.rt);
        }
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

    /// Tiếp nhận và xử lý sự kiện trả về từ background workers thông qua State Reducer (O(1))
    #[inline]
    pub fn apply_event(&mut self, event: AppEvent) {
        crate::reducer::reduce(self, event);
    }
}

impl eframe::App for UwuGuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.egui_ctx = Some(ctx.clone());
        if self.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // PHA 0: Drain background worker events in O(1) với Frame-Budget Guard
        // Giới hạn tối đa 64 events mỗi frame để giữ vững ngân sách 144 FPS (< 6.94ms) khi gặp bão log
        const MAX_EVENTS_PER_FRAME: usize = 64;
        let mut drained = 0;
        while let Ok(event) = self.event_rx.try_recv() {
            self.apply_event(event);
            drained += 1;
            if drained >= MAX_EVENTS_PER_FRAME {
                ctx.request_repaint(); // Còn event dở dang, lập tức repaint ở frame sau để drain tiếp
                break;
            }
        }

        // PHA 1: Detect search query change or needs_search and spawn async worker search
        if self.workspaces.has_active_session() {
            let query_changed = {
                let active = self.workspaces.active_session_mut();
                let changed = active.view.search.query != active.view.search.last_query
                    || active.view.search.needs_search;
                if changed {
                    active.view.search.last_query = active.view.search.query.clone();
                    active.view.search.needs_search = false;
                }
                changed
            };
            if query_changed {
                self.spawn_search();
            } else {
                let active = self.workspaces.active_session();
                let new_logs_arrived = active.session.engine.total_processed()
                    != active.view.main_tab().last_processed_count()
                    && std::time::Instant::now()
                        .duration_since(active.view.search.last_search_time)
                        > Duration::from_millis(150);
                if new_logs_arrived {
                    self.spawn_incremental_filter();
                }
            }
        }

        self.tick();
        ctx.request_repaint_after(Duration::from_millis(100));
        crate::views::render_ui(ui, self);
        if self.workspaces.has_active_session()
            && self
                .workspaces
                .active_session_mut()
                .view
                .shared
                .columns
                .poll_pending_save()
        {
            self.workspaces.save_current_workspace();
        }
    }

    fn on_exit(&mut self) {
        self.workspaces.save_all_open_workspaces();
        for s in &mut self.workspaces.sessions {
            s.session.stop_source();
        }
    }
}

#[cfg(test)]
mod tests;
