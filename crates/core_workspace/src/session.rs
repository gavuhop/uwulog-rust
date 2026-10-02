use crate::environment::EnvLoadStatus;
use crate::remote::{RemoteConnectionOptions, SshConnectionOptions};
use crate::{Workspace, WorkspaceLocation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot, watch};
use uuid::Uuid;
use uwu_core_engine::SystemEngine;
use uwu_core_schema::{RawLogEntry, RawPayload};
use uwu_driver_sources::{
    FileSource, LogSource, ProcessSource, RemoteLogSourceSpec, RemoteSource, WslSource,
    WslTargetMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    #[default]
    Process,
    File,
}

#[derive(Clone, Debug)]
pub struct SourceConfig {
    pub source_type: SourceType,
    pub command_str: String,
    pub file_path: String,
    pub working_dir: String,
    pub capacity: usize,
    pub display_limit: usize,
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            source_type: SourceType::Process,
            command_str: String::new(),
            file_path: String::new(),
            working_dir: String::new(),
            capacity: 200_000,
            display_limit: 5_000,
        }
    }
}

/// Headless Workspace Session: Đại diện cho một phiên chạy runtime độc lập của một project.
/// Sở hữu một `SystemEngine` riêng, quản lý tiến trình con, forwarder task, và biến môi trường.
pub struct WorkspaceSession {
    pub id: Uuid,
    pub name: String,
    pub location: WorkspaceLocation,
    pub source_config: SourceConfig,
    pub engine: Arc<SystemEngine>,
    pub capacity: usize,
    pub display_limit: usize,

    pub is_source_running: bool,
    pub source_task: Option<tokio::task::JoinHandle<()>>,
    pub kill_signal: Option<oneshot::Sender<()>>,

    pub env_status: EnvLoadStatus,
    pub env_vars: HashMap<String, String>,
    pub env_watch_tx: watch::Sender<Option<HashMap<String, String>>>,
    pub env_watch_rx: watch::Receiver<Option<HashMap<String, String>>>,
    pub env_channel_rx: Option<mpsc::UnboundedReceiver<Result<HashMap<String, String>, String>>>,
}

impl WorkspaceSession {
    pub fn new(
        name: impl Into<String>,
        location: WorkspaceLocation,
        source_config: SourceConfig,
    ) -> Self {
        let capacity = source_config.capacity;
        let display_limit = source_config.display_limit;
        let engine = Arc::new(SystemEngine::new(capacity));
        let (env_watch_tx, env_watch_rx) = watch::channel(None);
        let name = crate::sanitize_project_name(&name.into(), &location);

        Self {
            id: Uuid::new_v4(),
            name,
            location,
            source_config,
            engine,
            capacity,
            display_limit,
            is_source_running: false,
            source_task: None,
            kill_signal: None,
            env_status: EnvLoadStatus::Idle,
            env_vars: HashMap::new(),
            env_watch_tx,
            env_watch_rx,
            env_channel_rx: None,
        }
    }

    pub fn new_default(capacity: usize, display_limit: usize) -> Self {
        let working_dir = std::env::current_dir()
            .ok()
            .map(|p| crate::clean_path(&p.to_string_lossy()))
            .unwrap_or_default();

        let name = crate::extract_project_name(&working_dir);

        let source_config = SourceConfig {
            capacity,
            display_limit,
            working_dir: working_dir.clone(),
            ..Default::default()
        };

        Self::new(name, WorkspaceLocation::local(working_dir), source_config)
    }

    /// Tên của máy chủ remote nếu có
    pub fn server_name(&self) -> Option<&str> {
        self.location.server_name()
    }

    pub fn from_workspace(ws: &Workspace, capacity: usize, display_limit: usize) -> Self {
        let source_config = SourceConfig {
            capacity,
            display_limit,
            ..Default::default()
        };
        let mut session = Self::new(&ws.name, ws.location.clone(), source_config);
        session.id = ws.id;
        session.apply_workspace(ws);
        session
    }

    /// Đồng bộ WorkspaceLocation từ SourceConfig hiện tại
    pub fn sync_location(&mut self) {
        let dir = if !self.source_config.working_dir.trim().is_empty() {
            self.source_config.working_dir.clone()
        } else if !self.location.working_dir().trim().is_empty() {
            self.location.working_dir().to_string()
        } else if !self.location.is_remote() {
            std::env::current_dir()
                .map(|p| crate::clean_path(&p.to_string_lossy()))
                .unwrap_or_default()
        } else {
            String::new()
        };

        if !dir.is_empty() {
            self.location.set_working_dir(dir);
        }
    }

