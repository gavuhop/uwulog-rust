use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use uwu_core_protocol::{
    ClientEnvelope, FramedReader, FramedWriter, RemoteLogSourceSpec, ServerEnvelope,
};
use uwu_core_schema::RawLogEntry;
use uwu_driver_sources::{FileSource, LogSource, ProcessSource};

/// uwu-agent: Headless log streaming agent for remote environments (Linux / WSL / SSH / Containers)
#[derive(Parser, Debug, Clone)]
#[command(
    name = "uwu-agent",
    about = "Headless log collector & RPC agent for uwu-log",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct AgentCli {
    #[command(subcommand)]
    pub command: Option<AgentSubcommand>,

    /// Run a command and capture its stdout/stderr (standalone mode)
    #[arg(
        short = 'c',
        short_alias = 'r',
        long = "cmd",
        visible_aliases = ["run", "command", "exec"],
        help = "Execute a command and stream its output"
    )]
    pub cmd: Option<String>,

    /// Watch and tail a log file (standalone mode)
    #[arg(
        short = 'f',
        long = "file",
        value_hint = clap::ValueHint::FilePath,
        help = "Path to the log file to tail"
    )]
    pub file: Option<String>,

    /// Working directory for command execution or relative file paths
    #[arg(
        short = 'd',
        long = "dir",
        visible_aliases = ["cwd", "working-dir"],
        value_hint = clap::ValueHint::DirPath,
        help = "Working directory"
    )]
    pub working_dir: Option<String>,

    /// Positional argument fallback for file path or command
    #[arg(value_name = "TARGET", value_hint = clap::ValueHint::AnyPath)]
    pub target: Option<String>,

    /// Command arguments passed after `--` (e.g. `uwu-agent -- cargo run --bin server`)
    #[arg(last = true)]
    pub trailing_cmd: Vec<String>,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AgentSubcommand {
    /// Start RPC proxy mode communicating via framed stdin/stdout
    Proxy {
        /// Optional working directory override for proxy mode
        #[arg(
            short = 'd',
            long = "dir",
            visible_aliases = ["cwd", "working-dir"]
        )]
        working_dir: Option<String>,
    },
    /// Print version information for client verification
    Version,
}

impl AgentCli {
    /// Phân giải nguồn log mục tiêu (spec) và thư mục làm việc từ các cờ dòng lệnh
    pub fn resolve_target(&self) -> Result<(RemoteLogSourceSpec, Option<String>)> {
        let mut cmd = self.cmd.clone().unwrap_or_default();
        if cmd.is_empty() && !self.trailing_cmd.is_empty() {
            cmd = self.trailing_cmd.join(" ");
        }

        let file = self.file.clone().unwrap_or_default();
        let dir = self.working_dir.clone().filter(|d| !d.trim().is_empty());

        if !cmd.is_empty() {
            Ok((RemoteLogSourceSpec::Command(cmd), dir))
        } else if !file.is_empty() {
            Ok((RemoteLogSourceSpec::File(file), dir))
        } else if let Some(ref target_str) = self.target {
            let p = std::path::Path::new(target_str);
            if p.is_file() || p.extension().is_some() {
                Ok((RemoteLogSourceSpec::File(target_str.clone()), dir))
            } else {
                Ok((RemoteLogSourceSpec::Command(target_str.clone()), dir))
            }
        } else {
            anyhow::bail!(
                "Please specify a command (-c/--cmd), a log file (-f/--file), or trailing arguments (-- <cmd>)"
            );
        }
    }

    /// Kiểm tra xem người dùng có truyền tham số để chạy trực tiếp (standalone) hay không
    pub fn is_standalone(&self) -> bool {
        self.cmd.is_some()
            || self.file.is_some()
            || self.target.is_some()
            || !self.trailing_cmd.is_empty()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = AgentCli::parse();

    match cli.command {
        Some(AgentSubcommand::Proxy { working_dir }) => {
            setup_proxy_diagnostics();
            run_rpc_proxy_mode(working_dir).await
        }
        Some(AgentSubcommand::Version) => {
            println!("uwu-agent {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None => {
            if cli.is_standalone() {
                run_standalone_mode(cli).await
            } else {
                // Mặc định không truyền cờ nào: chạy proxy mode giao tiếp qua stdin/stdout
                setup_proxy_diagnostics();
                run_rpc_proxy_mode(cli.working_dir).await
            }
        }
    }
}

/// Đảm bảo môi trường RPC tách biệt hoàn toàn stdout (Framed stream) và stderr (Diagnostics / Panic traces)
fn setup_proxy_diagnostics() {
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[uwu-agent PANIC] {}", info);
        default_panic(info);
    }));
}

