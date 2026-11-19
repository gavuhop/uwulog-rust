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
        store.open_workspace_ids = sessions
            .iter()
            .filter(|s| !s.session.is_following_container_logs())
            .map(|s| s.session.id)
            .collect();
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
            if let Some(prev) = self.sessions.get_mut(self.active_index) {
                prev.view.shared.columns.clear_header_drag();
                let _ = prev.view.shared.columns.take_dirty_for_save();
                Self::persist_session_into_store(&mut self.store, prev);
            }
            self.active_index = index;
            if !self.sessions[index].session.is_following_container_logs() {
                self.store.active_workspace_id = Some(self.sessions[index].session.id);
            }
            self.store.open_workspace_ids = self
                .sessions
                .iter()
                .filter(|s| !s.session.is_following_container_logs())
                .map(|s| s.session.id)
                .collect();
            let _ = self.store.save();
            let active = self.active_session_mut();
            let needs_initial_search = active.view.main_tab().last_processed_count() == 0
                && active.session.engine.total_processed() > 0;
            let query_pending = active.view.search.query != active.view.search.last_query;
            if needs_initial_search || query_pending {
                active.view.search.mark_needs_search();
            }
        }
    }

    fn push_new_session(&mut self, ws: &Workspace, rt: &Handle) {
        let (cap, limit) = if let Some(active) = self.active_session_opt() {
            (active.session.capacity(), active.session.display_limit())
        } else {
            (500_000, 5_000)
        };
        let mut gui_session = GuiSession::from_workspace(ws, cap, limit);
        if let Some(prefs) = self
            .store
            .get_viewer_prefs(&gui_session.viewer_key())
            .cloned()
        {
            gui_session.apply_viewer_prefs(&prefs);
        }
        gui_session.session.spawn_load_environment(rt);
        self.sessions.push(gui_session);
        self.active_index = self.sessions.len() - 1;
        self.active_session_mut().view.search.mark_needs_search();
    }

    /// Chỉ tự động lấy log cho các tab container gắn chế độ stream Docker (`follow_container_logs`);
    /// KHÔNG tự chạy lệnh run/file đối với dự án bình thường hay app container.
    pub fn auto_start_container_log_if_needed(&mut self, rt: &Handle) {
        let Some(active) = self.active_session_opt_mut() else {
            return;
        };
        if active.session.is_following_container_logs()
            && !active.has_auto_started_source
            && !active.user_stopped_source
            && !active.session.is_source_running
        {
            active.has_auto_started_source = true;
            active.session.start_source(rt);
        }
    }

    /// Mở hoặc kích hoạt một Workspace từ danh sách lưu trữ
    pub fn open_or_switch_workspace(&mut self, ws: &Workspace, rt: &Handle) {
        if let Some(pos) = self.sessions.iter().position(|s| {
            s.session.id == ws.id
                || s.session.location.is_same(&ws.location)
                || s.session.location.is_same_docker_project(&ws.location)
        }) {
            self.switch_session(pos);
            self.auto_start_container_log_if_needed(rt);
            return;
        }

        self.push_new_session(ws, rt);
        self.auto_start_container_log_if_needed(rt);
        self.store.active_workspace_id = Some(ws.id);
        if let uwu_core_workspace::WorkspaceLocation::Remote(
            uwu_core_workspace::RemoteConnectionOptions::Docker(ref d),
        ) = ws.location
        {
            self.store
                .upsert_docker_connection(uwu_core_workspace::DockerConnection::from(d));
            if !d.working_dir.trim().is_empty() {
                self.store
                    .add_remote_project_to_docker_server(&d.container_name, &d.working_dir);
            }
        }
        self.store.add_or_update(ws.clone());
        self.store.open_workspace_ids = self
            .sessions
            .iter()
            .filter(|s| !s.session.is_following_container_logs())
            .map(|s| s.session.id)
            .collect();
        let _ = self.store.save();
    }

    /// Mở hoặc chuyển sang session xem log của một container anh em (`docker logs -f`)
    /// theo cơ chế giống `This Window`: giữ nguyên buffer & stream của các container đang mở khác.
    /// Trả về `true` nếu vừa tạo session mới (cần lưu/khởi chạy stream).
    pub fn open_container_log_session(&mut self, ws: &Workspace, rt: &Handle) -> bool {
        let target_container = ws.location.display_name();

        if let Some(pos) = self.sessions.iter().position(|s| {
            let same_docker_target = match &s.session.location {
                uwu_core_workspace::WorkspaceLocation::Remote(
                    uwu_core_workspace::RemoteConnectionOptions::Docker(d),
                ) => d.container_name.eq_ignore_ascii_case(target_container),
                _ => false,
            };
            same_docker_target || s.session.location.is_same(&ws.location)
        }) {
            self.switch_session(pos);
            self.auto_start_container_log_if_needed(rt);
            return false;
        }

        if let Some(prev) = self.sessions.get_mut(self.active_index) {
            prev.view.shared.columns.clear_header_drag();
            let _ = prev.view.shared.columns.take_dirty_for_save();
            Self::persist_session_into_store(&mut self.store, prev);
        }
        self.push_new_session(ws, rt);
        self.auto_start_container_log_if_needed(rt);
        true
    }

    /// Đóng một tab session cụ thể
    pub fn close_session(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }

        let mut removed = self.sessions.remove(index);
        removed.session.stop_source();
        removed.view.shared.columns.clear_header_drag();
        let _ = removed.view.shared.columns.take_dirty_for_save();
        Self::persist_session_into_store(&mut self.store, &mut removed);

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
        self.store.open_workspace_ids = self
            .sessions
            .iter()
            .filter(|s| {
                !s.session.is_following_container_logs()
                    || self
                        .store
                        .recent_workspaces
                        .iter()
                        .any(|w| w.id == s.session.id)
            })
            .map(|s| s.session.id)
            .collect();
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

    /// Lưu trạng thái viewer (một đường duy nhất cho cả project thường và container phụ)
    /// và cập nhật danh sách project gốc (`recent_workspaces`) nếu không phải container phụ.
    fn persist_session_into_store(store: &mut WorkspaceStore, gui_session: &mut GuiSession) {
        if gui_session.session.name.trim().is_empty() {
            gui_session.session.name = "Workspace".to_string();
        }
        gui_session.session.sync_location();

        store.upsert_viewer_prefs(gui_session.viewer_key(), gui_session.to_viewer_prefs());

        let is_project_root = !gui_session.session.is_following_container_logs()
            || store
                .recent_workspaces
                .iter()
                .any(|w| w.id == gui_session.session.id);
        if is_project_root {
            store.add_or_update(gui_session.session.to_workspace());
        }
    }

    pub fn save_current_workspace(&mut self) {
        if self.sessions.is_empty() {
            self.store.active_workspace_id = None;
            self.store.open_workspace_ids.clear();
            let _ = self.store.save();
            return;
        }
        let active_idx = self.active_index;
        let gui_session = &mut self.sessions[active_idx];
        Self::persist_session_into_store(&mut self.store, gui_session);
        if !gui_session.session.is_following_container_logs() {
            self.store.active_workspace_id = Some(gui_session.session.id);
        }
        self.store.open_workspace_ids = self
            .sessions
            .iter()
            .filter(|s| !s.session.is_following_container_logs())
            .map(|s| s.session.id)
            .collect();
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
            Self::persist_session_into_store(&mut self.store, s);
        }
        self.store.open_workspace_ids = self
            .sessions
            .iter()
            .filter(|s| !s.session.is_following_container_logs())
            .map(|s| s.session.id)
            .collect();
        self.store.active_workspace_id = self
            .sessions
            .get(self.active_index)
            .filter(|s| !s.session.is_following_container_logs())
            .map(|s| s.session.id)
            .or_else(|| self.store.open_workspace_ids.first().copied());
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
        gui_session.has_auto_started_source = false;
        gui_session.user_stopped_source = false;
        gui_session.view.main_tab_mut().stream.latch();
        gui_session.view.search.clear();
        gui_session.view.search.last_query.clear();
        if let Some(prefs) = self
            .store
            .get_viewer_prefs(&gui_session.viewer_key())
            .cloned()
        {
            gui_session.apply_viewer_prefs(&prefs);
        }
        gui_session.session.spawn_load_environment(rt);
        self.save_current_workspace();
        self.auto_start_container_log_if_needed(rt);
    }

    /// Bắt đầu stream nguồn cho session đang hoạt động
    pub fn start_configured_source(&mut self, rt: &Handle) {
        if self.sessions.is_empty() {
            return;
        }
        self.save_current_workspace();
        let active_idx = self.active_index;
        self.sessions[active_idx].has_auto_started_source = true;
        self.sessions[active_idx].user_stopped_source = false;
        self.sessions[active_idx].session.start_source(rt);
    }

    /// Dừng stream nguồn hiện tại
    pub fn stop_current_source(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let active_idx = self.active_index;
        self.sessions[active_idx].user_stopped_source = true;
        self.sessions[active_idx].session.stop_source();
    }

    /// Khởi động lại stream nguồn hiện tại
    pub fn restart_current_source(&mut self, rt: &Handle) {
        if self.sessions.is_empty() {
            return;
        }
        self.save_current_workspace();
        let active_idx = self.active_index;
        self.sessions[active_idx].has_auto_started_source = true;
        self.sessions[active_idx].user_stopped_source = false;
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
