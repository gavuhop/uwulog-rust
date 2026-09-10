use super::{BoxedRead, BoxedWrite, RemoteTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

pub struct SshTransport {
    host: String,
    user: Option<String>,
    port: Option<u16>,
    agent_cmd: String,
    name: String,
}

impl SshTransport {
    pub fn new(host: impl Into<String>) -> Self {
        let host_str = host.into();
        let name = format!("SSH ({})", host_str);
        Self {
            host: host_str,
            user: None,
            port: None,
            agent_cmd: "uwu-agent".to_string(),
            name,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
}

#[async_trait]
impl RemoteTransport for SshTransport {
    fn name(&self) -> &str {
        &self.name
    }

    async fn spawn_proxy(&self) -> Result<(BoxedRead, BoxedWrite)> {
        let mut cmd = Command::new("ssh");

        if let Some(port) = self.port {
            cmd.arg("-p").arg(port.to_string());
        }

        let target = if let Some(user) = &self.user {
            format!("{}@{}", user, self.host)
        } else {
            self.host.clone()
        };
        cmd.arg(target);
        cmd.arg(format!("{} proxy", self.agent_cmd));

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn SSH proxy to {}", self.host))?;

        super::wrap_child_stdio(child, "SSH Agent")
    }
}
