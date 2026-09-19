use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Cấu hình kết nối Remote (WSL, SSH, tương lai: Dev Container / Docker...)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteConnectionOptions {
    Wsl(WslConnectionOptions),
    Ssh(SshConnectionOptions),
    // Docker(DockerConnectionOptions), // Mở rộng sau
}

impl RemoteConnectionOptions {
    /// Khởi tạo cấu hình Remote từ chuỗi target (ví dụ: "wsl:Ubuntu", "Ubuntu", "ssh:user@host:22") cùng thư mục làm việc
    pub fn parse(target: &str, working_dir: impl Into<String>) -> Self {
        let working_dir = working_dir.into();
        if let Some(stripped) = target.strip_prefix("ssh:") {
            if let Ok(opts) = SshConnectionOptions::parse_command_line(stripped, &working_dir) {
                return Self::Ssh(opts);
            }
            return Self::Ssh(SshConnectionOptions::new(stripped, working_dir));
        }
        let distro = target.strip_prefix("wsl:").unwrap_or(target);
        Self::Wsl(WslConnectionOptions::new(distro, working_dir))
    }

    /// Tên hiển thị định danh cho remote (ví dụ tên Distro đối với WSL, hoặc Host đối với SSH)
    pub fn display_name(&self) -> &str {
        match self {
            RemoteConnectionOptions::Wsl(opts) => &opts.distro,
            RemoteConnectionOptions::Ssh(opts) => opts.display_name(),
        }
    }

    /// Định danh loại kết nối remote ("wsl", "ssh", "docker")
    pub fn connection_type(&self) -> &'static str {
        match self {
            RemoteConnectionOptions::Wsl(_) => "wsl",
            RemoteConnectionOptions::Ssh(_) => "ssh",
        }
    }

    /// Thư mục làm việc trên môi trường remote
    pub fn working_dir(&self) -> &str {
        match self {
            RemoteConnectionOptions::Wsl(opts) => &opts.working_dir,
            RemoteConnectionOptions::Ssh(opts) => &opts.working_dir,
        }
    }

    /// Biểu tượng đại diện cho môi trường remote
    pub fn icon(&self) -> uwu_icons::IconName {
        match self {
            RemoteConnectionOptions::Wsl(_) => uwu_icons::IconName::Linux,
            RemoteConnectionOptions::Ssh(_) => uwu_icons::IconName::Server,
        }
    }

    /// Cập nhật thư mục làm việc trên remote
    pub fn set_working_dir(&mut self, dir: impl Into<String>) {
        let dir = dir.into();
        match self {
            RemoteConnectionOptions::Wsl(opts) => opts.working_dir = dir,
            RemoteConnectionOptions::Ssh(opts) => opts.working_dir = dir,
        }
    }

    /// Chuỗi tóm tắt vị trí remote (ví dụ: "/home/user (Ubuntu)" hoặc "/var/log (SSH: prod-server)")
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
            (RemoteConnectionOptions::Ssh(s1), RemoteConnectionOptions::Ssh(s2)) => {
                s1.host.eq_ignore_ascii_case(&s2.host)
                    && s1.username == s2.username
                    && s1.port == s2.port
                    && crate::normalize_workdir(&s1.working_dir)
                        == crate::normalize_workdir(&s2.working_dir)
            }
            _ => false,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
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
            args: None,
            projects: BTreeSet::new(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.username = Some(user.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_nickname(mut self, nickname: impl Into<String>) -> Self {
        self.nickname = Some(nickname.into());
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = if args.is_empty() { None } else { Some(args) };
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

    /// Tên hiển thị ưu tiên: nickname (nếu có) hoặc host
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.host)
    }

    /// Định dạng user@host:port hoặc user@host hoặc host
    pub fn target_string(&self) -> String {
        let base = if let Some(ref u) = self.username {
            format!("{}@{}", u, self.host)
        } else {
            self.host.clone()
        };
        if let Some(port) = self.port {
            format!("{}:{}", base, port)
        } else {
            base
        }
    }

    /// Chuyển đổi thành SshConnectionOptions kèm thư mục làm việc
    pub fn to_options(&self, working_dir: impl Into<String>) -> SshConnectionOptions {
        let mut opts = SshConnectionOptions::new(&self.host, working_dir);
        opts.port = self.port;
        opts.username = self.username.clone();
        opts.nickname = self.nickname.clone();
        opts.args = self.args.clone();
        opts
    }
}

