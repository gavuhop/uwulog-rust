use crate::actions::AppAction;
use crate::state::{ActiveTab, GuiViewState, ScrollCommand, RAW_STREAM_LIMIT};
use eframe::egui;
use std::time::{Duration, Instant};
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{ViewerPrefs, Workspace, WorkspaceSession};

/// Trích xuất nhẹ (Copy) trạng thái stream hiển thị cho bảng log
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSummary {
    pub is_live: bool,
    pub reached_oldest: bool,
    pub is_loading_older: bool,
    pub had_forced_scroll: bool,
}

/// Thực thể đại diện cho một Workspace đang mở trong GUI.
/// Hợp nhất: `session` (SSOT cho Runtime/Engine/Process/Store) + `view` (ViewModel với các Sub-Models chuyên trách).
pub struct GuiSession {
    pub session: WorkspaceSession,
    pub view: GuiViewState,
    /// Đánh dấu tab này đã từng được tự động start source một lần khi mở/kích hoạt.
    pub has_auto_started_source: bool,
    /// Đánh dấu người dùng đã bấm Stop thủ công trên tab này (tránh tự chạy lại khi chuyển tab qua lại).
    pub user_stopped_source: bool,
}

impl GuiSession {
    pub fn new(session: WorkspaceSession) -> Self {
        let view = GuiViewState::with_schema(session.engine.get_schema_map());
        Self {
            session,
            view,
            has_auto_started_source: false,
            user_stopped_source: false,
        }
    }

    pub fn from_workspace(ws: &Workspace, capacity: usize, display_limit: usize) -> Self {
        let session = WorkspaceSession::from_workspace(ws, capacity, display_limit);
        Self::new(session)
    }

    /// Khóa định danh viewer chuẩn hóa của session này.
    #[inline]
    pub fn viewer_key(&self) -> String {
        self.session.viewer_key()
    }

    /// Khôi phục trạng thái hiển thị (query, lịch sử lọc, bố cục cột, độ rộng Inspector) từ `ViewerPrefs`.
    pub fn apply_viewer_prefs(&mut self, prefs: &ViewerPrefs) {
        self.view.search.query = prefs.main_query.clone();
        self.view.search.history.entries = prefs.filter_history.clone();
        if self.view.search.history.entries.is_empty() && !prefs.main_query.trim().is_empty() {
            self.view.search.history.record(&prefs.main_query);
        }
        self.view.shared.columns.restore_from_saved(&prefs.columns);
        if (0.15..=0.85).contains(&prefs.inspector_width_ratio) {
            self.view.shared.inspector.width_ratio = prefs.inspector_width_ratio;
        }
    }

    /// Xuất trạng thái hiển thị hiện tại thành `ViewerPrefs` để lưu xuống `WorkspaceStore::viewers`.
    pub fn to_viewer_prefs(&self) -> ViewerPrefs {
        ViewerPrefs {
            main_query: self.view.search.query.clone(),
            filter_history: self.view.search.history.entries.clone(),
            columns: self.view.shared.columns.to_saved_columns(),
            inspector_width_ratio: self.view.shared.inspector.width_ratio,
        }
    }

    /// Xử lý cập nhật định kỳ cho từng session (incremental search, debounced history, latch scroll sync, unfiltered sync)
    pub fn tick(&mut self, now: Instant) {
        self.session.tick();
        let collisions_ver = self.session.engine.get_collisions_version();
        if collisions_ver != self.view.warnings.last_version {
            let collisions = self.session.engine.get_field_collisions();
            self.view.warnings.sync(collisions_ver, collisions);
        }

        let total_processed = self.session.engine.total_processed();

        let query_changed = self.view.search.query != self.view.search.last_query;
        if query_changed {
            self.view.search.last_query = self.view.search.query.clone();
            if self.view.search.query.trim().is_empty()
                && self.view.active_tab == ActiveTab::Unfiltered
            {
                self.close_unfiltered_stream();
            }
        }

        let q = self.view.search.query.clone();
        self.view.search.history.try_debounced_record(&q, now);

        let prev_processed = self.view.main_tab().last_processed_count();
        let new_logs_arrived = total_processed != prev_processed
            && now.duration_since(self.view.search.last_search_time) > Duration::from_millis(150);

        if self.should_stream_filtered() && new_logs_arrived && !query_changed {
            let (new_matched_count, new_matching_logs) = self
                .session
                .engine
                .filter_incremental(&self.view.search.query, prev_processed);
            self.apply_incremental_logs(new_matched_count, new_matching_logs, total_processed);
        }

        if self.should_stream_unfiltered() && new_logs_arrived {
            let (_new_count, new_logs) = self.session.engine.filter_incremental("", prev_processed);
            self.apply_incremental_unfiltered(new_logs);
        }

        if new_logs_arrived {
            self.sync_engine_schema_if_changed();
            self.view
                .main_tab_mut()
                .set_last_processed_count(total_processed);
            self.view.search.last_search_time = now;
        }
    }

