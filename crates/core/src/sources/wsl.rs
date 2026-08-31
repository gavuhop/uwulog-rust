use crate::protocol::RemoteLogSourceSpec;
use crate::schema::{RawLogEntry, RawPayload};
use crate::sources::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

pub type WslTargetMode = RemoteLogSourceSpec;

pub struct WslSource {
    distro: String,
    mode: WslTargetMode,
    working_dir: Option<String>,
    source_id: String,
}

impl WslSource {
    pub fn new(distro: impl Into<String>, mode: WslTargetMode) -> Self {
        Self::new_with_dir(distro, mode, None)
    }

    pub fn new_with_dir(
        distro: impl Into<String>,
        mode: WslTargetMode,
        working_dir: Option<String>,
    ) -> Self {
        let distro_str = distro.into();
        let source_id = match &mode {
            WslTargetMode::Command(cmd) => format!("wsl:{}:cmd:{}", distro_str, cmd),
            WslTargetMode::File(file) => format!("wsl:{}:file:{}", distro_str, file),
            WslTargetMode::Journald(Some(unit)) => format!("wsl:{}:journald:{}", distro_str, unit),
            WslTargetMode::Journald(None) => format!("wsl:{}:journald", distro_str),
        };
        Self {
            distro: distro_str,
            mode,
            working_dir,
            source_id,
        }
    }
}

#[async_trait]
impl LogSource for WslSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        let mut cmd = Command::new("wsl.exe");
        cmd.arg("-d").arg(&self.distro);

        if let Some(dir) = &self.working_dir {
            if !dir.trim().is_empty() {
                cmd.arg("--cd").arg(dir);
            } else {
                cmd.arg("--cd").arg("~");
            }
        } else {
            cmd.arg("--cd").arg("~");
        }

        cmd.arg("--");

        match &self.mode {
            WslTargetMode::Command(cmd_str) => {
                let cd_prefix = if let Some(dir) = &self.working_dir {
                    if !dir.trim().is_empty() {
                        format!("cd '{}' 2>/dev/null || true; ", dir.replace('\'', "'\\''"))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let full_cmd = format!("{}{}", cd_prefix, cmd_str);
                let quoted_cmd = format!("'{}'", full_cmd.replace('\'', "'\\''"));
                // Giống Zed: Chạy qua interactive shell của người dùng ($SHELL -i -c hoặc bash -i -c)
                // để nạp đầy đủ PATH, nvm, fnm, asdf, node, pnpm, cargo, go...
                cmd.arg("sh").arg("-c").arg(format!(
                    "if [ -n \"$SHELL\" ] && [ -x \"$SHELL\" ]; then exec \"$SHELL\" -i -c {0}; else exec bash -i -c {0}; fi",
                    quoted_cmd
                ));
            }
            WslTargetMode::File(file_path) => {
                cmd.arg("tail").arg("-n").arg("+1").arg("-F").arg(file_path);
            }
            WslTargetMode::Journald(unit) => {
                cmd.arg("journalctl").arg("-f").arg("-o").arg("json");
                if let Some(u) = unit {
                    cmd.arg("-u").arg(u);
                }
            }
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn WSL process on distro '{}'", self.distro))?;

        let source_id_out = self.source_id.clone();
        let source_id_err = format!("{}:stderr", self.source_id);

        let tx_out = tx.clone();
        let stdout_handle = if let Some(stdout) = child.stdout.take() {
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
            })
        } else {
            tokio::spawn(async {})
        };

        let tx_err = tx.clone();
        let stderr_handle = if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let entry = RawLogEntry {
                        source_id: source_id_err.clone(),
                        payload: RawPayload::Text(line),
                    };
                    if tx_err.send(entry).await.is_err() {
                        break;
                    }
                }
            })
        } else {
            tokio::spawn(async {})
        };

        tokio::select! {
            _ = stdout_handle => {},
            _ = stderr_handle => {},
            _ = child.wait() => {},
            _ = tx.closed() => {
                let _ = child.kill().await;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wsl_source_naming() {
        let src = WslSource::new("Ubuntu", WslTargetMode::Command("python3 app.py".into()));
        assert_eq!(src.name(), "wsl:Ubuntu:cmd:python3 app.py");

        let src_file = WslSource::new("Ubuntu", WslTargetMode::File("/var/log/syslog".into()));
        assert_eq!(src_file.name(), "wsl:Ubuntu:file:/var/log/syslog");
    }
}
