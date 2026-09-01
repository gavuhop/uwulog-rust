use super::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::mpsc;
use uwu_core_schema::{RawLogEntry, RawPayload};

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

    // 2. Lắng nghe sự kiện file thay đổi (notify watcher) với bộ lọc đường dẫn chính xác
    let (sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<()>(100);
    let target_path = path.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: std::result::Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Any
                        if event.paths.is_empty()
                            || event.paths.iter().any(|p| {
                                p == &target_path || p.file_name() == target_path.file_name()
                            }) =>
                    {
                        let _ = sync_tx.try_send(());
                    }
                    _ => {}
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

    // Vòng lặp tail dữ liệu mới bổ sung, kết hợp ticker 250ms phòng ngừa watcher bỏ sót sự kiện
    loop {
        tokio::select! {
            res = sync_rx.recv() => {
                if res.is_none() {
                    break;
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {}
        }

        // Kiểm tra log rotation / truncate: nếu file bị thu nhỏ hơn vị trí đọc hiện tại -> seek về 0
        if let Ok(metadata) = tokio::fs::metadata(&path).await {
            let file_len = metadata.len();
            if let Ok(current_pos) = reader.stream_position().await {
                if file_len < current_pos {
                    let _ = reader.seek(SeekFrom::Start(0)).await;
                }
            }
        } else {
            // File có thể vừa bị rename trong quá trình logrotate -> thử mở lại file mới tạo
            if let Ok(new_file) = File::open(&path).await {
                reader = BufReader::new(new_file);
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_file_source_tailing() {
        let temp_file_path =
            std::env::temp_dir().join(format!("uwu_test_tail_{}.log", uuid::Uuid::new_v4()));

        // 1. Ghi 2 dòng log ban đầu
        {
            let mut file = std::fs::File::create(&temp_file_path).unwrap();
            writeln!(file, "Line 1: Initial startup").unwrap();
            writeln!(file, "Line 2: Ready to serve").unwrap();
            file.flush().unwrap();
        }

        let source = FileSource::new(&temp_file_path);
        assert_eq!(source.name(), format!("file:{}", temp_file_path.display()));

        let (tx, mut rx) = mpsc::channel(100);
        source.start_stream(tx).await.unwrap();

        // 2. Nhận 2 dòng đầu tiên
        let mut lines = Vec::new();
        for _ in 0..2 {
            if let Ok(Some(entry)) = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
                if let RawPayload::Text(t) = entry.payload {
                    lines.push(t);
                }
            }
        }

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Line 1"));
        assert!(lines[1].contains("Line 2"));

        // Dọn dẹp file tạm
        let _ = std::fs::remove_file(&temp_file_path);
    }

    #[tokio::test]
    async fn test_file_source_truncation_rotation() {
        let temp_file_path =
            std::env::temp_dir().join(format!("uwu_test_trunc_{}.log", uuid::Uuid::new_v4()));

        // 1. Tạo file và ghi dữ liệu ban đầu
        {
            let mut file = std::fs::File::create(&temp_file_path).unwrap();
            writeln!(file, "Initial large line 1").unwrap();
            writeln!(file, "Initial large line 2").unwrap();
            file.flush().unwrap();
        }

        let source = FileSource::new(&temp_file_path);
        let (tx, mut rx) = mpsc::channel(100);
        source.start_stream(tx).await.unwrap();

        // Nhận 2 dòng đầu
        let mut lines = Vec::new();
        for _ in 0..2 {
            if let Ok(Some(entry)) = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                if let RawPayload::Text(t) = entry.payload {
                    lines.push(t);
                }
            }
        }
        assert_eq!(lines.len(), 2);

        // 2. Truncate file và ghi dòng mới ngắn hơn
        {
            let mut file = std::fs::File::create(&temp_file_path).unwrap(); // create truncates file to 0
            writeln!(file, "Rotated new log").unwrap();
            file.flush().unwrap();
        }

        // 3. Tailer phải phát hiện file_len < current_pos và seek(0) để đọc được dòng mới
        let mut rotated_lines = Vec::new();
        if let Ok(Some(entry)) = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            if let RawPayload::Text(t) = entry.payload {
                rotated_lines.push(t);
            }
        }

        assert_eq!(rotated_lines.len(), 1);
        assert!(rotated_lines[0].contains("Rotated new log"));

        let _ = std::fs::remove_file(&temp_file_path);
    }
}