    pub fn trigger_full_search(&mut self) {
        let (matched, logs) = self
            .session
            .engine
            .search_with_count(&self.view.search.query, self.session.display_limit);
        self.apply_search_results(self.view.search.active_query_id, matched, logs, true);
    }

    /// Cập nhật kết quả tìm kiếm bất đồng bộ gửi về từ Background Search Worker (O(1))
    pub fn apply_search_results(
        &mut self,
        query_id: u64,
        total_matched: usize,
        logs: Vec<LogEvent>,
        is_count_exact: bool,
    ) {
        // Loại bỏ kết quả lỗi thời nếu người dùng đã gõ query mới hơn
        if query_id < self.view.search.active_query_id {
            return;
        }

        self.sync_discovered_fields(&logs);
        let prev_processed = self.view.main_tab().last_processed_count();
        let new_processed = self.session.engine.total_processed();
        {
            let main = self.view.main_tab_mut();
            main.set_search_counts(total_matched, is_count_exact);
            main.stream.cached_logs = logs;
            main.stream.reached_oldest = false;
            if matches!(
                main.stream.pending_scroll,
                Some(ScrollCommand::MaintainOffset(_))
            ) {
                main.stream.pending_scroll = None;
            }
            main.set_last_processed_count(new_processed);
        }
        self.view.search.last_query = self.view.search.query.clone();
        self.view.search.last_search_time = Instant::now();
        if self.should_stream_unfiltered() && new_processed > prev_processed {
            let (_count, new_logs) = self.session.engine.filter_incremental("", prev_processed);
            self.apply_incremental_unfiltered(new_logs);
        }
    }

    /// Cập nhật tổng số kết quả chính xác từ Pha 2 (đếm Cold Segments ở nền)
    pub fn apply_search_count_update(&mut self, query_id: u64, total_matched: usize) {
        if query_id < self.view.search.active_query_id {
            return;
        }
        self.view
            .main_tab_mut()
            .set_search_counts(total_matched, true);
    }

    /// Cho phép append log mới vào bảng Filtered khi đang Live hoặc chưa chạm trần `display_limit`
    #[inline]
    pub fn should_stream_filtered(&self) -> bool {
        let main = self.view.main_tab();
        main.stream.is_auto_scroll || main.stream.cached_logs.len() < self.session.display_limit
    }

    /// Cho phép append log mới vào tab Unfiltered khi đang mở live-raw (không phải snapshot `target_id`) và đang Live hoặc chưa chạm trần `RAW_STREAM_LIMIT`
    #[inline]
    pub fn should_stream_unfiltered(&self) -> bool {
        self.view.raw_tab().is_some_and(|raw| {
            raw.stream.is_auto_scroll
                || (raw.target_id().is_none() && raw.stream.cached_logs.len() < RAW_STREAM_LIMIT)
        })
    }

    /// Cập nhật log tăng dần từ background worker cho Filtered stream
    pub fn apply_incremental_logs(
        &mut self,
        matched: usize,
        logs: Vec<LogEvent>,
        total_processed: u64,
    ) {
        if matched > 0 && self.should_stream_filtered() {
            self.sync_discovered_fields(&logs);
            let limit = self.session.display_limit;
            let main = self.view.main_tab_mut();
            let is_auto_scroll = main.stream.is_auto_scroll;
            let added = main.stream.append_logs(logs, limit);
            main.add_matched(if is_auto_scroll { matched } else { added });
        }
        self.view
            .main_tab_mut()
            .set_last_processed_count(total_processed);
    }

