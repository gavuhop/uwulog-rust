use super::{BoxedRead, BoxedWrite, RemoteTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use uwu_core_util::command::{new_std_command, new_tokio_command};

/// Kiến trúc CPU mục tiêu trong môi trường WSL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WslArch {
    X86_64,
    Aarch64,
}

impl WslArch {
    pub fn as_str(&self) -> &'static str {
        match self {
            WslArch::X86_64 => "x86_64",
            WslArch::Aarch64 => "aarch64",
        }
    }
}

impl std::fmt::Display for WslArch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Phân giải output của `uname -sm` để xác định kiến trúc CPU của WSL (giống Zed)
pub fn parse_wsl_arch(output: &str) -> Result<WslArch> {
    let output = output.trim();
    let uname = output.rsplit_once('\n').map_or(output, |(_, last)| last);
    let arch_part = if let Some((_, arch)) = uname.split_once(' ') {
        arch.trim()
    } else {
        uname.trim()
    };

    if arch_part.starts_with("armv8")
        || arch_part.starts_with("armv9")
        || arch_part.starts_with("arm64")
        || arch_part.starts_with("aarch64")
    {
        Ok(WslArch::Aarch64)
    } else if arch_part.starts_with("x86") || arch_part.starts_with("amd64") {
        Ok(WslArch::X86_64)
    } else {
        anyhow::bail!("Unsupported WSL architecture: '{arch_part}'");
    }
}

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

    /// Tự động phát hiện danh sách các WSL Distro đã cài đặt trên máy Windows.
    /// Ưu tiên đọc trực tiếp từ Windows Registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss)
    /// như Zed để đạt tốc độ tức thì (< 1ms), và fallback sang `wsl.exe --list --quiet` nếu key rỗng.
    pub fn detect_distros() -> Vec<String> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(distros) = Self::detect_distros_via_registry() {
                if !distros.is_empty() {
                    return distros;
                }
            }
        }

        Self::detect_distros_via_cmd()
    }

    /// Đọc danh sách distro trực tiếp từ Windows Registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss)
    #[cfg(target_os = "windows")]
    pub fn detect_distros_via_registry() -> Result<Vec<String>> {
        use windows_registry::CURRENT_USER;

        let lxss_key = CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Lxss")
            .context("failed to open Lxss registry key")?;

        let mut distros = Vec::new();
        if let Ok(keys) = lxss_key.keys() {
            for key in keys {
                if let Ok(sub) = lxss_key.open(&key) {
                    if let Ok(name) = sub.get_string("DistributionName") {
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            distros.push(trimmed.to_string());
                        }
                    }
                }
            }
        }

        Ok(distros)
    }

    /// Fallback phát hiện danh sách Distro qua lệnh `wsl.exe --list --quiet`
    pub fn detect_distros_via_cmd() -> Vec<String> {
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

    /// Kiểm tra kiến trúc CPU của Distro (x86_64 hoặc aarch64) qua `uname -sm`
    pub async fn detect_arch(&self) -> Result<WslArch> {
        let output = new_wsl_cmd()
            .arg("-d")
            .arg(&self.distro)
            .arg("--cd")
            .arg("~")
            .arg("--")
            .arg("uname")
            .arg("-sm")
            .output()
            .await
            .context("failed to run uname -sm in WSL")?;

        if !output.status.success() {
            anyhow::bail!(
                "uname -sm failed in WSL distro '{}': {}",
                self.distro,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let text = String::from_utf8_lossy(&output.stdout);
        parse_wsl_arch(&text)
    }

    /// Phát hiện remote shell của người dùng trong WSL (bash, zsh, sh...)
    pub async fn detect_shell(&self) -> String {
        let out = new_wsl_cmd()
            .arg("-d")
            .arg(&self.distro)
            .arg("--cd")
            .arg("~")
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo $SHELL")
            .output()
            .await;

        if let Ok(o) = out {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !s.is_empty() {
                    return s;
                }
            }
        }
        "sh".to_string()
    }

    /// Kiểm tra tính năng WSL Interop có được bật hay không
    pub async fn detect_has_wsl_interop(&self) -> bool {
        let check = new_wsl_cmd()
            .arg("-d")
            .arg(&self.distro)
            .arg("--cd")
            .arg("~")
            .arg("--")
            .arg("cat")
            .arg("/proc/sys/fs/binfmt_misc/WSLInterop")
            .output()
            .await;

        if let Ok(out) = check {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if s.contains("enabled") {
                    return true;
                }
            }
        }

        let check_late = new_wsl_cmd()
            .arg("-d")
            .arg(&self.distro)
            .arg("--cd")
            .arg("~")
            .arg("--")
            .arg("cat")
            .arg("/proc/sys/fs/binfmt_misc/WSLInterop-late")
            .output()
            .await;

        if let Ok(out) = check_late {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                return s.contains("enabled");
            }
        }

        false
    }

    /// Xác định thư mục HOME mặc định của user trong WSL distro (ví dụ: `/home/truongviet/`).
    /// Nếu không xác định được, trả về `/home/`.
    pub fn resolve_home_dir(distro: &str) -> String {
        #[cfg(target_os = "windows")]
        {
            let home_unc = format!(r"\\wsl.localhost\{}\home", distro);
            if let Ok(entries) = std::fs::read_dir(&home_unc) {
                let mut users = Vec::new();
                for entry in entries.flatten() {
                    if let Ok(ft) = entry.file_type() {
                        if ft.is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !name.starts_with('.') {
                                users.push(name);
                            }
                        }
                    }
                }
                if let Some(first_user) = users
                    .iter()
                    .find(|u| *u != "root" && *u != "ubuntu")
                    .or_else(|| users.first())
                {
                    return format!("/home/{}/", first_user);
                }
            }
        }

        // Fallback: Gọi `wsl.exe -d <distro> -- sh -c "echo $HOME"`
        let out = new_std_command("wsl.exe")
            .arg("-d")
            .arg(distro)
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo $HOME")
            .output();

        if let Ok(output) = out {
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

        "/home/".to_string()
    }

    /// Liệt kê danh sách các thư mục con trong một đường dẫn WSL (chỉ lấy directory, bỏ file lẻ).
    /// Ưu tiên đọc qua Windows UNC path (< 1ms), và fallback sang lệnh `wsl.exe -- ls -1ap` nếu cần.
    pub fn list_remote_directories(distro: &str, unix_path: &str) -> Result<Vec<String>> {
        let clean_path = unix_path.trim();
        let clean_path = if clean_path.is_empty() {
            "/"
        } else {
            clean_path
        };

        #[cfg(target_os = "windows")]
        {
            let relative = clean_path.trim_start_matches('/');
            let unc_candidates = [
                format!(
                    r"\\wsl.localhost\{}\{}",
                    distro,
                    relative.replace('/', "\\")
                ),
                format!(r"\\wsl$\{}\{}", distro, relative.replace('/', "\\")),
            ];

            for unc in &unc_candidates {
                let p = Path::new(unc);
                if p.exists() {
                    if let Ok(entries) = std::fs::read_dir(p) {
                        let mut dirs = Vec::new();
                        for entry in entries.flatten() {
                            if let Ok(ft) = entry.file_type() {
                                if ft.is_dir() {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if name != "." && name != ".." {
                                        dirs.push(name);
                                    }
                                }
                            }
                        }
                        dirs.sort_by_key(|a| a.to_lowercase());
                        return Ok(dirs);
                    }
                }
            }
        }

        // Fallback: Gọi wsl.exe -d <distro> -- ls -1ap <clean_path>
        let output = new_std_command("wsl.exe")
            .arg("-d")
            .arg(distro)
            .arg("--")
            .arg("ls")
            .arg("-1ap")
            .arg(clean_path)
            .output()
            .context("failed to execute ls in WSL")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Cannot list directory: {}", err.trim());
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

    /// Tìm đường dẫn binary local của uwu-agent trên host Windows phù hợp với kiến trúc của Distro.
    /// Hỗ trợ cả binary build sẵn đóng gói cùng installer ({app}\agents\),
    /// cũng như các target biên dịch nội bộ (cross-compile musl / gnu).
    pub fn find_local_agent_binary(arch: WslArch) -> Option<PathBuf> {
        let arch_str = arch.as_str(); // "x86_64" hoặc "aarch64"
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        // Danh sách tên file ưu tiên theo kiến trúc
        let arch_file_names = [
            format!("uwu-agent-{}", arch_str),
            format!("uwu-agent-{}-unknown-linux-musl", arch_str),
            format!("uwu-agent-{}-unknown-linux-gnu", arch_str),
            "uwu-agent".to_string(),
        ];

        // 1. Kiểm tra trong thư mục cài đặt của ứng dụng (bundled với file cài Windows)
        if let Some(ref dir) = exe_dir {
            let bundle_subdirs = [
                dir.join("agents"),
                dir.join("resources").join("agents"),
                dir.join("resources"),
                dir.clone(),
            ];

            for sub in &bundle_subdirs {
                for file_name in &arch_file_names {
                    let candidate = sub.join(file_name);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }

        // 2. Kiểm tra trong môi trường phát triển Cargo workspace (target/...)
        let cargo_candidates = [
            // Target cross-compile theo kiến trúc
            format!("target/{}-unknown-linux-musl/release/uwu-agent", arch_str),
            format!("target/{}-unknown-linux-gnu/release/uwu-agent", arch_str),
            format!("target/{}-unknown-linux-musl/debug/uwu-agent", arch_str),
            format!(
                "../target/{}-unknown-linux-musl/release/uwu-agent",
                arch_str
            ),
            format!("../target/{}-unknown-linux-gnu/release/uwu-agent", arch_str),
            // Fallback bản release/debug thông thường
            "target/release/uwu-agent".to_string(),
            "target/debug/uwu-agent".to_string(),
            "../target/release/uwu-agent".to_string(),
            "../target/debug/uwu-agent".to_string(),
        ];

        for rel in &cargo_candidates {
            let p = Path::new(rel);
            if p.is_file() {
                if let Ok(canonical) = p.canonicalize() {
                    return Some(canonical);
                }
                return Some(p.to_path_buf());
            }
        }

        // 3. Fallback: Nếu không tìm thấy Linux binary, kiểm tra binary Windows .exe
        // (chỉ hoạt động nếu WSL interop được bật)
        if let Some(ref dir) = exe_dir {
            let win_candidate = dir.join("uwu-agent.exe");
            if win_candidate.is_file() {
                return Some(win_candidate);
            }
        }

        let win_candidates = [
            "target/release/uwu-agent.exe",
            "target/debug/uwu-agent.exe",
            "../target/release/uwu-agent.exe",
            "../target/debug/uwu-agent.exe",
        ];
        for rel in &win_candidates {
            let p = Path::new(rel);
            if p.is_file() {
                if let Ok(canonical) = p.canonicalize() {
                    return Some(canonical);
                }
                return Some(p.to_path_buf());
            }
        }

        None
    }

    /// Đảm bảo binary agent có đúng version và đúng kiến trúc tồn tại và thực thi được trong WSL
    pub async fn ensure_server_binary(&self) -> Result<String> {
        if let Some(custom) = &self.agent_cmd {
            return Ok(custom.clone());
        }

        // 1. Nhận diện kiến trúc Distro (x86_64 hay aarch64)
        let arch = self.detect_arch().await.unwrap_or_else(|e| {
            log::warn!(
                "Failed to detect WSL arch for distro '{}': {:#}. Defaulting to x86_64",
                self.distro,
                e
            );
            WslArch::X86_64
        });
        log::info!(
            "Detected WSL distro '{}' architecture: {}",
            self.distro,
            arch
        );

        let version = env!("CARGO_PKG_VERSION");
        let binary_name = format!("uwu-agent-{}-{}", arch.as_str(), version);
        let remote_binary_path = format!("\"$HOME/.local/share/uwu/server_state/{}\"", binary_name);

        // 2. Kiểm tra xem binary đã tồn tại và chạy được chưa
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
                        "WSL Remote Agent verified at {} (Arch: {}, Version: {})",
                        remote_binary_path,
                        arch,
                        version
                    );
                    return Ok(remote_binary_path);
                }
            }
        }

        log::info!(
            "WSL Remote Agent missing or outdated on distro '{}' (Arch: {}). Provisioning...",
            self.distro,
            arch
        );

        // 3. Tìm binary local trên Windows host theo đúng arch để upload
        if let Some(local_path) = Self::find_local_agent_binary(arch) {
            log::info!(
                "Provisioning WSL Remote Agent from local path: {}",
                local_path.display()
            );

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

            // 4. Kiểm tra xác minh lại sau khi upload
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
    fn test_wsl_arch_display() {
        assert_eq!(WslArch::X86_64.as_str(), "x86_64");
        assert_eq!(WslArch::Aarch64.as_str(), "aarch64");
        assert_eq!(format!("{}", WslArch::X86_64), "x86_64");
        assert_eq!(format!("{}", WslArch::Aarch64), "aarch64");
    }

    #[test]
    fn test_parse_wsl_arch() {
        assert_eq!(parse_wsl_arch("Linux x86_64\n").unwrap(), WslArch::X86_64);
        assert_eq!(parse_wsl_arch("Linux x86_64").unwrap(), WslArch::X86_64);
        assert_eq!(parse_wsl_arch("Linux amd64").unwrap(), WslArch::X86_64);
        assert_eq!(parse_wsl_arch("x86_64").unwrap(), WslArch::X86_64);

        assert_eq!(parse_wsl_arch("Linux aarch64\n").unwrap(), WslArch::Aarch64);
        assert_eq!(parse_wsl_arch("Linux arm64").unwrap(), WslArch::Aarch64);
        assert_eq!(parse_wsl_arch("Linux armv8l").unwrap(), WslArch::Aarch64);
        assert_eq!(
            parse_wsl_arch("some banner\nLinux aarch64").unwrap(),
            WslArch::Aarch64
        );

        assert!(parse_wsl_arch("Linux armv7l").is_err());
        assert!(parse_wsl_arch("unknown_arch").is_err());
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

    #[test]
    #[cfg(target_os = "windows")]
    fn test_detect_distros_via_registry() {
        // Không panic khi đọc Registry trên Windows
        let res = WslTransport::detect_distros_via_registry();
        assert!(res.is_ok());
        if let Ok(distros) = res {
            println!("Detected WSL distros via registry: {:?}", distros);
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_resolve_home_dir_and_list_remote_dirs() {
        let distros = WslTransport::detect_distros();
        if let Some(distro) = distros.first() {
            let home = WslTransport::resolve_home_dir(distro);
            assert!(home.starts_with('/'));
            println!("Resolved home for {}: {}", distro, home);

            let dirs = WslTransport::list_remote_directories(distro, &home);
            assert!(dirs.is_ok());
            if let Ok(list) = dirs {
                println!(
                    "Listed directories in {} ({}): {:?}",
                    home,
                    list.len(),
                    list
                );
            }
        }
    }
}