impl From<&SshConnectionOptions> for SshConnection {
    fn from(opts: &SshConnectionOptions) -> Self {
        Self {
            host: opts.host.clone(),
            port: opts.port,
            username: opts.username.clone(),
            nickname: opts.nickname.clone(),
            args: opts.args.clone(),
            projects: BTreeSet::new(),
        }
    }
}

impl From<SshConnectionOptions> for SshConnection {
    fn from(opts: SshConnectionOptions) -> Self {
        Self::from(&opts)
    }
}

/// Tùy chọn kết nối SSH (Secure Shell) kèm thư mục làm việc (chuẩn Zed Editor)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshConnectionOptions {
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub working_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

impl SshConnectionOptions {
    pub fn new(host: impl Into<String>, working_dir: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            username: None,
            port: None,
            working_dir: working_dir.into(),
            nickname: None,
            args: None,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.username = Some(user.into());
        self
    }

    pub fn with_username(self, user: impl Into<String>) -> Self {
        self.with_user(user)
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_nickname(mut self, nickname: impl Into<String>) -> Self {
        self.nickname = Some(nickname.into());
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = Some(args);
        self
    }

    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.host)
    }

    pub fn target_string(&self) -> String {
        let base = if let Some(ref u) = self.username {
            format!("{}@{}", u, self.host)
        } else {
            self.host.clone()
        };
        if let Some(port) = self.port {
            format!("{}:{}", base, port)
        } else {
            base
        }
    }

    /// Khởi tạo SshTransport đã cấu hình đầy đủ từ options này
    pub fn to_transport(&self) -> uwu_driver_transport::SshTransport {
        let mut transport = uwu_driver_transport::SshTransport::new(&self.host);
        if let Some(ref u) = self.username {
            transport = transport.with_user(u);
        }
        if let Some(p) = self.port {
            transport = transport.with_port(p);
        }
        if let Some(ref args) = self.args {
            transport = transport.with_args(args.clone());
        }
        if !self.working_dir.trim().is_empty() {
            transport = transport.with_working_dir(&self.working_dir);
        }
        transport
    }

    /// Phân tích cú pháp dòng lệnh SSH theo chuẩn của Zed Editor (`SshConnectionOptions::parse_command_line`)
    /// Hỗ trợ cả định dạng URL: `user@hostname:2222`, `[ipv6]:port`
    /// Lẫn cú pháp dòng lệnh: `ssh -p 22 -i ~/.ssh/key -J bastion user@host`
    pub fn parse_command_line(input: &str, working_dir: impl Into<String>) -> anyhow::Result<Self> {
        let input = input.trim();
        let input = input.strip_prefix("ssh ").unwrap_or(input).trim();
        let mut hostname: Option<String> = None;
        let mut username: Option<String> = None;
        let mut port: Option<u16> = None;
        let mut args = Vec::new();

        const ALLOWED_OPTS: &[&str] = &[
            "-4", "-6", "-A", "-a", "-C", "-K", "-k", "-X", "-x", "-Y", "-y",
        ];
        const ALLOWED_ARGS: &[&str] = &[
            "-B", "-b", "-c", "-D", "-F", "-I", "-i", "-J", "-l", "-m", "-o", "-P", "-p", "-R",
            "-w",
        ];

        let tokens = split_shell_args(input);
        let mut iter = tokens.into_iter();

        'outer: while let Some(arg) = iter.next() {
            if ALLOWED_OPTS.contains(&arg.as_str()) {
                args.push(arg);
                continue;
            }
            if arg == "-p" {
                if let Some(p) = iter.next() {
                    port = p.parse().ok();
                }
                continue;
            } else if let Some(p) = arg.strip_prefix("-p") {
                port = p.parse().ok();
                continue;
            }
            if arg == "-l" {
                username = iter.next();
                continue;
            } else if let Some(l) = arg.strip_prefix("-l") {
                username = Some(l.to_string());
                continue;
            }

            if arg == "-o" {
                if let Some(next) = iter.next() {
                    if let Ok(p) = next.parse::<u16>() {
                        port = Some(p);
                    } else if let Some(p_str) = next.strip_prefix("Port=") {
                        port = p_str.parse().ok();
                    } else {
                        args.push(arg);
                        args.push(next);
                    }
                }
                continue;
            } else if let Some(rest) = arg.strip_prefix("-o") {
                if let Ok(p) = rest.parse::<u16>() {
                    port = Some(p);
                    continue;
                } else if let Some(p_str) = rest.strip_prefix("Port=") {
                    port = p_str.parse().ok();
                    continue;
                }
            }

            for a in ALLOWED_ARGS {
                if arg == *a {
                    args.push(arg);
                    if let Some(next) = iter.next() {
                        args.push(next);
                    }
                    continue 'outer;
                } else if arg.starts_with(a) {
                    args.push(arg);
                    continue 'outer;
                }
            }

            if arg.starts_with('-') || hostname.is_some() {
                continue;
            }

            let mut target = arg.as_str();
            if let Some((u, rest)) = target.rsplit_once('@') {
                target = rest;
                username = Some(u.to_string());
            }

            // Xử lý IPv6 và port
            if target.starts_with('[') {
                if let Some((rest, p)) = target.rsplit_once("]:") {
                    target = rest.strip_prefix('[').unwrap_or(rest);
                    port = p.parse().ok();
                } else if target.ends_with(']') {
                    target = target.strip_prefix('[').unwrap_or(target);
                    target = target.strip_suffix(']').unwrap_or(target);
                }
            } else if let Some((rest, p)) = target.rsplit_once(':') {
                if !rest.contains(':') {
                    target = rest;
                    port = p.parse().ok();
                }
            }

            hostname = Some(target.to_string());
        }

        let host = hostname.ok_or_else(|| anyhow::anyhow!("Missing hostname in SSH input"))?;

        Ok(Self {
            host,
            username,
            port,
            working_dir: working_dir.into(),
            nickname: None,
            args: if args.is_empty() { None } else { Some(args) },
        })
    }
}

