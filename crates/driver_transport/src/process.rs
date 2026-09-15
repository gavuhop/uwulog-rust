use super::{BoxedRead, BoxedWrite, RemoteTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use uwu_core_util::command::new_tokio_command;

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
        let mut cmd = new_tokio_command(&self.command);
        cmd.args(&self.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn process transport: {} {:?}",
                self.command, self.args
            )
        })?;

        super::wrap_child_stdio(child, "Process Agent")
    }
}
