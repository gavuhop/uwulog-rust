use crate::ui::autocomplete::AutocompleteState;
use crate::ui::history::SearchHistoryState;
use eframe::egui;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use uwu_core_engine::SystemEngine;
use uwu_core_schema::{LogEvent, RawLogEntry};
use uwu_core_workspace::{Workspace, WorkspaceLocation, WorkspaceStore};
use uwu_driver_sources::{FileSource, LogSource, ProcessSource, WslSource, WslTargetMode};
use uwu_driver_transport::WslTransport;

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Filtered,
    Unfiltered,
}

#[derive(PartialEq, Clone, Debug)]
pub enum SourceType {
    Process,
    File,
    Wsl,
}

#[derive(PartialEq, Clone, Debug)]
pub enum WslSubMode {
    Command,
    File,
}

#[derive(Clone, Debug)]
pub struct WslConfig {
    pub distro: String,
    pub working_dir: String,
    pub sub_mode: WslSubMode,
    pub command_str: String,
    pub file_path: String,
}

impl Default for WslConfig {
    fn default() -> Self {
        Self {
            distro: "Ubuntu".to_string(),
            working_dir: String::new(),
            sub_mode: WslSubMode::Command,
            command_str: "python3 app.py".to_string(),
            file_path: "/var/log/syslog".to_string(),
        }
    }
}

pub struct SourceConfig {
    pub source_type: SourceType,
    pub command_str: String,
    pub file_path: String,
    pub working_dir: String,
    pub wsl_config: WslConfig,
    pub capacity: usize,
    pub display_limit: usize,
}

pub struct UwuGuiApp {
    pub engine: Arc<SystemEngine>,
    pub active_tab: ActiveTab,
    pub query: String,
    pub last_query: String,
    pub display_limit: usize,
    pub capacity: usize,
    pub total_matched: usize,
    pub cached_logs: Vec<LogEvent>,
    pub selected_log: Option<LogEvent>,
    pub is_auto_scroll: bool,
    /// Cờ yêu cầu cuộn ngay xuống dòng mới nhất (khi bấm nút Latch hoặc phím End)
    pub request_scroll_to_bottom: bool,
    /// Cờ báo có log mới được nạp vào cached_logs trong frame hiện tại
    pub has_new_data: bool,
    /// Số dòng bảng ở frame trước. Dùng để biết khi nào có data mới → chỉ scroll_to_row lúc đó.
    pub prev_table_row_count: usize,
    pub is_source_running: bool,
    pub show_launch_modal: bool,
    pub source_config: SourceConfig,
    pub rt: Handle,
    pub last_processed_count: u64,
    pub last_search_time: Instant,
    /// Kill signal: Khi gửi tín hiệu vào đây, forwarder task sẽ thoát → drop source_rx
    /// → tx.closed() trong ProcessSource kích hoạt → taskkill diệt toàn bộ cây tiến trình.
    pub kill_signal: Option<oneshot::Sender<()>>,
    pub autocomplete_state: AutocompleteState,
    pub history_state: SearchHistoryState,
    pub column_state: crate::ui::columns_modal::ColumnState,
    pub highlighted_row_ids: std::collections::HashSet<u64>,
    pub highlighted_terms: std::collections::HashSet<String>,
    /// Tỷ lệ chiều rộng của Log Inspector so với màn hình (mặc định 0.35 = 35%)
    pub inspector_width_ratio: f32,
    pub prev_screen_width: f32,
    pub unfiltered_state: UnfilteredViewState,
    pub global_seen_at_pause: u64,
    pub filtered_seen_at_pause: usize,
    pub filtered_processed_at_pause: u64,
    pub paused_new_matched_count: usize,
    pub discovered_fields_cache:
        std::collections::BTreeMap<String, crate::ui::autocomplete::FieldType>,
    pub available_wsl_distros: Vec<String>,
    pub wsl_distro_rx: Option<tokio::sync::oneshot::Receiver<Vec<String>>>,
    pub env_status: uwu_core_workspace::EnvLoadStatus,
    pub env_vars: std::collections::HashMap<String, String>,
    pub env_watch_tx: tokio::sync::watch::Sender<Option<std::collections::HashMap<String, String>>>,
    pub env_watch_rx:
        tokio::sync::watch::Receiver<Option<std::collections::HashMap<String, String>>>,
    pub env_channel_rx: Option<
        tokio::sync::mpsc::UnboundedReceiver<
            Result<std::collections::HashMap<String, String>, String>,
        >,
    >,
    pub workspace_store: WorkspaceStore,
    pub project_name_input: String,
    pub project_picker_open: bool,
    pub project_search_query: String,
}

#[derive(Clone, Debug, Default)]
pub struct UnfilteredViewState {
    pub is_open: bool,
    pub target_id: Option<u64>,
    pub cached_unfiltered: Vec<LogEvent>,
    pub target_index: Option<usize>,
    pub request_scroll_to_target: bool,
    pub is_live: bool,
    pub request_scroll_to_bottom: bool,
    pub has_new_data: bool,
    pub snapshot_processed_count: u64,
}

impl UwuGuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle) -> Self {
        crate::ui::theme::apply_theme(&cc.egui_ctx);
        #[cfg(target_os = "windows")]
        crate::ui::theme::apply_windows_titlebar_theme(cc);

        let args: Vec<String> = std::env::args().collect();
        let mut display_limit: usize = 5_000;
        let mut capacity: usize = 200_000;

        let mut cmd_to_run = String::new();
        let mut file_to_read = String::new();
        let mut working_dir = String::new();
        let mut source_type = SourceType::Process;
        let mut wsl_config = WslConfig::default();
        let mut custom_source_specified = false;