    /// Cập nhật log tăng dần từ background worker cho Unfiltered stream
    pub fn apply_incremental_unfiltered(&mut self, logs: Vec<LogEvent>) {
        if self.should_stream_unfiltered() {
            if let Some(raw) = self.view.raw_tab_mut() {
                raw.stream.append_logs(logs, RAW_STREAM_LIMIT);
            }
        }
    }

    /// Cập nhật kết quả phân trang ngược từ background worker
    pub fn apply_reverse_pagination(
        &mut self,
        older: Vec<LogEvent>,
        is_unfiltered: bool,
        reached_oldest: bool,
    ) {
        if !is_unfiltered && !older.is_empty() {
            self.sync_discovered_fields(&older);
        }
        let tab = if is_unfiltered {
            ActiveTab::Unfiltered
        } else {
            ActiveTab::Filtered
        };
        self.view
            .stream_mut(tab)
            .prepend_older_logs(older, reached_oldest);
    }

    pub fn latch(&mut self) {
        if self.view.main_tab_mut().stream.latch() {
            self.trigger_full_search();
        }
    }

    pub fn unlatch(&mut self) {
        self.view.main_tab_mut().stream.unlatch();
    }

    pub fn toggle_latch(&mut self) {
        if self.view.main_tab_mut().stream.toggle_latch() {
            self.view.search.mark_needs_search();
        }
    }

    /// Trích xuất nhẹ (Copy) trạng thái stream hiển thị cho bảng log
    pub fn stream_summary(&self, tab: ActiveTab) -> StreamSummary {
        let stream = self.view.stream(tab);
        StreamSummary {
            is_live: stream.is_auto_scroll,
            reached_oldest: stream.reached_oldest,
            is_loading_older: stream.is_loading_older,
            had_forced_scroll: stream.pending_scroll.is_some(),
        }
    }

    /// Tiêu thụ yêu cầu cuộn (scroll intent) và trả về vị trí cần cuộn đến (nếu có)
    pub fn consume_scroll_request(
        &mut self,
        tab: ActiveTab,
        row_count: usize,
    ) -> Option<(usize, Option<egui::Align>)> {
        self.view.stream_mut(tab).consume_scroll_request(row_count)
    }

    #[inline]
    pub fn sync_engine_schema_if_changed(&mut self) {
        let ver = self.session.engine.get_schema_version();
        if ver != self.view.search.last_synced_schema_version {
            self.view.search.last_synced_schema_version = ver;
            self.view
                .search
                .sync_schema(self.session.engine.get_schema_map());
        }
    }

    pub fn sync_discovered_fields(&mut self, logs: &[LogEvent]) {
        self.view.shared.columns.sync_discovered_keys(logs);
        self.sync_engine_schema_if_changed();
    }

    pub fn toggle_row_highlight(&mut self, id: u64) {
        self.view.shared.inspector.toggle_row_highlight(id);
    }

    pub fn is_row_highlighted(&self, id: &u64) -> bool {
        self.view.shared.inspector.is_row_highlighted(id)
    }

    pub fn toggle_term_highlight(&mut self, term: &str) {
        self.view.shared.inspector.toggle_term_highlight(term);
    }

    pub fn is_term_highlighted(&self, term: &str) -> bool {
        self.view.shared.inspector.is_term_highlighted(term)
    }

    pub fn has_any_highlights(&self) -> bool {
        self.view.shared.inspector.has_any_highlights()
    }

    pub fn clear_all_highlights(&mut self) {
        self.view.shared.inspector.clear_all_highlights();
    }

    pub fn apply_filter_term(&mut self, term: &str) {
        self.view.search.apply_filter_term(term);
        let q = self.view.search.query.clone();
        self.view.search.history.record(&q);
        self.view.search.history.mark_recorded();
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        self.view.search.exclude_filter_term(term);
        let q = self.view.search.query.clone();
        self.view.search.history.record(&q);
        self.view.search.history.mark_recorded();
    }