    pub fn to_workspace(&self) -> Workspace {
        let mut ws = Workspace::new(
            &self.name,
            self.location.clone(),
            self.source_config.source_type,
        );
        ws.id = self.id;
        ws.env_vars = self.env_vars.clone();

        match self.source_config.source_type {
            SourceType::Process => {
                ws.command_str = self.source_config.command_str.clone();
            }
            SourceType::File => {
                ws.file_path = self.source_config.file_path.clone();
            }
        }

        ws
    }

    pub fn apply_workspace(&mut self, ws: &Workspace) {
        self.name = ws.name.clone();
        self.location = ws.location.clone();
        self.source_config.source_type = ws.source_type;
        self.source_config.command_str = ws.command_str.clone();
        self.source_config.file_path = ws.file_path.clone();
        self.source_config.working_dir = ws.location.working_dir().to_string();

        if !ws.env_vars.is_empty() {
            self.env_vars = ws.env_vars.clone();
            self.env_watch_tx.send_replace(Some(self.env_vars.clone()));
        } else {
            self.env_vars.clear();
            self.env_watch_tx.send_replace(None);
        }
    }

    pub fn start_source(&mut self, rt: &Handle) {
        self.stop_source();

        let (source_tx, source_rx) = mpsc::channel::<RawLogEntry>(10_000);
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        self.kill_signal = Some(kill_tx);

        let engine_tx = self.engine.get_channel();

        self.source_task = Some(rt.spawn(async move {
            tokio::select! {
                _ = async {
                    let mut rx = source_rx;
                    while let Some(entry) = rx.recv().await {
                        if engine_tx.send(entry).await.is_err() {
                            break;
                        }
                    }
                } => {},
                _ = kill_rx => {}
            }
        }));

        let config = &self.source_config;

        match config.source_type {
            SourceType::Process => {
                if !config.command_str.trim().is_empty() {
                    match &self.location {
                        WorkspaceLocation::Remote(RemoteConnectionOptions::Wsl(wsl_opts)) => {
                            let distro = wsl_opts.distro.clone();
                            let target_mode = WslTargetMode::Command(config.command_str.clone());
                            let workdir = if !wsl_opts.working_dir.trim().is_empty() {
                                Some(wsl_opts.working_dir.clone())
                            } else if !config.working_dir.trim().is_empty() {
                                Some(config.working_dir.clone())
                            } else {
                                None
                            };
                            let tx = source_tx.clone();
                            rt.spawn(async move {
                                let _ = WslSource::new_with_dir(distro, target_mode, workdir)
                                    .start_stream(tx)
                                    .await;
                            });
                            self.is_source_running = true;
                        }
                        WorkspaceLocation::Remote(RemoteConnectionOptions::Ssh(ssh_opts)) => {
                            Self::spawn_ssh_stream(
                                rt,
                                ssh_opts,
                                RemoteLogSourceSpec::Command(config.command_str.clone()),
                                &config.working_dir,
                                source_tx.clone(),
                            );
                            self.is_source_running = true;
                        }
                        WorkspaceLocation::Local { .. } => {
                            let cmd_parts: Vec<&str> =
                                config.command_str.split_whitespace().collect();
                            if !cmd_parts.is_empty() {
                                let prog = cmd_parts[0].to_string();
                                let proc_args: Vec<String> =
                                    cmd_parts[1..].iter().map(|s| s.to_string()).collect();

                                let workdir = if !config.working_dir.trim().is_empty() {
                                    Some(config.working_dir.clone())
                                } else if !self.location.working_dir().trim().is_empty() {
                                    Some(self.location.working_dir().to_string())
                                } else {
                                    std::env::current_dir()
                                        .ok()
                                        .map(|p| p.to_string_lossy().to_string())
                                };

                                let mut watch_rx = self.env_watch_rx.clone();
                                let current_envs = self.env_vars.clone();
                                let tx = source_tx.clone();
                                rt.spawn(async move {
                                    let envs = if !current_envs.is_empty() {
                                        if let Some(latest) = watch_rx.borrow().as_ref() {
                                            latest.clone()
                                        } else {
                                            current_envs
                                        }
                                    } else {
                                        let wait_res = tokio::time::timeout(
                                            std::time::Duration::from_millis(500),
                                            async {
                                                while watch_rx.borrow().is_none() {
                                                    if watch_rx.changed().await.is_err() {
                                                        break;
                                                    }
                                                }
                                                watch_rx.borrow().clone()
                                            },
                                        )
                                        .await;

                                        wait_res.ok().flatten().unwrap_or_default()
                                    };

                                    let mut proc_src =
                                        ProcessSource::new_with_dir(prog, proc_args, workdir);
                                    if !envs.is_empty() {
                                        proc_src = proc_src.with_envs(envs);
                                    }
                                    let _ = proc_src.start_stream(tx).await;
                                });
                                self.is_source_running = true;
                            }
                        }
                    }
                }
            }
            SourceType::File => {
                if !config.file_path.trim().is_empty() {
                    match &self.location {
                        WorkspaceLocation::Remote(RemoteConnectionOptions::Wsl(wsl_opts)) => {
                            let distro = wsl_opts.distro.clone();
                            let target_mode = WslTargetMode::File(config.file_path.clone());
                            let workdir = if !wsl_opts.working_dir.trim().is_empty() {
                                Some(wsl_opts.working_dir.clone())
                            } else {
                                None
                            };
                            let tx = source_tx.clone();
                            rt.spawn(async move {
                                let _ = WslSource::new_with_dir(distro, target_mode, workdir)
                                    .start_stream(tx)
                                    .await;
                            });
                            self.is_source_running = true;
                        }
                        WorkspaceLocation::Remote(RemoteConnectionOptions::Ssh(ssh_opts)) => {
                            Self::spawn_ssh_stream(
                                rt,
                                ssh_opts,
                                RemoteLogSourceSpec::File(config.file_path.clone()),
                                &config.working_dir,
                                source_tx.clone(),
                            );
                            self.is_source_running = true;
                        }
                        WorkspaceLocation::Local { .. } => {
                            let path = config.file_path.clone();
                            let tx = source_tx.clone();
                            rt.spawn(async move {
                                let _ = FileSource::new(path).start_stream(tx).await;
                            });
                            self.is_source_running = true;
                        }
                    }
                }
            }
        }

        // Hướng dẫn người dùng khi vừa mở Workspace SSH mà chưa cấu hình File/Command
        if let WorkspaceLocation::Remote(RemoteConnectionOptions::Ssh(ref ssh_opts)) = self.location
        {
            if config.command_str.trim().is_empty() && config.file_path.trim().is_empty() {
                let tx_info = source_tx.clone();
                let target = ssh_opts.target_string();
                let workdir = if !ssh_opts.working_dir.is_empty() {
                    format!(" (directory: {})", ssh_opts.working_dir)
                } else {
                    String::new()
                };
                rt.spawn(async move {
                    let _ = tx_info
                        .send(RawLogEntry {
                            payload: RawPayload::Text(format!(
                                "[SSH READY] Connected to {}{}. Open Settings (⚙) on the toolbar to choose a log file or command to stream.",
                                target, workdir
                            )),
                        })
                        .await;
                });
            }
        }
    }

