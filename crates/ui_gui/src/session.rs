use crate::actions::AppAction;
use crate::state::{
    ActiveTab, FieldType, GuiViewState, SearchState, SuggestionItem, RAW_STREAM_LIMIT,
};
use std::time::{Duration, Instant};
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{Workspace, WorkspaceSession};

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
            self.trigger_full_search();
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

            if new_matched_count > 0 {
                self.view.viewport.total_matched += new_matched_count;
                self.sync_discovered_fields(&new_matching_logs);
                self.view.viewport.cached_logs.extend(new_matching_logs);

                if self.view.viewport.cached_logs.len() > self.session.display_limit {
                    let overflow =
                        self.view.viewport.cached_logs.len() - self.session.display_limit;
                    self.view.viewport.cached_logs.drain(0..overflow);
                }
                self.view.viewport.has_new_data = true;
            }
        }

        if !self.view.viewport.is_auto_scroll
            && !self.view.search.query.trim().is_empty()
            && new_logs_arrived
        {
            let (new_matched, _) = self.session.engine.filter_incremental(
                &self.view.search.query,
                self.view.viewport.pause_snapshot.filtered_processed,
            );
            self.view.viewport.pause_snapshot.paused_new_matched_count = new_matched;
        }

        if self.view.viewport.is_auto_scroll {
            self.view.viewport.record_pause(total_processed);
        }

        self.view.unfiltered.has_new_data = false;
        if self.view.unfiltered.is_open && self.view.unfiltered.is_live && new_logs_arrived {
            let (new_count, new_logs) = self.session.engine.filter_incremental("", prev_processed);
            if new_count > 0 {
                self.view.unfiltered.cached_unfiltered.extend(new_logs);
                if self.view.unfiltered.cached_unfiltered.len() > RAW_STREAM_LIMIT {
                    let overflow = self.view.unfiltered.cached_unfiltered.len() - RAW_STREAM_LIMIT;
                    self.view.unfiltered.cached_unfiltered.drain(0..overflow);
                }
                self.view.unfiltered.has_new_data = true;
            }
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
        self.view.viewport.total_matched = matched;
        self.sync_discovered_fields(&logs);
        self.view.viewport.cached_logs = logs;
        self.view.search.last_query = self.view.search.query.clone();
        self.view.viewport.last_processed_count = self.session.engine.total_processed();
        self.view.search.last_search_time = Instant::now();
        self.view
            .viewport
            .record_pause(self.view.viewport.last_processed_count);
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
            self.trigger_full_search();
        }
    }

    pub fn sync_discovered_fields(&mut self, logs: &[LogEvent]) {
        self.view.columns.sync_discovered_keys(logs);
        self.view
            .search
            .sync_schema(self.session.engine.get_schema_map());
    }

    pub fn get_available_log_fields(&self) -> Vec<(String, FieldType)> {
        self.view.search.get_available_fields()
    }

    pub fn apply_autocomplete_suggestion(&mut self, item: &SuggestionItem) {
        if self.view.search.apply_autocomplete_suggestion(item) {
            self.trigger_full_search();
        }
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

    pub fn format_field_term(field: &str, val: &str) -> String {
        SearchState::format_field_term(field, val)
    }

    pub fn format_selection_term(text: &str) -> String {
        SearchState::format_selection_term(text)
    }

    pub fn apply_filter_term(&mut self, term: &str) {
        self.view.search.apply_filter_term(term);
        let q = self.view.search.query.clone();
        self.view.search.history.record(&q);
        self.view.search.history.mark_recorded();
        self.trigger_full_search();
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        self.view.search.exclude_filter_term(term);
        let q = self.view.search.query.clone();
        self.view.search.history.record(&q);
        self.view.search.history.mark_recorded();
        self.trigger_full_search();
    }

    pub fn open_unfiltered_stream(&mut self, target_id: Option<u64>) {
        self.view.unfiltered.is_open = true;
        self.view.unfiltered.target_id = target_id;
        self.view.unfiltered.is_live = target_id.is_none();
        self.refresh_unfiltered_snapshot();
        self.view.unfiltered.has_new_data = false;
        self.view.active_tab = ActiveTab::Unfiltered;
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        self.view.unfiltered.snapshot_processed_count = self.session.engine.total_processed();
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
        self.trigger_full_search();
        self.close_unfiltered_stream();
    }

    /// Xử lý các hành động tác động trực tiếp lên View State & Engine của Session
    pub fn handle_action(&mut self, action: &AppAction) -> bool {
        match action {
            AppAction::SelectLog(log) => {
                self.view.inspector.selected_log = log.clone();
                true
            }
            AppAction::SwitchTab(tab) => {
                if *tab == ActiveTab::Unfiltered && !self.view.unfiltered.is_open {
                    self.open_unfiltered_stream(None);
                }
                self.view.active_tab = *tab;
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
                self.trigger_full_search();
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
                self.toggle_latch();
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
                self.open_unfiltered_stream(*id);
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