/// RPC Proxy Mode: Giao tiếp 2 chiều với Local Client qua Framed Envelope trên stdin/stdout
pub async fn run_rpc_proxy_mode(default_workdir: Option<String>) -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let mut reader = FramedReader::new(stdin);
    let writer = Arc::new(Mutex::new(FramedWriter::new(stdout)));

    let is_paused = Arc::new(AtomicBool::new(false));
    let mut active_task: Option<tokio::task::JoinHandle<()>> = None;

    while let Ok(Some(envelope)) = reader.next::<ClientEnvelope>().await {
        match envelope {
            ClientEnvelope::Ping { seq } => {
                let mut guard = writer.lock().await;
                let _ = guard.send(&ServerEnvelope::Pong { seq }).await;
            }
            ClientEnvelope::StartStream {
                spec,
                filter_pushdown: _,
            } => {
                // Dừng task stream cũ nếu đang chạy
                if let Some(handle) = active_task.take() {
                    handle.abort();
                }

                let source = match create_source_from_spec(&spec, default_workdir.clone()) {
                    Ok(s) => s,
                    Err(err) => {
                        let mut guard = writer.lock().await;
                        let _ = guard
                            .send(&ServerEnvelope::Error {
                                message: err.to_string(),
                            })
                            .await;
                        continue;
                    }
                };

                let (tx, mut rx) = mpsc::channel::<RawLogEntry>(10_000);
                if let Err(err) = source.start_stream(tx).await {
                    let mut guard = writer.lock().await;
                    let _ = guard
                        .send(&ServerEnvelope::Error {
                            message: format!("Failed to start source: {:?}", err),
                        })
                        .await;
                    continue;
                }

                let writer_clone = Arc::clone(&writer);
                let is_paused_clone = Arc::clone(&is_paused);
                is_paused.store(false, Ordering::SeqCst);

                // Task gom batch log và stream về Client qua framed writer (50ms interval)
                let handle = tokio::spawn(async move {
                    let mut batch = Vec::with_capacity(256);
                    let mut interval = tokio::time::interval(Duration::from_millis(50));

                    loop {
                        tokio::select! {
                            Some(entry) = rx.recv() => {
                                if !is_paused_clone.load(Ordering::Relaxed) {
                                    batch.push(entry);
                                    if batch.len() >= 256 {
                                        let mut guard = writer_clone.lock().await;
                                        let logs = std::mem::replace(&mut batch, Vec::with_capacity(256));
                                        if guard.send(&ServerEnvelope::LogBatch(logs)).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            _ = interval.tick() => {
                                if !batch.is_empty() && !is_paused_clone.load(Ordering::Relaxed) {
                                    let mut guard = writer_clone.lock().await;
                                    let logs = std::mem::replace(&mut batch, Vec::with_capacity(256));
                                    if guard.send(&ServerEnvelope::LogBatch(logs)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            else => {
                                // Nguồn log đã hoàn thành
                                if !batch.is_empty() {
                                    let mut guard = writer_clone.lock().await;
                                    let logs = std::mem::take(&mut batch);
                                    let _ = guard.send(&ServerEnvelope::LogBatch(logs)).await;
                                }
                                let mut guard = writer_clone.lock().await;
                                let _ = guard.send(&ServerEnvelope::Terminated { exit_code: Some(0) }).await;
                                break;
                            }
                        }
                    }
                });

                active_task = Some(handle);
            }
            ClientEnvelope::PauseStream => {
                is_paused.store(true, Ordering::SeqCst);
            }
            ClientEnvelope::ResumeStream => {
                is_paused.store(false, Ordering::SeqCst);
            }
            ClientEnvelope::StopStream => {
                if let Some(handle) = active_task.take() {
                    handle.abort();
                }
            }
        }
    }

    if let Some(handle) = active_task.take() {
        handle.abort();
    }

    Ok(())
}

/// Standalone CLI Mode: Dùng để test trực tiếp dòng lệnh trên remote
async fn run_standalone_mode(cli: AgentCli) -> Result<()> {
    let (spec, workdir) = cli.resolve_target()?;
    let source = create_source_from_spec(&spec, workdir)?;

    let (tx, mut rx) = mpsc::channel::<RawLogEntry>(10_000);
    source
        .start_stream(tx)
        .await
        .context("Failed to start log stream on agent")?;

    while let Some(entry) = rx.recv().await {
        println!("{}", serde_json::to_string(&entry)?);
    }

    Ok(())
}

/// Khởi tạo LogSource tương ứng từ cấu hình remote spec
pub fn create_source_from_spec(
    spec: &RemoteLogSourceSpec,
    working_dir: Option<String>,
) -> Result<Box<dyn LogSource>> {
    match spec {
        RemoteLogSourceSpec::Command(cmd_str) => {
            let cmd_str = cmd_str.trim();
            if cmd_str.is_empty() {
                anyhow::bail!("Command cannot be empty");
            }

            // Tương tự Zed: Thực thi qua shell để nạp đầy đủ PATH môi trường và hỗ trợ quotes/pipes
            #[cfg(unix)]
            {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                let args = vec!["-c".to_string(), cmd_str.to_string()];
                Ok(Box::new(ProcessSource::new_with_dir(
                    shell,
                    args,
                    working_dir,
                )))
            }
            #[cfg(windows)]
            {
                let args = vec!["/C".to_string(), cmd_str.to_string()];
                Ok(Box::new(ProcessSource::new_with_dir(
                    "cmd.exe",
                    args,
                    working_dir,
                )))
            }
        }
        RemoteLogSourceSpec::File(path) => {
            let file_path = if let Some(ref dir) = working_dir {
                let p = std::path::Path::new(path);
                if p.is_relative() {
                    std::path::Path::new(dir)
                        .join(path)
                        .to_string_lossy()
                        .to_string()
                } else {
                    path.clone()
                }
            } else {
                path.clone()
            };
            Ok(Box::new(FileSource::new(file_path)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_cli_parsing_flags() {
        // 1. Kiểm tra cờ --cmd và alias -r
        let cli = AgentCli::try_parse_from(["uwu-agent", "-c", "cargo run"]).unwrap();
        assert_eq!(cli.cmd.as_deref(), Some("cargo run"));
        let (spec, _) = cli.resolve_target().unwrap();
        assert_eq!(spec, RemoteLogSourceSpec::Command("cargo run".into()));

        let cli_alias = AgentCli::try_parse_from(["uwu-agent", "-r", "python app.py"]).unwrap();
        assert_eq!(cli_alias.cmd.as_deref(), Some("python app.py"));

        // 2. Kiểm tra cờ --file và -f
        let cli_file = AgentCli::try_parse_from(["uwu-agent", "-f", "/var/log/syslog"]).unwrap();
        assert_eq!(cli_file.file.as_deref(), Some("/var/log/syslog"));
        let (spec, _) = cli_file.resolve_target().unwrap();
        assert_eq!(spec, RemoteLogSourceSpec::File("/var/log/syslog".into()));

        // 3. Kiểm tra cờ --dir và alias cwd
        let cli_dir =
            AgentCli::try_parse_from(["uwu-agent", "-c", "make", "-d", "/home/user/project"])
                .unwrap();
        assert_eq!(cli_dir.working_dir.as_deref(), Some("/home/user/project"));
        let (_, dir) = cli_dir.resolve_target().unwrap();
        assert_eq!(dir.as_deref(), Some("/home/user/project"));

        // 4. Trailing args với --
        let cli_trailing =
            AgentCli::try_parse_from(["uwu-agent", "-d", "/work", "--", "npm", "run", "dev"])
                .unwrap();
        assert_eq!(cli_trailing.trailing_cmd, vec!["npm", "run", "dev"]);
        let (spec, dir) = cli_trailing.resolve_target().unwrap();
        assert_eq!(spec, RemoteLogSourceSpec::Command("npm run dev".into()));
        assert_eq!(dir.as_deref(), Some("/work"));
    }

    #[test]
    fn test_agent_cli_subcommands() {
        // Proxy subcommand
        let cli_proxy = AgentCli::try_parse_from(["uwu-agent", "proxy"]).unwrap();
        assert_eq!(
            cli_proxy.command,
            Some(AgentSubcommand::Proxy { working_dir: None })
        );

        let cli_proxy_dir =
            AgentCli::try_parse_from(["uwu-agent", "proxy", "-d", "/var/log"]).unwrap();
        assert_eq!(
            cli_proxy_dir.command,
            Some(AgentSubcommand::Proxy {
                working_dir: Some("/var/log".to_string())
            })
        );

        // Version subcommand
        let cli_version = AgentCli::try_parse_from(["uwu-agent", "version"]).unwrap();
        assert_eq!(cli_version.command, Some(AgentSubcommand::Version));
    }

    #[test]
    fn test_agent_cli_positional_target() {
        let cli_pos_file = AgentCli::try_parse_from(["uwu-agent", "app.log"]).unwrap();
        let (spec, _) = cli_pos_file.resolve_target().unwrap();
        assert_eq!(spec, RemoteLogSourceSpec::File("app.log".into()));

        let cli_pos_cmd = AgentCli::try_parse_from(["uwu-agent", "journalctl -f"]).unwrap();
        let (spec, _) = cli_pos_cmd.resolve_target().unwrap();
        assert_eq!(spec, RemoteLogSourceSpec::Command("journalctl -f".into()));
    }

    #[test]
    fn test_create_source_from_spec_empty_cmd_errors() {
        let spec = RemoteLogSourceSpec::Command("   ".into());
        let res = create_source_from_spec(&spec, None);
        assert!(res.is_err());
    }
}
