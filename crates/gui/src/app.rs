use crate::ui::autocomplete::AutocompleteState;
use crate::ui::history::SearchHistoryState;
use eframe::egui;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use uwu_core::{FileSource, LogEvent, LogSource, ProcessSource, RawLogEntry, SystemEngine};

#[cfg(target_os = "windows")]
use uwu_core::WinEventSource;

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
    /// Cờ yêu cầu cuộn ngay xuống dòng mới nhất (khi bấm nút Latch hoặc phím End)
    pub request_scroll_to_bottom: bool,
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
    /// Tỷ lệ chiều rộng của Log Inspector so với màn hình (mặc định 0.35 = 35%)
    pub inspector_width_ratio: f32,
    pub prev_screen_width: f32,
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
            request_scroll_to_bottom: false,
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
            inspector_width_ratio: 0.35,
            prev_screen_width: 0.0,
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
        if query_changed {
            self.trigger_full_search();
        }

        // Debounced: tự động lưu lịch sử sau 500ms dừng gõ
        self.history_state
            .try_debounced_record(&self.query.clone(), now);

        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(150);

        if self.is_auto_scroll && new_logs_arrived && !query_changed {
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

        // Tự động đồng bộ các trường key mới phát hiện từ logs
        self.column_state.sync_discovered_keys(&self.cached_logs);
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
        self.request_scroll_to_bottom = true;
        self.prev_table_row_count = 0;

        // 3. Khởi tạo lại Nguồn Log mới sạch hoàn toàn
        self.start_configured_source();
        self.trigger_full_search();
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
        self.is_auto_scroll = false;
    }

    pub fn toggle_latch(&mut self) {
        if self.is_auto_scroll {
            self.unlatch();
        } else {
            self.latch();
        }
    }

    pub fn get_available_log_fields(&self) -> Vec<(String, crate::ui::autocomplete::FieldType)> {
        use std::collections::BTreeMap;
        let mut fields_map = BTreeMap::new();

        // Các trường cốt lõi của cấu trúc LogEvent
        fields_map.insert(
            "level".to_string(),
            crate::ui::autocomplete::FieldType::Text,
        );
        fields_map.insert(
            "timestamp".to_string(),
            crate::ui::autocomplete::FieldType::Time,
        );
        fields_map.insert(
            "message".to_string(),
            crate::ui::autocomplete::FieldType::Text,
        );

        // Chỉ thêm các trường thực sự xuất hiện trong dữ liệu log đã nhận (bỏ qua source)
        for log in &self.cached_logs {
            for (key, val) in &log.fields {
                if key == "source" || key == "source_id" {
                    continue;
                }
                if !fields_map.contains_key(key) {
                    let field_type = if val.is_number() {
                        crate::ui::autocomplete::FieldType::Number
                    } else if key.to_lowercase().contains("time")
                        || key.to_lowercase().contains("date")
                        || key.to_lowercase() == "ts"
                    {
                        crate::ui::autocomplete::FieldType::Time
                    } else {
                        crate::ui::autocomplete::FieldType::Text
                    };
                    fields_map.insert(key.clone(), field_type);
                }
            }
        }

        fields_map.into_iter().collect()
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
    use uwu_core::LogLevel;

    fn create_test_app() -> UwuGuiApp {
        let engine = Arc::new(SystemEngine::new(100));
        let rt = tokio::runtime::Handle::current();

        let source_config = SourceConfig {
            source_type: SourceType::Process,
            command_str: String::new(),
            file_path: String::new(),
            win_channel: "System".to_string(),
            capacity: 100,
            display_limit: 50,
        };

        UwuGuiApp {
            engine,
            query: String::new(),
            last_query: String::new(),
            display_limit: 50,
            capacity: 100,
            total_matched: 0,
            cached_logs: Vec::new(),
            selected_log: None,
            is_auto_scroll: true,
            request_scroll_to_bottom: false,
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
            inspector_width_ratio: 0.35,
            prev_screen_width: 0.0,
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
    async fn test_get_available_log_fields_inference() {
        let mut app = create_test_app();

        let mut fields = HashMap::new();
        fields.insert("latency_ms".to_string(), serde_json::json!(250));
        fields.insert(
            "created_time".to_string(),
            serde_json::json!("2026-08-20T10:00:00Z"),
        );
        fields.insert("environment".to_string(), serde_json::json!("production"));

        let log = LogEvent::new(
            "2026-08-20T10:00:00Z",
            LogLevel::Info,
            "test",
            "msg",
            fields,
            "raw",
        );
        app.cached_logs.push(log);

        let available = app.get_available_log_fields();
        let field_types: HashMap<String, FieldType> = available.into_iter().collect();

        // Core fields
        assert_eq!(field_types.get("level"), Some(&FieldType::Text));
        assert_eq!(field_types.get("message"), Some(&FieldType::Text));
        assert_eq!(field_types.get("timestamp"), Some(&FieldType::Time));
        assert_eq!(field_types.get("source"), None);

        // Inferred fields
        assert_eq!(field_types.get("latency_ms"), Some(&FieldType::Number));
        assert_eq!(field_types.get("created_time"), Some(&FieldType::Time));
        assert_eq!(field_types.get("environment"), Some(&FieldType::Text));
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
}
