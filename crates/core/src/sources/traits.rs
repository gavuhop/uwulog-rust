use crate::schema::RawLogEntry;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Trait định nghĩa giao diện chung cho mọi nguồn log (File, WinEvent, Journald...)
#[async_trait]
pub trait LogSource: Send + Sync {
    /// Định danh hiển thị của nguồn log
    fn name(&self) -> &str;

    /// Bắt đầu stream dữ liệu bất đồng bộ vào channel sender
    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()>;
}
