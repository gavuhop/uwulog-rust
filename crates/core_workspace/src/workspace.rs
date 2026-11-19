use crate::remote::RemoteConnectionOptions;
use crate::session::SourceType;
use crate::store::WorkspaceStore;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;
use uwu_icons::IconName;

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

    /// Nhãn ngắn gọn của máy chủ remote hiển thị kèm tên project (ẩn với Docker vì đã có icon Box)
    pub fn short_server_name(&self) -> Option<&str> {
        match self.as_remote()? {
            RemoteConnectionOptions::Docker(_) => None,
            remote => Some(remote.short_label()),
        }
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

    /// Kiểm tra hai `WorkspaceLocation` Docker có thuộc cùng một dự án/compose (Dev Container) hay không.
    pub fn is_same_docker_project(&self, other: &Self) -> bool {
        let (
            WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(da)),
            WorkspaceLocation::Remote(RemoteConnectionOptions::Docker(db)),
        ) = (self, other)
        else {
            return false;
        };

        let compose_a = da.compose_project.as_deref().filter(|s| !s.is_empty());
        let compose_b = db.compose_project.as_deref().filter(|s| !s.is_empty());
        if let (Some(p1), Some(p2)) = (compose_a, compose_b) {
            return p1 == p2;
        }

        let same_folder = da
            .local_folder
            .as_deref()
            .zip(db.local_folder.as_deref())
            .is_some_and(|(f1, f2)| !f1.is_empty() && f1.eq_ignore_ascii_case(f2));
        if same_folder {
            return true;
        }

        if compose_a.is_some() || compose_b.is_some() {
            let dir_a = normalize_workdir(&da.working_dir);
            let dir_b = normalize_workdir(&db.working_dir);
            return !dir_a.is_empty() && dir_a == dir_b;
        }

        da.container_name.eq_ignore_ascii_case(&db.container_name)
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

fn default_inspector_width_ratio() -> f32 {
    0.35
}

/// Cấu hình hiển thị thống nhất lưu theo `ViewerKey` cho mọi nguồn log (project thường & container).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewerPrefs {
    #[serde(default)]
    pub main_query: String,
    #[serde(default)]
    pub filter_history: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<SavedColumnConfig>,
    #[serde(default = "default_inspector_width_ratio")]
    pub inspector_width_ratio: f32,
}

impl Default for ViewerPrefs {
    fn default() -> Self {
        Self {
            main_query: String::new(),
            filter_history: Vec::new(),
            columns: Vec::new(),
            inspector_width_ratio: default_inspector_width_ratio(),
        }
    }
}

/// Cấu hình bố cục cột đã lưu cho từng viewer trong `workspaces.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedColumnConfig {
    pub name: String,
    pub visible: bool,
    pub width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub location: WorkspaceLocation,
    pub source_type: SourceType,
    pub command_str: String,
    pub file_path: String,
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
            last_opened: Utc::now(),
            env_vars: HashMap::new(),
        }
    }

    /// Khóa định danh viewer chuẩn hóa của workspace này.
    pub fn viewer_key(&self) -> String {
        WorkspaceStore::viewer_key(&self.location, self.source_type, &self.file_path)
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

    /// Nhãn ngắn của server remote cho UI (vd: `app` thay vì tên container đầy đủ)
    pub fn short_server_name(&self) -> Option<&str> {
        self.location.short_server_name()
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

    /// Kiểm tra xem workspace này có hợp lệ để mở lại hay không
    pub fn is_valid_to_open(&self) -> bool {
        match &self.location {
            WorkspaceLocation::Local { working_dir } => {
                if !working_dir.trim().is_empty() {
                    Path::new(working_dir).exists()
                } else if !self.file_path.trim().is_empty() {
                    Path::new(&self.file_path).exists()
                } else {
                    false
                }
            }
            WorkspaceLocation::Remote(remote) => match remote {
                #[cfg(not(target_os = "windows"))]
                RemoteConnectionOptions::Wsl(_) => false,
                #[cfg(target_os = "windows")]
                RemoteConnectionOptions::Wsl(wsl) => !wsl.distro.trim().is_empty(),
                RemoteConnectionOptions::Ssh(ssh) => !ssh.host.trim().is_empty(),
                RemoteConnectionOptions::Docker(docker) => {
                    !docker.container_name.trim().is_empty()
                        || docker
                            .container_id
                            .as_deref()
                            .is_some_and(|id| !id.trim().is_empty())
                }
            },
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
