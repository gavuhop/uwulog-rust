use crate::remote::{
    DockerConnection, RemoteConnectionOptions, SshConnection, SshConnectionOptions, WslConnection,
    WslConnectionOptions,
};
use crate::session::SourceType;
use crate::workspace::{
    extract_project_name, normalize_workdir, sanitize_project_name, ViewerPrefs, Workspace,
    WorkspaceLocation,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

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
    #[serde(default)]
    pub docker_connections: Vec<DockerConnection>,
    #[serde(default)]
    pub open_workspace_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub viewers: HashMap<String, ViewerPrefs>,
    #[serde(skip)]
    storage_path: Option<PathBuf>,
}

impl WorkspaceStore {
    /// Tạo khóa định danh chuẩn hóa (`ViewerKey`) cho mọi nguồn log:
    /// - Local:  `local:<dir>` hoặc `local:<dir>#file:<file>`
    /// - SSH:    `ssh:<user@><host>:<port><dir>[#file:<file>]`
    /// - WSL:    `wsl:<distro>:<dir>[#file:<file>]`
    /// - Docker: `docker:<host_folder_or_compose_or_workdir>/<service_or_container>[#file:<file>]`
    pub fn viewer_key(
        location: &WorkspaceLocation,
        source_type: SourceType,
        file_path: &str,
    ) -> String {
        let base = match location {
            WorkspaceLocation::Local { working_dir } => {
                format!("local:{}", normalize_workdir(working_dir))
            }
            WorkspaceLocation::Remote(RemoteConnectionOptions::Ssh(ssh)) => {
                let host = ssh.host.trim().to_ascii_lowercase();
                let port = ssh.port.unwrap_or(22);
                let dir = normalize_workdir(&ssh.working_dir);
                match ssh
                    .username
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    Some(user) => format!("ssh:{user}@{host}:{port}{dir}"),
                    None => format!("ssh:{host}:{port}{dir}"),
                }
            }
            WorkspaceLocation::Remote(RemoteConnectionOptions::Wsl(wsl)) => {
                let distro = wsl.distro.trim().to_ascii_lowercase();
                let dir = normalize_workdir(&wsl.working_dir);
                format!("wsl:{distro}:{dir}")
            }
            WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(d)) => {
                let root = d
                    .local_folder
                    .as_deref()
                    .map(normalize_workdir)
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        d.compose_project
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_ascii_lowercase)
                    })
                    .unwrap_or_else(|| normalize_workdir(&d.working_dir));
                let target = d
                    .compose_service
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_ascii_lowercase)
                    .unwrap_or_else(|| d.container_name.trim().to_ascii_lowercase());
                format!("docker:{root}/{target}")
            }
        };

        if source_type == SourceType::File && !file_path.trim().is_empty() {
            format!("{base}#file:{}", normalize_workdir(file_path))
        } else {
            base
        }
    }

    pub fn upsert_viewer_prefs(&mut self, key: impl Into<String>, prefs: ViewerPrefs) {
        let key = key.into();
        if !key.is_empty() {
            self.viewers.insert(key, prefs);
        }
    }

    pub fn get_viewer_prefs(&self, key: &str) -> Option<&ViewerPrefs> {
        self.viewers.get(key)
    }

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
        self.open_workspace_ids.retain(|&oid| oid != id);
        if self.active_workspace_id == Some(id) {
            self.active_workspace_id = self
                .open_workspace_ids
                .first()
                .copied()
                .or_else(|| self.recent_workspaces.first().map(|w| w.id));
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
            let idx = self.wsl_connections.len() - 1;
            &mut self.wsl_connections[idx]
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
            let idx = self.ssh_connections.len() - 1;
            &mut self.ssh_connections[idx]
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

    /// Tìm kết nối Docker / Dev Container theo tên container hoặc ID
    pub fn find_docker_connection(&self, container: &str) -> Option<&DockerConnection> {
        let trimmed = container.trim();
        self.docker_connections.iter().find(|c| {
            c.container_name.eq_ignore_ascii_case(trimmed)
                || c.container_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        })
    }

    /// Tìm kết nối Docker / Dev Container dạng mutable theo tên container hoặc ID
    pub fn find_docker_connection_mut(&mut self, container: &str) -> Option<&mut DockerConnection> {
        let trimmed = container.trim();
        self.docker_connections.iter_mut().find(|c| {
            c.container_name.eq_ignore_ascii_case(trimmed)
                || c.container_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        })
    }

    /// Đảm bảo một kết nối Docker / Dev Container tồn tại trong danh sách (tạo mới nếu chưa có)
    pub fn ensure_docker_connection(
        &mut self,
        container: impl Into<String>,
    ) -> &mut DockerConnection {
        let name = container.into();
        let trimmed = name.trim().to_string();
        if let Some(pos) = self.docker_connections.iter().position(|c| {
            c.container_name.eq_ignore_ascii_case(&trimmed)
                || c.container_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(&trimmed))
                    .unwrap_or(false)
        }) {
            &mut self.docker_connections[pos]
        } else {
            self.docker_connections
                .push(DockerConnection::new(&trimmed));
            let _ = self.save();
            let idx = self.docker_connections.len() - 1;
            &mut self.docker_connections[idx]
        }
    }

    /// Thêm mới hoặc cập nhật metadata cho một kết nối Docker / Dev Container
    pub fn upsert_docker_connection(&mut self, conn: DockerConnection) -> &mut DockerConnection {
        let pos = self.docker_connections.iter().position(|c| {
            c.container_name.eq_ignore_ascii_case(&conn.container_name)
                || conn
                    .container_id
                    .as_deref()
                    .zip(c.container_id.as_deref())
                    .map(|(a, b)| a.eq_ignore_ascii_case(b))
                    .unwrap_or(false)
        });

        let idx = if let Some(pos) = pos {
            let existing = &mut self.docker_connections[pos];
            existing.container_name = conn.container_name;
            if conn.container_id.is_some() {
                existing.container_id = conn.container_id;
            }
            if conn.image.is_some() {
                existing.image = conn.image;
            }
            if conn.remote_user.is_some() {
                existing.remote_user = conn.remote_user;
            }
            if conn.local_folder.is_some() {
                existing.local_folder = conn.local_folder;
            }
            if conn.compose_project.is_some() {
                existing.compose_project = conn.compose_project;
            }
            if conn.compose_service.is_some() {
                existing.compose_service = conn.compose_service;
            }
            existing.is_devcontainer = existing.is_devcontainer || conn.is_devcontainer;
            for proj in conn.projects {
                existing.projects.insert(proj);
            }
            pos
        } else {
            self.docker_connections.push(conn);
            self.docker_connections.len() - 1
        };

        let _ = self.save();
        &mut self.docker_connections[idx]
    }

    /// Xóa một kết nối Docker / Dev Container khỏi danh sách và xóa các workspace thuộc container đó
    pub fn remove_docker_connection(&mut self, container: &str) {
        let trimmed = container.trim();
        self.docker_connections.retain(|c| {
            !c.container_name.eq_ignore_ascii_case(trimmed)
                && !c
                    .container_id
                    .as_deref()
                    .map(|id| id.eq_ignore_ascii_case(trimmed))
                    .unwrap_or(false)
        });
        self.recent_workspaces.retain(|ws| {
            if let Some(WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(ref d))) =
                Some(&ws.location)
            {
                !d.container_name.eq_ignore_ascii_case(trimmed)
                    && !d
                        .container_id
                        .as_deref()
                        .map(|id| id.eq_ignore_ascii_case(trimmed))
                        .unwrap_or(false)
            } else {
                true
            }
        });
        let _ = self.save();
    }

    /// Thêm project trực tiếp vào Docker container tương ứng
    pub fn add_remote_project_to_docker_server(
        &mut self,
        container: &str,
        path: impl Into<String>,
    ) {
        let server = self.ensure_docker_connection(container);
        if server.add_project(path) {
            let _ = self.save();
        }
    }

    /// Xóa project trực tiếp khỏi Docker container tương ứng
    pub fn remove_remote_project_from_docker_server(&mut self, container: &str, path: &str) {
        if let Some(server) = self.find_docker_connection_mut(container) {
            if server.remove_project(path) {
                let clean = normalize_workdir(path);
                self.recent_workspaces.retain(|w| {
                    if let Some(remote) = w.location.as_remote() {
                        !(remote.display_name().eq_ignore_ascii_case(container)
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
        if let Some(server) = self.find_docker_connection_mut(server_name) {
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

        for conn in &self.docker_connections {
            for ws in &mut self.recent_workspaces {
                if let WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(ref mut opts)) =
                    ws.location
                {
                    if opts
                        .container_name
                        .eq_ignore_ascii_case(&conn.container_name)
                    {
                        if let Some(ref id) = conn.container_id {
                            opts.container_id = Some(id.clone());
                        }
                        if let Some(ref user) = conn.remote_user {
                            opts.remote_user = Some(user.clone());
                        }
                        if let Some(ref folder) = conn.local_folder {
                            opts.local_folder = Some(folder.clone());
                        }
                        if let Some(ref proj) = conn.compose_project {
                            opts.compose_project = Some(proj.clone());
                        }
                        if let Some(ref svc) = conn.compose_service {
                            opts.compose_service = Some(svc.clone());
                        }
                        opts.is_devcontainer = opts.is_devcontainer || conn.is_devcontainer;
                    }
                }
            }

            for proj in &conn.projects {
                let clean = normalize_workdir(&proj.path);
                let exists = self.recent_workspaces.iter().any(|ws| {
                    if let Some(remote) = ws.location.as_remote() {
                        remote
                            .display_name()
                            .eq_ignore_ascii_case(&conn.container_name)
                            && ws.location.normalized_dir() == clean
                    } else {
                        false
                    }
                });
                if !exists {
                    let ws = Workspace::new(
                        extract_project_name(&proj.path),
                        WorkspaceLocation::remote(conn.to_options(&proj.path)),
                        SourceType::Process,
                    );
                    self.recent_workspaces.push(ws);
                }
            }
        }
    }
}
