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
#[derive(Parser, Debug)]
#[command(
    name = "uwu-agent",
    version = "0.1.0",
    about = "Headless log collector & RPC agent for uwu-log"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Watch and tail a log file (standalone CLI mode)
    #[arg(short = 'f', long)]
    file: Option<String>,

    /// Run a command and capture its stdout/stderr (standalone CLI mode)
    #[arg(short = 'r', long, alias = "cmd")]
    cmd: Option<String>,

    /// Positional argument fallback for file path
    file_pos: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start RPC proxy mode communicating via stdin/stdout framing
    Proxy,
    /// Print version information for client verification
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Proxy) => run_rpc_proxy_mode().await,
        Some(Commands::Version) => {
            println!("uwu-agent {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None => {
            // Nếu không truyền cờ nào mà stdin là pipe thì tự động fallback vào proxy mode
            if cli.file.is_none() && cli.cmd.is_none() && cli.file_pos.is_none() {
                run_rpc_proxy_mode().await
            } else {
                run_standalone_mode(cli).await
            }
        }
    }
}

/// RPC Proxy Mode: Giao tiếp 2 chiều với Local Client qua Framed Envelope trên stdin/stdout
async fn run_rpc_proxy_mode() -> Result<()> {
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

                let source: Box<dyn LogSource> = match spec {
                    RemoteLogSourceSpec::Command(cmd_str) => {
                        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
                        if parts.is_empty() {
                            let mut guard = writer.lock().await;
                            let _ = guard
                                .send(&ServerEnvelope::Error {
                                    message: "Command cannot be empty".to_string(),
                                })
                                .await;
                            continue;
                        }
                        let prog = parts[0].to_string();
                        let args = parts[1..].iter().map(|s| s.to_string()).collect();
                        Box::new(ProcessSource::new(prog, args))
                    }
                    RemoteLogSourceSpec::File(path) => Box::new(FileSource::new(path)),
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

                // Task gom batch log và stream về Client qua framed writer
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
async fn run_standalone_mode(cli: Cli) -> Result<()> {
    let source: Box<dyn LogSource> = if let Some(cmd_str) = cli.cmd {
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Command string cannot be empty");
        }
        let prog = parts[0].to_string();
        let args = parts[1..].iter().map(|s| s.to_string()).collect();
        Box::new(ProcessSource::new(prog, args))
    } else if let Some(file_path) = cli.file.or(cli.file_pos) {
        Box::new(FileSource::new(file_path))
    } else {
        anyhow::bail!("Please specify a log file (-f <path>) or command (-r '<cmd>')");
    };

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
