use serde::{Deserialize, Serialize};

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
