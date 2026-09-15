use super::{BoxedRead, BoxedWrite, RemoteTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use uwu_core_util::command::{new_std_command, new_tokio_command};

pub struct WslTransport {
    distro: String,
    working_dir: Option<String>,
    agent_cmd: Option<String>,
    name: String,
}

#[inline]
fn new_wsl_cmd() -> tokio::process::Command {
    new_tokio_command("wsl.exe")
}

impl WslTransport {
    pub fn new(distro: impl Into<String>) -> Self {
        let distro_str = distro.into();
        let name = format!("WSL ({})", distro_str);
        Self {
            distro: distro_str,
            working_dir: None,
            agent_cmd: None,
            name,
        }
    }

    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_agent_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.agent_cmd = Some(cmd.into());
        self
    }

    pub fn distro(&self) -> &str {
        &self.distro
    }

    /// Tự động phát hiện danh sách các WSL Distro đã cài đặt trên máy Windows
    pub fn detect_distros() -> Vec<String> {
        let output = match new_std_command("wsl.exe")
            .arg("--list")
            .arg("--quiet")
            .output()
        {
            Ok(out) => out.stdout,
            Err(_) => return Vec::new(),
        };

        if output.is_empty() {
            return Vec::new();
        }

        let text = decode_utf16le_or_utf8(&output);
        text.lines()
            .map(|l| l.trim().trim_matches('\0').trim())
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    /// Tìm đường dẫn binary local uwu-agent trên host Windows
    fn find_local_agent_binary() -> Option<PathBuf> {
        // 1. Kiểm tra cùng thư mục với tiến trình hiện tại (ví dụ cạnh uwu-gui.exe)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(dir) = exe_path.parent() {
                let candidate1 = dir.join("uwu-agent.exe");
                if candidate1.exists() {
                    return Some(candidate1);
                }
                let candidate2 = dir.join("uwu-agent");
                if candidate2.exists() {
                    return Some(candidate2);
                }
            }
        }

        // 2. Kiểm tra trong target/release và target/debug của workspace
        let candidates = [
            "target/release/uwu-agent",
            "target/release/uwu-agent.exe",
            "target/debug/uwu-agent",
            "target/debug/uwu-agent.exe",
            "../target/release/uwu-agent",
            "../target/debug/uwu-agent",
        ];

        for rel in &candidates {
            let p = Path::new(rel);
            if p.exists() {
                if let Ok(canonical) = p.canonicalize() {
                    return Some(canonical);
                }
                return Some(p.to_path_buf());
            }
        }

        None
    }

    /// Đảm bảo binary agent có đúng version tồn tại và thực thi được trong WSL
    pub async fn ensure_server_binary(&self) -> Result<String> {
        if let Some(custom) = &self.agent_cmd {
            return Ok(custom.clone());
        }

        let version = env!("CARGO_PKG_VERSION");
        let binary_name = format!("uwu-agent-{}", version);
        let remote_binary_path = format!("\"$HOME/.local/share/uwu/server_state/{}\"", binary_name);

        // 1. Kiểm tra xem binary đã tồn tại và chạy được chưa
        let check_cmd = format!("{} version", remote_binary_path);
        let check_output = new_wsl_cmd()
            .arg("-d")
            .arg(&self.distro)
            .arg("--cd")
            .arg("~")
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg(&check_cmd)
            .output()
            .await;

        if let Ok(out) = check_output {
            if out.status.success() {
                let stdout_str = String::from_utf8_lossy(&out.stdout);
                if stdout_str.contains(&format!("uwu-agent {}", version)) {
                    log::info!(
                        "WSL Remote Agent verified at {} (Version: {})",
                        remote_binary_path,
                        version
                    );
                    return Ok(remote_binary_path);
                }
            }
        }

        log::info!(
            "WSL Remote Agent missing or outdated on distro '{}'. Provisioning...",
            self.distro
        );

        // 2. Tìm binary local trên Windows host để upload
        if let Some(local_path) = Self::find_local_agent_binary() {
            // Method 1: Upload qua wslpath + cp
            let win_path_str = local_path.to_string_lossy().to_string();
            let wslpath_out = new_wsl_cmd()
                .arg("-d")
                .arg(&self.distro)
                .arg("--cd")
                .arg("~")
                .arg("--")
                .arg("wslpath")
                .arg("-u")
                .arg(&win_path_str)
                .output()
                .await;

            let mut upload_success = false;
            if let Ok(out) = wslpath_out {
                if out.status.success() {
                    let wsl_src = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let cp_cmd = format!(
                        "mkdir -p \"$HOME/.local/share/uwu/server_state\" && cp '{}' {} && chmod 755 {}",
                        wsl_src, remote_binary_path, remote_binary_path
                    );

                    let cp_status = new_wsl_cmd()
                        .arg("-d")
                        .arg(&self.distro)
                        .arg("--cd")
                        .arg("~")
                        .arg("--")
                        .arg("sh")
                        .arg("-c")
                        .arg(&cp_cmd)
                        .status()
                        .await;

                    if let Ok(status) = cp_status {
                        if status.success() {
                            upload_success = true;
                        }
                    }
                }
            }

            // Method 2 (Fallback): Stream bytes qua wsl.exe stdin
            if !upload_success {
                if let Ok(bytes) = tokio::fs::read(&local_path).await {
                    let stream_cmd = format!(
                        "mkdir -p \"$HOME/.local/share/uwu/server_state\" && cat > {} && chmod 755 {}",
                        remote_binary_path, remote_binary_path
                    );

                    let child = new_wsl_cmd()
                        .arg("-d")
                        .arg(&self.distro)
                        .arg("--cd")
                        .arg("~")
                        .arg("--")
                        .arg("sh")
                        .arg("-c")
                        .arg(&stream_cmd)
                        .stdin(Stdio::piped())
                        .spawn();

                    if let Ok(mut c) = child {
                        if let Some(mut stdin) = c.stdin.take() {
                            let _ = stdin.write_all(&bytes).await;
                            let _ = stdin.flush().await;
                            drop(stdin);
                        }
                        let _ = c.wait().await;
                    }
                }
            }

            // 3. Kiểm tra xác minh lại sau khi upload
            let recheck = new_wsl_cmd()
                .arg("-d")
                .arg(&self.distro)
                .arg("--cd")
                .arg("~")
                .arg("--")
                .arg("sh")
                .arg("-c")
                .arg(&check_cmd)
                .output()
                .await;

            if let Ok(out) = recheck {
                if out.status.success() {
                    return Ok(remote_binary_path);
                }
            }
        }

        // Fallback cuối cùng: thử gọi binary có sẵn trong PATH nếu provision thất bại
        Ok(format!("{} || uwu-agent", remote_binary_path))
    }
}