    fn spawn_ssh_stream(
        rt: &Handle,
        ssh_opts: &SshConnectionOptions,
        spec: RemoteLogSourceSpec,
        working_dir: &str,
        source_tx: tokio::sync::mpsc::Sender<RawLogEntry>,
    ) {
        let mut transport = ssh_opts.to_transport();
        if !working_dir.trim().is_empty() {
            transport = transport.with_working_dir(working_dir);
        }
        let source = RemoteSource::new(Box::new(transport), spec, None);
        let tx = source_tx.clone();
        let tx_err = source_tx;
        rt.spawn(async move {
            if let Err(e) = source.start_stream(tx).await {
                log::error!("SSH stream error: {:#}", e);
                let _ = tx_err
                    .send(RawLogEntry {
                        payload: RawPayload::Text(format!(
                            "[SSH ERROR] Failed to start log stream: {:#}",
                            e
                        )),
                    })
                    .await;
            }
        });
    }

    pub fn stop_source(&mut self) {
        if let Some(kill) = self.kill_signal.take() {
            let _ = kill.send(());
        }
        if let Some(task) = self.source_task.take() {
            task.abort();
        }
        self.is_source_running = false;
    }

    pub fn restart_source(&mut self, rt: &Handle) {
        self.stop_source();
        self.capacity = self.source_config.capacity;
        self.display_limit = self.source_config.display_limit;
        if self.capacity != self.engine.max_capacity() {
            self.engine = Arc::new(SystemEngine::new(self.capacity));
        } else {
            self.engine.clear();
        }
        self.start_source(rt);
    }

