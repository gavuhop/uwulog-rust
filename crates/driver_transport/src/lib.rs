pub mod process;
pub mod ssh;
pub mod wsl;

use anyhow::{Context, Result};
use async_trait::async_trait;
pub use process::ProcessTransport;
pub use ssh::SshTransport;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
pub use wsl::{WslArch, WslTransport};

pub type BoxedRead = Box<dyn AsyncRead + Send + Unpin>;
pub type BoxedWrite = Box<dyn AsyncWrite + Send + Unpin>;

/// RemoteTransport: Trừu tượng hóa cách kết nối và khởi chạy proxy trên môi trường từ xa (WSL / SSH / Process / Docker)
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