    pub fn open_unfiltered_stream(&mut self, target_id: Option<u64>) {
        self.view.main_tab_mut().stream.selected_log =
            self.view.shared.inspector.selected_log.clone();
        self.view.open_raw_tab(target_id);
        self.refresh_unfiltered_snapshot();
        self.view.active_tab = ActiveTab::Unfiltered;

        if let Some(id) = target_id {
            let target_event = self
                .view
                .raw_tab()
                .and_then(|raw| raw.stream.cached_logs.iter().find(|e| e.id == id).cloned());
            if let Some(raw) = self.view.raw_tab_mut() {
                raw.stream.selected_log = target_event.clone();
            }
            self.view.shared.inspector.selected_log = target_event;
        } else {
            self.view.shared.inspector.selected_log = self
                .view
                .raw_tab()
                .and_then(|raw| raw.stream.selected_log.clone());
        }
    }

    /// Unified Reverse Pagination / Infinite Scroll Up (Synchronous fallback cho unit test)
    pub fn load_older_logs(&mut self, is_unfiltered: bool, page_size: usize) {
        let tab = if is_unfiltered {
            ActiveTab::Unfiltered
        } else {
            ActiveTab::Filtered
        };
        if self.view.stream(tab).reached_oldest {
            return;
        }
        self.view.stream_mut(tab).unlatch();
        let Some(before_id) = self.view.stream(tab).cached_logs.first().map(|e| e.id) else {
            return;
        };
        let query = if is_unfiltered {
            ""
        } else {
            self.view.search.query.as_str()
        };
        let older = self
            .session
            .engine
            .search_before(query, before_id, page_size);
        let reached = older.len() < page_size;
        self.apply_reverse_pagination(older, is_unfiltered, reached);
    }

    #[inline]
    pub fn load_more_older_logs(&mut self, page_size: usize) {
        self.load_older_logs(false, page_size);
    }

    #[inline]
    pub fn load_more_older_unfiltered(&mut self, page_size: usize) {
        self.load_older_logs(true, page_size);
    }

    /// Trả về danh sách logs hiện đang được hiển thị theo tab tích cực (Filtered hoặc Unfiltered)
    #[inline]
    pub fn active_logs(&self) -> &[LogEvent] {
        &self.view.stream(self.view.active_tab).cached_logs
    }

    /// Đặt yêu cầu cuộn bảng tới dòng chỉ định kèm kiểu căn lề tương ứng cho tab hiện tại
    #[inline]
    pub fn scroll_to_row(&mut self, row: usize, align: Option<egui::Align>) {
        self.view.stream_mut(self.view.active_tab).pending_scroll =
            Some(ScrollCommand::Row(row, align));
    }

    /// Trả về chỉ số dòng log đầu tiên đang nhìn thấy trong viewport
    #[inline]
    pub fn first_visible_row(&self) -> usize {
        self.view
            .stream(self.view.active_tab)
            .first_visible_row
            .unwrap_or(0)
    }

    fn on_tab_relatched(&mut self, tab: ActiveTab) {
        match tab {
            ActiveTab::Filtered => self.view.search.mark_needs_search(),
            ActiveTab::Unfiltered => {
                self.refresh_unfiltered_snapshot();
                if let Some(raw) = self.view.raw_tab_mut() {
                    raw.stream.pending_scroll = Some(ScrollCommand::Bottom);
                }
            }
        }
    }

    /// Tạm dừng auto-scroll (latch) cho tab đang xem
    pub fn unlatch_active(&mut self) {
        self.view.stream_mut(self.view.active_tab).unlatch();
    }

    /// Kích hoạt auto-scroll (latch) cho tab đang xem
    pub fn latch_active(&mut self) {
        let tab = self.view.active_tab;
        if self.view.stream_mut(tab).latch() {
            self.on_tab_relatched(tab);
        }
    }

    /// Đảo trạng thái Live/Paused cho tab đang xem
    pub fn toggle_active_latch(&mut self) {
        let tab = self.view.active_tab;
        if self.view.stream_mut(tab).toggle_latch() {
            self.on_tab_relatched(tab);
        }
    }

