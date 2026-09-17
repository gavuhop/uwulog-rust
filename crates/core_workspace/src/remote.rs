use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Cấu hình kết nối Remote (WSL, tương lai: SSH, Dev Container / Docker...)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteConnectionOptions {
    Wsl(WslConnectionOptions),
    // Ssh(SshConnectionOptions),       // Mở rộng sau
    // Docker(DockerConnectionOptions), // Mở rộng sau
}

impl RemoteConnectionOptions {
    /// Khởi tạo cấu hình Remote từ chuỗi target (ví dụ: "wsl:Ubuntu", "Ubuntu") cùng thư mục làm việc
    pub fn parse(target: &str, working_dir: impl Into<String>) -> Self {
        let distro = target.strip_prefix("wsl:").unwrap_or(target);
        Self::Wsl(WslConnectionOptions::new(distro, working_dir))
    }

    /// Tên hiển thị định danh cho remote (ví dụ tên Distro đối với WSL, hoặc Host đối với SSH)
    pub fn display_name(&self) -> &str {
        match self {
            RemoteConnectionOptions::Wsl(opts) => &opts.distro,
        }
    }

    /// Định danh loại kết nối remote ("wsl", "ssh", "docker")
    pub fn connection_type(&self) -> &'static str {
        match self {
            RemoteConnectionOptions::Wsl(_) => "wsl",
        }
    }

    /// Thư mục làm việc trên môi trường remote
    pub fn working_dir(&self) -> &str {
        match self {
            RemoteConnectionOptions::Wsl(opts) => &opts.working_dir,
        }
    }

    /// Biểu tượng đại diện cho môi trường remote
    pub fn icon(&self) -> uwu_icons::IconName {
        match self {
            RemoteConnectionOptions::Wsl(_) => uwu_icons::IconName::Linux,
        }
    }

    /// Cập nhật thư mục làm việc trên remote
    pub fn set_working_dir(&mut self, dir: impl Into<String>) {
        let dir = dir.into();
        match self {
            RemoteConnectionOptions::Wsl(opts) => opts.working_dir = dir,
        }
    }

    /// Chuỗi tóm tắt vị trí remote (ví dụ: "/home/user (Ubuntu)")
    pub fn summary(&self) -> String {
        let dir = self.working_dir();
        if !dir.is_empty() {
            format!("{} ({})", dir, self.display_name())
        } else {
            format!(
                "{} ({})",
                self.connection_type().to_uppercase(),
                self.display_name()
            )
        }
    }

    /// So sánh xem 2 cấu hình remote có cùng trỏ tới một môi trường/thư mục hay không
    pub fn is_same(&self, other: &Self) -> bool {
        match (self, other) {
            (RemoteConnectionOptions::Wsl(w1), RemoteConnectionOptions::Wsl(w2)) => {
                w1.distro.eq_ignore_ascii_case(&w2.distro)
                    && crate::normalize_workdir(&w1.working_dir)
                        == crate::normalize_workdir(&w2.working_dir)
            }
        }
    }
}

impl std::fmt::Display for RemoteConnectionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Tùy chọn kết nối WSL (Windows Subsystem for Linux)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WslConnectionOptions {
    pub distro: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    pub working_dir: String,
}

impl WslConnectionOptions {
    pub fn new(distro: impl Into<String>, working_dir: impl Into<String>) -> Self {
        Self {
            distro: distro.into(),
            user: None,
            working_dir: working_dir.into(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }
}

impl std::fmt::Display for WslConnectionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.working_dir.is_empty() {
            write!(f, "{} ({})", self.working_dir, self.distro)
        } else {
            write!(f, "WSL ({})", self.distro)
        }
    }
}

impl From<WslConnectionOptions> for RemoteConnectionOptions {
    fn from(opts: WslConnectionOptions) -> Self {
        Self::Wsl(opts)
    }
}

/// Một dự án từ xa (tương đương `RemoteProject` trong Zed settings)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RemoteProject {
    pub path: String,
}

impl RemoteProject {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl std::fmt::Display for RemoteProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

/// Cấu hình kết nối WSL chứa trực tiếp danh sách project của distro đó (chuẩn `WslConnection` của Zed)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WslConnection {
    pub distro: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default)]
    pub projects: BTreeSet<RemoteProject>,
}

impl WslConnection {
    pub fn new(distro: impl Into<String>) -> Self {
        Self {
            distro: distro.into(),
            user: None,
            projects: BTreeSet::new(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Thêm project vào server, trả về true nếu project mới được thêm vào tập hợp
    pub fn add_project(&mut self, path: impl Into<String>) -> bool {
        self.projects.insert(RemoteProject::new(path))
    }

    /// Xóa project khỏi server, trả về true nếu project tồn tại và bị xóa
    pub fn remove_project(&mut self, path: &str) -> bool {
        self.projects.remove(&RemoteProject::new(path))
    }
}

/// Cấu hình kết nối SSH chứa danh sách project (chuẩn `SshConnection` của Zed)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshConnection {
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(default)]
    pub projects: BTreeSet<RemoteProject>,
}

impl SshConnection {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: None,
            username: None,
            nickname: None,
            projects: BTreeSet::new(),
        }
    }
}

/// Enum đa hình đại diện cho một kết nối Server (tương đương `Connection` trong Zed)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerConnection {
    Wsl(WslConnection),
    Ssh(SshConnection),
}

impl From<WslConnection> for ServerConnection {
    fn from(conn: WslConnection) -> Self {
        Self::Wsl(conn)
    }
}

impl From<SshConnection> for ServerConnection {
    fn from(conn: SshConnection) -> Self {
        Self::Ssh(conn)
    }
}

// ----------------------------------------------------------------------------
// Placeholder structs cho các loại remote kết nối trong tương lai:
// ----------------------------------------------------------------------------
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
// pub struct SshConnectionOptions {
//     pub host: String,
//     pub port: Option<u16>,
//     pub username: Option<String>,
//     pub working_dir: String,
// }
//
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
// pub struct DockerConnectionOptions {
//     pub container_id: String,
//     pub use_podman: bool,
//     pub working_dir: String,
// }
