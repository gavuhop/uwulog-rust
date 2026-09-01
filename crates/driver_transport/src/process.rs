use super::{BoxedRead, BoxedWrite, RemoteTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct ProcessTransport {
    command: String,
    args: Vec<String>,
    name: String,
}

impl ProcessTransport {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        let cmd_str = command.into();
        let name = format!("Process ({})", cmd_str);
        Self {
            command: cmd_str,
            args,
            name,
        }
    }

    /// Helper tạo Docker exec transport
    pub fn docker(container: impl Into<String>) -> Self {
        let container_str = container.into();
        Self {
            name: format!("Docker ({})", container_str),
            command: "docker".to_string(),
            args: vec![
                "exec".to_string(),
                "-i".to_string(),
                container_str,
                "uwu-agent".to_string(),
                "proxy".to_string(),
            ],
        }
    }
}

#[async_trait]
impl RemoteTransport for ProcessTransport {
    fn name(&self) -> &str {
        &self.name
    }

    async fn spawn_proxy(&self) -> Result<(BoxedRead, BoxedWrite)> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn process transport: {} {:?}",
                self.command, self.args
            )
        })?;

        let stdout = child.stdout.take().context("Failed to open child stdout")?;
        let stdin = child.stdin.take().context("Failed to open child stdin")?;

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log::warn!("[Process Agent STDERR] {}", line);
                }
            });
        }

        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok((Box::new(stdout), Box::new(stdin)))
    }
}
