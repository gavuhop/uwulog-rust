pub mod environment;
pub mod manager;
pub mod remote;
pub mod session;

pub use environment::{load_workspace_environment, parse_dot_env, EnvLoadStatus};
pub use manager::MultiWorkspaceManager;
pub use remote::{
    RemoteConnectionOptions, RemoteProject, ServerConnection, SshConnection, SshConnectionOptions,
    WslConnection, WslConnectionOptions,
};
pub use session::{SourceConfig, SourceType, WorkspaceSession};
pub use uwu_icons::IconName;

#[allow(unused_imports)]
use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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

    pub fn icon(&self) -> IconName {
        match self {
            WorkspaceLocation::Local { .. } => IconName::Screen,
            WorkspaceLocation::Remote(remote) => remote.icon(),
        }
    }

    /// Tên của máy chủ remote nếu có (ví dụ: "Ubuntu", "Debian")
    pub fn server_name(&self) -> Option<&str> {
        self.as_remote().map(|r| r.display_name())
    }

    /// Cập nhật thư mục làm việc (áp dụng cho cả Local lẫn Remote)
    pub fn set_working_dir(&mut self, dir: impl Into<String>) {
        let dir = dir.into();
        match self {
            WorkspaceLocation::Local { working_dir } => *working_dir = dir,
            WorkspaceLocation::Remote(remote) => remote.set_working_dir(dir),
        }
    }

    /// Chuỗi tóm tắt vị trí thư mục làm việc
    pub fn summary(&self) -> String {
        match self {
            WorkspaceLocation::Local { working_dir } => {
                if !working_dir.is_empty() {
                    working_dir.clone()
                } else {
                    "Local Workspace".to_string()
                }
            }
            WorkspaceLocation::Remote(remote) => remote.summary(),
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
            ) => {
                let n1 = normalize_workdir(d1);
                let n2 = normalize_workdir(d2);
                !n1.is_empty() && n1 == n2
            }
            (WorkspaceLocation::Remote(r1), WorkspaceLocation::Remote(r2)) => r1.is_same(r2),
            _ => false,
        }
    }
}