/// Giải mã byte stream từ output của lệnh Windows (thường là UTF-16LE từ wsl.exe hoặc UTF-8)
pub fn decode_utf16le_or_utf8(output: &[u8]) -> String {
    if output.len() >= 2
        && output.len().is_multiple_of(2)
        && output.iter().skip(1).step_by(2).any(|&b| b == 0)
    {
        let u16_vec: Vec<u16> = output
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16_vec)
    } else {
        String::from_utf8_lossy(output).to_string()
    }
}

#[async_trait]
impl RemoteTransport for WslTransport {
    fn name(&self) -> &str {
        &self.name
    }

    async fn spawn_proxy(&self) -> Result<(BoxedRead, BoxedWrite)> {
        // Tự động bảo đảm binary tồn tại và lấy absolute path
        let remote_binary = self
            .ensure_server_binary()
            .await
            .unwrap_or_else(|_| "uwu-agent".to_string());

        let mut cmd = new_wsl_cmd();
        cmd.arg("-d").arg(&self.distro);
        let cd_dir = self
            .working_dir
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or("~");
        cmd.arg("--cd").arg(cd_dir);
        cmd.arg("--")
            .arg("sh")
            .arg("-c")
            .arg(format!("{} proxy", remote_binary));

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn WSL proxy process (Distro: {}, Agent: {})",
                self.distro, remote_binary
            )
        })?;

        super::wrap_child_stdio(child, "WSL Agent")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wsl_transport_builder() {
        let transport = WslTransport::new("Ubuntu")
            .with_working_dir("/var/log")
            .with_agent_cmd("/usr/local/bin/uwu-agent");

        assert_eq!(transport.distro(), "Ubuntu");
        assert_eq!(transport.name(), "WSL (Ubuntu)");
        assert_eq!(transport.working_dir.as_deref(), Some("/var/log"));
        assert_eq!(
            transport.agent_cmd.as_deref(),
            Some("/usr/local/bin/uwu-agent")
        );
    }

    #[test]
    fn test_decode_utf16le_or_utf8() {
        // 1. Kiểm tra giải mã chuỗi UTF-8 tiêu chuẩn
        let utf8_bytes = b"Ubuntu-22.04\nDebian\n";
        assert_eq!(decode_utf16le_or_utf8(utf8_bytes), "Ubuntu-22.04\nDebian\n");

        // 2. Kiểm tra giải mã chuỗi UTF-16LE từ Windows wsl --list --quiet
        let utf16_str = "Ubuntu\r\nDebian\r\n";
        let mut utf16_bytes = Vec::new();
        for u in utf16_str.encode_utf16() {
            utf16_bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_utf16le_or_utf8(&utf16_bytes), utf16_str);

        // 3. Chuỗi rỗng
        assert_eq!(decode_utf16le_or_utf8(&[]), "");
    }
}
