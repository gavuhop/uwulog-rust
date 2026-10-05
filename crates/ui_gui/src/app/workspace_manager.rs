use crate::session::GuiSession;
use std::time::Instant;
use tokio::runtime::Handle;
use uwu_core_workspace::{Workspace, WorkspaceStore};

/// Zed-style Sub-Manager chịu trách nhiệm quản lý Workspace sessions, state persistence và data fetching.
pub struct WorkspaceManager {
    pub sessions: Vec<GuiSession>,
    pub active_index: usize,
    pub store: WorkspaceStore,
}

impl WorkspaceManager {
    pub fn new(initial_session: GuiSession, store: WorkspaceStore) -> Self {
        let active_id = initial_session.session.id;
        let mut mgr = Self {
            sessions: vec![initial_session],
            active_index: 0,
            store,
        };
        mgr.store.active_workspace_id = Some(active_id);
        mgr.store.open_workspace_ids = vec![active_id];
        mgr
    }

    pub fn empty(store: WorkspaceStore) -> Self {
        Self {
            sessions: Vec::new(),
            active_index: 0,
            store,
        }
    }

    pub fn with_sessions(
        sessions: Vec<GuiSession>,
        active_index: usize,
        mut store: WorkspaceStore,
    ) -> Self {
        let active_index = if sessions.is_empty() {
            0
        } else {
            active_index.min(sessions.len() - 1)
        };
        store.open_workspace_ids = sessions.iter().map(|s| s.session.id).collect();
        store.active_workspace_id = sessions.get(active_index).map(|s| s.session.id);
        let _ = store.save();

        Self {
            sessions,
            active_index,
            store,
        }
    }

    /// Kiểm tra xem hiện có session nào đang mở hay không
    #[inline]
    pub fn has_active_session(&self) -> bool {
        !self.sessions.is_empty()
    }

    /// Lấy tham chiếu bất biến tùy chọn tới session đang hoạt động
    #[inline]
    pub fn active_session_opt(&self) -> Option<&GuiSession> {
        self.sessions.get(self.active_index)
    }

    /// Lấy tham chiếu khả biến tùy chọn tới session đang hoạt động
    #[inline]
    pub fn active_session_opt_mut(&mut self) -> Option<&mut GuiSession> {
        let idx = self.active_index;
        self.sessions.get_mut(idx)
    }

    /// Lấy tham chiếu bất biến tới session đang hoạt động
    #[inline]
    pub fn active_session(&self) -> &GuiSession {
        &self.sessions[self.active_index]
    }

    /// Lấy tham chiếu khả biến tới session đang hoạt động
    #[inline]
    pub fn active_session_mut(&mut self) -> &mut GuiSession {
        let idx = self.active_index;
        &mut self.sessions[idx]
    }

    /// Chuyển đổi session đang xem sang chỉ mục tương ứng
    pub fn switch_session(&mut self, index: usize) {
        if index < self.sessions.len() {
            if self.active_index == index {
                return;
            }
            self.active_index = index;
            self.store.active_workspace_id = Some(self.sessions[index].session.id);
            self.store.open_workspace_ids = self.sessions.iter().map(|s| s.session.id).collect();
            let _ = self.store.save();
            self.active_session_mut().view.search.mark_needs_search();
        }
    }

    /// Mở hoặc kích hoạt một Workspace từ danh sách lưu trữ
    pub fn open_or_switch_workspace(&mut self, ws: &Workspace, rt: &Handle) {
        if let Some(pos) = self
            .sessions
            .iter()
            .position(|s| s.session.id == ws.id || s.session.location.is_same(&ws.location))
        {
            self.switch_session(pos);
            return;
        }

        let (cap, limit) = if let Some(active) = self.active_session_opt() {
            (
                active.session.source_config.capacity,
                active.session.display_limit,
            )
        } else {
            (500_000, 5_000)
        };
        let mut gui_session = GuiSession::from_workspace(ws, cap, limit);
        gui_session.session.spawn_load_environment(rt);
        self.sessions.push(gui_session);
        self.active_index = self.sessions.len() - 1;
        self.store.active_workspace_id = Some(ws.id);
        self.store.add_or_update(ws.clone());
        self.store.open_workspace_ids = self.sessions.iter().map(|s| s.session.id).collect();
        let _ = self.store.save();
        self.active_session_mut().view.search.mark_needs_search();
    }

    /// Đóng một tab session cụ thể
    pub fn close_session(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }

        let mut removed = self.sessions.remove(index);
        removed.session.stop_source();

