pub mod environment;
pub mod manager;
pub mod remote;
pub mod session;

pub use environment::{load_workspace_environment, parse_dot_env, EnvLoadStatus};
pub use manager::MultiWorkspaceManager;
pub use remote::{RemoteConnectionOptions, WslConnectionOptions};
pub use session::{SourceConfig, SourceType, WorkspaceSession};

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
    Remote(RemoteConnectionOptions),
}

impl WorkspaceLocation {
    pub fn local(working_dir: impl Into<String>) -> Self {
        Self::Local {
            working_dir: working_dir.into(),
        }
    }

    pub fn remote(options: impl Into<RemoteConnectionOptions>) -> Self {
        Self::Remote(options.into())
    }

    /// Lấy working directory dưới dạng tham chiếu chuỗi
    pub fn working_dir(&self) -> &str {
        match self {
            WorkspaceLocation::Local { working_dir } => working_dir,
            WorkspaceLocation::Remote(remote) => remote.working_dir(),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, WorkspaceLocation::Remote(_))
    }

    pub fn as_remote(&self) -> Option<&RemoteConnectionOptions> {
        match self {
            WorkspaceLocation::Remote(remote) => Some(remote),
            WorkspaceLocation::Local { .. } => None,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            WorkspaceLocation::Local { working_dir } => working_dir,
            WorkspaceLocation::Remote(remote) => remote.display_name(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            WorkspaceLocation::Local { .. } => "🖥",
            WorkspaceLocation::Remote(remote) => remote.icon(),
        }
    }

    /// Lấy working directory đã được chuẩn hóa để so sánh
    pub fn normalized_dir(&self) -> String {
        normalize_workdir(self.working_dir())
    }

    /// So sánh xem 2 location có cùng trỏ tới một vị trí hay không
    pub fn is_same(&self, other: &Self) -> bool {
        match (self, other) {
            (
                WorkspaceLocation::Local { working_dir: d1 },
                WorkspaceLocation::Local { working_dir: d2 },
            ) => normalize_workdir(d1) == normalize_workdir(d2),
            (WorkspaceLocation::Remote(r1), WorkspaceLocation::Remote(r2)) => r1.is_same(r2),
            _ => false,
        }
    }
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

    /// Icon đại diện cho loại workspace (Remote, File, hoặc Process)
    pub fn icon(&self) -> &'static str {
        match &self.location {
            WorkspaceLocation::Remote(remote) => remote.icon(),
            WorkspaceLocation::Local { .. } => match self.source_type {
                SourceType::File => "📄",
                SourceType::Process => "🖥",
            },
        }
    }

    /// Tên hiển thị kèm distro/remote nếu có (dùng cho title/label)
    pub fn display_label(&self) -> String {
        let name = if self.name.is_empty() {
            "Workspace"
        } else {
            &self.name
        };
        match &self.location {
            WorkspaceLocation::Remote(remote) => format!("{} ({})", name, remote.display_name()),
            WorkspaceLocation::Local { .. } => name.to_string(),
        }
    }

