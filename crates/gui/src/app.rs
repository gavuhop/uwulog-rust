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

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Filtered,
    Unfiltered,
}

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
    pub highlighted_row_ids: std::collections::HashSet<uuid::Uuid>,
    pub highlighted_terms: std::collections::HashSet<String>,
    /// Tỷ lệ chiều rộng của Log Inspector so với màn hình (mặc định 0.35 = 35%)
    pub inspector_width_ratio: f32,
    pub prev_screen_width: f32,
    pub unfiltered_state: UnfilteredViewState,
    pub global_seen_at_pause: u64,
    pub filtered_seen_at_pause: usize,
    pub filtered_processed_at_pause: u64,
}

#[derive(Clone, Debug, Default)]
pub struct UnfilteredViewState {
    pub is_open: bool,
    pub target_id: Option<uuid::Uuid>,
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
            active_tab: ActiveTab::Filtered,
            query: String::new(),
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
    pub fn tick(&mut self) {
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
                self.has_new_data = true;
            }

            self.last_processed_count = total_processed;
            self.last_search_time = now;
        }

        if self.is_auto_scroll {
            self.global_seen_at_pause = total_processed;
            self.filtered_seen_at_pause = self.total_matched;
            self.filtered_processed_at_pause = total_processed;
        }

        // Bảng Unfiltered: Giới hạn buffer 500 logs cho việc soi context log gốc
        self.unfiltered_state.has_new_data = false;
        if self.unfiltered_state.is_open && self.unfiltered_state.is_live && new_logs_arrived {
            let (new_count, new_logs) = self
                .engine
                .filter_incremental("", self.last_processed_count);
            if new_count > 0 {
                self.unfiltered_state.cached_unfiltered.extend(new_logs);
                if self.unfiltered_state.cached_unfiltered.len() > RAW_STREAM_LIMIT {
                    let overflow = self.unfiltered_state.cached_unfiltered.len() - RAW_STREAM_LIMIT;
                    self.unfiltered_state.cached_unfiltered.drain(0..overflow);
                }
                self.unfiltered_state.has_new_data = true;
            }
        }

        // Tự động đồng bộ các trường key mới phát hiện từ logs
        self.column_state.sync_discovered_keys(&self.cached_logs);
    }

    pub fn stop_current_source(&mut self) {
        if let Some(kill_tx) = self.kill_signal.take() {
            let _ = kill_tx.send(());
        }
        self.is_source_running = false;
    }

    pub fn restart_current_source(&mut self) {
        self.stop_current_source();
        if self.source_config.capacity != self.capacity {
            self.capacity = self.source_config.capacity;
            self.engine = Arc::new(SystemEngine::new(self.capacity));
        } else {
            self.engine.clear();
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
        self.cached_logs = logs;
        self.last_query = self.query.clone();
        self.last_processed_count = self.engine.total_processed();
        self.last_search_time = Instant::now();
        self.global_seen_at_pause = self.last_processed_count;
        self.filtered_seen_at_pause = self.total_matched;
        self.filtered_processed_at_pause = self.last_processed_count;
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

    pub fn toggle_row_highlight(&mut self, id: uuid::Uuid) {
        if self.highlighted_row_ids.contains(&id) {
            self.highlighted_row_ids.remove(&id);
        } else {
            self.highlighted_row_ids.insert(id);
        }
    }

    pub fn is_row_highlighted(&self, id: &uuid::Uuid) -> bool {
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

    pub fn open_unfiltered_stream(&mut self, target_id: Option<uuid::Uuid>) {
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
    use uwu_core::{LogLevel, RawPayload};

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
        let id1 = uuid::Uuid::new_v4();
        let id2 = uuid::Uuid::new_v4();

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

    #[tokio::test]
    async fn test_unfiltered_stream_open_close_and_focus() {
        let mut app = create_test_app();
        let tx = app.engine.get_channel();

        for i in 0..5 {
            let log = RawLogEntry {
                source_id: "test".to_string(),
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
                source_id: "test".to_string(),
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
                source_id: "test".to_string(),
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
                source_id: "test".to_string(),
                payload: RawPayload::Text(format!("[{}] Message {}", level, i)),
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
                source_id: "test".to_string(),
                payload: RawPayload::Text(format!("[{}] Message {}", level, i)),
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
                source_id: "test".to_string(),
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
                source_id: "test".to_string(),
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
}
