use crate::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use uwu_schema::{RawLogEntry, RawPayload};

pub struct FileSource {
    path: PathBuf,
    source_id: String,
}

impl FileSource {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let p = path.as_ref().to_path_buf();
        let source_id = format!("file:{}", p.display());
        Self { path: p, source_id }
    }
}

#[async_trait]
impl LogSource for FileSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        let path = self.path.clone();
        let source_id = self.source_id.clone();

        tokio::spawn(async move {
            if let Err(e) = run_file_tailer(path, source_id, tx).await {
                log::error!("FileSource error: {:?}", e);
            }
        });

        Ok(())
    }
}

async fn run_file_tailer(
    path: PathBuf,
    source_id: String,
    tx: mpsc::Sender<RawLogEntry>,
) -> Result<()> {
    let file = File::open(&path)
        .await
        .with_context(|| format!("Failed to open log file: {}", path.display()))?;

    let mut reader = BufReader::new(file);
    let mut line = String::new();

    // 1. Đọc tất cả các dòng log có sẵn hiện tại
    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim_end().to_string();
        line.clear();
        if !trimmed.is_empty() {
            let entry = RawLogEntry {
                source_id: source_id.clone(),
                payload: RawPayload::Text(trimmed),
            };
            if tx.send(entry).await.is_err() {
                return Ok(()); // Channel closed
            }
        }
    }

    // 2. Lắng nghe sự kiện file thay đổi (notify watcher)
    let (sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<()>(100);
    let mut watcher = RecommendedWatcher::new(
        move |res: std::result::Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_)) {
                    let _ = sync_tx.try_send(());
                }
            }
        },
        Config::default(),
    )?;

    if let Some(parent) = path.parent() {
        watcher.watch(parent, RecursiveMode::NonRecursive)?;
    } else {
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
    }

    // Vòng lặp tail dữ liệu mới bổ sung
    while sync_rx.recv().await.is_some() {
        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim_end().to_string();
            line.clear();
            if !trimmed.is_empty() {
                let entry = RawLogEntry {
                    source_id: source_id.clone(),
                    payload: RawPayload::Text(trimmed),
                };
                if tx.send(entry).await.is_err() {
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}
