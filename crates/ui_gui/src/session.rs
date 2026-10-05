use crate::actions::AppAction;
use crate::state::{ActiveTab, GuiViewState, RAW_STREAM_LIMIT};
use eframe::egui;
use std::time::{Duration, Instant};
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{Workspace, WorkspaceSession};

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
}

impl GuiSession {
    pub fn new(session: WorkspaceSession) -> Self {
        let view = GuiViewState::with_schema(session.engine.get_schema_map());
        Self { session, view }
    }

    pub fn from_workspace(ws: &Workspace, capacity: usize, display_limit: usize) -> Self {
        let session = WorkspaceSession::from_workspace(ws, capacity, display_limit);
        let mut view = GuiViewState::with_schema(session.engine.get_schema_map());
        view.search.query = ws.last_query.clone();
        Self { session, view }
    }

    /// Xử lý cập nhật định kỳ cho từng session (incremental search, debounced history, latch scroll sync, unfiltered sync)
    pub fn tick(&mut self, now: Instant) {
        self.session.tick();
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

        let new_logs_arrived = total_processed != self.view.viewport.last_processed_count
            && now.duration_since(self.view.search.last_search_time) > Duration::from_millis(150);

        self.view.viewport.has_new_data = false;
        let prev_processed = self.view.viewport.last_processed_count;

        if self.view.viewport.is_auto_scroll && new_logs_arrived && !query_changed {
            let (new_matched_count, new_matching_logs) = self
                .session
                .engine
                .filter_incremental(&self.view.search.query, prev_processed);
            self.apply_incremental_logs(new_matched_count, new_matching_logs, total_processed);
        }

        if !self.view.viewport.is_auto_scroll && new_logs_arrived {
            let (new_matched, _) = self.session.engine.filter_incremental(
                &self.view.search.query,
                self.view.viewport.pause_snapshot.filtered_processed,
            );
            self.view.viewport.pause_snapshot.paused_new_matched_count = new_matched;
        }

        if self.view.unfiltered.is_open && self.view.unfiltered.is_live && new_logs_arrived {
            let (_new_count, new_logs) = self.session.engine.filter_incremental("", prev_processed);
            self.apply_incremental_unfiltered(new_logs);
        }

        if new_logs_arrived {
            self.view
                .search
                .sync_schema(self.session.engine.get_schema_map());
            self.view.viewport.last_processed_count = total_processed;
            self.view.search.last_search_time = now;
        }
    }

    pub fn trigger_full_search(&mut self) {
        let (matched, logs) = self
            .session
            .engine
            .search_with_count(&self.view.search.query, self.session.display_limit);
        self.apply_search_results(self.view.search.active_query_id, matched, logs);
    }

    /// Cập nhật kết quả tìm kiếm bất đồng bộ gửi về từ Background Search Worker (O(1))
    pub fn apply_search_results(
        &mut self,
        query_id: u64,
        total_matched: usize,
        logs: Vec<LogEvent>,
    ) {
        // Loại bỏ kết quả lỗi thời nếu người dùng đã gõ query mới hơn
        if query_id < self.view.search.active_query_id {
            return;
        }

        self.view.viewport.total_matched = total_matched;
        self.sync_discovered_fields(&logs);
        self.view.viewport.cached_logs = logs;
        self.view.viewport.reached_oldest = false;
        self.view.viewport.request_maintain_scroll_offset = None;
        self.view.search.last_query = self.view.search.query.clone();
        self.view.viewport.last_processed_count = self.session.engine.total_processed();
        self.view.search.last_search_time = Instant::now();
        self.view
            .viewport
            .record_pause(self.view.viewport.last_processed_count);
        self.view.viewport.has_new_data = true;
    }