        let mut i = 1;
        while i < args.len() {
            if (args[i] == "-n" || args[i] == "--limit") && i + 1 < args.len() {
                if let Ok(val) = args[i + 1].parse::<usize>() {
                    display_limit = val;
                }
                i += 1;
            } else if (args[i] == "-cap" || args[i] == "--capacity") && i + 1 < args.len() {
                if let Ok(val) = args[i + 1].parse::<usize>() {
                    capacity = val;
                }
                i += 1;
            } else if (args[i] == "--wsl-distro") && i + 1 < args.len() {
                wsl_config.distro = args[i + 1].clone();
                i += 1;
            } else if (args[i] == "--wsl-dir" || args[i] == "--wsl-cwd") && i + 1 < args.len() {
                wsl_config.working_dir = args[i + 1].clone();
                i += 1;
            } else if (args[i] == "--wsl-cmd") && i + 1 < args.len() {
                wsl_config.command_str = args[i + 1].clone();
                wsl_config.sub_mode = WslSubMode::Command;
                source_type = SourceType::Wsl;
                custom_source_specified = true;
                i += 1;
            } else if (args[i] == "--wsl-file") && i + 1 < args.len() {
                wsl_config.file_path = args[i + 1].clone();
                wsl_config.sub_mode = WslSubMode::File;
                source_type = SourceType::Wsl;
                custom_source_specified = true;
                i += 1;
            } else if (args[i] == "-r"
                || args[i] == "--run"
                || args[i] == "-c"
                || args[i] == "--cmd")
                && i + 1 < args.len()
            {
                cmd_to_run = args[i + 1].clone();
                source_type = SourceType::Process;
                custom_source_specified = true;
                i += 1;
            } else if (args[i] == "-f" || args[i] == "--file") && i + 1 < args.len() {
                file_to_read = args[i + 1].clone();
                source_type = SourceType::File;
                custom_source_specified = true;
                i += 1;
            } else if !args[i].starts_with('-') {
                file_to_read = args[i].clone();
                source_type = SourceType::File;
                custom_source_specified = true;
            }
            i += 1;
        }

        let workspace_store = WorkspaceStore::load();

        let is_wsl_invoked = args.contains(&"--wsl-distro".to_string())
            || args.contains(&"--wsl-cmd".to_string())
            || args.contains(&"--wsl-file".to_string())
            || args.contains(&"--wsl-dir".to_string())
            || source_type == SourceType::Wsl;

        if is_wsl_invoked && source_type != SourceType::Wsl {
            source_type = SourceType::Wsl;
        }

        let active_workdir = if is_wsl_invoked {
            if !wsl_config.working_dir.is_empty() {
                wsl_config.working_dir.clone()
            } else {
                "/home".to_string()
            }
        } else if !working_dir.is_empty() {
            working_dir.clone()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.to_string_lossy().to_string()
        } else {
            String::new()
        };

        if is_wsl_invoked && wsl_config.working_dir.is_empty() {
            wsl_config.working_dir = active_workdir.clone();
        } else if working_dir.is_empty() {
            working_dir = active_workdir.clone();
        }

        fn extract_project_name(path_str: &str) -> String {
            let clean = path_str.trim().trim_end_matches(&['/', '\\'][..]);
            if clean.is_empty() {
                return "Workspace".to_string();
            }
            let parts: Vec<&str> = clean
                .split(&['/', '\\'][..])
                .filter(|s| !s.is_empty())
                .collect();
            if let Some(last) = parts.last() {
                if *last == "." || *last == ".." {
                    "Workspace".to_string()
                } else {
                    last.to_string()
                }
            } else {
                "Workspace".to_string()
            }
        }

        let mut initial_project_name = extract_project_name(&active_workdir);
        let mut initial_query = String::new();

        if let Some(ws) = workspace_store.find_by_workdir(&active_workdir) {
            initial_project_name = ws.name.clone();
            initial_query = ws.last_query.clone();

            if !custom_source_specified {
                match ws.source_type.as_str() {
                    "wsl" => {
                        source_type = SourceType::Wsl;
                        if let WorkspaceLocation::Wsl {
                            distro,
                            working_dir,
                        } = &ws.location
                        {
                            wsl_config.distro = distro.clone();
                            wsl_config.working_dir = working_dir.clone();
                        }
                        if !ws.command_str.is_empty() {
                            wsl_config.sub_mode = WslSubMode::Command;
                            wsl_config.command_str = ws.command_str.clone();
                        } else if !ws.file_path.is_empty() {
                            wsl_config.sub_mode = WslSubMode::File;
                            wsl_config.file_path = ws.file_path.clone();
                        }
                    }
                    "file" => {
                        source_type = SourceType::File;
                        file_to_read = ws.file_path.clone();
                    }
                    "process" => {
                        source_type = SourceType::Process;
                        cmd_to_run = ws.command_str.clone();
                    }
                    _ => {}
                }
            }
        } else if !custom_source_specified {
            cmd_to_run = String::new();
        }

        let engine = Arc::new(SystemEngine::new(capacity));

        // Khởi động không block: Phát hiện WSL distros trong background task thay vì chạy đồng bộ làm chậm app
        let (wsl_tx, wsl_rx) = tokio::sync::oneshot::channel();
        rt.spawn(async move {
            let distros = WslTransport::detect_distros();
            let _ = wsl_tx.send(distros);
        });

        // Watch channel đồng bộ hóa bất đồng bộ biến môi trường
        let (env_watch_tx, env_watch_rx) = tokio::sync::watch::channel(None);

        let source_config = SourceConfig {
            source_type,
            command_str: cmd_to_run,
            file_path: file_to_read,
            working_dir,
            wsl_config,
            capacity,
            display_limit,
        };

        let mut app = Self {
            engine,
            active_tab: ActiveTab::Filtered,
            query: initial_query,
            last_query: String::new(),
            display_limit,
            capacity,
            total_matched: 0,
            cached_logs: Vec::new(),
            selected_log: None,
            is_auto_scroll: true,
            request_scroll_to_bottom: false,
            has_new_data: false,
            prev_table_row_count: 0,
            is_source_running: false,
            show_launch_modal: false,
            source_config,
            rt,
            last_processed_count: 0,
            last_search_time: Instant::now(),
            kill_signal: None,
            autocomplete_state: AutocompleteState::default(),
            history_state: SearchHistoryState::default(),
            column_state: crate::ui::columns_modal::ColumnState::default(),
            highlighted_row_ids: std::collections::HashSet::new(),
            highlighted_terms: std::collections::HashSet::new(),
            inspector_width_ratio: 0.35,
            prev_screen_width: 0.0,
            unfiltered_state: UnfilteredViewState::default(),
            global_seen_at_pause: 0,
            filtered_seen_at_pause: 0,
            filtered_processed_at_pause: 0,
            paused_new_matched_count: 0,
            discovered_fields_cache: Self::default_discovered_fields(),
            available_wsl_distros: Vec::new(),
            wsl_distro_rx: Some(wsl_rx),
            env_status: uwu_core_workspace::EnvLoadStatus::Idle,
            env_vars: std::collections::HashMap::new(),
            env_watch_tx,
            env_watch_rx,
            env_channel_rx: None,
            workspace_store,
            project_name_input: initial_project_name,
            project_picker_open: false,
            project_search_query: String::new(),
        };

        // Nếu workspace đã lưu sẵn env_vars trong store, nạp trước để dùng ngay
        if let Some(ws) = app.workspace_store.find_by_workdir(&active_workdir) {
            if !ws.env_vars.is_empty() {
                app.env_vars = ws.env_vars.clone();
                app.env_watch_tx.send_replace(Some(app.env_vars.clone()));
                app.env_status = uwu_core_workspace::EnvLoadStatus::Ready {
                    source_summary: format!("{} variable (Workspace cache)", app.env_vars.len()),
                    updated_at: Instant::now(),
                };
            }
        }

