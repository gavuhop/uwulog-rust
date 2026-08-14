use eframe::egui;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use uwu_engine::SystemEngine;
use uwu_schema::{LogEvent, RawLogEntry};
use uwu_sources::{FileSource, LogSource, ProcessSource};

#[cfg(target_os = "windows")]
use uwu_sources::WinEventSource;

#[derive(PartialEq, Clone)]
pub enum SourceType {
    Process,
    File,
    WinEvent,
}

pub struct SourceConfig {
    pub source_type: SourceType,
    pub command_str: String,
    pub file_path: String,
    pub win_channel: String,
    pub capacity: usize,
    pub display_limit: usize,
}

pub struct UwuGuiApp {
    pub engine: Arc<SystemEngine>,
    pub query: String,
    pub last_query: String,
    pub display_limit: usize,
    pub capacity: usize,
    pub total_matched: usize,
    pub cached_logs: Vec<LogEvent>,
    pub selected_log: Option<LogEvent>,
    pub is_auto_scroll: bool,
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
}

impl UwuGuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle) -> Self {
        crate::ui::theme::apply_theme(&cc.egui_ctx);

        let args: Vec<String> = std::env::args().collect();
        let mut display_limit: usize = 5_000;
        let mut capacity: usize = 50_000;

        let mut cmd_to_run = String::new();
        let mut file_to_read = String::new();
        let mut source_type = SourceType::Process;
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

        if !custom_source_specified {
            cmd_to_run = "go run gen_logs.go".to_string();
        }

        let engine = Arc::new(SystemEngine::new(capacity));

        let source_config = SourceConfig {
            source_type,
            command_str: cmd_to_run,
            file_path: file_to_read,
            win_channel: "System".to_string(),
            capacity,
            display_limit,
        };

        let mut app = Self {
            engine,
            query: String::new(),
            last_query: String::new(),
            display_limit,
            capacity,
            total_matched: 0,
            cached_logs: Vec::new(),
            selected_log: None,
            is_auto_scroll: true,
            prev_table_row_count: 0,
            is_source_running: false,
            show_launch_modal: false,
            source_config,
            rt,
            last_processed_count: 0,
            last_search_time: Instant::now(),
            kill_signal: None,
        };

        app.start_configured_source();
        app.trigger_full_search();

        app
    }

    pub fn start_configured_source(&mut self) {
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

                        self.rt.spawn(async move {
                            let _ = ProcessSource::new(prog, proc_args)
                                .start_stream(source_tx)
                                .await;
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
            SourceType::WinEvent => {
                #[cfg(target_os = "windows")]
                {
                    let channel = config.win_channel.clone();
                    self.rt.spawn(async move {
                        let _ = WinEventSource::new(channel).start_stream(source_tx).await;
                    });
                    self.is_source_running = true;
                }
            }
        }
    }

    pub fn trigger_full_search(&mut self) {
        let (matched, logs) = self
            .engine
            .search_with_count(&self.query, self.display_limit);
        self.total_matched = matched;
        self.cached_logs = logs;
        self.last_query = self.query.clone();
        self.last_processed_count = self.engine.total_processed();
        self.last_search_time = Instant::now();
    }

    pub fn tick(&mut self) {
        let total_processed = self.engine.total_processed();
        let now = Instant::now();

        let query_changed = self.query != self.last_query;
        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(150);

        if query_changed {
            self.trigger_full_search();
        } else if new_logs_arrived {
            let (new_matched_count, new_matching_logs) = self
                .engine
                .filter_incremental(&self.query, self.last_processed_count);

            if new_matched_count > 0 {
                self.total_matched += new_matched_count;
                self.cached_logs.extend(new_matching_logs);

                if self.cached_logs.len() > self.display_limit {
                    let overflow = self.cached_logs.len() - self.display_limit;
                    self.cached_logs.drain(0..overflow);
                }
            }

            self.last_processed_count = total_processed;
            self.last_search_time = now;
        }
    }

    pub fn stop_current_source(&mut self) {
        // Gửi kill signal → forwarder task thoát → drop source_rx
        // → tx.closed() kích hoạt trong ProcessSource → taskkill diệt cây tiến trình
        if let Some(kill_tx) = self.kill_signal.take() {
            let _ = kill_tx.send(());
        }
        self.is_source_running = false;
    }

    pub fn restart_current_source(&mut self) {
        // 1. Dừng tiến trình đang chạy
        self.stop_current_source();

        // 2. Clear toàn bộ log cũ trong Engine & UI state
        if self.source_config.capacity != self.capacity {
            self.capacity = self.source_config.capacity;
            self.engine = Arc::new(SystemEngine::new(self.capacity));
        } else {
            self.engine.clear();
        }

        self.display_limit = self.source_config.display_limit;
        self.cached_logs.clear();
        self.selected_log = None;
        self.total_matched = 0;
        self.last_processed_count = 0;
        self.is_auto_scroll = true;
        self.prev_table_row_count = 0;

        // 3. Khởi tạo lại Nguồn Log mới sạch hoàn toàn
        self.start_configured_source();
        self.trigger_full_search();
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
