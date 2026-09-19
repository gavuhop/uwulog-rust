use super::{BoxedRead, BoxedWrite, RemoteTransport, WslArch, WslTransport};
use crate::wsl::parse_wsl_arch;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use uwu_core_util::command::{new_std_command, new_tokio_command};

/// SshTransport: Quản lý kết nối SSH, ControlMaster multiplexing, provisioning agent, và stream dữ liệu.
/// Thiết kế mô phỏng theo kiến trúc Zed Editor (`crates/remote/src/transport/ssh.rs`).
#[derive(Debug, Clone)]
pub struct SshTransport {
    host: String,
    user: Option<String>,
    port: Option<u16>,
    args: Vec<String>,
    working_dir: Option<String>,
    agent_cmd: Option<String>,
    name: String,
    control_socket: Option<PathBuf>,
}

impl SshTransport {
    pub fn new(host: impl Into<String>) -> Self {
        let host_str = host.into();
        let name = format!("SSH ({})", host_str);
        Self {
            host: host_str,
            user: None,
            port: None,
            args: Vec::new(),
            working_dir: None,
            agent_cmd: None,
            name,
            control_socket: None,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_agent_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.agent_cmd = Some(cmd.into());
        self
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn port(&self) -> Option<u16> {
        self.port
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn working_dir(&self) -> Option<&str> {
        self.working_dir.as_deref()
    }

    pub fn target_string(&self) -> String {
        if let Some(ref u) = self.user {
            format!("{}@{}", u, self.host)
        } else {
            self.host.clone()
        }
    }

    /// Lấy đường dẫn UNIX domain socket cho SSH ControlMaster connection multiplexing (chuẩn Zed)
    pub fn control_socket_path(&self) -> PathBuf {
        Self::compute_control_socket_path(&self.host, self.user.as_deref(), self.port)
    }

    pub fn compute_control_socket_path(
        host: &str,
        user: Option<&str>,
        port: Option<u16>,
    ) -> PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        host.hash(&mut hasher);
        user.hash(&mut hasher);
        port.hash(&mut hasher);
        let hash = hasher.finish();
        std::env::temp_dir().join(format!("uwu-ssh-{:x}.sock", hash))
    }

    /// Kiểm tra xem SSH ControlMaster session đã kết nối và còn sống không (`ssh -O check`)
    #[cfg(not(target_os = "windows"))]
    pub fn is_master_alive(socket_path: &Path, target: &str, additional_args: &[String]) -> bool {
        if !socket_path.exists() {
            return false;
        }
        let mut cmd = new_std_command("ssh");
        cmd.arg("-O")
            .arg("check")
            .arg("-o")
            .arg(format!("ControlPath={}", socket_path.display()));
        for arg in additional_args {
            cmd.arg(arg);
        }
        cmd.arg(target);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.status() {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Đảm bảo tiến trình SSH ControlMaster nền đang chạy (multiplexing kết nối để thực thi tức thì < 5ms)
    #[cfg(not(target_os = "windows"))]
    pub async fn ensure_master_connection(
        host: &str,
        user: Option<&str>,
        port: Option<u16>,
        additional_args: &[String],
    ) -> Result<PathBuf> {
        let socket_path = Self::compute_control_socket_path(host, user, port);
        let target = if let Some(u) = user {
            format!("{}@{}", u, host)
        } else {
            host.to_string()
        };

        if Self::is_master_alive(&socket_path, &target, additional_args) {
            return Ok(socket_path);
        }

        let _ = std::fs::remove_file(&socket_path);

        let mut cmd = new_tokio_command("ssh");
        cmd.arg("-N")
            .arg("-f")
            .arg("-o")
            .arg("ControlMaster=yes")
            .arg("-o")
            .arg("ControlPersist=10m")
            .arg("-o")
            .arg(format!("ControlPath={}", socket_path.display()));

        if let Some(p) = port {
            cmd.arg("-p").arg(p.to_string());
        }
        for arg in additional_args {
            cmd.arg(arg);
        }
        cmd.arg(&target);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn SSH ControlMaster for {}", host))?;
        let output = child
            .wait_with_output()
            .await
            .context("Failed waiting for SSH master")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            log::warn!("SSH ControlMaster spawn warning: {}", err.trim());
        }

        Ok(socket_path)
    }

    /// Tạo command SSH đã áp dụng ControlPath multiplexing nếu khả dụng
    pub fn build_tokio_command(&self) -> tokio::process::Command {
        let mut cmd = new_tokio_command("ssh");
        #[cfg(not(target_os = "windows"))]
        {
            let socket = self
                .control_socket
                .clone()
                .unwrap_or_else(|| self.control_socket_path());
            if socket.exists() {
                cmd.arg("-o")
                    .arg("ControlMaster=no")
                    .arg("-o")
                    .arg(format!("ControlPath={}", socket.display()));
            }
        }
        if let Some(p) = self.port {
            cmd.arg("-p").arg(p.to_string());
        }
        for arg in &self.args {
            cmd.arg(arg);
        }
        cmd.arg(self.target_string());
        cmd
    }

    /// Liệt kê danh sách các thư mục con trong một đường dẫn SSH từ xa (chuẩn Zed: bàn phím ưu tiên, < 10ms khi multiplexed)
    pub fn list_remote_directories(
        host: &str,
        unix_path: &str,
        user: Option<&str>,
        port: Option<u16>,
        args: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let clean_path = unix_path.trim();
        let clean_path = if clean_path.is_empty() {
            "~"
        } else {
            clean_path
        };

        let mut cmd = new_std_command("ssh");
        cmd.stdin(Stdio::null());
        cmd.arg("-o").arg("BatchMode=yes");
        cmd.arg("-o").arg("ConnectTimeout=5");
        #[cfg(not(target_os = "windows"))]
        {
            let socket_path = Self::compute_control_socket_path(host, user, port);
            if socket_path.exists() {
                cmd.arg("-o")
                    .arg("ControlMaster=no")
                    .arg("-o")
                    .arg(format!("ControlPath={}", socket_path.display()));
            }
        }
        if let Some(p) = port {
            cmd.arg("-p").arg(p.to_string());
        }
        if let Some(extra_args) = args {
            for a in extra_args {
                cmd.arg(a);
            }
        }
        let target = if let Some(u) = user {
            format!("{}@{}", u, host)
        } else {
            host.to_string()
        };
        cmd.arg(target);
        cmd.arg(format!("ls -1ap {}", clean_path));

        let output = cmd.output().context("failed to execute ls over SSH")?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Cannot list SSH directory: {}", err.trim());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut dirs = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with('/') {
                let folder_name = trimmed.trim_end_matches('/');
                if folder_name != "." && folder_name != ".." && !folder_name.is_empty() {
                    dirs.push(folder_name.to_string());
                }
            }
        }
        dirs.sort_by_key(|a| a.to_lowercase());
        Ok(dirs)
    }

    /// Xác định thư mục HOME của người dùng trên remote SSH (`echo $HOME`)
    pub fn resolve_home_dir(
        host: &str,
        user: Option<&str>,
        port: Option<u16>,
        args: Option<&[String]>,
    ) -> String {
        let mut cmd = new_std_command("ssh");
        cmd.stdin(Stdio::null());
        cmd.arg("-o").arg("BatchMode=yes");
        cmd.arg("-o").arg("ConnectTimeout=5");
        #[cfg(not(target_os = "windows"))]
        {
            let socket_path = Self::compute_control_socket_path(host, user, port);
            if socket_path.exists() {
                cmd.arg("-o")
                    .arg("ControlMaster=no")
                    .arg("-o")
                    .arg(format!("ControlPath={}", socket_path.display()));
            }
        }
        if let Some(p) = port {
            cmd.arg("-p").arg(p.to_string());
        }
        if let Some(extra_args) = args {
            for a in extra_args {
                cmd.arg(a);
            }
        }
        let target = if let Some(u) = user {
            format!("{}@{}", u, host)
        } else {
            host.to_string()
        };
        cmd.arg(target);
        cmd.arg("echo $HOME");

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.starts_with('/') {
                    if path.ends_with('/') {
                        return path;
                    } else {
                        return format!("{}/", path);
                    }
                }
            }
        }
        "~/".to_string()
    }

    /// Kết nối đến SSH server, kích hoạt multiplexing ControlMaster (nếu hỗ trợ) và probe thư mục ban đầu.
    /// Hàm này chạy đồng bộ (thích hợp gọi trong background worker thread).
    pub fn connect_and_probe(
        host: &str,
        user: Option<&str>,
        port: Option<u16>,
        args: Option<&[String]>,
        password: Option<&str>,
    ) -> std::result::Result<SshConnectionSuccess, String> {
        let target = if let Some(u) = user {
            format!("{}@{}", u, host)
        } else {
            host.to_string()
        };

        let askpass = password
            .filter(|p| !p.is_empty())
            .and_then(|p| AskPassGuard::new(p).ok());

        #[cfg(not(target_os = "windows"))]
        {
            let socket_path = Self::compute_control_socket_path(host, user, port);
            let additional_args = args.unwrap_or(&[]);
            if socket_path.exists()
                && !Self::is_master_alive(&socket_path, &target, additional_args)
            {
                let _ = std::fs::remove_file(&socket_path);
            }

            if !Self::is_master_alive(&socket_path, &target, additional_args) {
                let mut cmd = new_std_command("ssh");
                cmd.arg("-N")
                    .arg("-f")
                    .arg("-o")
                    .arg("ControlMaster=yes")
                    .arg("-o")
                    .arg("ControlPersist=10m")
                    .arg("-o")
                    .arg(format!("ControlPath={}", socket_path.display()))
                    .arg("-o")
                    .arg("StrictHostKeyChecking=accept-new")
                    .arg("-o")
                    .arg("ConnectTimeout=10");

                if let Some(ref ap) = askpass {
                    cmd.env("SSH_ASKPASS_REQUIRE", "force")
                        .env("SSH_ASKPASS", ap.script_path());
                }

                if let Some(p) = port {
                    cmd.arg("-p").arg(p.to_string());
                }
                for arg in additional_args {
                    cmd.arg(arg);
                }
                cmd.arg(&target);
                cmd.stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let output = cmd
                    .output()
                    .map_err(|e| format!("Failed to execute ssh: {}", e))?;

                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let msg = if err.is_empty() {
                        format!(
                            "SSH connection failed (exit code {:?})",
                            output.status.code()
                        )
                    } else {
                        err
                    };
                    return Err(msg);
                }

                // Chờ socket file sẵn sàng
                for _ in 0..10 {
                    if socket_path.exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let mut cmd = new_std_command("ssh");
            cmd.arg("-o")
                .arg("StrictHostKeyChecking=accept-new")
                .arg("-o")
                .arg("ConnectTimeout=10");

            if let Some(ref ap) = askpass {
                cmd.env("SSH_ASKPASS_REQUIRE", "force")
                    .env("SSH_ASKPASS", ap.script_path());
            }

            if let Some(p) = port {
                cmd.arg("-p").arg(p.to_string());
            }
            if let Some(extra_args) = args {
                for a in extra_args {
                    cmd.arg(a);
                }
            }
            cmd.arg(&target);
            cmd.arg("echo $HOME");
            cmd.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let output = cmd
                .output()
                .map_err(|e| format!("Failed to execute ssh: {}", e))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let msg = if err.is_empty() {
                    format!(
                        "SSH connection failed (exit code {:?})",
                        output.status.code()
                    )
                } else {
                    err
                };
                return Err(msg);
            }
        }

        let initial_dir = Self::resolve_home_dir(host, user, port, args);
        let entries =
            Self::list_remote_directories(host, &initial_dir, user, port, args).unwrap_or_default();

        Ok(SshConnectionSuccess {
            initial_dir,
            entries,
        })
    }

    /// Phát hiện kiến trúc CPU của máy chủ SSH từ xa (`uname -sm`)
    pub async fn detect_arch(&self) -> Result<WslArch> {
        let mut cmd = self.build_tokio_command();
        cmd.arg("uname -sm");
        let output = cmd
            .output()
            .await
            .context("failed to run uname -sm over SSH")?;
        if !output.status.success() {
            anyhow::bail!(
                "uname -sm failed over SSH: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let text = String::from_utf8_lossy(&output.stdout);
        parse_wsl_arch(&text)
    }

    /// Đảm bảo binary `uwu-agent` tồn tại, đúng phiên bản và kiến trúc trên máy chủ từ xa
    /// Tương tự Zed: vị trí `~/.local/share/uwu/server_state/uwu-agent-{arch}-{version}`
    pub async fn ensure_server_binary(&self) -> Result<String> {
        if let Some(ref custom) = self.agent_cmd {
            return Ok(custom.clone());
        }

        let arch = self.detect_arch().await.unwrap_or_else(|e| {
            log::warn!(
                "Failed to detect SSH remote arch for host '{}': {:#}. Defaulting to x86_64",
                self.host,
                e
            );
            WslArch::X86_64
        });
        log::info!("Detected SSH host '{}' architecture: {}", self.host, arch);

        let version = env!("CARGO_PKG_VERSION");
        let binary_name = format!("uwu-agent-{}-{}", arch.as_str(), version);
        let remote_binary_path = format!("\"$HOME/.local/share/uwu/server_state/{}\"", binary_name);

        // 1. Kiểm tra xem binary đã tồn tại và chạy được chưa
        let check_cmd = format!("{} version", remote_binary_path);
        let mut check_process = self.build_tokio_command();
        check_process.arg(&check_cmd);
        if let Ok(output) = check_process.output().await {
            if output.status.success() {
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                if stdout_str.contains(&format!("uwu-agent {}", version)) {
                    log::info!(
                        "SSH Remote Agent verified at {} (Arch: {}, Version: {})",
                        remote_binary_path,
                        arch,
                        version
                    );
                    return Ok(remote_binary_path);
                }
            }
        }

        log::info!(
            "SSH Remote Agent missing or outdated on host '{}' (Arch: {}). Provisioning...",
            self.host,
            arch
        );

        // 2. Tìm binary local phù hợp để upload
        if let Some(local_path) = WslTransport::find_local_agent_binary(arch) {
            log::info!(
                "Provisioning SSH Remote Agent from local path: {}",
                local_path.display()
            );
            if let Ok(bytes) = tokio::fs::read(&local_path).await {
                let stream_cmd = format!(
                    "mkdir -p \"$HOME/.local/share/uwu/server_state\" && cat > {} && chmod 755 {}",
                    remote_binary_path, remote_binary_path
                );
                let mut upload_process = self.build_tokio_command();
                upload_process.arg(&stream_cmd);
                upload_process.stdin(Stdio::piped());
                upload_process.stdout(Stdio::piped());
                upload_process.stderr(Stdio::piped());

                if let Ok(mut child) = upload_process.spawn() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&bytes).await;
                        let _ = stdin.flush().await;
                        drop(stdin);
                    }
                    if let Ok(status) = child.wait().await {
                        if status.success() {
                            log::info!(
                                "Successfully provisioned SSH Remote Agent at {}",
                                remote_binary_path
                            );
                            return Ok(remote_binary_path);
                        }
                    }
                }
            }
        }

        // Fallback: Nếu không upload được, fallback gọi binary `uwu-agent` sẵn có trong PATH
        log::warn!("Could not provision uwu-agent binary to SSH host, falling back to 'uwu-agent'");
        Ok("uwu-agent".to_string())
    }
}

