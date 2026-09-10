use super::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::mpsc;
use uwu_core_protocol::RemoteLogSourceSpec;
use uwu_core_schema::RawLogEntry;

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
            WslTargetMode::Command(cmd) => {
                if distro_str.trim().is_empty() {
                    format!("wsl:default:cmd:{}", cmd)
                } else {
                    format!("wsl:{}:cmd:{}", distro_str, cmd)
                }
            }
            WslTargetMode::File(file) => {
                if distro_str.trim().is_empty() {
                    format!("wsl:default:file:{}", file)
                } else {
                    format!("wsl:{}:file:{}", distro_str, file)
                }
            }
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
        if !self.distro.trim().is_empty() {
            cmd.arg("-d").arg(&self.distro);
        }

        let cd_dir = self
            .working_dir
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or("~");
        cmd.arg("--cd").arg(cd_dir);

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
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn WSL process on distro '{}'", self.distro))?;

        let stdout_handle = crate::spawn_line_reader(child.stdout.take(), tx.clone());
        let stderr_handle = crate::spawn_line_reader(child.stderr.take(), tx.clone());

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

        let src_default = WslSource::new("", WslTargetMode::Command("python3 app.py".into()));
        assert_eq!(src_default.name(), "wsl:default:cmd:python3 app.py");
    }
}
