use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    pub source_type: String, // "process", "file", "wsl", "winevent"
    pub command_str: String,
    pub file_path: String,
    pub journald_unit: String,
    pub win_channel: String,
    pub last_query: String,
    pub last_opened: DateTime<Utc>,
}

impl Workspace {
    pub fn new(
        name: impl Into<String>,
        location: WorkspaceLocation,
        source_type: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            location,
            source_type: source_type.into(),
            command_str: String::new(),
            file_path: String::new(),
            journald_unit: String::new(),
            win_channel: "System".to_string(),
            last_query: String::new(),
            last_opened: Utc::now(),
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

    /// Lưu danh sách workspace ra file
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
            "wsl",
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
            "process",
        );
        let id2 = ws2.id;
        store.add_or_update(ws2);
        assert_eq!(store.recent_workspaces.len(), 2);
        assert_eq!(store.active_workspace_id, Some(id2));

        store.remove(id2);
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, Some(id1));
    }
}