    /// Cập nhật dòng log đang chọn cho tab hiện tại và đồng bộ sang Inspector
    pub fn set_selected_log(&mut self, log: Option<LogEvent>) {
        let active = self.view.active_tab;
        self.view.stream_mut(active).selected_log = log.clone();
        self.view.shared.inspector.selected_log = log;
    }

    /// Chuyển đổi giữa tab Filtered (Main) và Unfiltered (Raw), lưu nhớ và khôi phục dòng đang chọn
    pub fn switch_tab(&mut self, next_tab: ActiveTab) {
        if self.view.active_tab == next_tab {
            return;
        }
        let prev_tab = self.view.active_tab;
        let current_selected = self.view.shared.inspector.selected_log.clone();
        if let Some(prev) = self.view.tab_mut(prev_tab) {
            prev.stream.selected_log = current_selected;
        }
        self.view.active_tab = next_tab;
        self.view.shared.inspector.selected_log = self.view.stream(next_tab).selected_log.clone();
    }

    /// Di chuyển lựa chọn dòng log (khi đã pick dòng) hoặc cuộn bảng với bước nhảy tuỳ ý
    pub fn navigate_row_by_step(&mut self, step: usize, is_up: bool) {
        let row_count = self.active_logs().len();
        if row_count == 0 {
            return;
        }

        if let Some(selected_id) = self
            .view
            .shared
            .inspector
            .selected_log
            .as_ref()
            .map(|l| l.id)
        {
            // 1. Khi đang pick/chọn một dòng log: phím lên/xuống/page chuyển dòng tương ứng
            let logs = self.active_logs();
            let curr = logs.iter().position(|e| e.id == selected_id).unwrap_or(0);
            let next = if is_up {
                curr.saturating_sub(step)
            } else {
                (curr + step).min(row_count - 1)
            };
            self.set_selected_log(Some(logs[next].clone()));
            self.scroll_to_row(next, None);
            self.unlatch_active();
        } else {
            // 2. Khi không focus/chọn dòng nào: phím lên/xuống/page dùng để scroll bảng
            let curr = self.first_visible_row();
            let next = if is_up {
                curr.saturating_sub(step)
            } else {
                (curr + step).min(row_count - 1)
            };
            let logs = self.active_logs();
            if step == 1 && next < logs.len() {
                self.set_selected_log(Some(logs[next].clone()));
            }
            self.scroll_to_row(next, Some(egui::Align::Min));
            if !is_up && next >= row_count - 1 {
                self.latch_active();
            } else {
                self.unlatch_active();
            }
        }
    }

    /// Di chuyển lựa chọn dòng log (khi đã pick dòng) hoặc cuộn bảng (khi chưa chọn dòng nào)
    pub fn navigate_row(&mut self, is_up: bool) {
        let step = if self.view.shared.inspector.selected_log.is_some() {
            1
        } else {
            4
        };
        self.navigate_row_by_step(step, is_up);
    }

    /// Nhảy một trang màn hình log (Page Up / Page Down: cuộn bảng 20 dòng nhưng vẫn giữ nguyên dòng log đang chọn nếu có)
    pub fn navigate_page(&mut self, is_up: bool) {
        let row_count = self.active_logs().len();
        if row_count == 0 {
            return;
        }
        const PAGE_STEP: usize = 20;
        let curr = self.first_visible_row();
        let next = if is_up {
            curr.saturating_sub(PAGE_STEP)
        } else {
            (curr + PAGE_STEP).min(row_count - 1)
        };
        self.scroll_to_row(next, Some(egui::Align::Min));
        if !is_up && next >= row_count - 1 {
            self.latch_active();
        } else {
            self.unlatch_active();
        }
    }

    /// Nhảy tức thì lên dòng log đầu tiên trên cùng (Top / Oldest row)
    pub fn scroll_to_top(&mut self) {
        let row_count = self.active_logs().len();
        if row_count == 0 {
            return;
        }
        if self.view.shared.inspector.selected_log.is_some() {
            let logs = self.active_logs();
            self.set_selected_log(Some(logs[0].clone()));
        }
        self.scroll_to_row(0, Some(egui::Align::Min));
        self.unlatch_active();
    }