impl std::fmt::Display for WorkspaceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Chuẩn hóa tên dự án: loại bỏ hậu tố server "(Ubuntu)" nếu có từ dữ liệu cũ,
/// fallback về tên thư mục hoặc "Workspace" nếu chuỗi rỗng.
pub fn sanitize_project_name(name: &str, location: &WorkspaceLocation) -> String {
    let mut clean = name.trim();
    if let Some(remote) = location.as_remote() {
        let suffix = format!(" ({})", remote.display_name());
        if let Some(stripped) = clean.strip_suffix(&suffix) {
            clean = stripped.trim();
        }
    }
    if clean.is_empty() {
        let dir = location.working_dir();
        if !dir.is_empty() {
            let extracted = extract_project_name(dir);
            if !extracted.is_empty() {
                return extracted;
            }
        }
        "Workspace".to_string()
    } else {
        clean.to_string()
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
        let clean_name = sanitize_project_name(&name.into(), &location);
        Self {
            id: Uuid::new_v4(),
            name: clean_name,
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
    pub fn icon(&self) -> IconName {
        match &self.location {
            WorkspaceLocation::Remote(remote) => remote.icon(),
            WorkspaceLocation::Local { .. } => match self.source_type {
                SourceType::File => IconName::File,
                SourceType::Process => IconName::Screen,
            },
        }
    }

    /// Tên của máy chủ remote nếu có (ví dụ: "Ubuntu", "Debian")
    pub fn server_name(&self) -> Option<&str> {
        self.location.server_name()
    }

    /// Đường dẫn tóm tắt mục tiêu (dùng cho tooltip hoặc subtitle)
    pub fn target_summary(&self) -> String {
        if !self.location.working_dir().is_empty() {
            self.location.summary()
        } else if !self.file_path.is_empty() {
            self.file_path.clone()
        } else if !self.command_str.is_empty() {
            self.command_str.clone()
        } else {
            self.location.summary()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceStore {
    pub active_workspace_id: Option<Uuid>,
    #[serde(default)]
    pub active_theme: Option<String>,
    pub recent_workspaces: Vec<Workspace>,
    #[serde(default)]
    pub wsl_connections: Vec<WslConnection>,
    #[serde(default)]
    pub ssh_connections: Vec<SshConnection>,
    #[serde(skip)]
    storage_path: Option<PathBuf>,
}

impl WorkspaceStore {
    /// Lấy đường dẫn lưu file cấu hình workspaces.json mặc định
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

    /// Đọc và tải danh sách workspace từ file cấu hình mặc định
    pub fn load() -> Self {
        Self::load_from_path(Self::get_storage_path())
    }

    /// Đọc và tải danh sách workspace từ một file cấu hình cụ thể
    pub fn load_from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut store = (|| {
            let content = fs::read_to_string(&path).ok()?;
            let mut loaded = serde_json::from_str::<WorkspaceStore>(&content).ok()?;
            for ws in &mut loaded.recent_workspaces {
                ws.name = sanitize_project_name(&ws.name, &ws.location);
            }
            #[cfg(not(target_os = "windows"))]
            {
                loaded.wsl_connections.clear();
            }
            loaded.sync_remote_projects();
            Some(loaded)
        })()
        .unwrap_or_default();

        store.storage_path = Some(path);
        store
    }

    /// Đường dẫn file lưu trữ liên kết với store (nếu có)
    pub fn storage_path(&self) -> Option<&Path> {
        self.storage_path.as_deref()
    }

    /// Thiết lập đường dẫn file lưu trữ
    pub fn set_storage_path(&mut self, path: impl Into<PathBuf>) {
        self.storage_path = Some(path.into());
    }

    /// Lưu danh sách workspace ra file (nếu store có liên kết với file lưu trữ)
    pub fn save(&self) -> Result<()> {
        let Some(ref path) = self.storage_path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory {:?}", parent))?;
            }
        }
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize WorkspaceStore")?;
        fs::write(path, json)
            .with_context(|| format!("Failed to write workspaces to {:?}", path))?;
        Ok(())
    }

    /// Thêm hoặc cập nhật một workspace, tự động sắp xếp theo thời gian mở gần nhất
    pub fn add_or_update(&mut self, mut ws: Workspace) {
        ws.name = sanitize_project_name(&ws.name, &ws.location);
        ws.last_opened = Utc::now();
        let ws_id = ws.id;

        // Xóa workspace cũ nếu trùng id hoặc cùng location (hoặc trùng tên nếu local không có working_dir)
        self.recent_workspaces.retain(|w| {
            if w.id == ws_id {
                return false;
            }
            if w.location.is_same(&ws.location) {
                return false;
            }
            if w.location.working_dir().is_empty()
                && ws.location.working_dir().is_empty()
                && w.name == ws.name
            {
                return false;
            }
            true
        });

        self.recent_workspaces.insert(0, ws);
        self.active_workspace_id = Some(ws_id);

        // Giữ tối đa 50 workspace gần nhất
        if self.recent_workspaces.len() > 50 {
            self.recent_workspaces.truncate(50);
        }

        let _ = self.save();
    }

    /// Xóa một workspace theo ID (nếu là remote workspace, tự động đồng bộ gỡ khỏi server connection)
    pub fn remove(&mut self, id: Uuid) {
        if let Some(pos) = self.recent_workspaces.iter().position(|w| w.id == id) {
            let ws = self.recent_workspaces.remove(pos);
            if let Some(remote) = ws.location.as_remote() {
                self.remove_remote_project_from_server(remote.display_name(), remote.working_dir());
            }
        }
        if self.active_workspace_id == Some(id) {
            self.active_workspace_id = self.recent_workspaces.first().map(|w| w.id);
        }
        let _ = self.save();
    }

    /// Tìm kết nối WSL theo tên distro
    pub fn find_wsl_connection(&self, distro: &str) -> Option<&WslConnection> {
        let trimmed = distro.trim();
        self.wsl_connections
            .iter()
            .find(|c| c.distro.eq_ignore_ascii_case(trimmed))
    }

    /// Tìm kết nối WSL dạng mutable theo tên distro
    pub fn find_wsl_connection_mut(&mut self, distro: &str) -> Option<&mut WslConnection> {
        let trimmed = distro.trim();
        self.wsl_connections
            .iter_mut()
            .find(|c| c.distro.eq_ignore_ascii_case(trimmed))
    }

    /// Đảm bảo một kết nối WSL distro tồn tại trong danh sách (tạo mới nếu chưa có)
    pub fn ensure_wsl_connection(&mut self, distro: impl Into<String>) -> &mut WslConnection {
        let d = distro.into();
        let trimmed = d.trim().to_string();
        if let Some(pos) = self
            .wsl_connections
            .iter()
            .position(|c| c.distro.eq_ignore_ascii_case(&trimmed))
        {
            &mut self.wsl_connections[pos]
        } else {
            self.wsl_connections.push(WslConnection::new(&trimmed));
            let _ = self.save();
            self.wsl_connections.last_mut().unwrap()
        }
    }

    /// Xóa một kết nối WSL distro khỏi danh sách và xóa các workspace remote thuộc distro đó
    pub fn remove_wsl_connection(&mut self, distro: &str) {
        let trimmed = distro.trim();
        self.wsl_connections
            .retain(|c| !c.distro.eq_ignore_ascii_case(trimmed));
        self.recent_workspaces.retain(|ws| {
            if let Some(remote) = ws.location.as_remote() {
                !remote.display_name().eq_ignore_ascii_case(trimmed)
            } else {
                true
            }
        });
        let _ = self.save();
    }

    /// Tìm kết nối SSH theo host hoặc nickname
    pub fn find_ssh_connection(&self, host: &str) -> Option<&SshConnection> {
        let trimmed = host.trim();
        self.ssh_connections.iter().find(|c| {
            c.host.eq_ignore_ascii_case(trimmed)
                || c.nickname
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        })
    }

    /// Tìm kết nối SSH dạng mutable theo host hoặc nickname
    pub fn find_ssh_connection_mut(&mut self, host: &str) -> Option<&mut SshConnection> {
        let trimmed = host.trim();
        self.ssh_connections.iter_mut().find(|c| {
            c.host.eq_ignore_ascii_case(trimmed)
                || c.nickname
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        })
    }

    /// Đảm bảo một kết nối SSH tồn tại trong danh sách (tạo mới nếu chưa có)
    pub fn ensure_ssh_connection(&mut self, host: impl Into<String>) -> &mut SshConnection {
        let h = host.into();
        let trimmed = h.trim().to_string();
        if let Some(pos) = self.ssh_connections.iter().position(|c| {
            c.host.eq_ignore_ascii_case(&trimmed)
                || c.nickname
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(&trimmed))
                    .unwrap_or(false)
        }) {
            &mut self.ssh_connections[pos]
        } else {
            self.ssh_connections.push(SshConnection::new(&trimmed));
            let _ = self.save();
            self.ssh_connections.last_mut().unwrap()
        }
    }

    /// Xóa một kết nối SSH khỏi danh sách và xóa các workspace remote thuộc server đó
    pub fn remove_ssh_connection(&mut self, host: &str) {
        let trimmed = host.trim();
        self.ssh_connections.retain(|c| {
            !c.host.eq_ignore_ascii_case(trimmed)
                && !c
                    .nickname
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        });
        self.recent_workspaces.retain(|ws| {
            if let Some(remote) = ws.location.as_remote() {
                !remote.display_name().eq_ignore_ascii_case(trimmed)
            } else {
                true
            }
        });
        let _ = self.save();
    }

    /// Thêm project trực tiếp vào SSH server tương ứng
    pub fn add_remote_project_to_ssh_server(&mut self, host: &str, path: impl Into<String>) {
        let server = self.ensure_ssh_connection(host);
        if server.add_project(path) {
            let _ = self.save();
        }
    }

    /// Xóa project trực tiếp khỏi SSH server tương ứng
    pub fn remove_remote_project_from_ssh_server(&mut self, host: &str, path: &str) {
        if let Some(server) = self.find_ssh_connection_mut(host) {
            if server.remove_project(path) {
                let clean = normalize_workdir(path);
                self.recent_workspaces.retain(|w| {
                    if let Some(remote) = w.location.as_remote() {
                        !(remote.display_name().eq_ignore_ascii_case(host)
                            && w.location.normalized_dir() == clean)
                    } else {
                        true
                    }
                });
                let _ = self.save();
            }
        }
    }

    /// Thêm project trực tiếp vào server connection tương ứng (WSL, SSH, Container...)
    pub fn add_remote_project_to_server(&mut self, server_name: &str, path: impl Into<String>) {
        let server = self.ensure_wsl_connection(server_name);
        if server.add_project(path) {
            let _ = self.save();
        }
    }

    /// Xóa project trực tiếp khỏi server connection tương ứng (WSL, SSH, Container...)
    pub fn remove_remote_project_from_server(&mut self, server_name: &str, path: &str) {
        let mut modified = false;
        if let Some(server) = self.find_wsl_connection_mut(server_name) {
            if server.remove_project(path) {
                modified = true;
            }
        }
        if let Some(server) = self.find_ssh_connection_mut(server_name) {
            if server.remove_project(path) {
                modified = true;
            }
        }
        if modified {
            let clean = normalize_workdir(path);
            self.recent_workspaces.retain(|w| {
                if let Some(remote) = w.location.as_remote() {
                    !(remote.display_name().eq_ignore_ascii_case(server_name)
                        && w.location.normalized_dir() == clean)
                } else {
                    true
                }
            });
            let _ = self.save();
        }
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

    /// Tìm workspace theo WorkspaceLocation (so sánh cả Remote/Local và đường dẫn)
    pub fn find_by_location(&self, location: &WorkspaceLocation) -> Option<&Workspace> {
        self.recent_workspaces
            .iter()
            .find(|w| w.location.is_same(location))
    }

    /// Lấy workspace đang được kích hoạt
    pub fn get_active(&self) -> Option<&Workspace> {
        if let Some(id) = self.active_workspace_id {
            self.recent_workspaces.iter().find(|w| w.id == id)
        } else {
            self.recent_workspaces.first()
        }
    }

    /// Đồng bộ các dự án remote giữa `wsl_connections`, `ssh_connections` và `recent_workspaces`
    pub fn sync_remote_projects(&mut self) {
        for conn in &self.wsl_connections {
            for proj in &conn.projects {
                let clean = normalize_workdir(&proj.path);
                let exists = self.recent_workspaces.iter().any(|ws| {
                    if let Some(remote) = ws.location.as_remote() {
                        remote.display_name().eq_ignore_ascii_case(&conn.distro)
                            && ws.location.normalized_dir() == clean
                    } else {
                        false
                    }
                });
                if !exists {
                    let ws = Workspace::new(
                        extract_project_name(&proj.path),
                        WorkspaceLocation::remote(WslConnectionOptions::new(
                            &conn.distro,
                            &proj.path,
                        )),
                        SourceType::Process,
                    );
                    self.recent_workspaces.push(ws);
                }
            }
        }

        for conn in &self.ssh_connections {
            // Tự động đồng bộ / cập nhật thông tin SSH (username, port, args) cho các workspace đã lưu trước đó
            for ws in &mut self.recent_workspaces {
                if let WorkspaceLocation::Remote(RemoteConnectionOptions::Ssh(ref mut opts)) =
                    ws.location
                {
                    if opts.host.eq_ignore_ascii_case(&conn.host)
                        || conn
                            .nickname
                            .as_deref()
                            .map(|n| n.eq_ignore_ascii_case(&opts.host))
                            .unwrap_or(false)
                    {
                        opts.host = conn.host.clone();
                        if let Some(ref u) = conn.username {
                            opts.username = Some(u.clone());
                        }
                        if let Some(p) = conn.port {
                            opts.port = Some(p);
                        }
                        if let Some(ref a) = conn.args {
                            opts.args = Some(a.clone());
                        }
                        if let Some(ref n) = conn.nickname {
                            opts.nickname = Some(n.clone());
                        }
                    }
                }
            }

            for proj in &conn.projects {
                let clean = normalize_workdir(&proj.path);
                let exists = self.recent_workspaces.iter().any(|ws| {
                    if let Some(remote) = ws.location.as_remote() {
                        remote
                            .display_name()
                            .eq_ignore_ascii_case(conn.display_name())
                            && ws.location.normalized_dir() == clean
                    } else {
                        false
                    }
                });
                if !exists {
                    let mut opts = SshConnectionOptions::new(&conn.host, &proj.path);
                    if let Some(ref u) = conn.username {
                        opts = opts.with_user(u);
                    }
                    if let Some(p) = conn.port {
                        opts = opts.with_port(p);
                    }
                    if let Some(ref nick) = conn.nickname {
                        opts = opts.with_nickname(nick);
                    }
                    if let Some(ref args) = conn.args {
                        opts.args = Some(args.clone());
                    }
                    let ws = Workspace::new(
                        extract_project_name(&proj.path),
                        WorkspaceLocation::remote(opts),
                        SourceType::Process,
                    );
                    self.recent_workspaces.push(ws);
                }
            }
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
            WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/user/backend")),
            SourceType::Process,
        );
        let id1 = ws1.id;

        store.add_or_update(ws1);
        assert_eq!(store.recent_workspaces.len(), 1);
        assert_eq!(store.active_workspace_id, Some(id1));
        assert_eq!(store.get_active().unwrap().name, "backend-service");

        let ws2 = Workspace::new(
            "frontend-app",
            WorkspaceLocation::local("D:\\Projects\\frontend"),
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
            WorkspaceLocation::local("D:\\Learn\\Go\\uwulog-rust"),
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
            WorkspaceLocation::local("D:\\test\\service2"),
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
            WorkspaceLocation::local("C:\\Projects\\app"),
            SourceType::Process,
        );

        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"source_type\":\"process\""));

        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source_type, SourceType::Process);
        assert_eq!(deserialized.name, "test-app");

        let wsl_ws = Workspace::new(
            "wsl-app",
            WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/user")),
            SourceType::Process,
        );
        let wsl_json = serde_json::to_string(&wsl_ws).unwrap();
        let deserialized_wsl: Workspace = serde_json::from_str(&wsl_json).unwrap();
        assert_eq!(deserialized_wsl.source_type, SourceType::Process);
        assert!(deserialized_wsl.location.is_remote());
        assert_eq!(deserialized_wsl.location.display_name(), "Ubuntu");
        assert_eq!(deserialized_wsl.location.icon(), IconName::Linux);
        assert_eq!(deserialized_wsl.location.working_dir(), "/home/user");
    }

    #[tokio::test]
    async fn test_workspace_domain_methods() {
        let mut local_ws = Workspace::new(
            "my-app",
            WorkspaceLocation::local("C:\\Projects\\app"),
            SourceType::Process,
        );
        assert_eq!(local_ws.icon(), IconName::Screen);
        assert_eq!(local_ws.server_name(), None);
        assert_eq!(local_ws.target_summary(), "C:\\Projects\\app");
        assert!(!local_ws.location.is_remote());
        assert_eq!(format!("{}", local_ws.location), "C:\\Projects\\app");

        local_ws.location.set_working_dir("C:\\Projects\\app2");
        assert_eq!(local_ws.location.working_dir(), "C:\\Projects\\app2");

        let mut wsl_ws = Workspace::new(
            "ubuntu-service",
            WorkspaceLocation::remote(WslConnectionOptions::new(
                "Ubuntu-22.04",
                "/home/user/service",
            )),
            SourceType::Process,
        );
        assert_eq!(wsl_ws.icon(), IconName::Linux);
        assert_eq!(wsl_ws.server_name(), Some("Ubuntu-22.04"));
        assert_eq!(wsl_ws.target_summary(), "/home/user/service (Ubuntu-22.04)");
        assert!(wsl_ws.location.is_remote());
        assert_eq!(
            format!("{}", wsl_ws.location),
            "/home/user/service (Ubuntu-22.04)"
        );

        wsl_ws.location.set_working_dir("/home/user/service2");
        assert_eq!(wsl_ws.location.working_dir(), "/home/user/service2");

        let file_ws = Workspace::new(
            "syslog",
            WorkspaceLocation::local("C:\\Logs"),
            SourceType::File,
        );
        assert_eq!(file_ws.icon(), IconName::File);

        let session = WorkspaceSession::from_workspace(&wsl_ws, 100, 50);
        assert_eq!(session.icon(), IconName::Linux);
        assert_eq!(
            session.location.as_remote().map(|r| r.display_name()),
            Some("Ubuntu-22.04")
        );
        assert_eq!(
            session.target_summary(),
            "/home/user/service2 (Ubuntu-22.04)"
        );
    }

    #[test]
    fn test_workspace_store_wsl_connections_crud() {
        let mut store = WorkspaceStore::default();
        store.ensure_wsl_connection("Ubuntu");
        store.ensure_wsl_connection("Debian");
        assert_eq!(store.wsl_connections.len(), 2);
        assert_eq!(store.wsl_connections[0].distro, "Ubuntu");
        assert_eq!(store.wsl_connections[1].distro, "Debian");

        // Thêm project vào Ubuntu
        store.add_remote_project_to_server("Ubuntu", "/home/user/backend");
        store.add_remote_project_to_server("Ubuntu", "/home/user/frontend");
        // Kiểm tra BTreeSet deduplication
        store.add_remote_project_to_server("Ubuntu", "/home/user/backend");

        let ubuntu = store.find_wsl_connection("Ubuntu").unwrap();
        assert_eq!(ubuntu.projects.len(), 2);
        assert!(ubuntu
            .projects
            .contains(&RemoteProject::new("/home/user/backend")));
        assert!(ubuntu
            .projects
            .contains(&RemoteProject::new("/home/user/frontend")));

        let ws_ubuntu = Workspace::new(
            "ubuntu-app",
            WorkspaceLocation::remote(WslConnectionOptions::new("Ubuntu", "/home/user/app")),
            SourceType::Process,
        );
        let ws_debian = Workspace::new(
            "debian-app",
            WorkspaceLocation::remote(WslConnectionOptions::new("Debian", "/home/user/app")),
            SourceType::Process,
        );
        let ws_local = Workspace::new(
            "local-app",
            WorkspaceLocation::local("C:\\Projects\\local"),
            SourceType::Process,
        );
        store.add_or_update(ws_ubuntu);
        store.add_or_update(ws_debian);
        store.add_or_update(ws_local);
        assert_eq!(store.recent_workspaces.len(), 3);

        // Xóa project khỏi Ubuntu
        store.remove_remote_project_from_server("Ubuntu", "/home/user/frontend");
        assert_eq!(
            store.find_wsl_connection("Ubuntu").unwrap().projects.len(),
            1
        );

        // Xóa server Ubuntu -> Xóa cả server lẫn workspaces remote thuộc về nó
        store.remove_wsl_connection("Ubuntu");
        assert_eq!(store.wsl_connections.len(), 1);
        assert_eq!(store.wsl_connections[0].distro, "Debian");
        assert_eq!(store.recent_workspaces.len(), 2);
        assert!(store
            .recent_workspaces
            .iter()
            .all(|w| { w.location.as_remote().map(|r| r.display_name()) != Some("Ubuntu") }));
    }

    #[test]
    fn test_workspace_project_name_deduplication() {
        // Test 1: ws.name already has (Ubuntu) suffix -> sanitized in Workspace::new
        let ws = Workspace::new(
            "ai-learn-english (Ubuntu)",
            WorkspaceLocation::remote(WslConnectionOptions::new(
                "Ubuntu",
                "/home/truongviet/project/ai-learn-english",
            )),
            SourceType::Process,
        );

        assert_eq!(ws.name, "ai-learn-english");
        assert_eq!(ws.server_name(), Some("Ubuntu"));

        // Test 2: ws.name does not have suffix
        let ws2 = Workspace::new(
            "ai-learn-english",
            WorkspaceLocation::remote(WslConnectionOptions::new(
                "Ubuntu",
                "/home/truongviet/project/ai-learn-english",
            )),
            SourceType::Process,
        );

        assert_eq!(ws2.name, "ai-learn-english");
        assert_eq!(ws2.server_name(), Some("Ubuntu"));

        // Test 3: Local workspace
        let ws_local = Workspace::new(
            "my-app",
            WorkspaceLocation::local("D:\\projects\\my-app"),
            SourceType::Process,
        );
        assert_eq!(ws_local.name, "my-app");
        assert_eq!(ws_local.server_name(), None);
    }

    #[test]
    fn test_workspace_store_ssh_connections_crud() {
        let mut store = WorkspaceStore::default();
        store.ensure_ssh_connection("prod-server");
        store.ensure_ssh_connection("staging-server");
        assert_eq!(store.ssh_connections.len(), 2);
        assert_eq!(store.ssh_connections[0].host, "prod-server");
        assert_eq!(store.ssh_connections[1].host, "staging-server");

        // Thêm project vào prod-server
        store.add_remote_project_to_ssh_server("prod-server", "/var/log/nginx");
        store.add_remote_project_to_ssh_server("prod-server", "/var/log/backend");
        // Kiểm tra deduplication
        store.add_remote_project_to_ssh_server("prod-server", "/var/log/nginx");

        let prod = store.find_ssh_connection("prod-server").unwrap();
        assert_eq!(prod.projects.len(), 2);
        assert!(prod
            .projects
            .contains(&RemoteProject::new("/var/log/nginx")));
        assert!(prod
            .projects
            .contains(&RemoteProject::new("/var/log/backend")));

        let ws_ssh = Workspace::new(
            "nginx-log",
            WorkspaceLocation::remote(SshConnectionOptions::new("prod-server", "/var/log/nginx")),
            SourceType::Process,
        );
        store.add_or_update(ws_ssh);
        assert_eq!(store.recent_workspaces.len(), 1);

        // Xóa project khỏi prod-server
        store.remove_remote_project_from_ssh_server("prod-server", "/var/log/nginx");
        assert_eq!(
            store
                .find_ssh_connection("prod-server")
                .unwrap()
                .projects
                .len(),
            1
        );
        assert_eq!(store.recent_workspaces.len(), 0);

        // Xóa SSH server
        store.remove_ssh_connection("prod-server");
        assert_eq!(store.ssh_connections.len(), 1);
        assert_eq!(store.ssh_connections[0].host, "staging-server");
    }
}