    /// Cập nhật log tăng dần từ background worker cho Filtered stream
    pub fn apply_incremental_logs(
        &mut self,
        matched: usize,
        logs: Vec<LogEvent>,
        total_processed: u64,
    ) {
        if matched > 0 && self.view.viewport.is_auto_scroll {
            self.view.viewport.total_matched += matched;
            self.sync_discovered_fields(&logs);
            self.view.viewport.cached_logs.extend(logs);

            if self.view.viewport.cached_logs.len() > self.session.display_limit {
                let overflow = self.view.viewport.cached_logs.len() - self.session.display_limit;
                self.view.viewport.cached_logs.drain(0..overflow);
            }
            self.view.viewport.has_new_data = true;
        }
        if self.view.viewport.is_auto_scroll {
            self.view.viewport.record_pause(total_processed);
        }
        self.view.viewport.last_processed_count = total_processed;
    }

    /// Cập nhật log tăng dần từ background worker cho Unfiltered stream
    pub fn apply_incremental_unfiltered(&mut self, logs: Vec<LogEvent>) {
        if self.view.unfiltered.is_open && self.view.unfiltered.is_live && !logs.is_empty() {
            self.view.unfiltered.cached_unfiltered.extend(logs);
            if self.view.unfiltered.cached_unfiltered.len() > RAW_STREAM_LIMIT {
                let overflow = self.view.unfiltered.cached_unfiltered.len() - RAW_STREAM_LIMIT;
                self.view.unfiltered.cached_unfiltered.drain(0..overflow);
            }
            self.view.unfiltered.has_new_data = true;
        }
    }

    /// Cập nhật kết quả phân trang ngược từ background worker
    pub fn apply_reverse_pagination(
        &mut self,
        older: Vec<LogEvent>,
        is_unfiltered: bool,
        reached_oldest: bool,
    ) {
        let count = older.len();
        if is_unfiltered {
            self.view.unfiltered.is_loading_older = false;
            self.view.unfiltered.reached_oldest = reached_oldest;
            if count > 0 {
                let mut new_cache = older;
                new_cache.extend(std::mem::take(&mut self.view.unfiltered.cached_unfiltered));
                self.view.unfiltered.cached_unfiltered = new_cache;
                self.view.unfiltered.request_maintain_scroll_offset = Some(count);
            }
        } else {
            self.view.viewport.is_loading_older = false;
            self.view.viewport.reached_oldest = reached_oldest;
            if count > 0 {
                self.sync_discovered_fields(&older);
                let mut new_cache = older;
                new_cache.extend(std::mem::take(&mut self.view.viewport.cached_logs));
                self.view.viewport.cached_logs = new_cache;
                self.view.viewport.request_maintain_scroll_offset = Some(count);
            }
        }
    }

    pub fn latch(&mut self) {
        if self.view.viewport.latch() {
            self.trigger_full_search();
        }
    }

    pub fn unlatch(&mut self) {
        let total = self.session.engine.total_processed();
        self.view.viewport.unlatch(total);
    }

    pub fn toggle_latch(&mut self) {
        let total = self.session.engine.total_processed();
        if self.view.viewport.toggle_latch(total) {
            self.view.search.mark_needs_search();
        }
    }

    /// Trích xuất nhẹ (Copy) trạng thái stream hiển thị cho bảng log
    pub fn stream_summary(&self, tab: ActiveTab) -> StreamSummary {
        match tab {
            ActiveTab::Filtered => StreamSummary {
                is_live: self.view.viewport.is_auto_scroll,
                reached_oldest: self.view.viewport.reached_oldest,
                is_loading_older: self.view.viewport.is_loading_older,
                had_forced_scroll: self.view.viewport.request_scroll_to_bottom
                    || self.view.viewport.request_maintain_scroll_offset.is_some(),
            },
            ActiveTab::Unfiltered => StreamSummary {
                is_live: self.view.unfiltered.is_live,
                reached_oldest: self.view.unfiltered.reached_oldest,
                is_loading_older: self.view.unfiltered.is_loading_older,
                had_forced_scroll: self.view.unfiltered.request_scroll_to_bottom
                    || self.view.unfiltered.request_scroll_to_target
                    || self
                        .view
                        .unfiltered
                        .request_maintain_scroll_offset
                        .is_some(),
            },
        }
    }

