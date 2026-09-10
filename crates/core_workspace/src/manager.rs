use crate::session::WorkspaceSession;
use crate::{Workspace, WorkspaceStore};
use tokio::runtime::Handle;
use uuid::Uuid;

/// Quản lý tập hợp các WorkspaceSession trong cùng một cửa sổ ứng dụng (tương tự MultiWorkspace của Zed).
pub struct MultiWorkspaceManager {
    pub sessions: Vec<WorkspaceSession>,
    pub active_index: usize,
    pub store: WorkspaceStore,
}

impl MultiWorkspaceManager {
    pub fn new(initial_session: WorkspaceSession, store: WorkspaceStore) -> Self {
        let active_id = initial_session.id;
        let mut mgr = Self {
            sessions: vec![initial_session],
            active_index: 0,
            store,
        };
        mgr.store.active_workspace_id = Some(active_id);
        mgr
    }

    pub fn active_session(&self) -> &WorkspaceSession {
        &self.sessions[self.active_index]
    }

    pub fn active_session_mut(&mut self) -> &mut WorkspaceSession {
        &mut self.sessions[self.active_index]
    }

    pub fn switch_session(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active_index = index;
            self.store.active_workspace_id = Some(self.sessions[index].id);
            let _ = self.store.save();
        }
    }

    pub fn find_session_by_id(&self, id: Uuid) -> Option<usize> {
        self.sessions.iter().position(|s| s.id == id)
    }

    pub fn find_session_by_workdir(&self, dir: &str) -> Option<usize> {
        if dir.trim().is_empty() {
            return None;
        }
        let clean_dir = crate::normalize_workdir(dir);
        self.sessions
            .iter()
            .position(|s| s.location.normalized_dir() == clean_dir)
    }

    pub fn add_session(&mut self, session: WorkspaceSession, activate: bool) -> usize {
        let sid = session.id;
        if let Some(existing_idx) = self.find_session_by_id(sid) {
            if activate {
                self.switch_session(existing_idx);
            }
            return existing_idx;
        }

        self.sessions.push(session);
        let new_idx = self.sessions.len() - 1;
        if activate {
            self.switch_session(new_idx);
        }
        new_idx
    }

    pub fn open_or_switch_workspace(
        &mut self,
        ws: &Workspace,
        capacity: usize,
        display_limit: usize,
        rt: &Handle,
        auto_start: bool,
    ) -> usize {
        // 1. Kiểm tra xem workspace này đã mở trong window chưa (theo ID hoặc Location)
        if let Some(existing_idx) = self
            .sessions
            .iter()
            .position(|s| s.id == ws.id || s.location.is_same(&ws.location))
        {
            self.switch_session(existing_idx);
            return existing_idx;
        }

        // 2. Nếu chưa mở, tạo session mới từ workspace
        let mut session = WorkspaceSession::from_workspace(ws, capacity, display_limit);
        session.spawn_load_environment(rt);
        if auto_start {
            session.start_source(rt);
        }

        self.sessions.push(session);
        let new_idx = self.sessions.len() - 1;
        self.switch_session(new_idx);
        new_idx
    }

    pub fn close_session(
        &mut self,
        index: usize,
        default_capacity: usize,
        default_display_limit: usize,
    ) -> usize {
        if index < self.sessions.len() {
            let mut removed = self.sessions.remove(index);
            removed.stop_source();
        }

        if self.sessions.is_empty() {
            let default_session =
                WorkspaceSession::new_default(default_capacity, default_display_limit);
            self.sessions.push(default_session);
            self.active_index = 0;
        } else if self.active_index >= self.sessions.len() {
            self.active_index = self.sessions.len() - 1;
        }

        self.store.active_workspace_id = Some(self.sessions[self.active_index].id);
        let _ = self.store.save();
        self.active_index
    }

    pub fn cycle_session(&mut self, forward: bool) {
        if self.sessions.is_empty() {
            return;
        }
        let len = self.sessions.len();
        let next_idx = if forward {
            (self.active_index + 1) % len
        } else {
            (self.active_index + len - 1) % len
        };
        self.switch_session(next_idx);
    }

    pub fn tick_all(&mut self) {
        for session in &mut self.sessions {
            session.tick();
        }
    }

    pub fn stop_all(&mut self) {
        for session in &mut self.sessions {
            session.stop_source();
        }
    }
}