        // Kích hoạt background task nạp biến môi trường từ lúc khởi động app (tương tự kiến trúc Zed)
        app.spawn_load_environment();

        // Tự động lưu cấu hình workspace và chỉ stream nếu người dùng chỉ định cờ CLI (như -r, -f, -j)
        if custom_source_specified {
            app.save_current_workspace();
            app.start_configured_source();
        }
        app.trigger_full_search();

        app
    }

    pub fn save_current_workspace(&mut self) {
        let name = if self.project_name_input.trim().is_empty() {
            "Workspace".to_string()
        } else {
            self.project_name_input.trim().to_string()
        };

        let location = match self.source_config.source_type {
            SourceType::Wsl => WorkspaceLocation::Wsl {
                distro: self.source_config.wsl_config.distro.clone(),
                working_dir: self.source_config.wsl_config.working_dir.clone(),
            },
            _ => {
                let dir = if !self.source_config.working_dir.trim().is_empty() {
                    self.source_config.working_dir.clone()
                } else {
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                WorkspaceLocation::Local { working_dir: dir }
            }
        };

        let source_type_str = match self.source_config.source_type {
            SourceType::Process => "process",
            SourceType::File => "file",
            SourceType::Wsl => "wsl",
        };

        let mut ws = Workspace::new(name, location, source_type_str);
        ws.last_query = self.query.clone();
        ws.env_vars = self.env_vars.clone();

        match self.source_config.source_type {
            SourceType::Process => {
                ws.command_str = self.source_config.command_str.clone();
            }
            SourceType::File => {
                ws.file_path = self.source_config.file_path.clone();
            }
            SourceType::Wsl => match self.source_config.wsl_config.sub_mode {
                WslSubMode::Command => {
                    ws.command_str = self.source_config.wsl_config.command_str.clone();
                }
                WslSubMode::File => {
                    ws.file_path = self.source_config.wsl_config.file_path.clone();
                }
            },
        }

        self.workspace_store.add_or_update(ws);
    }

    pub fn load_workspace(&mut self, ws: &Workspace) {
        self.project_name_input = ws.name.clone();
        self.query = ws.last_query.clone();

        match &ws.location {
            WorkspaceLocation::Wsl {
                distro,
                working_dir,
            } => {
                self.source_config.wsl_config.distro = distro.clone();
                self.source_config.wsl_config.working_dir = working_dir.clone();
            }
            WorkspaceLocation::Local { working_dir } => {
                self.source_config.working_dir = working_dir.clone();
                if !working_dir.trim().is_empty() {
                    let _ = std::env::set_current_dir(working_dir);
                }
            }
        }

        match ws.source_type.as_str() {
            "wsl" => {
                self.source_config.source_type = SourceType::Wsl;
                if !ws.command_str.is_empty() {
                    self.source_config.wsl_config.sub_mode = WslSubMode::Command;
                    self.source_config.wsl_config.command_str = ws.command_str.clone();
                } else if !ws.file_path.is_empty() {
                    self.source_config.wsl_config.sub_mode = WslSubMode::File;
                    self.source_config.wsl_config.file_path = ws.file_path.clone();
                }
            }
            "file" => {
                self.source_config.source_type = SourceType::File;
                self.source_config.file_path = ws.file_path.clone();
            }
            "process" => {
                self.source_config.source_type = SourceType::Process;
                self.source_config.command_str = ws.command_str.clone();
            }
            _ => {}
        }

        // Nếu workspace đã lưu sẵn env_vars trong store, nạp trước để dùng ngay
        if !ws.env_vars.is_empty() {
            self.env_vars = ws.env_vars.clone();
            self.env_watch_tx.send_replace(Some(self.env_vars.clone()));
            self.env_status = uwu_core_workspace::EnvLoadStatus::Ready {
                source_summary: format!("{} biến (Workspace cache)", self.env_vars.len()),
                updated_at: Instant::now(),
            };
        } else {
            self.env_vars.clear();
            self.env_watch_tx.send_replace(None);
        }

        // Đồng thời spawn background task để làm mới biến môi trường mới nhất từ disk / .env
        self.spawn_load_environment();
    }

    pub fn start_configured_source(&mut self) {
        // Tự động lưu lại lệnh command và cấu hình dự án mỗi khi chạy source
        self.save_current_workspace();

        // Dừng tiến trình cũ nếu đang chạy
        self.stop_current_source();

        // Tạo kênh trung gian: source → forwarder → engine
        let (source_tx, source_rx) = mpsc::channel::<RawLogEntry>(10_000);

        // Tạo kill signal: oneshot channel để GUI có thể ra lệnh dừng forwarder bất kỳ lúc nào
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        self.kill_signal = Some(kill_tx);

        let engine_tx = self.engine.get_channel();

        // Forwarder task: chuyển log từ source channel sang engine channel.
        // Khi nhận kill signal → thoát vòng lặp → drop source_rx
        // → tx.closed() trong ProcessSource lifecycle task kích hoạt TỨC THÌ
        // → taskkill diệt cả cây tiến trình con.
        self.rt.spawn(async move {
            tokio::select! {
                // Nhánh bình thường: forward log entries
                _ = async {
                    let mut rx = source_rx;
                    while let Some(entry) = rx.recv().await {
                        if engine_tx.send(entry).await.is_err() {
                            break;
                        }
                    }
                } => {},
                // Nhánh kill: nhận tín hiệu dừng từ GUI
                _ = kill_rx => {
                    // kill_rx resolved → drop source_rx (implicit) → channel đóng
                }
            }
            // source_rx bị drop ở đây → tx.closed() trong ProcessSource kích hoạt
        });

        let config = &self.source_config;

        match config.source_type {
            SourceType::Process => {
                if !config.command_str.trim().is_empty() {
                    let cmd_parts: Vec<&str> = config.command_str.split_whitespace().collect();
                    if !cmd_parts.is_empty() {
                        let prog = cmd_parts[0].to_string();
                        let proc_args: Vec<String> =
                            cmd_parts[1..].iter().map(|s| s.to_string()).collect();

                        let workdir = if !config.working_dir.trim().is_empty() {
                            Some(config.working_dir.clone())
                        } else {
                            std::env::current_dir()
                                .ok()
                                .map(|p| p.to_string_lossy().to_string())
                        };

                        let mut watch_rx = self.env_watch_rx.clone();
                        let current_envs = self.env_vars.clone();
                        self.rt.spawn(async move {
                            // Đợi biến môi trường nạp xong nếu đang load dở dang (tương tự cơ chế Shared Task của Zed)
                            let envs = if !current_envs.is_empty() {
                                // Nếu đã có sẵn cache, lấy giá trị mới nhất nếu có, hoặc dùng ngay cache cũ không chờ
                                if let Some(latest) = watch_rx.borrow().as_ref() {
                                    latest.clone()
                                } else {
                                    current_envs
                                }
                            } else {
                                // Nếu workspace mới tinh chưa có cache, kiên nhẫn đợi background task nạp tối đa 500ms
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
                            let _ = proc_src.start_stream(source_tx).await;
                        });
                        self.is_source_running = true;
                    }
                }
            }
            SourceType::File => {
                if !config.file_path.trim().is_empty() {
                    let path = config.file_path.clone();
                    self.rt.spawn(async move {
                        let _ = FileSource::new(path).start_stream(source_tx).await;
                    });
                    self.is_source_running = true;
                }
            }
            SourceType::Wsl => {
                let wsl_cfg = &config.wsl_config;
                let mode = match wsl_cfg.sub_mode {
                    WslSubMode::Command => {
                        if !wsl_cfg.command_str.trim().is_empty() {
                            Some(WslTargetMode::Command(wsl_cfg.command_str.clone()))
                        } else {
                            None
                        }
                    }
                    WslSubMode::File => {
                        if !wsl_cfg.file_path.trim().is_empty() {
                            Some(WslTargetMode::File(wsl_cfg.file_path.clone()))
                        } else {
                            None
                        }
                    }
                };

                if let Some(target_mode) = mode {
                    let distro = if wsl_cfg.distro.trim().is_empty() {
                        "Ubuntu".to_string()
                    } else {
                        wsl_cfg.distro.clone()
                    };
                    let working_dir = if wsl_cfg.working_dir.trim().is_empty() {
                        None
                    } else {
                        Some(wsl_cfg.working_dir.clone())
                    };
                    self.rt.spawn(async move {
                        let _ = WslSource::new_with_dir(distro, target_mode, working_dir)
                            .start_stream(source_tx)
                            .await;
                    });
                    self.is_source_running = true;
                }
            }
        }
    }

    /// Kích hoạt background task nạp biến môi trường của workspace hiện tại (lấy cảm hứng từ ProjectEnvironment của Zed)
    /// Hoàn toàn non-blocking, không làm chậm tốc độ khởi động hay đơ giao diện
    pub fn spawn_load_environment(&mut self) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.env_channel_rx = Some(rx);
        // Chỉ hiển thị spinner Loading khi chưa từng có biến môi trường (lần đầu nạp).
        // Nếu đã có cache cũ, chạy ngầm âm thầm (Silent Refresh) để không gây nhấp nháy UI.
        if self.env_vars.is_empty() {
            self.env_status = uwu_core_workspace::EnvLoadStatus::Loading {
                started_at: Instant::now(),
            };
        }

        let workdir = if !self.source_config.working_dir.trim().is_empty() {
            self.source_config.working_dir.clone()
        } else {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let watch_tx = self.env_watch_tx.clone();
        self.rt.spawn(async move {
            let path = std::path::PathBuf::from(workdir);
            let res = uwu_core_workspace::load_workspace_environment(&path).await;
            if let Ok(envs) = &res {
                let _ = watch_tx.send_replace(Some(envs.clone()));
            }
            let _ = tx.send(res.map_err(|e| e.to_string()));
        });
    }

    pub fn tick(&mut self) {
        // 1. Nhận kết quả phát hiện WSL distros từ background task
        if let Some(mut rx) = self.wsl_distro_rx.take() {
            match rx.try_recv() {
                Ok(distros) => {
                    self.available_wsl_distros = distros;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    self.wsl_distro_rx = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {}
            }
        }

        // 2. Nhận kết quả nạp biến môi trường từ background task (như Zed)
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
                        self.env_status = uwu_core_workspace::EnvLoadStatus::Ready {
                            source_summary,
                            updated_at: Instant::now(),
                        };
                        self.env_vars = envs;
                        self.env_watch_tx.send_replace(Some(self.env_vars.clone()));

                        // Đồng bộ vào workspace hiện tại trong store
                        if let Some(ws_id) = self.workspace_store.active_workspace_id {
                            if let Some(ws) = self
                                .workspace_store
                                .recent_workspaces
                                .iter_mut()
                                .find(|w| w.id == ws_id)
                            {
                                ws.env_vars = self.env_vars.clone();
                                let _ = self.workspace_store.save();
                            }
                        }
                    }
                    Err(error) => {
                        self.env_status = uwu_core_workspace::EnvLoadStatus::Failed { error };
                    }
                }
            }
        }

        let total_processed = self.engine.total_processed();
        let now = Instant::now();

        let query_changed = self.query != self.last_query;
        if query_changed {
            self.trigger_full_search();
            if self.query.trim().is_empty() && self.active_tab == ActiveTab::Unfiltered {
                self.close_unfiltered_stream();
            }
        }

        // Debounced: tự động lưu lịch sử sau 500ms dừng gõ
        self.history_state
            .try_debounced_record(&self.query.clone(), now);

        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(150);

        self.has_new_data = false;
        let prev_processed = self.last_processed_count;

        if self.is_auto_scroll && new_logs_arrived && !query_changed {
            let (new_matched_count, new_matching_logs) =
                self.engine.filter_incremental(&self.query, prev_processed);

            if new_matched_count > 0 {
                self.total_matched += new_matched_count;
                self.sync_discovered_fields(&new_matching_logs);
                self.cached_logs.extend(new_matching_logs);

                if self.cached_logs.len() > self.display_limit {
                    let overflow = self.cached_logs.len() - self.display_limit;
                    self.cached_logs.drain(0..overflow);
                }
                self.has_new_data = true;
            }
        }

        // Khi đang Pause có bộ lọc: tính sẵn số lượng log mới khớp trong tick() để tránh tính mỗi frame render
        if !self.is_auto_scroll && !self.query.trim().is_empty() && new_logs_arrived {
            let (new_matched, _) = self
                .engine
                .filter_incremental(&self.query, self.filtered_processed_at_pause);
            self.paused_new_matched_count = new_matched;
        }

        if self.is_auto_scroll {
            self.global_seen_at_pause = total_processed;
            self.filtered_seen_at_pause = self.total_matched;
            self.filtered_processed_at_pause = total_processed;
            self.paused_new_matched_count = 0;
        }

        // Bảng Unfiltered: Giới hạn buffer 500 logs cho việc soi context log gốc
        self.unfiltered_state.has_new_data = false;
        if self.unfiltered_state.is_open && self.unfiltered_state.is_live && new_logs_arrived {
            let (new_count, new_logs) = self.engine.filter_incremental("", prev_processed);
            if new_count > 0 {
                self.unfiltered_state.cached_unfiltered.extend(new_logs);
                if self.unfiltered_state.cached_unfiltered.len() > RAW_STREAM_LIMIT {
                    let overflow = self.unfiltered_state.cached_unfiltered.len() - RAW_STREAM_LIMIT;
                    self.unfiltered_state.cached_unfiltered.drain(0..overflow);
                }
                self.unfiltered_state.has_new_data = true;
            }
        }

        if new_logs_arrived {
            self.last_processed_count = total_processed;
            self.last_search_time = now;
        }
    }

    pub fn stop_current_source(&mut self) {
        if let Some(kill_tx) = self.kill_signal.take() {
            let _ = kill_tx.send(());
        }
        self.is_source_running = false;
    }

    pub fn restart_current_source(&mut self) {
        self.stop_current_source();
        self.save_current_workspace();
        if self.source_config.capacity != self.capacity {
            self.capacity = self.source_config.capacity;
            self.engine = Arc::new(SystemEngine::new(self.capacity));
        } else {
            self.engine.clear();
            self.engine.reset_runtime_detection();
        }

        self.display_limit = self.source_config.display_limit;
        self.cached_logs.clear();
        self.total_matched = 0;
        self.selected_log = None;
        self.last_processed_count = 0;
        self.unfiltered_state.cached_unfiltered.clear();
        self.start_configured_source();
        self.trigger_full_search();
    }

    pub fn trigger_full_search(&mut self) {
        let (matched, logs) = self
            .engine
            .search_with_count(&self.query, self.display_limit);
        self.total_matched = matched;
        self.sync_discovered_fields(&logs);
        self.cached_logs = logs;
        self.last_query = self.query.clone();
        self.last_processed_count = self.engine.total_processed();
        self.last_search_time = Instant::now();
        self.global_seen_at_pause = self.last_processed_count;
        self.filtered_seen_at_pause = self.total_matched;
        self.filtered_processed_at_pause = self.last_processed_count;
        self.paused_new_matched_count = 0;
    }

    pub fn latch(&mut self) {
        let was_unlatched = !self.is_auto_scroll;
        self.is_auto_scroll = true;
        self.request_scroll_to_bottom = true;
        if was_unlatched {
            self.trigger_full_search();
        }
    }

    pub fn unlatch(&mut self) {
        if !self.is_auto_scroll {
            return;
        }
        self.is_auto_scroll = false;
        let total = self.engine.total_processed();
        self.global_seen_at_pause = total;
        self.filtered_seen_at_pause = self.total_matched;
        self.filtered_processed_at_pause = total;
        self.paused_new_matched_count = 0;
    }

    pub fn toggle_latch(&mut self) {
        if self.is_auto_scroll {
            self.unlatch();
        } else {
            self.latch();
        }
    }

    pub fn default_discovered_fields(
    ) -> std::collections::BTreeMap<String, crate::ui::autocomplete::FieldType> {
        use std::collections::BTreeMap;
        let mut fields_map = BTreeMap::new();
        for field in uwu_core_schema::StandardField::default_columns() {
            fields_map.insert(field.canonical_name().to_string(), field.field_type());
        }
        fields_map
    }

    pub fn sync_discovered_fields(&mut self, logs: &[LogEvent]) {
        self.column_state.sync_discovered_keys(logs);
        // Đồng bộ Schema Registry trực tiếp từ Engine (O(1) read lock, zero loops)
        self.discovered_fields_cache = self.engine.get_schema_map().into_iter().collect();
    }

    pub fn get_available_log_fields(&self) -> Vec<(String, crate::ui::autocomplete::FieldType)> {
        self.discovered_fields_cache
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    pub fn apply_autocomplete_suggestion(
        &mut self,
        item: &crate::ui::autocomplete::SuggestionItem,
    ) {
        let (start, end) = self.autocomplete_state.active_token_range;
        if start <= end && end <= self.query.len() {
            let mut new_query = String::new();
            new_query.push_str(&self.query[..start]);
            new_query.push_str(&item.insert_text);
            new_query.push_str(&self.query[end..]);
            self.query = new_query;
        } else {
            self.query = item.insert_text.clone();
        }

        self.autocomplete_state.just_applied = true;

        match item.kind {
            crate::ui::autocomplete::SuggestionKind::Key => {
                // Phase 1 (Key) -> Mở Phase 2
                let available_fields = self.get_available_log_fields();
                let (suggestions, token_range) =
                    crate::ui::autocomplete::generate_suggestions(&self.query, &available_fields);
                if !suggestions.is_empty() {
                    self.autocomplete_state.suggestions = suggestions;
                    self.autocomplete_state.active_token_range = token_range;
                    self.autocomplete_state.selected_index = 0;
                    self.autocomplete_state.is_open = true;
                } else {
                    self.autocomplete_state.is_open = false;
                }
            }
            crate::ui::autocomplete::SuggestionKind::OperatorOrValue => {
                // Phase 2 (Operator / Value) -> ĐÓNG MENU NGAY LẬP TỨC để user tự do gõ dữ liệu
                self.autocomplete_state.is_open = false;
                self.trigger_full_search();
            }
        }
    }

    pub fn toggle_row_highlight(&mut self, id: u64) {
        if self.highlighted_row_ids.contains(&id) {
            self.highlighted_row_ids.remove(&id);
        } else {
            self.highlighted_row_ids.insert(id);
        }
    }

    pub fn is_row_highlighted(&self, id: &u64) -> bool {
        self.highlighted_row_ids.contains(id)
    }

    pub fn toggle_term_highlight(&mut self, term: &str) {
        let clean = term.trim().to_lowercase();
        if clean.is_empty() {
            return;
        }
        if self.highlighted_terms.contains(&clean) {
            self.highlighted_terms.remove(&clean);
        } else {
            self.highlighted_terms.insert(clean);
        }
    }

    #[allow(dead_code)]
    pub fn is_term_highlighted(&self, term: &str) -> bool {
        let clean = term.trim().to_lowercase();
        if clean.is_empty() {
            false
        } else {
            self.highlighted_terms.contains(&clean)
        }
    }

    pub fn has_any_highlights(&self) -> bool {
        !self.highlighted_row_ids.is_empty() || !self.highlighted_terms.is_empty()
    }

    pub fn clear_all_highlights(&mut self) {
        self.highlighted_row_ids.clear();
        self.highlighted_terms.clear();
    }

    pub fn format_field_term(field: &str, val: &str) -> String {
        let clean_val = val.trim();
        if field.eq_ignore_ascii_case("level") {
            format!("level:{}", clean_val.to_lowercase())
        } else if clean_val.contains(' ') || clean_val.contains('"') || clean_val.contains(':') {
            format!("{}:\"{}\"", field, clean_val.replace('"', "\\\""))
        } else {
            format!("{}:{}", field, clean_val)
        }
    }

    pub fn format_selection_term(text: &str) -> String {
        let clean = text
            .replace(" ↵ ", " ")
            .replace('\n', " ")
            .replace('\r', "");
        let clean = clean.trim();
        if clean.contains(' ') || clean.contains('"') || clean.contains(':') {
            format!("\"{}\"", clean.replace('"', "\\\""))
        } else {
            clean.to_string()
        }
    }

    pub fn apply_filter_term(&mut self, term: &str) {
        let current = self.query.trim();
        if current.is_empty() {
            self.query = term.to_string();
        } else {
            let tokens: Vec<&str> = current.split_whitespace().collect();
            if !tokens.contains(&term) {
                self.query = format!("{current} {term}");
            }
        }
        self.trigger_full_search();
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        let exclude_term = if let Some(stripped) = term.strip_prefix('-') {
            stripped.to_string()
        } else {
            format!("-{term}")
        };

        let current = self.query.trim();
        if current.is_empty() {
            self.query = exclude_term;
        } else {
            let tokens: Vec<&str> = current.split_whitespace().collect();
            if !tokens.contains(&exclude_term.as_str()) {
                self.query = format!("{current} {exclude_term}");
            }
        }
        self.trigger_full_search();
    }

    pub fn open_unfiltered_stream(&mut self, target_id: Option<u64>) {
        self.unfiltered_state.is_open = true;
        self.unfiltered_state.target_id = target_id;
        // Mặc định: Nếu mở theo 1 log mục tiêu -> Đóng băng (Freeze/Snapshot) để điều tra không bị trôi
        // Nếu mở xem luồng chung -> Chế độ Live
        self.unfiltered_state.is_live = target_id.is_none();
        self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
        let (target_idx, unfiltered) = self
            .engine
            .get_unfiltered_events(target_id, RAW_STREAM_LIMIT);
        self.unfiltered_state.cached_unfiltered = unfiltered;
        self.unfiltered_state.target_index = target_idx;
        self.unfiltered_state.request_scroll_to_target = true;
        self.unfiltered_state.has_new_data = false;
        self.active_tab = ActiveTab::Unfiltered;
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
        let (target_idx, unfiltered) = self
            .engine
            .get_unfiltered_events(self.unfiltered_state.target_id, RAW_STREAM_LIMIT);
        self.unfiltered_state.cached_unfiltered = unfiltered;
        self.unfiltered_state.target_index = target_idx;
        self.unfiltered_state.request_scroll_to_target = true;
    }

    pub fn toggle_unfiltered_live(&mut self) {
        self.unfiltered_state.is_live = !self.unfiltered_state.is_live;
        if self.unfiltered_state.is_live {
            self.refresh_unfiltered_snapshot();
            self.unfiltered_state.request_scroll_to_bottom = true;
        } else {
            self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
        }
    }

    pub fn unlatch_unfiltered(&mut self) {
        if !self.unfiltered_state.is_live {
            return;
        }
        self.unfiltered_state.is_live = false;
        self.unfiltered_state.snapshot_processed_count = self.engine.total_processed();
    }

    pub fn close_unfiltered_stream(&mut self) {
        self.unfiltered_state.is_open = false;
        self.unfiltered_state.target_id = None;
        self.unfiltered_state.target_index = None;
        self.unfiltered_state.cached_unfiltered.clear();
        self.active_tab = ActiveTab::Filtered;
    }

    pub fn focus_in_main_and_clear_filter(&mut self) {
        if let Some(target_id) = self.unfiltered_state.target_id {
            if let Some(target_event) = self
                .unfiltered_state
                .cached_unfiltered
                .iter()
                .find(|e| e.id == target_id)
                .cloned()
            {
                self.selected_log = Some(target_event);
            }
        }
        self.query.clear();
        self.trigger_full_search();
        self.close_unfiltered_stream();
    }
}