    /// Nhảy tức thì xuống dòng log mới nhất ở đáy bảng (Bottom / Latest row) và bật Latch
    pub fn scroll_to_bottom(&mut self) {
        let row_count = self.active_logs().len();
        if row_count == 0 {
            return;
        }
        let target_idx = row_count - 1;
        if self.view.shared.inspector.selected_log.is_some() {
            let logs = self.active_logs();
            self.set_selected_log(Some(logs[target_idx].clone()));
        }
        self.scroll_to_row(target_idx, Some(egui::Align::Max));
        self.latch_active();
    }

    /// Gửi yêu cầu cuộn ngang bảng một khoảng delta (pixel)
    pub fn scroll_horizontal(&mut self, delta: f32) {
        let stream = self.view.stream_mut(self.view.active_tab);
        let curr = stream.request_horizontal_scroll.unwrap_or(0.0);
        stream.request_horizontal_scroll = Some(curr + delta);
    }

    /// Tiêu thụ delta cuộn ngang
    pub fn consume_horizontal_scroll(&mut self, tab: ActiveTab) -> Option<f32> {
        self.view.stream_mut(tab).request_horizontal_scroll.take()
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        let target_id = self.view.raw_tab().and_then(|r| r.target_id());
        let (target_idx, unfiltered) = self
            .session
            .engine
            .get_unfiltered_events(target_id, RAW_STREAM_LIMIT);
        if let Some(raw) = self.view.raw_tab_mut() {
            raw.stream.reached_oldest = false;
            raw.stream.cached_logs = unfiltered;
            raw.stream.pending_scroll =
                target_idx.map(|idx| ScrollCommand::Row(idx, Some(egui::Align::Center)));
        }
    }

    pub fn close_unfiltered_stream(&mut self) {
        self.switch_tab(ActiveTab::Filtered);
        self.view.close_raw_tab();
    }

    pub fn focus_in_main_and_clear_filter(&mut self) {
        let target_event = self.view.raw_tab().and_then(|raw| {
            let target_id = raw.target_id()?;
            raw.stream
                .cached_logs
                .iter()
                .find(|e| e.id == target_id)
                .cloned()
        });
        if let Some(target_event) = target_event {
            self.view.main_tab_mut().stream.selected_log = Some(target_event.clone());
            self.view.shared.inspector.selected_log = Some(target_event);
        }
        self.view.search.clear();
        self.close_unfiltered_stream();
    }

