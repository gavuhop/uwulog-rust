use super::wsl::{parse_wsl_arch, WslArch, WslTransport};
use super::{wrap_child_stdio, BoxedRead, BoxedWrite, RemoteTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use uwu_core_util::command::{new_std_command, new_tokio_command};

const DOCKER_PS_FORMAT: &str = "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Label \"devcontainer.local_folder\"}}\t{{.Label \"devcontainer.config_file\"}}\t{{.Label \"vsch.local.folder\"}}\t{{.Label \"com.docker.compose.project\"}}\t{{.Label \"com.docker.compose.service\"}}\t{{.Labels}}";

/// Thông tin một Docker container đang chạy (bao gồm metadata Dev Container nếu có)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DockerContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub local_folder: Option<String>,
    pub config_file: Option<String>,
    pub compose_project: Option<String>,
    pub compose_service: Option<String>,
    pub is_devcontainer: bool,
}

impl DockerContainerInfo {
    /// Tên thư mục dự án gốc rút gọn từ `local_folder` (nếu là Dev Container)
    pub fn local_folder_name(&self) -> Option<&str> {
        let folder = self.local_folder.as_deref()?;
        let trimmed = folder.trim_end_matches(['/', '\\']);
        trimmed.rsplit(['/', '\\']).find(|seg| !seg.is_empty())
    }

    /// Tên ngắn gọn ưu tiên service Compose nếu có (vd: `app`, `db`, `frontend`)
    pub fn short_label(&self) -> &str {
        self.compose_service
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.name)
    }

    /// Tên project rút gọn từ `local_folder` hoặc `compose_project` (bỏ hậu tố `_devcontainer` / `-devcontainer`)
    pub fn project_label(&self) -> Option<String> {
        if let Some(folder) = self.local_folder_name() {
            return Some(folder.to_string());
        }
        let proj = self.compose_project.as_deref()?.trim();
        if proj.is_empty() {
            return None;
        }
        let cleaned = proj
            .strip_suffix("_devcontainer")
            .or_else(|| proj.strip_suffix("-devcontainer"))
            .unwrap_or(proj);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned.to_string())
        }
    }
}

/// Transport kết nối tới Docker / Dev Container thông qua `docker exec` (tham chiếu kiến trúc Zed `DockerExecConnection`)
#[derive(Debug, Clone)]
pub struct DockerTransport {
    container: String,
    remote_user: Option<String>,
    working_dir: Option<String>,
    agent_cmd: Option<String>,
    name: String,
}