    /// Tiêu thụ yêu cầu cuộn (scroll intent) và trả về vị trí cần cuộn đến (nếu có)
    pub fn consume_scroll_request(
        &mut self,
        tab: ActiveTab,
        row_count: usize,
    ) -> Option<(usize, Option<egui::Align>)> {
        match tab {
            ActiveTab::Filtered => {
                if let Some(target) = self.view.viewport.request_scroll_to_row.take() {
                    Some(target)
                } else if let Some(offset) =
                    self.view.viewport.request_maintain_scroll_offset.take()
                {
                    Some((offset, Some(egui::Align::Min)))
                } else if row_count > 0 {
                    let force = self.view.viewport.request_scroll_to_bottom;
                    let is_live = self.view.viewport.is_auto_scroll;
                    self.view.viewport.prev_table_row_count = row_count;
                    if force || is_live {
                        self.view.viewport.request_scroll_to_bottom = false;
                        Some((row_count - 1, Some(egui::Align::Max)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            ActiveTab::Unfiltered => {
                if let Some(target) = self.view.unfiltered.request_scroll_to_row.take() {
                    Some(target)
                } else if let Some(offset) =
                    self.view.unfiltered.request_maintain_scroll_offset.take()
                {
                    Some((offset, Some(egui::Align::Min)))
                } else if self.view.unfiltered.request_scroll_to_target && row_count > 0 {
                    self.view.unfiltered.request_scroll_to_target = false;
                    self.view
                        .unfiltered
                        .target_index
                        .map(|idx| (idx, Some(egui::Align::Center)))
                } else if row_count > 0 {
                    let force = self.view.unfiltered.request_scroll_to_bottom;
                    let is_live = self.view.unfiltered.is_live;
                    if force || is_live {
                        self.view.unfiltered.request_scroll_to_bottom = false;
                        Some((row_count - 1, Some(egui::Align::Max)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }

    pub fn sync_discovered_fields(&mut self, logs: &[LogEvent]) {
        self.view.columns.sync_discovered_keys(logs);
        self.view
            .search
            .sync_schema(self.session.engine.get_schema_map());
    }

    pub fn toggle_row_highlight(&mut self, id: u64) {
        self.view.inspector.toggle_row_highlight(id);
    }

    pub fn is_row_highlighted(&self, id: &u64) -> bool {
        self.view.inspector.is_row_highlighted(id)
    }

    pub fn toggle_term_highlight(&mut self, term: &str) {
        self.view.inspector.toggle_term_highlight(term);
    }

    pub fn is_term_highlighted(&self, term: &str) -> bool {
        self.view.inspector.is_term_highlighted(term)
    }

    pub fn has_any_highlights(&self) -> bool {
        self.view.inspector.has_any_highlights()
    }

    pub fn clear_all_highlights(&mut self) {
        self.view.inspector.clear_all_highlights();
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
        self.view.unfiltered.is_open = true;
        self.view.unfiltered.target_id = target_id;
        self.view.unfiltered.is_live = target_id.is_none();
        self.refresh_unfiltered_snapshot();
        self.view.unfiltered.has_new_data = false;
        self.view.active_tab = ActiveTab::Unfiltered;
    }

    /// Unified Reverse Pagination / Infinite Scroll Up (Synchronous fallback cho unit test)
    pub fn load_older_logs(&mut self, is_unfiltered: bool, page_size: usize) {
        let (query, oldest_id) = if is_unfiltered {
            if self.view.unfiltered.reached_oldest {
                return;
            }
            self.unlatch_unfiltered();
            (
                "",
                self.view.unfiltered.cached_unfiltered.first().map(|e| e.id),
            )
        } else {
            if self.view.viewport.reached_oldest {
                return;
            }
            self.unlatch();
            (
                self.view.search.query.as_str(),
                self.view.viewport.cached_logs.first().map(|e| e.id),
            )
        };

        let Some(before_id) = oldest_id else {
            return;
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
        match self.view.active_tab {
            ActiveTab::Filtered => &self.view.viewport.cached_logs,
            ActiveTab::Unfiltered => &self.view.unfiltered.cached_unfiltered,
        }
    }

    /// Đặt yêu cầu cuộn bảng tới dòng chỉ định kèm kiểu căn lề tương ứng cho tab hiện tại
    #[inline]
    pub fn scroll_to_row(&mut self, row: usize, align: Option<egui::Align>) {
        match self.view.active_tab {
            ActiveTab::Filtered => self.view.viewport.request_scroll_to_row = Some((row, align)),
            ActiveTab::Unfiltered => {
                self.view.unfiltered.request_scroll_to_row = Some((row, align))
            }
        }
    }

    /// Trả về chỉ số dòng log đầu tiên đang nhìn thấy trong viewport
    #[inline]
    pub fn first_visible_row(&self) -> usize {
        match self.view.active_tab {
            ActiveTab::Filtered => self.view.viewport.first_visible_row.unwrap_or(0),
            ActiveTab::Unfiltered => self.view.unfiltered.first_visible_row.unwrap_or(0),
        }
    }

    /// Tạm dừng auto-scroll (latch) cho tab đang xem
    pub fn unlatch_active(&mut self) {
        match self.view.active_tab {
            ActiveTab::Filtered => self.unlatch(),
            ActiveTab::Unfiltered => self.unlatch_unfiltered(),
        }
    }

    /// Kích hoạt auto-scroll (latch) cho tab đang xem
    pub fn latch_active(&mut self) {
        match self.view.active_tab {
            ActiveTab::Filtered => {
                if self.view.viewport.latch() {
                    self.view.search.mark_needs_search();
                }
            }
            ActiveTab::Unfiltered => self.view.unfiltered.is_live = true,
        }
    }

    /// Di chuyển lựa chọn dòng log (khi đã pick dòng) hoặc cuộn bảng với bước nhảy tuỳ ý
    pub fn navigate_row_by_step(&mut self, step: usize, is_up: bool) {
        let row_count = self.active_logs().len();
        if row_count == 0 {
            return;
        }

        if let Some(selected_id) = self.view.inspector.selected_log.as_ref().map(|l| l.id) {
            // 1. Khi đang pick/chọn một dòng log: phím lên/xuống/page chuyển dòng tương ứng
            let logs = self.active_logs();
            let curr = logs.iter().position(|e| e.id == selected_id).unwrap_or(0);
            let next = if is_up {
                curr.saturating_sub(step)
            } else {
                (curr + step).min(row_count - 1)
            };
            self.view.inspector.selected_log = Some(logs[next].clone());
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
                self.view.inspector.selected_log = Some(logs[next].clone());
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
        let step = if self.view.inspector.selected_log.is_some() {
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
        if self.view.inspector.selected_log.is_some() {
            let logs = self.active_logs();
            self.view.inspector.selected_log = Some(logs[0].clone());
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
        if self.view.inspector.selected_log.is_some() {
            let logs = self.active_logs();
            self.view.inspector.selected_log = Some(logs[target_idx].clone());
        }
        self.scroll_to_row(target_idx, Some(egui::Align::Max));
        self.latch_active();
    }

    /// Gửi yêu cầu cuộn ngang bảng một khoảng delta (pixel)
    pub fn scroll_horizontal(&mut self, delta: f32) {
        match self.view.active_tab {
            ActiveTab::Filtered => {
                let curr = self.view.viewport.request_horizontal_scroll.unwrap_or(0.0);
                self.view.viewport.request_horizontal_scroll = Some(curr + delta);
            }
            ActiveTab::Unfiltered => {
                let curr = self
                    .view
                    .unfiltered
                    .request_horizontal_scroll
                    .unwrap_or(0.0);
                self.view.unfiltered.request_horizontal_scroll = Some(curr + delta);
            }
        }
    }

    /// Tiêu thụ delta cuộn ngang
    pub fn consume_horizontal_scroll(&mut self, tab: ActiveTab) -> Option<f32> {
        match tab {
            ActiveTab::Filtered => self.view.viewport.request_horizontal_scroll.take(),
            ActiveTab::Unfiltered => self.view.unfiltered.request_horizontal_scroll.take(),
        }
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        self.view.unfiltered.snapshot_processed_count = self.session.engine.total_processed();
        self.view.unfiltered.reached_oldest = false;
        self.view.unfiltered.request_maintain_scroll_offset = None;
        let (target_idx, unfiltered) = self
            .session
            .engine
            .get_unfiltered_events(self.view.unfiltered.target_id, RAW_STREAM_LIMIT);
        self.view.unfiltered.cached_unfiltered = unfiltered;
        self.view.unfiltered.target_index = target_idx;
        self.view.unfiltered.request_scroll_to_target = true;
    }

    pub fn toggle_unfiltered_live(&mut self) {
        self.view.unfiltered.is_live = !self.view.unfiltered.is_live;
        if self.view.unfiltered.is_live {
            self.refresh_unfiltered_snapshot();
            self.view.unfiltered.request_scroll_to_bottom = true;
        } else {
            self.view.unfiltered.snapshot_processed_count = self.session.engine.total_processed();
        }
    }

    pub fn unlatch_unfiltered(&mut self) {
        if !self.view.unfiltered.is_live {
            return;
        }
        self.view.unfiltered.is_live = false;
        self.view.unfiltered.snapshot_processed_count = self.session.engine.total_processed();
    }

    pub fn close_unfiltered_stream(&mut self) {
        self.view.unfiltered.is_open = false;
        self.view.unfiltered.target_id = None;
        self.view.unfiltered.target_index = None;
        self.view.unfiltered.cached_unfiltered.clear();
        self.view.active_tab = ActiveTab::Filtered;
    }

    pub fn focus_in_main_and_clear_filter(&mut self) {
        if let Some(target_id) = self.view.unfiltered.target_id {
            if let Some(target_event) = self
                .view
                .unfiltered
                .cached_unfiltered
                .iter()
                .find(|e| e.id == target_id)
                .cloned()
            {
                self.view.inspector.selected_log = Some(target_event);
            }
        }
        self.view.search.clear();
        self.close_unfiltered_stream();
    }

    /// Xử lý các hành động tác động trực tiếp lên View State & Engine của Session
    pub fn handle_action(&mut self, action: &AppAction) -> bool {
        match action {
            AppAction::SelectLog(log) => {
                self.view.inspector.selected_log = log.clone();
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
                    if !self.view.unfiltered.is_open {
                        self.open_unfiltered_stream(None);
                    }
                }
                self.view.active_tab = *tab;
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
                if next == ActiveTab::Unfiltered && !self.view.unfiltered.is_open {
                    self.open_unfiltered_stream(None);
                }
                self.view.active_tab = next;
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
            AppAction::ToggleLatch => {
                if self.view.active_tab == ActiveTab::Unfiltered {
                    self.toggle_unfiltered_live();
                } else {
                    self.toggle_latch();
                }
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
            AppAction::ToggleUnfilteredLive => {
                self.toggle_unfiltered_live();
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
                    self.view.active_tab = ActiveTab::Filtered;
                    return true;
                }
                if self.view.search.query.trim().is_empty() {
                    return false;
                }
                let target_id = self.view.inspector.selected_log.as_ref().map(|l| l.id);
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
