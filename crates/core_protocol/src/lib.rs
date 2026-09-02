use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::ErrorKind;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uwu_core_schema::RawLogEntry;

pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024; // 64 MB frame limit

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RemoteLogSourceSpec {
    Command(String),
    File(String),
}

/// ClientEnvelope: Thông điệp điều khiển từ Local Client gửi tới Remote Agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientEnvelope {
    /// Heartbeat Ping định kỳ để kiểm tra liveness
    Ping { seq: u64 },
    /// Yêu cầu Remote Agent bắt đầu thu thập log theo cấu hình
    StartStream {
        spec: RemoteLogSourceSpec,
        filter_pushdown: Option<String>,
    },
    /// Tạm dừng stream log từ xa
    PauseStream,
    /// Tiếp tục stream log
    ResumeStream,
    /// Dừng hoàn toàn tiến trình thu thập log và dọn dẹp
    StopStream,
}

/// ServerEnvelope: Thông điệp phản hồi và luồng dữ liệu từ Remote Agent gửi về Local Client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerEnvelope {
    /// Phản hồi Pong cho Heartbeat Ping
    Pong { seq: u64 },
    /// Batch danh sách các log entry thô vừa thu thập
    LogBatch(Vec<RawLogEntry>),
    /// Báo lỗi từ phía remote (file không tồn tại, permission denied, command crash...)
    Error { message: String },
    /// Tiến trình thu thập kết thúc
    Terminated { exit_code: Option<i32> },
}

/// FramedWriter: Bộ ghi khung dữ liệu chuẩn với tiền tố 4 bytes Big-Endian
pub struct FramedWriter<W> {
    writer: W,
}

impl<W: AsyncWrite + Unpin> FramedWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub async fn send<T: Serialize>(&mut self, item: &T) -> Result<()> {
        let payload = serde_json::to_vec(item).context("Failed to serialize envelope")?;
        let len = payload.len() as u32;
        self.writer
            .write_all(&len.to_be_bytes())
            .await
            .context("Failed to write frame length")?;
        self.writer
            .write_all(&payload)
            .await
            .context("Failed to write frame payload")?;
        self.writer
            .flush()
            .await
            .context("Failed to flush framed writer")?;
        Ok(())
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

/// FramedReader: Bộ đọc khung dữ liệu chuẩn với tiền tố 4 bytes Big-Endian
pub struct FramedReader<R> {
    reader: R,
}

impl<R: AsyncRead + Unpin> FramedReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub async fn next<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        let mut len_buf = [0u8; 4];
        match self.reader.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e).context("Failed to read frame length"),
        }

        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_FRAME_SIZE {
            anyhow::bail!(
                "Frame size {} exceeds limit of {} bytes",
                len,
                MAX_FRAME_SIZE
            );
        }

        let mut payload_buf = vec![0u8; len];
        self.reader
            .read_exact(&mut payload_buf)
            .await
            .context("Failed to read frame payload")?;

        let item: T = serde_json::from_slice(&payload_buf)
            .context("Failed to deserialize envelope payload")?;
        Ok(Some(item))
    }

    pub fn into_inner(self) -> R {
        self.reader
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_core_schema::RawPayload;

    #[tokio::test]
    async fn test_framed_envelope_roundtrip() {
        let (client_io, server_io) = tokio::io::duplex(4096);

        let mut client_writer = FramedWriter::new(client_io);
        let mut server_reader = FramedReader::new(server_io);

        let ping = ClientEnvelope::Ping { seq: 42 };
        client_writer.send(&ping).await.unwrap();

        let received: Option<ClientEnvelope> = server_reader.next().await.unwrap();
        assert_eq!(received, Some(ping));
    }

    #[tokio::test]
    async fn test_log_batch_roundtrip() {
        let (server_io, client_io) = tokio::io::duplex(8192);

        let mut server_writer = FramedWriter::new(server_io);
        let mut client_reader = FramedReader::new(client_io);

        let batch = ServerEnvelope::LogBatch(vec![
            RawLogEntry {
                payload: RawPayload::Text("[INFO] Server started".into()),
            },
            RawLogEntry {
                payload: RawPayload::Text("[ERROR] DB connection timeout".into()),
            },
        ]);

        server_writer.send(&batch).await.unwrap();

        let received: Option<ServerEnvelope> = client_reader.next().await.unwrap();
        match received {
            Some(ServerEnvelope::LogBatch(logs)) => {
                assert_eq!(logs.len(), 2);
            }
            _ => panic!("Expected LogBatch"),
        }
    }
}
