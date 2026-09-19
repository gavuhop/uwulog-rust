use super::{BoxedRead, BoxedWrite, RemoteTransport, WslArch, WslTransport};
use crate::wsl::parse_wsl_arch;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};
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

        if !Self::is_master_alive(&socket_path, &target, additional_args) {
            let _ = std::fs::remove_file(&socket_path);
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let msg = if !err.is_empty() {
                format!("Failed to establish SSH connection to {}: {}", target, err)
            } else {
                format!(
                    "Failed to establish SSH connection to {}: ControlMaster socket is not active",
                    target
                )
            };
            anyhow::bail!(msg);
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
        cmd.arg("-o").arg("LogLevel=ERROR");
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

    /// Kết nối đến SSH server theo mô hình tương tác tuần tự (chuẩn Zed Editor).
    /// Giao tiếp phản hồi theo từng bước: Kiểm tra kết nối -> Host Key (yes/no) -> Password.
    /// Không hỏi trước thông tin khi chưa có yêu cầu từ SSH daemon.
    pub fn connect_interactive(
        host: &str,
        user: Option<&str>,
        port: Option<u16>,
        args: Option<&[String]>,
        cancel_flag: Arc<AtomicBool>,
        event_tx: Sender<SshInteractiveEvent>,
    ) {
        let target = if let Some(u) = user {
            format!("{}@{}", u, host)
        } else {
            host.to_string()
        };

        let askpass_session = match InteractiveAskPassSession::new() {
            Ok(s) => s,
            Err(e) => {
                let _ = event_tx.send(SshInteractiveEvent::Failed(format!(
                    "Failed to initialize SSH askpass session: {}",
                    e
                )));
                return;
            }
        };

        #[cfg(not(target_os = "windows"))]
        {
            let socket_path = Self::compute_control_socket_path(host, user, port);
            let additional_args = args.unwrap_or(&[]);
            if socket_path.exists()
                && !Self::is_master_alive(&socket_path, &target, additional_args)
            {
                let _ = std::fs::remove_file(&socket_path);
            }

            if Self::is_master_alive(&socket_path, &target, additional_args) {
                let initial_dir = Self::resolve_home_dir(host, user, port, args);
                let entries = Self::list_remote_directories(host, &initial_dir, user, port, args)
                    .unwrap_or_default();
                let _ = event_tx.send(SshInteractiveEvent::Connected(SshConnectionSuccess {
                    initial_dir,
                    entries,
                }));
                return;
            }

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
                .arg("StrictHostKeyChecking=ask")
                .arg("-o")
                .arg("ConnectTimeout=10");

            cmd.env("SSH_ASKPASS_REQUIRE", "force")
                .env("SSH_ASKPASS", askpass_session.script_path());

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

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = event_tx.send(SshInteractiveEvent::Failed(format!(
                        "Failed to spawn ssh command: {}",
                        e
                    )));
                    return;
                }
            };

            let start_time = Instant::now();
            let timeout = Duration::from_secs(30);

            loop {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    return;
                }

                if start_time.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = event_tx.send(SshInteractiveEvent::Failed(
                        "SSH connection timed out after 30 seconds".to_string(),
                    ));
                    return;
                }

                // 1. Kiểm tra yêu cầu tương tác từ SSH_ASKPASS
                match askpass_session.listener().accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

                        let mut prompt_bytes = Vec::new();
                        let mut buf = [0u8; 512];
                        loop {
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    prompt_bytes.extend_from_slice(&buf[..n]);
                                    if prompt_bytes.contains(&0) || prompt_bytes.contains(&b'\n') {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }

                        let raw_prompt = String::from_utf8_lossy(&prompt_bytes)
                            .trim_matches(|c| c == '\0' || c == '\r' || c == '\n')
                            .to_string();

                        let prompt_lower = raw_prompt.to_lowercase();
                        let is_yes_no = prompt_lower.contains("yes/no");
                        let prompt_type = if is_yes_no {
                            SshInteractivePromptType::HostKeyConfirmation
                        } else {
                            SshInteractivePromptType::Password
                        };

                        let (resp_tx, resp_rx) = std::sync::mpsc::channel::<String>();
                        let _ = event_tx.send(SshInteractiveEvent::Prompt {
                            prompt_message: raw_prompt,
                            prompt_type,
                            response_sender: resp_tx,
                        });

                        loop {
                            if cancel_flag.load(Ordering::Relaxed) {
                                let _ = child.kill();
                                return;
                            }
                            match resp_rx.recv_timeout(Duration::from_millis(50)) {
                                Ok(answer) => {
                                    let _ = writeln!(stream, "{}", answer);
                                    let _ = stream.flush();
                                    break;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                    if let Ok(Some(_)) = child.try_wait() {
                                        break;
                                    }
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    let _ = child.kill();
                                    return;
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {}
                }

                // 2. Kiểm tra tiến trình child
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if status.success() {
                            for _ in 0..10 {
                                if socket_path.exists() {
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(50));
                            }
                            let initial_dir = Self::resolve_home_dir(host, user, port, args);
                            let entries =
                                Self::list_remote_directories(host, &initial_dir, user, port, args)
                                    .unwrap_or_default();
                            let _ = event_tx.send(SshInteractiveEvent::Connected(
                                SshConnectionSuccess {
                                    initial_dir,
                                    entries,
                                },
                            ));
                            return;
                        } else {
                            let mut err_msg = String::new();
                            if let Some(mut stderr) = child.stderr.take() {
                                let mut buf = Vec::new();
                                let _ = stderr.read_to_end(&mut buf);
                                err_msg = String::from_utf8_lossy(&buf).trim().to_string();
                            }
                            let msg = if err_msg.is_empty() {
                                format!("SSH connection failed (exit code {:?})", status.code())
                            } else {
                                err_msg
                            };
                            let _ = event_tx.send(SshInteractiveEvent::Failed(msg));
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = event_tx.send(SshInteractiveEvent::Failed(format!(
                            "Error monitoring SSH process: {}",
                            e
                        )));
                        return;
                    }
                }

                // 3. Kiểm tra socket master connection đã sẵn sàng
                if socket_path.exists()
                    && Self::is_master_alive(&socket_path, &target, additional_args)
                {
                    let initial_dir = Self::resolve_home_dir(host, user, port, args);
                    let entries =
                        Self::list_remote_directories(host, &initial_dir, user, port, args)
                            .unwrap_or_default();
                    let _ = event_tx.send(SshInteractiveEvent::Connected(SshConnectionSuccess {
                        initial_dir,
                        entries,
                    }));
                    return;
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        }

        #[cfg(target_os = "windows")]
        {
            let mut cmd = new_std_command("ssh");
            cmd.arg("-o")
                .arg("StrictHostKeyChecking=ask")
                .arg("-o")
                .arg("ConnectTimeout=10");

            cmd.env("SSH_ASKPASS_REQUIRE", "force")
                .env("SSH_ASKPASS", askpass_session.script_path());

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

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = event_tx.send(SshInteractiveEvent::Failed(format!(
                        "Failed to spawn ssh command: {}",
                        e
                    )));
                    return;
                }
            };

            let start_time = Instant::now();
            let timeout = Duration::from_secs(30);

            loop {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    return;
                }

                if start_time.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = event_tx.send(SshInteractiveEvent::Failed(
                        "SSH connection timed out after 30 seconds".to_string(),
                    ));
                    return;
                }

                // Kiểm tra prompt từ askpass
                match askpass_session.listener().accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

                        let mut prompt_bytes = Vec::new();
                        let mut buf = [0u8; 512];
                        loop {
                            match stream.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    prompt_bytes.extend_from_slice(&buf[..n]);
                                    if prompt_bytes.contains(&0) || prompt_bytes.contains(&b'\n') {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }

                        let raw_prompt = String::from_utf8_lossy(&prompt_bytes)
                            .trim_matches(|c| c == '\0' || c == '\r' || c == '\n')
                            .to_string();

                        let prompt_lower = raw_prompt.to_lowercase();
                        let is_yes_no = prompt_lower.contains("yes/no");
                        let prompt_type = if is_yes_no {
                            SshInteractivePromptType::HostKeyConfirmation
                        } else {
                            SshInteractivePromptType::Password
                        };

                        let (resp_tx, resp_rx) = std::sync::mpsc::channel::<String>();
                        let _ = event_tx.send(SshInteractiveEvent::Prompt {
                            prompt_message: raw_prompt,
                            prompt_type,
                            response_sender: resp_tx,
                        });

                        loop {
                            if cancel_flag.load(Ordering::Relaxed) {
                                let _ = child.kill();
                                return;
                            }
                            match resp_rx.recv_timeout(Duration::from_millis(50)) {
                                Ok(answer) => {
                                    let _ = writeln!(stream, "{}", answer);
                                    let _ = stream.flush();
                                    break;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                    if let Ok(Some(_)) = child.try_wait() {
                                        break;
                                    }
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    let _ = child.kill();
                                    return;
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {}
                }

                // Kiểm tra trạng thái child
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if status.success() {
                            let initial_dir = Self::resolve_home_dir(host, user, port, args);
                            let entries =
                                Self::list_remote_directories(host, &initial_dir, user, port, args)
                                    .unwrap_or_default();
                            let _ = event_tx.send(SshInteractiveEvent::Connected(
                                SshConnectionSuccess {
                                    initial_dir,
                                    entries,
                                },
                            ));
                            return;
                        } else {
                            let mut err_msg = String::new();
                            if let Some(mut stderr) = child.stderr.take() {
                                let mut buf = Vec::new();
                                let _ = stderr.read_to_end(&mut buf);
                                err_msg = String::from_utf8_lossy(&buf).trim().to_string();
                            }
                            let msg = if err_msg.is_empty() {
                                format!("SSH connection failed (exit code {:?})", status.code())
                            } else {
                                err_msg
                            };
                            let _ = event_tx.send(SshInteractiveEvent::Failed(msg));
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = event_tx.send(SshInteractiveEvent::Failed(format!(
                            "Error monitoring SSH process: {}",
                            e
                        )));
                        return;
                    }
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        }
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
        let remote_dir = "$HOME/.local/share/uwu/server_state";
        let remote_binary_path = format!("{}/{}", remote_dir, binary_name);

        // 1. Kiểm tra xem binary đã tồn tại và chạy được chưa
        let check_cmd = format!("\"{}\" version", remote_binary_path);
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
                    "mkdir -p \"{}\" && cat > \"{}\" && chmod 755 \"{}\"",
                    remote_dir, remote_binary_path, remote_binary_path
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
                            // Xác thực lại binary vừa upload có thực sự chạy được trên remote không (tránh lỗi libc incompatibility)
                            let mut verify_process = self.build_tokio_command();
                            verify_process.arg(&check_cmd);
                            if let Ok(output) = verify_process.output().await {
                                if output.status.success() {
                                    log::info!(
                                        "Successfully provisioned and verified SSH Remote Agent at {}",
                                        remote_binary_path
                                    );
                                    return Ok(remote_binary_path);
                                } else {
                                    let err_str = String::from_utf8_lossy(&output.stderr);
                                    log::error!(
                                        "Provisioned binary failed to execute on remote host '{}': {}",
                                        self.host,
                                        err_str.trim()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: Kiểm tra xem uwu-agent có sẵn và chạy được trong PATH trên remote không
        let mut check_path = self.build_tokio_command();
        check_path.arg("uwu-agent version");
        if let Ok(output) = check_path.output().await {
            if output.status.success() {
                log::info!(
                    "Using system 'uwu-agent' found in PATH on SSH host '{}'",
                    self.host
                );
                return Ok("uwu-agent".to_string());
            }
        }

        anyhow::bail!(
            "Failed to deploy uwu-agent on SSH host '{}'. Could not provision binary to {} (or incompatible with remote libc) and 'uwu-agent' is not installed in remote PATH.",
            self.host,
            remote_binary_path
        );
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

/// Loại yêu cầu tương tác mà SSH daemon gửi về
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshInteractivePromptType {
    /// Xác thực khóa máy chủ (yes/no/[fingerprint])
    HostKeyConfirmation,
    /// Nhập mật khẩu / passphrase
    Password,
}

/// Sự kiện gửi từ background thread kết nối SSH về cho GUI (chuẩn tương tác Zed)
#[derive(Debug)]
pub enum SshInteractiveEvent {
    /// Cập nhật thông báo trạng thái kết nối
    Status(String),
    /// SSH yêu cầu người dùng nhập thông tin (HostKey hoặc Password)
    Prompt {
        prompt_message: String,
        prompt_type: SshInteractivePromptType,
        response_sender: Sender<String>,
    },
    /// Kết nối thành công, kèm thông tin thư mục ban đầu
    Connected(SshConnectionSuccess),
    /// Kết nối thất bại, kèm thông báo lỗi chi tiết
    Failed(String),
}

/// Session quản lý tiến trình SSH_ASKPASS giao tiếp hai chiều với SSH qua TCP loopback socket
pub struct InteractiveAskPassSession {
    listener: std::net::TcpListener,
    script_path: PathBuf,
    temp_dir: PathBuf,
}

impl InteractiveAskPassSession {
    pub fn new() -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let temp_dir = std::env::temp_dir().join(format!("uwu-askpass-{}-{}", pid, nanos));
        std::fs::create_dir_all(&temp_dir)?;

        #[cfg(unix)]
        let script_path = {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let s_path = temp_dir.join("askpass.sh");
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o700)
                .open(&s_path)?;

            let script_content = format!(
                r#"#!/bin/sh
PORT={}
if command -v python3 >/dev/null 2>&1; then
    exec python3 -c '
import sys, socket
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect(("127.0.0.1", int(sys.argv[1])))
    prompt = " ".join(sys.argv[2:])
    s.sendall(prompt.encode("utf-8") + b"\0")
    resp = b""
    while True:
        chunk = s.recv(4096)
        if not chunk: break
        resp += chunk
    sys.stdout.buffer.write(resp)
except Exception:
    pass
' "$PORT" "$@"
elif command -v nc >/dev/null 2>&1; then
    printf '%s\0' "$*" | nc -N 127.0.0.1 "$PORT"
else
    bash -c 'exec 3<>/dev/tcp/127.0.0.1/'"$PORT"'; printf "%s\0" "$*" >&3; cat <&3' dummy "$@"
fi
"#,
                port
            );
            file.write_all(script_content.as_bytes())?;
            s_path
        };

        #[cfg(windows)]
        let script_path = {
            let s_path = temp_dir.join("askpass.bat");
            let script_content = format!(
                r#"@powershell -NoProfile -ExecutionPolicy Bypass -Command "$client = New-Object System.Net.Sockets.TcpClient('127.0.0.1', {}); $stream = $client.GetStream(); $bytes = [System.Text.Encoding]::UTF8.GetBytes(\"$($args -join ' ')`0\"); $stream.Write($bytes, 0, $bytes.Length); $reader = New-Object System.IO.StreamReader($stream); [Console]::Out.Write($reader.ReadToEnd()); $client.Close()" %*"#,
                port
            );
            std::fs::write(&s_path, script_content)?;
            s_path
        };

        #[cfg(not(any(unix, windows)))]
        let script_path = PathBuf::new();

        Ok(Self {
            listener,
            script_path,
            temp_dir,
        })
    }

    pub fn script_path(&self) -> &Path {
        &self.script_path
    }

    pub fn listener(&self) -> &std::net::TcpListener {
        &self.listener
    }
}

impl Drop for InteractiveAskPassSession {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
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
                .await?;

        let mut transport = self.clone();
        #[cfg(not(target_os = "windows"))]
        {
            transport.control_socket = Some(socket_path);
        }

        let remote_binary = transport.ensure_server_binary().await?;

        let agent_invocation = if remote_binary.contains('/') {
            format!("\"{}\" proxy", remote_binary)
        } else {
            format!("{} proxy", remote_binary)
        };

        let mut cmd = transport.build_tokio_command();
        if let Some(ref workdir) = transport.working_dir {
            let safe_dir = if workdir.ends_with(".log")
                || workdir.ends_with(".txt")
                || workdir.ends_with(".json")
                || workdir.ends_with(".jsonl")
                || workdir.ends_with(".out")
            {
                std::path::Path::new(workdir)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or(workdir)
            } else {
                workdir.as_str()
            };
            if !safe_dir.trim().is_empty() {
                cmd.arg(format!("cd \"{}\" && {}", safe_dir, agent_invocation));
            } else {
                cmd.arg(agent_invocation);
            }
        } else {
            cmd.arg(agent_invocation);
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