        if self.sessions.is_empty() {
            self.active_index = 0;
            self.store.active_workspace_id = None;
            self.store.open_workspace_ids.clear();
            let _ = self.store.save();
            return;
        } else if self.active_index > index {
            self.active_index -= 1;
        } else if self.active_index >= self.sessions.len() {
            self.active_index = self.sessions.len() - 1;
        }

        self.store.active_workspace_id = Some(self.sessions[self.active_index].session.id);
        self.store.open_workspace_ids = self.sessions.iter().map(|s| s.session.id).collect();
        let _ = self.store.save();
        self.active_session_mut().view.search.mark_needs_search();
    }

    /// Chuyển đổi session theo vòng lặp (Next / Prev)
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

    /// Lưu trạng thái workspace hiện tại vào file lưu trữ cấu hình
    pub fn save_current_workspace(&mut self) {
        if self.sessions.is_empty() {
            self.store.active_workspace_id = None;
            self.store.open_workspace_ids.clear();
            let _ = self.store.save();
            return;
        }
        let active_idx = self.active_index;
        let gui_session = &mut self.sessions[active_idx];
        if gui_session.session.name.trim().is_empty() {
            gui_session.session.name = "Workspace".to_string();
        }

        gui_session.session.sync_location();

        let mut ws = gui_session.session.to_workspace();
        ws.last_query = gui_session.view.search.query.clone();
        self.store.add_or_update(ws);
        self.store.open_workspace_ids = self.sessions.iter().map(|s| s.session.id).collect();
        self.store.active_workspace_id = Some(self.sessions[active_idx].session.id);
        let _ = self.store.save();
    }

    /// Lưu trạng thái của toàn bộ các workspaces đang mở trước khi thoát ứng dụng
    pub fn save_all_open_workspaces(&mut self) {
        if self.sessions.is_empty() {
            self.store.active_workspace_id = None;
            self.store.open_workspace_ids.clear();
            let _ = self.store.save();
            return;
        }
        for s in &mut self.sessions {
            if s.session.name.trim().is_empty() {
                s.session.name = "Workspace".to_string();
            }
            s.session.sync_location();
            let mut ws = s.session.to_workspace();
            ws.last_query = s.view.search.query.clone();
            self.store.add_or_update(ws);
        }
        self.store.open_workspace_ids = self.sessions.iter().map(|s| s.session.id).collect();
        self.store.active_workspace_id = self.sessions.get(self.active_index).map(|s| s.session.id);
        let _ = self.store.save();
    }

    /// Tải cấu hình từ một Workspace đã lưu
    pub fn load_workspace(&mut self, ws: &Workspace, rt: &Handle) {
        if self.sessions.is_empty() {
            self.open_or_switch_workspace(ws, rt);
            return;
        }
        let active_idx = self.active_index;
        let gui_session = &mut self.sessions[active_idx];
        gui_session.session.apply_workspace(ws);
        gui_session.view.search.query = ws.last_query.clone();
        gui_session.session.spawn_load_environment(rt);
        self.save_current_workspace();
    }

    /// Bắt đầu stream nguồn cho session đang hoạt động
    pub fn start_configured_source(&mut self, rt: &Handle) {
        if self.sessions.is_empty() {
            return;
        }
        self.save_current_workspace();
        let active_idx = self.active_index;
        self.sessions[active_idx].session.start_source(rt);
    }

    /// Dừng stream nguồn hiện tại
    pub fn stop_current_source(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let active_idx = self.active_index;
        self.sessions[active_idx].session.stop_source();
    }

    /// Khởi động lại stream nguồn hiện tại
    pub fn restart_current_source(&mut self, rt: &Handle) {
        if self.sessions.is_empty() {
            return;
        }
        self.save_current_workspace();
        let active_idx = self.active_index;
        self.sessions[active_idx].session.restart_source(rt);
        self.sessions[active_idx].view.reset_stream_data();
        self.active_session_mut().view.search.mark_needs_search();
    }

    /// Chạy tác vụ tải biến môi trường trong nền
    pub fn spawn_load_environment(&mut self, rt: &Handle) {
        if self.sessions.is_empty() {
            return;
        }
        let active_idx = self.active_index;
        self.sessions[active_idx].session.spawn_load_environment(rt);
    }

    /// Cập nhật trạng thái định kỳ cho tất cả các sessions
    pub fn tick(&mut self, now: Instant) {
        for (idx, s) in self.sessions.iter_mut().enumerate() {
            if idx == self.active_index {
                s.tick(now);
            } else {
                s.session.tick();
            }
        }
    }
}
