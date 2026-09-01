use super::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use uwu_core_protocol::{
    ClientEnvelope, FramedReader, FramedWriter, RemoteLogSourceSpec, ServerEnvelope,
};
use uwu_core_schema::{RawLogEntry, RawPayload};
use uwu_driver_transport::RemoteTransport;

/// RemoteSource: Driver nguồn đa năng kết nối tới Headless Remote Agent qua bất kỳ RemoteTransport nào (WSL, SSH, Docker)
pub struct RemoteSource {
    transport: Box<dyn RemoteTransport>,
    spec: RemoteLogSourceSpec,
    filter_pushdown: Option<String>,
    source_id: String,
}

impl RemoteSource {
    pub fn new(
        transport: Box<dyn RemoteTransport>,
        spec: RemoteLogSourceSpec,
        filter_pushdown: Option<String>,
    ) -> Self {
        let source_id = match &spec {
            RemoteLogSourceSpec::Command(cmd) => {
                format!("{}:proc:{}", transport.name(), cmd)
            }
            RemoteLogSourceSpec::File(path) => {
                format!("{}:file:{}", transport.name(), path)
            }
            RemoteLogSourceSpec::Journald(Some(u)) => {
                format!("{}:journald:{}", transport.name(), u)
            }
            RemoteLogSourceSpec::Journald(None) => {
                format!("{}:journald:system", transport.name())
            }
        };

        Self {
            transport,
            spec,
            filter_pushdown,
            source_id,
        }
    }

    pub fn transport_name(&self) -> &str {
        self.transport.name()
    }

    pub fn spec(&self) -> &RemoteLogSourceSpec {
        &self.spec
    }
}

#[async_trait]
impl LogSource for RemoteSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        let (reader, writer) = self
            .transport
            .spawn_proxy()
            .await
            .context("Failed to spawn remote proxy transport")?;

        let mut framed_reader = FramedReader::new(reader);
        let framed_writer = Arc::new(Mutex::new(FramedWriter::new(writer)));

        // 1. Gửi lệnh bắt đầu stream log cho Remote Agent
        {
            let mut writer_guard = framed_writer.lock().await;
            writer_guard
                .send(&ClientEnvelope::StartStream {
                    spec: self.spec.clone(),
                    filter_pushdown: self.filter_pushdown.clone(),
                })
                .await
                .context("Failed to send StartStream envelope to remote agent")?;
        }

        // 2. Heartbeat Task: Gửi Ping định kỳ 5 giây để duy trì kết nối và phát hiện đứt gãy
        let heartbeat_writer = Arc::clone(&framed_writer);
        let heartbeat_handle = tokio::spawn(async move {
            let mut seq = 0u64;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                seq += 1;
                let ping = ClientEnvelope::Ping { seq };
                let mut guard = heartbeat_writer.lock().await;
                if guard.send(&ping).await.is_err() {
                    break;
                }
            }
        });

        // 3. Vòng lặp nhận dữ liệu (Ingestion Loop)
        let source_id = self.source_id.clone();
        tokio::spawn(async move {
            while let Ok(Some(envelope)) = framed_reader.next::<ServerEnvelope>().await {
                match envelope {
                    ServerEnvelope::LogBatch(batch) => {
                        for entry in batch {
                            if tx.send(entry).await.is_err() {
                                break;
                            }
                        }
                    }
                    ServerEnvelope::Pong { .. } => {
                        // Heartbeat phản hồi hợp lệ
                    }
                    ServerEnvelope::Error { message } => {
                        let err_entry = RawLogEntry {
                            source_id: format!("{}:error", source_id),
                            payload: RawPayload::Text(format!("[REMOTE ERROR] {}", message)),
                        };
                        let _ = tx.send(err_entry).await;
                    }
                    ServerEnvelope::Terminated { .. } => {
                        break;
                    }
                }
            }

            heartbeat_handle.abort();
            // Gửi tín hiệu StopStream dọn dẹp trước khi đóng
            let mut guard = framed_writer.lock().await;
            let _ = guard.send(&ClientEnvelope::StopStream).await;
        });

        Ok(())
    }
}