    /// Đường dẫn tóm tắt mục tiêu (dùng cho tooltip hoặc subtitle)
    pub fn target_summary(&self) -> String {
        match &self.location {
            WorkspaceLocation::Remote(remote) => remote.summary(),
            WorkspaceLocation::Local { working_dir } => {
                if !working_dir.is_empty() {
                    working_dir.clone()
                } else if !self.file_path.is_empty() {
                    self.file_path.clone()
                } else if !self.command_str.is_empty() {
                    self.command_str.clone()
                } else {
                    "Local Workspace".to_string()
                }
            }
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
        let clean_dir = normalize_workdir(dir);
        self.recent_workspaces
            .iter()
            .find(|w| w.location.normalized_dir() == clean_dir)
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

/// Làm sạch đường dẫn (bỏ khoảng trắng thừa, dấu gạch chéo ở cuối, và tiền tố verbatim Windows `\\?\`)
pub fn clean_path(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches(&['/', '\\'][..]);
    trimmed.strip_prefix(r"\\?\").unwrap_or(trimmed).to_string()
}

/// Chuẩn hóa đường dẫn thư mục để so sánh (bỏ dấu gạch chéo thừa, chuẩn hóa / và \, strip tiền tố Windows verbatim \\?\, chuyển về chữ thường)
pub fn normalize_workdir(path: &str) -> String {
    clean_path(path).replace('\\', "/").to_lowercase()
}

/// Trích xuất tên project từ đường dẫn thư mục (e.g. "D:\Projects\my-app" -> "my-app")
pub fn extract_project_name(path_str: &str) -> String {
    let clean = clean_path(path_str);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_store_crud() {
        let mut store = WorkspaceStore::default();
        assert!(store.recent_workspaces.is_empty());

        let ws1 = Workspace::new(
            "backend-service",
            WorkspaceLocation::remote(RemoteConnectionOptions::wsl("Ubuntu", "/home/user/backend")),
            SourceType::Process,
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

    #[test]
    fn test_find_by_workdir_normalization() {
        let mut store = WorkspaceStore::default();
        let ws = Workspace::new(
            "uwulog-rust",
            WorkspaceLocation::Local {
                working_dir: "D:\\Learn\\Go\\uwulog-rust".to_string(),
            },
            SourceType::Process,
        );
        store.add_or_update(ws);

        // Exact match
        assert!(store
            .find_by_workdir("D:\\Learn\\Go\\uwulog-rust")
            .is_some());
        // Forward slash match
        assert!(store.find_by_workdir("D:/Learn/Go/uwulog-rust").is_some());
        // Trailing slash
        assert!(store.find_by_workdir("D:/Learn/Go/uwulog-rust/").is_some());
        assert!(store
            .find_by_workdir("D:\\Learn\\Go\\uwulog-rust\\")
            .is_some());
        // Windows verbatim prefix
        assert!(store
            .find_by_workdir(r"\\?\D:\Learn\Go\uwulog-rust")
            .is_some());
        // Case insensitivity
        assert!(store.find_by_workdir("d:/learn/go/uwulog-rust").is_some());
        // Non-matching dir
        assert!(store.find_by_workdir("D:/Other/Path").is_none());
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

        let wsl_ws = Workspace::new(
            "wsl-app",
            WorkspaceLocation::remote(RemoteConnectionOptions::wsl("Ubuntu", "/home/user")),
            SourceType::Process,
        );
        let wsl_json = serde_json::to_string(&wsl_ws).unwrap();
        let deserialized_wsl: Workspace = serde_json::from_str(&wsl_json).unwrap();
        assert_eq!(deserialized_wsl.source_type, SourceType::Process);
        assert!(deserialized_wsl.location.is_remote());
        assert_eq!(deserialized_wsl.location.display_name(), "Ubuntu");
        assert_eq!(deserialized_wsl.location.icon(), "🐧");
        assert_eq!(deserialized_wsl.location.working_dir(), "/home/user");
    }

    #[tokio::test]
    async fn test_workspace_domain_methods() {
        let local_ws = Workspace::new(
            "my-app",
            WorkspaceLocation::Local {
                working_dir: "C:\\Projects\\app".to_string(),
            },
            SourceType::Process,
        );
        assert_eq!(local_ws.icon(), "🖥");
        assert_eq!(local_ws.display_label(), "my-app");
        assert_eq!(local_ws.target_summary(), "C:\\Projects\\app");

        let wsl_ws = Workspace::new(
            "ubuntu-service",
            WorkspaceLocation::remote(RemoteConnectionOptions::wsl(
                "Ubuntu-22.04",
                "/home/user/service",
            )),
            SourceType::Process,
        );
        assert_eq!(wsl_ws.icon(), "🐧");
        assert_eq!(wsl_ws.display_label(), "ubuntu-service (Ubuntu-22.04)");
        assert_eq!(wsl_ws.target_summary(), "/home/user/service (Ubuntu-22.04)");

        let file_ws = Workspace::new(
            "syslog",
            WorkspaceLocation::Local {
                working_dir: "C:\\Logs".to_string(),
            },
            SourceType::File,
        );
        assert_eq!(file_ws.icon(), "📄");

        let session = WorkspaceSession::from_workspace(&wsl_ws, 100, 50);
        assert_eq!(session.icon(), "🐧");
        assert_eq!(
            session.location.as_remote().map(|r| r.display_name()),
            Some("Ubuntu-22.04")
        );
        assert_eq!(
            session.target_summary(),
            "/home/user/service (Ubuntu-22.04)"
        );
    }
}
