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
                let shell_cmd = build_wsl_shell_command(self.working_dir.as_deref(), cmd_str);
                cmd.arg("sh").arg("-c").arg(shell_cmd);
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

/// Xây dựng câu lệnh shell tương tác để thực thi trong WSL (giống Zed)
/// Tự động nạp cấu hình $SHELL / bash và nạp đầy đủ PATH môi trường (nvm, cargo, go...)
pub fn build_wsl_shell_command(working_dir: Option<&str>, cmd_str: &str) -> String {
    let cd_prefix = if let Some(dir) = working_dir {
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
    format!(
        "if [ -n \"$SHELL\" ] && [ -x \"$SHELL\" ]; then exec \"$SHELL\" -i -c {0}; else exec bash -i -c {0}; fi",
        quoted_cmd
    )
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

    #[test]
    fn test_build_wsl_shell_command() {
        // 1. Không có working directory
        let cmd_no_dir = build_wsl_shell_command(None, "cargo run");
        assert!(cmd_no_dir.contains("exec \"$SHELL\" -i -c 'cargo run'"));

        // 2. Có working directory
        let cmd_with_dir = build_wsl_shell_command(Some("/home/user/project"), "cargo run");
        assert!(
            cmd_with_dir.contains("cd '\\''/home/user/project'\\'' 2>/dev/null || true; cargo run")
        );
    }
}