impl DockerTransport {
    pub fn new(container: impl Into<String>) -> Self {
        let container = container.into();
        let name = format!("Docker ({})", container);
        Self {
            container,
            remote_user: None,
            working_dir: None,
            agent_cmd: None,
            name,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        let u = user.into();
        if !u.trim().is_empty() {
            self.remote_user = Some(u);
        }
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        let d = dir.into();
        if !d.trim().is_empty() {
            self.working_dir = Some(d);
        }
        self
    }

    pub fn with_agent_cmd(mut self, cmd: impl Into<String>) -> Self {
        self.agent_cmd = Some(cmd.into());
        self
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    pub fn remote_user(&self) -> Option<&str> {
        self.remote_user.as_deref()
    }

    pub fn working_dir(&self) -> Option<&str> {
        self.working_dir.as_deref()
    }

    fn build_exec_tokio(&self, interactive: bool, with_workdir: bool) -> tokio::process::Command {
        let mut cmd = new_tokio_command("docker");
        cmd.arg("exec");
        if interactive {
            cmd.arg("-i");
        }
        if let Some(ref user) = self.remote_user {
            if !user.trim().is_empty() {
                cmd.arg("-u").arg(user.trim());
            }
        }
        if with_workdir {
            if let Some(ref dir) = self.working_dir {
                if !dir.trim().is_empty() {
                    cmd.arg("-w").arg(dir.trim());
                }
            }
        }
        cmd.arg(&self.container);
        cmd
    }

    fn build_exec_std(container: &str, remote_user: Option<&str>) -> std::process::Command {
        let mut cmd = new_std_command("docker");
        cmd.arg("exec");
        if let Some(user) = remote_user {
            if !user.trim().is_empty() {
                cmd.arg("-u").arg(user.trim());
            }
        }
        cmd.arg(container);
        cmd
    }

    /// Liệt kê tất cả Docker container đang chạy (gắn cờ `is_devcontainer` nếu có label `devcontainer.*` / `vsch.*`)
    pub async fn list_running_containers() -> Result<Vec<DockerContainerInfo>> {
        let output = new_tokio_command("docker")
            .arg("ps")
            .arg("--format")
            .arg(DOCKER_PS_FORMAT)
            .stdin(Stdio::null())
            .output()
            .await
            .context(
                "Failed to execute `docker ps`. Make sure Docker CLI is installed and in PATH.",
            )?;
        parse_ps_command_output(output)
    }

    /// Phiên bản đồng bộ của `list_running_containers` (dùng cho CLI hoặc thread nền riêng)
    pub fn list_running_containers_sync() -> Result<Vec<DockerContainerInfo>> {
        let output = new_std_command("docker")
            .arg("ps")
            .arg("--format")
            .arg(DOCKER_PS_FORMAT)
            .stdin(Stdio::null())
            .output()
            .context(
                "Failed to execute `docker ps`. Make sure Docker CLI is installed and in PATH.",
            )?;
        parse_ps_command_output(output)
    }

    /// Tìm đường dẫn mount bên trong container ứng với thư mục `local_folder` trên máy host
    pub fn resolve_mount_destination(container: &str, local_folder: &str) -> Option<String> {
        let target_norm = normalize_mount_path(local_folder);
        if target_norm.is_empty() {
            return None;
        }

        let output = new_std_command("docker")
            .arg("inspect")
            .arg("--format")
            .arg("{{range .Mounts}}{{.Source}}\t{{.Destination}}\n{{end}}")
            .arg(container)
            .stdin(Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let Some((src, dst)) = line.split_once('\t') else {
                continue;
            };
            if normalize_mount_path(src) == target_norm {
                let dst_clean = dst.trim();
                if dst_clean.starts_with('/') {
                    return Some(ensure_trailing_slash(dst_clean.to_string()));
                }
            }
        }
        None
    }

    /// Kiểm tra xem một thư mục có tồn tại bên trong container hay không
    pub fn directory_exists(container: &str, path: &str, remote_user: Option<&str>) -> bool {
        Self::build_exec_std(container, remote_user)
            .arg("test")
            .arg("-d")
            .arg(path)
            .stdin(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Xác định thư mục làm việc mặc định trong container (ưu tiên `Config.WorkingDir`, sau đó `$HOME`, cuối cùng `/`)
    pub fn resolve_default_workdir(container: &str, remote_user: Option<&str>) -> String {
        if let Ok(output) = new_std_command("docker")
            .arg("inspect")
            .arg("--format")
            .arg("{{.Config.WorkingDir}}")
            .arg(container)
            .stdin(Stdio::null())
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.starts_with('/') && path != "/" {
                    return ensure_trailing_slash(path);
                }
            }
        }

        if let Ok(output) = Self::build_exec_std(container, remote_user)
            .arg("sh")
            .arg("-c")
            .arg("echo \"$HOME\"")
            .stdin(Stdio::null())
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.starts_with('/') {
                    return ensure_trailing_slash(path);
                }
            }
        }

        "/".to_string()
    }

    /// Liệt kê các thư mục con trong một đường dẫn bên trong container
    pub fn list_remote_directories(
        container: &str,
        unix_path: &str,
        remote_user: Option<&str>,
    ) -> Result<Vec<String>> {
        let clean_path = unix_path.trim();
        let mut cmd = Self::build_exec_std(container, remote_user);
        cmd.stdin(Stdio::null());

        if clean_path.is_empty() || clean_path == "~" || clean_path == "~/" {
            cmd.arg("sh").arg("-c").arg("ls -1ap \"${HOME:-/}\"");
        } else {
            cmd.arg("ls").arg("-1ap").arg(clean_path);
        }

        let output = cmd
            .output()
            .context("Failed to execute `docker exec ... ls`")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Cannot list container directory: {}", err.trim());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        Ok(super::parse_ls_directories(&text))
    }

    /// Phát hiện kiến trúc CPU trong container (`uname -sm`)
    pub async fn detect_arch(&self) -> Result<WslArch> {
        let output = self
            .build_exec_tokio(false, false)
            .arg("uname")
            .arg("-sm")
            .stdin(Stdio::null())
            .output()
            .await
            .context("Failed to run `uname -sm` inside Docker container")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("`uname -sm` inside container failed: {}", err.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_wsl_arch(&stdout)
    }

    /// Xác định thư mục ghi được trong container để lưu binary `uwu-agent`
    async fn resolve_remote_agent_dir(&self) -> Result<String> {
        let output = self
            .build_exec_tokio(false, false)
            .arg("sh")
            .arg("-c")
            .arg(
                "dir=\"${HOME:-/tmp}/.local/share/uwu/server_state\"; \
                 if mkdir -p \"$dir\" 2>/dev/null && [ -w \"$dir\" ]; then \
                   printf '%s' \"$dir\"; \
                 else \
                   dir=\"/tmp/uwu-server_state-$(id -u)\"; \
                   mkdir -p \"$dir\" && [ -w \"$dir\" ] && printf '%s' \"$dir\"; \
                 fi",
            )
            .stdin(Stdio::null())
            .output()
            .await
            .context("Failed to prepare agent directory inside Docker container")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Cannot create agent directory in container '{}': {}",
                self.container,
                err.trim()
            );
        }

