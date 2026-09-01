use super::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use uwu_core_schema::{RawLogEntry, RawPayload};

pub struct JournaldSource {
    unit: Option<String>,
    source_id: String,
}

impl JournaldSource {
    pub fn new(unit: Option<impl Into<String>>) -> Self {
        let unit_opt = unit.map(|u| u.into());
        let source_id = match &unit_opt {
            Some(u) => format!("journald:{}", u),
            None => "journald:system".to_string(),
        };

        Self {
            unit: unit_opt,
            source_id,
        }
    }
}

#[async_trait]
impl LogSource for JournaldSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        let mut cmd = Command::new("journalctl");
        cmd.arg("-o").arg("json").arg("-f");

        if let Some(unit) = &self.unit {
            cmd.arg("-u").arg(unit);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn journalctl for unit: {:?}", self.unit))?;

        let source_id = self.source_id.clone();

        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let entry = RawLogEntry {
                        source_id: source_id.clone(),
                        payload: RawPayload::Text(line),
                    };
                    if tx.send(entry).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Drain stderr để tránh tràn pipe buffer gây treo journalctl
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log::warn!("journalctl stderr: {}", line);
                }
            });
        }

        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journald_source_name() {
        let src = JournaldSource::new(Some("nginx.service"));
        assert_eq!(src.name(), "journald:nginx.service");

        let src_all = JournaldSource::new(None::<String>);
        assert_eq!(src_all.name(), "journald:system");
    }
}