/// Kết quả probe thư mục ban đầu thành công sau khi kết nối SSH
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshConnectionSuccess {
    pub initial_dir: String,
    pub entries: Vec<String>,
}

/// RAII Guard tạo và dọn dẹp file script SSH_ASKPASS tạm thời phục vụ xác thực mật khẩu
pub struct AskPassGuard {
    script_path: PathBuf,
    secret_path: PathBuf,
}

impl AskPassGuard {
    pub fn new(password: &str) -> std::io::Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let temp_dir = std::env::temp_dir();
        let secret_path = temp_dir.join(format!("uwu-pass-{}-{}.secret", pid, nanos));

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&secret_path)?;
            file.write_all(password.as_bytes())?;
            file.write_all(b"\n")?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&secret_path, format!("{}\r\n", password))?;
        }

        #[cfg(unix)]
        let script_path = {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let s_path = temp_dir.join(format!("uwu-askpass-{}-{}.sh", pid, nanos));
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o700)
                .open(&s_path)?;
            let script_content = format!("#!/bin/sh\ncat \"{}\"\n", secret_path.display());
            file.write_all(script_content.as_bytes())?;
            s_path
        };

        #[cfg(windows)]
        let script_path = {
            let s_path = temp_dir.join(format!("uwu-askpass-{}-{}.bat", pid, nanos));
            let script_content = format!("@echo off\r\ntype \"{}\"\r\n", secret_path.display());
            std::fs::write(&s_path, script_content)?;
            s_path
        };

        #[cfg(not(any(unix, windows)))]
        let script_path = PathBuf::new();

        Ok(Self {
            script_path,
            secret_path,
        })
    }

    pub fn script_path(&self) -> &Path {
        &self.script_path
    }
}