impl std::fmt::Display for SshConnectionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.working_dir.is_empty() {
            write!(f, "{} (SSH: {})", self.working_dir, self.display_name())
        } else {
            write!(f, "SSH ({})", self.display_name())
        }
    }
}

impl From<SshConnectionOptions> for RemoteConnectionOptions {
    fn from(opts: SshConnectionOptions) -> Self {
        Self::Ssh(opts)
    }
}

fn split_shell_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && !in_single_quote {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }
        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }
        if ch.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                args.push(current);
                current = String::new();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_parse_command_line_user_syntax() {
        // Cú pháp chính xác: ssh user@example -o 2222
        let opts = SshConnectionOptions::parse_command_line("ssh user@example -o 2222", "/var/log")
            .unwrap();
        assert_eq!(opts.host, "example");
        assert_eq!(opts.username, Some("user".to_string()));
        assert_eq!(opts.port, Some(2222));
        assert_eq!(opts.working_dir, "/var/log");

        // Cú pháp -p 2222
        let opts2 =
            SshConnectionOptions::parse_command_line("ssh user@example -p 2222", "").unwrap();
        assert_eq!(opts2.host, "example");
        assert_eq!(opts2.username, Some("user".to_string()));
        assert_eq!(opts2.port, Some(2222));

        // Cú pháp -o Port=2222
        let opts3 =
            SshConnectionOptions::parse_command_line("ssh user@example -o Port=2222", "").unwrap();
        assert_eq!(opts3.host, "example");
        assert_eq!(opts3.username, Some("user".to_string()));
        assert_eq!(opts3.port, Some(2222));

        // URL syntax
        let opts4 = SshConnectionOptions::parse_command_line("user@example:2222", "").unwrap();
        assert_eq!(opts4.host, "example");
        assert_eq!(opts4.username, Some("user".to_string()));
        assert_eq!(opts4.port, Some(2222));
    }
}