    pub fn spawn_load_environment(&mut self, rt: &Handle) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.env_channel_rx = Some(rx);
        self.env_status = EnvLoadStatus::Loading {
            started_at: Instant::now(),
        };

        let workdir = if !self.source_config.working_dir.trim().is_empty() {
            self.source_config.working_dir.clone()
        } else {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let watch_tx = self.env_watch_tx.clone();
        rt.spawn(async move {
            let path = std::path::PathBuf::from(workdir);
            let start = std::time::Instant::now();
            let res = crate::load_workspace_environment(&path).await;
            // Gửi kết quả vào watch channel NGAY LẬP TỨC để unblock start_source đang chờ watch_rx.
            // Không để start_source phải đợi thêm 300ms spinner delay.
            if let Ok(envs) = &res {
                let _ = watch_tx.send_replace(Some(envs.clone()));
            }
            // Giữ spinner hiển thị tối thiểu 300ms để người dùng nhìn rõ phản hồi trực quan.
            // tick() đọc từ kênh `tx` bên dưới nên env_status chỉ chuyển Ready SAU sleep.
            if start.elapsed() < std::time::Duration::from_millis(300) {
                tokio::time::sleep(std::time::Duration::from_millis(300) - start.elapsed()).await;
            }
            let _ = tx.send(res.map_err(|e| e.to_string()));
        });
    }

    pub fn tick(&mut self) {
        // Kiểm tra xem background task của source đã hoàn thành chưa
        if self.is_source_running && self.source_task.as_ref().is_some_and(|t| t.is_finished()) {
            self.is_source_running = false;
            self.source_task = None;
        }

        // Kiểm tra kết quả nạp biến môi trường
        if let Some(rx) = &mut self.env_channel_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(envs) => {
                        let count = envs.len();
                        let has_dotenv = envs.iter().any(|(k, _)| std::env::var(k).is_err());
                        let source_summary = if has_dotenv {
                            format!("{count} biến (System + .env)")
                        } else {
                            format!("{count} biến (System)")
                        };
                        self.env_status = EnvLoadStatus::Ready {
                            source_summary,
                            updated_at: Instant::now(),
                        };
                        self.env_vars = envs;
                        self.env_watch_tx.send_replace(Some(self.env_vars.clone()));
                    }
                    Err(error) => {
                        self.env_status = EnvLoadStatus::Failed { error };
                    }
                }
            }
        }
    }

    /// Icon đại diện cho nguồn log của session (Remote, File, Command)
    pub fn icon(&self) -> uwu_icons::IconName {
        match &self.location {
            WorkspaceLocation::Remote(remote) => remote.icon(),
            WorkspaceLocation::Local { .. } => match self.source_config.source_type {
                SourceType::File => uwu_icons::IconName::File,
                SourceType::Process => uwu_icons::IconName::Screen,
            },
        }
    }

    /// Thư mục làm việc hiệu lực của session
    pub fn effective_working_dir(&self) -> &str {
        if !self.source_config.working_dir.trim().is_empty() {
            &self.source_config.working_dir
        } else {
            self.location.working_dir()
        }
    }

    /// Chuỗi tóm tắt vị trí/nguồn log dùng cho tooltip và picker
    pub fn target_summary(&self) -> String {
        let dir = self.effective_working_dir();
        if !dir.is_empty() {
            if let Some(remote) = self.location.as_remote() {
                format!("{} ({})", dir, remote.display_name())
            } else {
                dir.to_string()
            }
        } else if !self.source_config.file_path.is_empty() {
            self.source_config.file_path.clone()
        } else if !self.source_config.command_str.is_empty() {
            self.source_config.command_str.clone()
        } else {
            self.location.summary()
        }
    }
}

impl Drop for WorkspaceSession {
    fn drop(&mut self) {
        self.stop_source();
    }
}
