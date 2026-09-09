pub mod environment;
pub mod manager;
pub mod session;

pub use environment::{load_workspace_environment, parse_dot_env, EnvLoadStatus};
pub use manager::MultiWorkspaceManager;
pub use session::{SourceConfig, SourceType, WorkspaceSession, WslConfig, WslSubMode};

#[allow(unused_imports)]
use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkspaceLocation {
    Local { working_dir: String },
    Wsl { distro: String, working_dir: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub location: WorkspaceLocation,
    pub source_type: SourceType,
    pub command_str: String,
    pub file_path: String,
    pub last_query: String,
    pub last_opened: DateTime<Utc>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

impl Workspace {
    pub fn new(
        name: impl Into<String>,
        location: WorkspaceLocation,
        source_type: SourceType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            location,
            source_type,
            command_str: String::new(),
            file_path: String::new(),
            last_query: String::new(),
            last_opened: Utc::now(),
            env_vars: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceStore {
    pub active_workspace_id: Option<Uuid>,
    pub recent_workspaces: Vec<Workspace>,
}

impl WorkspaceStore {
    /// Lấy đường dẫn lưu file cấu hình workspaces.json
    pub fn get_storage_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                return PathBuf::from(local_app_data)
                    .join("uwulog")
                    .join("workspaces.json");
            }
        }

        if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            return PathBuf::from(home)
                .join(".config")
                .join("uwulog")
                .join("workspaces.json");
        }

        PathBuf::from("workspaces.json")
    }

    /// Đọc và tải danh sách workspace từ file
    pub fn load() -> Self {
        let path = Self::get_storage_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(store) = serde_json::from_str::<WorkspaceStore>(&content) {
                    return store;
                }
            }
        }
        Self::default()
    }

    /// Lưu danh sách workspace ra file (bỏ qua khi chạy unit tests để tránh làm bẩn cấu hình thật)
    #[cfg(not(test))]
    pub fn save(&self) -> Result<()> {
        let path = Self::get_storage_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize WorkspaceStore")?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write workspaces to {:?}", path))?;
        Ok(())
    }

    #[cfg(test)]
    pub fn save(&self) -> Result<()> {
        Ok(())
    }

    /// Thêm hoặc cập nhật một workspace, tự động sắp xếp theo thời gian mở gần nhất
    pub fn add_or_update(&mut self, mut ws: Workspace) {
        ws.last_opened = Utc::now();
        let ws_id = ws.id;

        // Xóa workspace cũ nếu trùng id hoặc trùng tên
        self.recent_workspaces
            .retain(|w| w.id != ws_id && w.name != ws.name);

        self.recent_workspaces.insert(0, ws);
        self.active_workspace_id = Some(ws_id);

        // Giữ tối đa 50 workspace gần nhất
        if self.recent_workspaces.len() > 50 {
            self.recent_workspaces.truncate(50);
        }

        let _ = self.save();
    }

    /// Xóa một workspace theo ID
    pub fn remove(&mut self, id: Uuid) {
        self.recent_workspaces.retain(|w| w.id != id);
        if self.active_workspace_id == Some(id) {
            self.active_workspace_id = self.recent_workspaces.first().map(|w| w.id);
        }
        let _ = self.save();
    }

    /// Tìm workspace theo đường dẫn working directory
    pub fn find_by_workdir(&self, dir: &str) -> Option<&Workspace> {
        if dir.trim().is_empty() {
            return None;
        }
        let clean_dir = dir.trim_end_matches(&['/', '\\'][..]);
        self.recent_workspaces.iter().find(|w| {
            let ws_dir = match &w.location {
                WorkspaceLocation::Local { working_dir } => {
                    working_dir.trim_end_matches(&['/', '\\'][..])
                }
                WorkspaceLocation::Wsl { working_dir, .. } => {
                    working_dir.trim_end_matches(&['/', '\\'][..])
                }
            };
            ws_dir.eq_ignore_ascii_case(clean_dir)
        })
    }

    /// Lấy workspace đang được kích hoạt
    pub fn get_active(&self) -> Option<&Workspace> {
        if let Some(id) = self.active_workspace_id {
            self.recent_workspaces.iter().find(|w| w.id == id)
        } else {
            self.recent_workspaces.first()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_store_crud() {
        let mut store = WorkspaceStore::default();
        assert!(store.recent_workspaces.is_empty());

        let ws1 = Workspace::new(
            "backend-service",
            WorkspaceLocation::Wsl {
                distro: "Ubuntu".to_string(),
                working_dir: "/home/user/backend".to_string(),
            },
            SourceType::Wsl,
        );
        let id1 = ws1.id;

        store.add_or_update(ws1);
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, Some(id1));
        assert_eq!(store.get_active().unwrap().name, "backend-service");

        let ws2 = Workspace::new(
            "frontend-app",
            WorkspaceLocation::Local {
                working_dir: "D:\\Projects\\frontend".to_string(),
            },
            SourceType::Process,
        );
        let id2 = ws2.id;
        store.add_or_update(ws2);
        assert_eq!(store.recent_workspaces.len(), 2);
        assert_eq!(store.active_workspace_id, Some(id2));

        store.remove(id2);
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, Some(id1));
    }

    #[tokio::test]
    async fn test_multi_workspace_manager_lifecycle() {
        let store = WorkspaceStore::default();
        let session1 = WorkspaceSession::new_default(100, 50);
        let id1 = session1.id;
        let mut mgr = MultiWorkspaceManager::new(session1, store);

        assert_eq!(mgr.sessions.len(), 1);
        assert_eq!(mgr.active_index, 0);
        assert_eq!(mgr.active_session().id, id1);

        // Add second session
        let ws2 = Workspace::new(
            "test-service-2",
            WorkspaceLocation::Local {
                working_dir: "D:\\test\\service2".to_string(),
            },
            SourceType::Process,
        );
        let rt = tokio::runtime::Handle::current();
        let idx2 = mgr.open_or_switch_workspace(&ws2, 100, 50, &rt, false);
        assert_eq!(idx2, 1);
        assert_eq!(mgr.sessions.len(), 2);
        assert_eq!(mgr.active_index, 1);
        assert_eq!(mgr.active_session().name, "test-service-2");

        // Switching between sessions
        mgr.switch_session(0);
        assert_eq!(mgr.active_index, 0);
        assert_eq!(mgr.active_session().id, id1);

        // Cycle sessions
        mgr.cycle_session(true);
        assert_eq!(mgr.active_index, 1);
        mgr.cycle_session(false);
        assert_eq!(mgr.active_index, 0);

        // Close session 0
        mgr.close_session(0, 100, 50);
        assert_eq!(mgr.sessions.len(), 1);
        assert_eq!(mgr.active_session().name, "test-service-2");

        // Close last remaining session -> should auto-generate clean default session
        mgr.close_session(0, 100, 50);
        assert_eq!(mgr.sessions.len(), 1);
        assert_eq!(mgr.active_index, 0);
    }

    #[test]
    fn test_workspace_serde_json_compatibility() {
        let ws = Workspace::new(
            "test-app",
            WorkspaceLocation::Local {
                working_dir: "C:\\Projects\\app".to_string(),
            },
            SourceType::Process,
        );

        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"source_type\":\"process\""));

        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source_type, SourceType::Process);
        assert_eq!(deserialized.name, "test-app");

        // Backward compatibility: parsing legacy json with "wsl" and "file"
        let legacy_json = r#"{
            "id": "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
            "name": "wsl-app",
            "location": {
                "Wsl": {
                    "distro": "Ubuntu",
                    "working_dir": "/home/user"
                }
            },
            "source_type": "wsl",
            "command_str": "cargo run",
            "file_path": "",
            "last_query": "",
            "last_opened": "2026-09-09T12:00:00Z",
            "env_vars": {}
        }"#;
        let ws_legacy: Workspace = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(ws_legacy.source_type, SourceType::Wsl);
    }
}