    /// Xử lý các hành động tác động trực tiếp lên View State & Engine của Session
    pub fn handle_action(&mut self, action: &AppAction) -> bool {
        match action {
            AppAction::SelectLog(log) => {
                self.set_selected_log(log.clone());
                true
            }
            AppAction::NavigateUp => {
                self.navigate_row(true);
                true
            }
            AppAction::NavigateDown => {
                self.navigate_row(false);
                true
            }
            AppAction::ScrollTableLeft => {
                self.scroll_horizontal(-120.0);
                true
            }
            AppAction::ScrollTableRight => {
                self.scroll_horizontal(120.0);
                true
            }
            AppAction::ScrollTableLeftMost => {
                self.scroll_horizontal(-999_999.0);
                true
            }
            AppAction::PageUp => {
                self.navigate_page(true);
                true
            }
            AppAction::PageDown => {
                self.navigate_page(false);
                true
            }
            AppAction::ScrollToTop => {
                self.scroll_to_top();
                true
            }
            AppAction::ScrollToBottom => {
                self.scroll_to_bottom();
                true
            }
            AppAction::SwitchTab(tab) => {
                if *tab == ActiveTab::Unfiltered {
                    if self.view.search.query.trim().is_empty() {
                        return false;
                    }
                    if !self.view.has_raw_tab() {
                        self.open_unfiltered_stream(None);
                    }
                }
                self.switch_tab(*tab);
                true
            }
            AppAction::ToggleStreamView => {
                if self.view.search.query.trim().is_empty() {
                    return false;
                }
                let next = match self.view.active_tab {
                    ActiveTab::Filtered => ActiveTab::Unfiltered,
                    ActiveTab::Unfiltered => ActiveTab::Filtered,
                };
                if next == ActiveTab::Unfiltered && !self.view.has_raw_tab() {
                    self.open_unfiltered_stream(None);
                }
                self.switch_tab(next);
                true
            }
            AppAction::ApplyFilterTerm(term) => {
                self.apply_filter_term(term);
                true
            }
            AppAction::ExcludeFilterTerm(term) => {
                self.exclude_filter_term(term);
                true
            }
            AppAction::ClearQuery => {
                self.view.search.clear();
                if self.view.active_tab == ActiveTab::Unfiltered {
                    self.close_unfiltered_stream();
                }
                true
            }
            AppAction::ToggleRowHighlight(id) => {
                self.toggle_row_highlight(*id);
                true
            }
            AppAction::ToggleTermHighlight(term) => {
                self.toggle_term_highlight(term);
                true
            }
            AppAction::ClearAllHighlights => {
                self.clear_all_highlights();
                true
            }
            AppAction::DismissFieldWarnings => {
                self.view.warnings.dismiss_current();
                true
            }
            AppAction::ToggleLatch => {
                self.toggle_active_latch();
                true
            }
            AppAction::Unlatch => {
                self.unlatch_active();
                true
            }
            AppAction::Latch => {
                self.latch_active();
                true
            }
            AppAction::RefreshUnfilteredSnapshot => {
                self.refresh_unfiltered_snapshot();
                true
            }
            AppAction::OpenUnfilteredStream(id) => {
                if self.view.search.query.trim().is_empty() {
                    return false;
                }
                self.open_unfiltered_stream(*id);
                true
            }
            AppAction::ViewRawContext => {
                if self.view.active_tab == ActiveTab::Unfiltered {
                    self.switch_tab(ActiveTab::Filtered);
                    return true;
                }
                if self.view.search.query.trim().is_empty() {
                    return false;
                }
                let target_id = self
                    .view
                    .shared
                    .inspector
                    .selected_log
                    .as_ref()
                    .map(|l| l.id);
                self.open_unfiltered_stream(target_id);
                true
            }
            AppAction::CloseUnfilteredStream => {
                self.close_unfiltered_stream();
                true
            }
            AppAction::FocusInMainAndClearFilter => {
                self.focus_in_main_and_clear_filter();
                true
            }
            AppAction::FocusSearch => {
                self.view.search.focus_requested = true;
                false
            }
            AppAction::ToggleSearchHistory => {
                let opened = self.view.search.history.toggle_popup();
                if opened {
                    self.view.search.autocomplete.is_open = false;
                    self.view.search.autocomplete.suggestions.clear();
                }
                true
            }
            AppAction::CommitSearch => {
                let q = self.view.search.query.clone();
                self.view.search.history.record(&q);
                self.view.search.history.mark_recorded();
                true
            }
            AppAction::AutocompleteNext
                if self.view.search.autocomplete.is_open
                    && !self.view.search.autocomplete.suggestions.is_empty() =>
            {
                self.view.search.autocomplete.selected_index =
                    (self.view.search.autocomplete.selected_index + 1)
                        % self.view.search.autocomplete.suggestions.len();
                true
            }
            AppAction::AutocompletePrev
                if self.view.search.autocomplete.is_open
                    && !self.view.search.autocomplete.suggestions.is_empty() =>
            {
                if self.view.search.autocomplete.selected_index == 0 {
                    self.view.search.autocomplete.selected_index =
                        self.view.search.autocomplete.suggestions.len() - 1;
                } else {
                    self.view.search.autocomplete.selected_index -= 1;
                }
                true
            }
            AppAction::AutocompleteConfirm if self.view.search.autocomplete.is_open => {
                if let Some(item) = self
                    .view
                    .search
                    .autocomplete
                    .suggestions
                    .get(self.view.search.autocomplete.selected_index)
                    .cloned()
                {
                    self.view.search.apply_autocomplete_suggestion(&item);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl std::ops::Deref for GuiSession {
    type Target = GuiViewState;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl std::ops::DerefMut for GuiSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.view
    }
}