impl Drop for AskPassGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.script_path);
        let _ = std::fs::remove_file(&self.secret_path);
    }
}

#[async_trait]
impl RemoteTransport for SshTransport {
    fn name(&self) -> &str {
        &self.name
    }

    async fn spawn_proxy(&self) -> Result<(BoxedRead, BoxedWrite)> {
        #[cfg(not(target_os = "windows"))]
        let socket_path =
            Self::ensure_master_connection(&self.host, self.user.as_deref(), self.port, &self.args)
                .await
                .ok();

        let mut transport = self.clone();
        #[cfg(not(target_os = "windows"))]
        {
            transport.control_socket = socket_path.or_else(|| self.control_socket.clone());
        }

        let remote_binary = transport
            .ensure_server_binary()
            .await
            .unwrap_or_else(|_| "uwu-agent".to_string());

        let mut cmd = transport.build_tokio_command();
        if let Some(ref workdir) = transport.working_dir {
            cmd.arg(format!("cd \"{}\" && {} proxy", workdir, remote_binary));
        } else {
            cmd.arg(format!("{} proxy", remote_binary));
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn SSH proxy to {}", self.host))?;

        super::wrap_child_stdio(child, "SSH Agent")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_transport_builder_and_target_string() {
        let t1 = SshTransport::new("myserver.com");
        assert_eq!(t1.target_string(), "myserver.com");
        assert_eq!(t1.name(), "SSH (myserver.com)");

        let t2 = SshTransport::new("192.168.1.10")
            .with_user("ubuntu")
            .with_port(2222)
            .with_args(vec!["-i".to_string(), "key.pem".to_string()])
            .with_working_dir("/var/log");

        assert_eq!(t2.target_string(), "ubuntu@192.168.1.10");
        assert_eq!(t2.port(), Some(2222));
        assert_eq!(t2.working_dir(), Some("/var/log"));
        assert_eq!(t2.args(), &["-i", "key.pem"]);
    }

    #[test]
    fn test_compute_control_socket_path() {
        let p1 = SshTransport::compute_control_socket_path("hostA", Some("user1"), Some(22));
        let p2 = SshTransport::compute_control_socket_path("hostA", Some("user1"), Some(22));
        let p3 = SshTransport::compute_control_socket_path("hostB", Some("user1"), Some(22));

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
        assert!(p1.to_string_lossy().contains("uwu-ssh-"));
    }
}
