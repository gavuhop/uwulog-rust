pub mod process;
pub mod ssh;
pub mod wsl;

use anyhow::Result;
use async_trait::async_trait;
pub use process::ProcessTransport;
pub use ssh::SshTransport;
use tokio::io::{AsyncRead, AsyncWrite};
pub use wsl::WslTransport;

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
