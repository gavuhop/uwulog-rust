use crate::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use uwu_schema::{RawLogEntry, RawPayload};

pub struct ProcessSource {
    command: String,
    args: Vec<String>,
    source_id: String,
}

impl ProcessSource {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        let cmd = command.into();
        let source_id = if args.is_empty() {
            format!("proc:{}", cmd)
        } else {
            format!("proc:{} {}", cmd, args.join(" "))
        };

        Self {
            command: cmd,
            args,
            source_id,
        }
    }
}

#[async_trait]
impl LogSource for ProcessSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn process: {} {:?}", self.command, self.args))?;

        let source_id_out = self.source_id.clone();
        let source_id_err = format!("{}:stderr", self.source_id);
        let tx_out = tx.clone();
        let tx_err = tx.clone();

        // Task đọc stdout
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let entry = RawLogEntry {
                        source_id: source_id_out.clone(),
                        payload: RawPayload::Text(line),
                    };
                    if tx_out.send(entry).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Task đọc stderr
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let formatted_err = if line.to_uppercase().contains("ERROR") {
                        line
                    } else {
                        format!("[ERROR] {}", line)
                    };
                    let entry = RawLogEntry {
                        source_id: source_id_err.clone(),
                        payload: RawPayload::Text(formatted_err),
                    };
                    if tx_err.send(entry).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Task giám sát lifecycle: khi channel đóng (ứng dụng thoát), kill ngay lập tức child process
        tokio::spawn(async move {
            tokio::select! {
                _ = child.wait() => {},
                _ = tx.closed() => {
                    let _ = child.kill().await;
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_source_echo() {
        let (tx, mut rx) = mpsc::channel(100);

        #[cfg(target_os = "windows")]
        let proc = ProcessSource::new("cmd", vec!["/c".to_string(), "echo Hello ProcessSource".to_string()]);

        #[cfg(not(target_os = "windows"))]
        let proc = ProcessSource::new("echo", vec!["Hello ProcessSource".to_string()]);

        proc.start_stream(tx).await.unwrap();

        let mut received = Vec::new();
        while let Ok(Some(entry)) = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {
            if let RawPayload::Text(text) = entry.payload {
                received.push(text);
                break;
            }
        }

        assert!(!received.is_empty());
        assert!(received[0].contains("Hello ProcessSource"));
    }
}