impl eframe::App for UwuGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick();

        // Continuous repainting when streaming logs
        ctx.request_repaint_after(Duration::from_millis(100));

        crate::ui::render_ui(ctx, self);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_current_source();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::autocomplete::{FieldType, SuggestionItem, SuggestionKind};
    use std::collections::HashMap;
    use uwu_core_schema::{LogLevel, RawPayload};

    fn create_test_app() -> UwuGuiApp {
        let engine = Arc::new(SystemEngine::new(100));
        let rt = tokio::runtime::Handle::current();

        let source_config = SourceConfig {
            source_type: SourceType::Process,
            command_str: String::new(),
            file_path: String::new(),
            working_dir: String::new(),
            wsl_config: WslConfig::default(),
            capacity: 100,
            display_limit: 50,
        };
        let (env_watch_tx, env_watch_rx) = tokio::sync::watch::channel(None);

        UwuGuiApp {
            engine,
            active_tab: ActiveTab::Filtered,
            query: String::new(),
            last_query: String::new(),
            display_limit: 50,
            capacity: 100,
            total_matched: 0,
            cached_logs: Vec::new(),
            selected_log: None,
            is_auto_scroll: true,
            request_scroll_to_bottom: false,
            has_new_data: false,
            prev_table_row_count: 0,
            is_source_running: false,
            show_launch_modal: false,
            source_config,
            rt,
            last_processed_count: 0,
            last_search_time: Instant::now(),
            kill_signal: None,
            autocomplete_state: AutocompleteState::default(),
            history_state: SearchHistoryState::default(),
            column_state: crate::ui::columns_modal::ColumnState::default(),
            highlighted_row_ids: std::collections::HashSet::new(),
            highlighted_terms: std::collections::HashSet::new(),
            inspector_width_ratio: 0.35,
            prev_screen_width: 0.0,
            unfiltered_state: UnfilteredViewState::default(),
            global_seen_at_pause: 0,
            filtered_seen_at_pause: 0,
            filtered_processed_at_pause: 0,
            paused_new_matched_count: 0,
            discovered_fields_cache: UwuGuiApp::default_discovered_fields(),
            available_wsl_distros: Vec::new(),
            wsl_distro_rx: None,
            env_status: uwu_core_workspace::EnvLoadStatus::Idle,
            env_vars: std::collections::HashMap::new(),
            env_watch_tx,
            env_watch_rx,
            env_channel_rx: None,
            workspace_store: WorkspaceStore::default(),
            project_name_input: "Test Project".to_string(),
            project_picker_open: false,
            project_search_query: String::new(),
        }
    }

    #[tokio::test]
    async fn test_latch_toggle() {
        let mut app = create_test_app();
        assert!(app.is_auto_scroll);

        app.unlatch();
        assert!(!app.is_auto_scroll);

        app.latch();
        assert!(app.is_auto_scroll);
        assert!(app.request_scroll_to_bottom);

        app.toggle_latch();
        assert!(!app.is_auto_scroll);

        app.toggle_latch();
        assert!(app.is_auto_scroll);
    }

    #[tokio::test]
    async fn test_highlight_toggle_and_clear() {
        let mut app = create_test_app();
        let id1 = 1001u64;
        let id2 = 1002u64;

        assert!(!app.is_row_highlighted(&id1));
        assert!(!app.has_any_highlights());

        app.toggle_row_highlight(id1);
        assert!(app.is_row_highlighted(&id1));
        assert!(app.has_any_highlights());

        app.toggle_row_highlight(id2);
        assert!(app.is_row_highlighted(&id2));

        // Term highlight
        assert!(!app.is_term_highlighted("timeout"));
        app.toggle_term_highlight("timeout");
        assert!(app.is_term_highlighted("timeout"));
        assert!(app.is_term_highlighted("TIMEOUT")); // Case-insensitive
        assert!(app.has_any_highlights());

        app.toggle_term_highlight("timeout");
        assert!(!app.is_term_highlighted("timeout"));

        app.toggle_term_highlight("error");
        assert!(app.has_any_highlights());

        app.clear_all_highlights();
        assert!(!app.is_row_highlighted(&id1));
        assert!(!app.is_term_highlighted("error"));
        assert!(!app.has_any_highlights());
        assert!(app.highlighted_row_ids.is_empty());
        assert!(app.highlighted_terms.is_empty());
    }

    #[tokio::test]
    async fn test_format_field_and_selection_term() {
        // Level lowercase
        assert_eq!(UwuGuiApp::format_field_term("level", "WARN"), "level:warn");
        assert_eq!(
            UwuGuiApp::format_field_term("level", "ERROR"),
            "level:error"
        );

        // Simple values
        assert_eq!(UwuGuiApp::format_field_term("status", "500"), "status:500");

        // Values with spaces or colons
        assert_eq!(
            UwuGuiApp::format_field_term("message", "Database connection lost"),
            "message:\"Database connection lost\""
        );

        // Free text selection formatting
        assert_eq!(UwuGuiApp::format_selection_term("timeout"), "timeout");
        assert_eq!(
            UwuGuiApp::format_selection_term("connection refused"),
            "\"connection refused\""
        );
        assert_eq!(
            UwuGuiApp::format_selection_term("line1\nline2"),
            "\"line1 line2\""
        );
    }

    #[tokio::test]
    async fn test_filter_and_exclude_term() {
        let mut app = create_test_app();

        // Apply first term
        app.apply_filter_term("level:warn");
        assert_eq!(app.query, "level:warn");

        // Apply second term (appended with space)
        app.apply_filter_term("status:500");
        assert_eq!(app.query, "level:warn status:500");

        // Duplicate term ignored
        app.apply_filter_term("level:warn");
        assert_eq!(app.query, "level:warn status:500");

        // Exclude term appended
        app.exclude_filter_term("level:debug");
        assert_eq!(app.query, "level:warn status:500 -level:debug");
    }

    #[tokio::test]
    async fn test_get_available_log_fields_inference() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "latency_ms": 250,
                "created_time": "2026-08-20T10:00:00Z",
                "environment": "production"
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        app.sync_discovered_fields(&[]);

        let available = app.get_available_log_fields();
        let field_types: HashMap<String, FieldType> = available.into_iter().collect();

        // Core fields
        assert_eq!(field_types.get("level"), Some(&FieldType::Text));
        assert_eq!(field_types.get("message"), Some(&FieldType::Text));
        assert_eq!(field_types.get("timestamp"), Some(&FieldType::Time));

        // Inferred fields
        assert_eq!(field_types.get("latency_ms"), Some(&FieldType::Number));
        assert_eq!(field_types.get("created_time"), Some(&FieldType::Text));
        assert_eq!(field_types.get("environment"), Some(&FieldType::Text));
    }

    #[tokio::test]
    async fn test_sync_discovered_fields_replaces_aliases() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "ts": "2026-08-20T10:00:00Z",
                "lvl": "WARN",
                "msg": "warning msg",
                "user_id": 42
            })),
        })
        .await
        .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        app.sync_discovered_fields(&[]);

        let available = app.get_available_log_fields();
        let field_types: HashMap<String, FieldType> = available.into_iter().collect();

        // Exact discovered keys replaced default names
        assert_eq!(field_types.get("lvl"), Some(&FieldType::Text));
        assert_eq!(field_types.get("msg"), Some(&FieldType::Text));
        assert_eq!(field_types.get("ts"), Some(&FieldType::Time));
        assert_eq!(field_types.get("user_id"), Some(&FieldType::Number));

        // Generic canonical names are removed
        assert!(!field_types.contains_key("level"));
        assert!(!field_types.contains_key("message"));
        assert!(!field_types.contains_key("timestamp"));
    }

    #[tokio::test]
    async fn test_apply_autocomplete_suggestion() {
        let mut app = create_test_app();
        app.query = "lev".to_string();
        app.autocomplete_state.active_token_range = (0, 3);

        let suggestion = SuggestionItem {
            kind: SuggestionKind::Key,
            op_symbol: "🔑",
            action_name: "level:".to_string(),
            example_syntax: "level:error".to_string(),
            insert_text: "level:".to_string(),
        };

        app.apply_autocomplete_suggestion(&suggestion);
        assert_eq!(app.query, "level:");
        assert!(app.autocomplete_state.just_applied);
    }

    #[tokio::test]
    async fn test_unfiltered_stream_open_close_and_focus() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        for i in 0..5 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        app.trigger_full_search();

        assert_eq!(app.cached_logs.len(), 5);
        let target_id = app.cached_logs[2].id;

        // Open unfiltered stream on target
        app.open_unfiltered_stream(Some(target_id));
        assert!(app.unfiltered_state.is_open);
        assert_eq!(app.unfiltered_state.target_id, Some(target_id));
        assert_eq!(app.unfiltered_state.target_index, Some(2));
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 5);
        assert!(app.unfiltered_state.request_scroll_to_target);

        // Close unfiltered stream
        app.close_unfiltered_stream();
        assert!(!app.unfiltered_state.is_open);
        assert_eq!(app.unfiltered_state.target_id, None);
        assert!(app.unfiltered_state.cached_unfiltered.is_empty());

        // Re-open and focus in main
        app.query = "level:error".to_string(); // simulate active filter
        app.open_unfiltered_stream(Some(target_id));
        app.focus_in_main_and_clear_filter();

        assert!(!app.unfiltered_state.is_open);
        assert_eq!(app.query, "");
        assert_eq!(app.selected_log.as_ref().map(|l| l.id), Some(target_id));
    }

    #[tokio::test]
    async fn test_unfiltered_frozen_snapshot_no_drift() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        for i in 0..5 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        app.trigger_full_search();

        let target_id = app.cached_logs[2].id;
        app.open_unfiltered_stream(Some(target_id));

        // When opened for a target log, it defaults to FROZEN snapshot
        assert!(!app.unfiltered_state.is_live);
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 5);

        // Ingest 10 new logs into the engine
        for i in 5..15 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Run tick
        app.tick();

        // Cached unfiltered MUST remain frozen at 5 logs with target log unchanged
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 5);
        assert_eq!(app.unfiltered_state.cached_unfiltered[2].id, target_id);
        assert!(!app.unfiltered_state.has_new_data);

        // Now toggle to LIVE stream
        app.toggle_unfiltered_live();
        assert!(app.unfiltered_state.is_live);
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 15);
    }

    #[tokio::test]
    async fn test_repeated_unlatch_idempotency_preserves_pause_state() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        // 1. Ingest initial 10 logs (5 ERROR, 5 INFO)
        for i in 0..10 {
            let level = if i % 2 == 0 { "ERROR" } else { "INFO" };
            let log = RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "level": level,
                    "message": format!("Message {}", i)
                })),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Filter query = "level:error" -> 5 matches out of 10 total
        app.query = "level:error".to_string();
        app.trigger_full_search();
        assert_eq!(app.total_matched, 5);
        assert_eq!(app.engine.total_processed(), 10);

        // First unlatch: locks in pause state
        app.unlatch();
        assert!(!app.is_auto_scroll);
        let locked_global_seen = app.global_seen_at_pause;
        let locked_filtered_seen = app.filtered_seen_at_pause;
        let locked_filtered_proc = app.filtered_processed_at_pause;
        assert_eq!(locked_global_seen, 10);
        assert_eq!(locked_filtered_seen, 5);
        assert_eq!(locked_filtered_proc, 10);

        // 2. Ingest 10 more logs (5 ERROR, 5 INFO) while PAUSED
        for i in 10..20 {
            let level = if i % 2 == 0 { "ERROR" } else { "INFO" };
            let log = RawLogEntry {
                payload: RawPayload::Json(serde_json::json!({
                    "level": level,
                    "message": format!("Message {}", i)
                })),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        assert_eq!(app.engine.total_processed(), 20);

        // Simulate user scrolling up repeatedly (which calls unlatch() multiple times)
        app.unlatch();
        app.unlatch();
        app.unlatch();

        // The pause snapshot values MUST REMAIN EXACTLY UNCHANGED!
        assert_eq!(app.global_seen_at_pause, locked_global_seen);
        assert_eq!(app.filtered_seen_at_pause, locked_filtered_seen);
        assert_eq!(app.filtered_processed_at_pause, locked_filtered_proc);

        // And incremental filter since pause correctly returns the 5 new matching logs
        let (new_matched, _) = app
            .engine
            .filter_incremental(&app.query, app.filtered_processed_at_pause);
        assert_eq!(new_matched, 5);
    }

    #[tokio::test]
    async fn test_unlatch_unfiltered_idempotency() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        for i in 0..10 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Open raw stream in LIVE mode (no target log)
        app.open_unfiltered_stream(None);
        assert!(app.unfiltered_state.is_live);
        assert_eq!(app.unfiltered_state.snapshot_processed_count, 10);

        // Unlatch raw stream
        app.unlatch_unfiltered();
        assert!(!app.unfiltered_state.is_live);
        assert_eq!(app.unfiltered_state.snapshot_processed_count, 10);

        // Ingest 5 more logs
        for i in 10..15 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Repeated unlatch must not overwrite snapshot_processed_count
        app.unlatch_unfiltered();
        app.unlatch_unfiltered();
        assert_eq!(app.unfiltered_state.snapshot_processed_count, 10);
        assert_eq!(app.engine.total_processed(), 15);
    }

    #[tokio::test]
    async fn test_unfiltered_live_tick_sync_both_branches() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        // 1. Initial 5 logs
        for i in 0..5 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        app.trigger_full_search();
        assert_eq!(app.cached_logs.len(), 5);

        // 2. Open unfiltered stream in LIVE mode
        app.open_unfiltered_stream(None);
        assert!(app.unfiltered_state.is_live);
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 5);

        // 3. Ingest 5 more logs
        for i in 5..10 {
            let log = RawLogEntry {
                payload: RawPayload::Text(format!("[INFO] Message {}", i)),
            };
            tx.send(log).await.unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Force last_search_time backward to satisfy 150ms debounce in tick()
        app.last_search_time = Instant::now() - Duration::from_millis(200);
        app.tick();

        // Both filtered and unfiltered live buffers must have received all 10 logs
        assert_eq!(app.cached_logs.len(), 10);
        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 10);
    }

    #[tokio::test]
    async fn test_close_unfiltered_stream_frees_ram() {
        let mut app = create_test_app();
        app.unfiltered_state.is_open = true;
        app.unfiltered_state.cached_unfiltered = vec![LogEvent::new(
            "2026-08-20T10:00:00Z",
            LogLevel::Info,
            "msg",
            HashMap::new(),
        )];
        app.active_tab = ActiveTab::Unfiltered;

        assert_eq!(app.unfiltered_state.cached_unfiltered.len(), 1);

        app.close_unfiltered_stream();

        assert!(!app.unfiltered_state.is_open);
        assert!(app.unfiltered_state.cached_unfiltered.is_empty());
        assert_eq!(app.active_tab, ActiveTab::Filtered);
    }

    #[tokio::test]
    async fn test_spawn_load_environment_and_tick() {
        let mut app = create_test_app();
        assert_eq!(app.env_status, uwu_core_workspace::EnvLoadStatus::Idle);

        app.spawn_load_environment();
        match app.env_status {
            uwu_core_workspace::EnvLoadStatus::Loading { .. } => {}
            _ => panic!("Expected Loading state"),
        }

        // Chờ background task hoàn thành
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        app.tick();

        match &app.env_status {
            uwu_core_workspace::EnvLoadStatus::Ready { .. } => {
                assert!(!app.env_vars.is_empty());
                assert!(app.env_watch_rx.borrow().is_some());
            }
            _ => panic!("Expected Ready state after tick"),
        }
    }
}