        let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if dir.is_empty() {
            anyhow::bail!(
                "Cannot resolve writable agent directory in container '{}'",
                self.container
            );
        }
        Ok(dir)
    }

    async fn verify_remote_binary(
        &self,
        remote_binary_path: &str,
        version: &str,
    ) -> std::result::Result<(), String> {
        let output = self
            .build_exec_tokio(false, false)
            .arg(remote_binary_path)
            .arg("version")
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| format!("failed to spawn verify command: {}", e))?;

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        if output.status.success() && stdout_str.contains(&format!("uwu-agent {}", version)) {
            return Ok(());
        }

        let detail = if !stderr_str.trim().is_empty() {
            stderr_str.trim().to_string()
        } else if !stdout_str.trim().is_empty() {
            format!("unexpected output: {}", stdout_str.trim())
        } else {
            format!("exit status {}", output.status)
        };
        Err(detail)
    }

    async fn upload_via_docker_cp(
        &self,
        local_path: &std::path::Path,
        remote_binary_path: &str,
    ) -> std::result::Result<(), String> {
        let dest = format!("{}:{}", self.container, remote_binary_path);
        let cp_out = new_tokio_command("docker")
            .arg("cp")
            .arg(local_path)
            .arg(&dest)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| format!("docker cp spawn error: {}", e))?;

        if !cp_out.status.success() {
            let err = String::from_utf8_lossy(&cp_out.stderr);
            return Err(format!("docker cp failed: {}", err.trim()));
        }

        let chmod_out = self
            .build_exec_tokio(false, false)
            .arg("chmod")
            .arg("755")
            .arg(remote_binary_path)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| format!("chmod spawn error: {}", e))?;

        if !chmod_out.status.success() {
            let err = String::from_utf8_lossy(&chmod_out.stderr);
            return Err(format!("chmod 755 failed: {}", err.trim()));
        }

        Ok(())
    }

    async fn upload_via_exec_stream(
        &self,
        local_path: &std::path::Path,
        remote_binary_path: &str,
    ) -> std::result::Result<(), String> {
        let bytes = tokio::fs::read(local_path)
            .await
            .map_err(|e| format!("read local agent failed: {}", e))?;

        let tmp_path = format!("{}.tmp.{}", remote_binary_path, std::process::id());
        let escaped_target = escape_single_quotes(remote_binary_path);
        let escaped_tmp = escape_single_quotes(&tmp_path);
        let stream_cmd = format!(
            "rm -f '{}' '{}' && cat > '{}' && chmod 755 '{}' && mv -f '{}' '{}'",
            escaped_target, escaped_tmp, escaped_tmp, escaped_tmp, escaped_tmp, escaped_target
        );

        let mut child = self
            .build_exec_tokio(true, false)
            .arg("sh")
            .arg("-c")
            .arg(&stream_cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("docker exec stream spawn failed: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(&bytes).await {
                return Err(format!("docker exec stdin write failed: {}", e));
            }
            if let Err(e) = stdin.flush().await {
                return Err(format!("docker exec stdin flush failed: {}", e));
            }
            drop(stdin);
        }

        let out = child
            .wait_with_output()
            .await
            .map_err(|e| format!("docker exec stream wait failed: {}", e))?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!("docker exec stream failed: {}", err.trim()));
        }

        Ok(())
    }

    /// Đảm bảo `uwu-agent` tồn tại, đúng version và thực thi được trong container
    /// (Ưu tiên `docker cp`, nếu chmod/uid không khớp hoặc lỗi thì stream qua `docker exec -i` dưới quyền container user)
    pub async fn ensure_server_binary(&self) -> Result<String> {
        if let Some(ref custom) = self.agent_cmd {
            return Ok(custom.clone());
        }

        let arch = self.detect_arch().await.unwrap_or_else(|e| {
            log::warn!(
                "Failed to detect container '{}' architecture: {:#}. Defaulting to x86_64",
                self.container,
                e
            );
            WslArch::X86_64
        });
        log::info!(
            "Detected Docker container '{}' architecture: {}",
            self.container,
            arch
        );

        let version = env!("CARGO_PKG_VERSION");
        let binary_name = format!("uwu-agent-{}-{}", arch.as_str(), version);
        let mut deploy_notes: Vec<String> = Vec::new();

        match self.resolve_remote_agent_dir().await {
            Ok(remote_dir) => {
                let remote_binary_path = format!("{}/{}", remote_dir, binary_name);

                if self
                    .verify_remote_binary(&remote_binary_path, version)
                    .await
                    .is_ok()
                {
                    log::info!(
                        "Docker Remote Agent verified at {} (Arch: {}, Version: {})",
                        remote_binary_path,
                        arch,
                        version
                    );
                    return Ok(remote_binary_path);
                }

                if let Some(local_path) = WslTransport::find_local_agent_binary(arch) {
                    log::info!(
                        "Provisioning Docker Remote Agent from local path: {}",
                        local_path.display()
                    );

                    // Method 1: `docker cp` + `chmod 755`
                    match self
                        .upload_via_docker_cp(&local_path, &remote_binary_path)
                        .await
                    {
                        Ok(()) => {
                            match self
                                .verify_remote_binary(&remote_binary_path, version)
                                .await
                            {
                                Ok(()) => {
                                    log::info!(
                                        "Successfully provisioned and verified Docker Remote Agent at {}",
                                        remote_binary_path
                                    );
                                    return Ok(remote_binary_path);
                                }
                                Err(err) => {
                                    deploy_notes.push(format!(
                                        "after docker cp verify ({}): {}",
                                        local_path.display(),
                                        err
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            deploy_notes.push(err);
                        }
                    }

                    // Method 2: stream qua `docker exec -i` dưới user hiện tại của container
                    match self
                        .upload_via_exec_stream(&local_path, &remote_binary_path)
                        .await
                    {
                        Ok(()) => {
                            match self
                                .verify_remote_binary(&remote_binary_path, version)
                                .await
                            {
                                Ok(()) => {
                                    log::info!(
                                        "Successfully streamed and verified Docker Remote Agent at {}",
                                        remote_binary_path
                                    );
                                    return Ok(remote_binary_path);
                                }
                                Err(err) => {
                                    deploy_notes.push(format!(
                                        "after exec stream verify ({}): {}",
                                        local_path.display(),
                                        err
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            deploy_notes.push(err);
                        }
                    }
                } else {
                    deploy_notes.push(format!(
                        "no local uwu-agent binary found for Linux ({})",
                        arch
                    ));
                }
            }
            Err(err) => {
                deploy_notes.push(format!("{:#}", err));
            }
        }

        // Kiểm tra `uwu-agent` có sẵn trong PATH của container
        if let Ok(output) = self
            .build_exec_tokio(false, false)
            .arg("uwu-agent")
            .arg("version")
            .stdin(Stdio::null())
            .output()
            .await
        {
            if output.status.success() {
                log::info!(
                    "Using system 'uwu-agent' found in PATH in container '{}'",
                    self.container
                );
                return Ok("uwu-agent".to_string());
            }
        }

        let detail = if deploy_notes.is_empty() {
            String::new()
        } else {
            format!(" Details: {}", deploy_notes.join("; "))
        };
        anyhow::bail!(
            "Failed to deploy uwu-agent into Docker container '{}'. Could not provision a compatible Linux ({}) binary and 'uwu-agent' is not installed in container PATH.{}",
            self.container,
            arch,
            detail
        )
    }
}

#[async_trait]
impl RemoteTransport for DockerTransport {
    fn name(&self) -> &str {
        &self.name
    }

    async fn spawn_proxy(&self) -> Result<(BoxedRead, BoxedWrite)> {
        let remote_binary = self.ensure_server_binary().await?;

        let mut cmd = self.build_exec_tokio(true, true);
        cmd.arg(&remote_binary);
        cmd.arg("proxy");
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn uwu-agent proxy in Docker container '{}'",
                self.container
            )
        })?;

        wrap_child_stdio(child, "Docker Agent")
    }
}

fn ensure_trailing_slash(path: String) -> String {
    if path.ends_with('/') {
        path
    } else {
        format!("{}/", path)
    }
}

pub(crate) fn normalize_mount_path(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches(['/', '\\']);
    let stripped = trimmed.strip_prefix(r"\\?\").unwrap_or(trimmed);
    stripped.replace('\\', "/").to_lowercase()
}

fn escape_single_quotes(s: &str) -> String {
    s.replace('\'', "'\"'\"'")
}

fn non_empty_opt(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_ps_command_output(output: std::process::Output) -> Result<Vec<DockerContainerInfo>> {
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let msg = err.trim();
        if msg.is_empty() {
            anyhow::bail!("`docker ps` failed with status {}", output.status);
        }
        anyhow::bail!("Docker error: {}", msg);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_docker_ps_output(&stdout))
}

/// Phân tích output của `docker ps --format DOCKER_PS_FORMAT`
pub(crate) fn parse_docker_ps_output(stdout: &str) -> Vec<DockerContainerInfo> {
    let mut containers = Vec::new();

    for line in stdout.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(10, '\t').collect();
        if parts.len() < 4 {
            continue;
        }

        let id = parts[0].trim().to_string();
        let name = parts[1].trim().to_string();
        if id.is_empty() || name.is_empty() {
            continue;
        }

        let image = parts[2].trim().to_string();
        let status = parts[3].trim().to_string();

        let dev_local_folder = parts.get(4).and_then(|s| non_empty_opt(s));
        let config_file = parts.get(5).and_then(|s| non_empty_opt(s));
        let vsch_local_folder = parts.get(6).and_then(|s| non_empty_opt(s));
        let compose_project = parts.get(7).and_then(|s| non_empty_opt(s));
        let compose_service = parts.get(8).and_then(|s| non_empty_opt(s));
        let all_labels = parts.get(9).copied().unwrap_or("");

        let local_folder = dev_local_folder.or(vsch_local_folder);
        let is_devcontainer = local_folder.is_some()
            || config_file.is_some()
            || all_labels.contains("devcontainer.")
            || all_labels.contains("vsch.");

        containers.push(DockerContainerInfo {
            id,
            name,
            image,
            status,
            local_folder,
            config_file,
            compose_project,
            compose_service,
            is_devcontainer,
        });
    }

    // Ưu tiên Dev Container lên đầu, sau đó sắp xếp theo tên container
    containers.sort_by(|a, b| {
        b.is_devcontainer
            .cmp(&a.is_devcontainer)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    containers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_docker_ps_output_mixed_containers() {
        let sample = "\
a1b2c3d4e5f6\tredis-cache\tredis:7-alpine\tUp 2 hours\t\t\t\t\t\tcom.docker.compose.service=redis
f9e8d7c6b5a4\trust-devcontainer\tmcr.microsoft.com/devcontainers/rust:1\tUp 10 minutes\t/home/user/projects/my-app\t/home/user/projects/my-app/.devcontainer/devcontainer.json\t\tmy-app_devcontainer\tapp\tdevcontainer.local_folder=/home/user/projects/my-app
112233445566\tlegacy-vsch\tnode:20\tUp 1 hour\t\t\tD:\\work\\web\t\t\tvsch.local.folder=D:\\work\\web
778899001122\tmeta-only-dc\tubuntu:24.04\tUp 5 minutes\t\t\t\t\t\tdevcontainer.metadata=[{\"remoteUser\":\"vscode\"}]
";

        let parsed = parse_docker_ps_output(sample);
        assert_eq!(parsed.len(), 4);

        // Dev containers phải được xếp trước container thường
        assert!(parsed[0].is_devcontainer);
        assert!(parsed[1].is_devcontainer);
        assert!(parsed[2].is_devcontainer);
        assert!(!parsed[3].is_devcontainer);
        assert_eq!(parsed[3].name, "redis-cache");

        let rust_dc = parsed
            .iter()
            .find(|c| c.name == "rust-devcontainer")
            .unwrap();
        assert_eq!(
            rust_dc.local_folder.as_deref(),
            Some("/home/user/projects/my-app")
        );
        assert_eq!(rust_dc.local_folder_name(), Some("my-app"));
        assert_eq!(
            rust_dc.compose_project.as_deref(),
            Some("my-app_devcontainer")
        );
        assert_eq!(rust_dc.compose_service.as_deref(), Some("app"));
        assert_eq!(rust_dc.short_label(), "app");
        assert!(rust_dc.config_file.is_some());

        let vsch_dc = parsed.iter().find(|c| c.name == "legacy-vsch").unwrap();
        assert_eq!(vsch_dc.local_folder.as_deref(), Some("D:\\work\\web"));
        assert_eq!(vsch_dc.local_folder_name(), Some("web"));
    }

    #[test]
    fn test_docker_transport_builders() {
        let t = DockerTransport::new("my_container")
            .with_user("vscode")
            .with_working_dir("/workspaces/app")
            .with_agent_cmd("/usr/local/bin/uwu-agent");

        assert_eq!(t.name(), "Docker (my_container)");
        assert_eq!(t.container(), "my_container");
        assert_eq!(t.remote_user(), Some("vscode"));
        assert_eq!(t.working_dir(), Some("/workspaces/app"));
    }
}
