pub mod devcontainer;
pub mod docker;
pub mod ssh;
pub mod ssh_config;
pub mod wsl;

use anyhow::{Context, Result};
use async_trait::async_trait;
pub use devcontainer::{find_devcontainer_config, DevContainerConfig, OpenedDevContainer};
pub use docker::{DockerContainerInfo, DockerTransport};
pub use ssh::{SshConnectionSuccess, SshInteractiveEvent, SshInteractivePromptType, SshTransport};
pub use ssh_config::{load_system_and_user_ssh_hosts, parse_ssh_config_hosts};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
pub use wsl::{WslArch, WslTransport};

pub type BoxedRead = Box<dyn AsyncRead + Send + Unpin>;
pub type BoxedWrite = Box<dyn AsyncWrite + Send + Unpin>;

/// RemoteTransport: Trừu tượng hóa cách kết nối và khởi chạy proxy trên môi trường từ xa (WSL / SSH / Docker)
#[async_trait]
pub trait RemoteTransport: Send + Sync {
    /// Định danh hiển thị của transport (ví dụ "WSL (Ubuntu)", "SSH (root@10.0.0.1)")
    fn name(&self) -> &str;

    /// Khởi chạy proxy process trên môi trường từ xa và trả về cặp reader/writer (stdio stream)
    async fn spawn_proxy(&self) -> Result<(BoxedRead, BoxedWrite)>;
}

pub(crate) fn wrap_child_stdio(
    mut child: tokio::process::Child,
    tag: &str,
) -> Result<(BoxedRead, BoxedWrite)> {
    let stdout = child.stdout.take().context("Failed to open child stdout")?;
    let stdin = child.stdin.take().context("Failed to open child stdin")?;

    if let Some(stderr) = child.stderr.take() {
        let tag = tag.to_string();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                log::warn!("[{} STDERR] {}", tag, line);
            }
        });
    }

    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    Ok((Box::new(stdout), Box::new(stdin)))
}

/// Phân tích output từ `ls -1ap` thành danh sách tên thư mục con đã sắp xếp
pub(crate) fn parse_ls_directories(stdout: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with('/') {
            let folder_name = trimmed.trim_end_matches('/');
            if folder_name != "." && folder_name != ".." && !folder_name.is_empty() {
                dirs.push(folder_name.to_string());
            }
        }
    }
    dirs.sort_by_key(|a| a.to_lowercase());
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ls_directories() {
        let output = "./\n../\nCargo.toml\nsrc/\n.git/\nREADME.md\ntarget/\n";
        assert_eq!(parse_ls_directories(output), vec![".git", "src", "target"]);
    }
}
