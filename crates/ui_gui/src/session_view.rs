use crate::ui::autocomplete::{AutocompleteState, FieldType, SuggestionItem, SuggestionKind};
use crate::ui::columns_modal::ColumnState;
use crate::ui::history::SearchHistoryState;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uwu_core_engine::SystemEngine;
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{Workspace, WorkspaceSession};

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Filtered,
    Unfiltered,
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

/// Lưu trữ trạng thái hiển thị giao diện (View State) thuần túy của từng workspace session.
/// Không chứa các trường runtime trùng lặp (như engine, source_config, name, env_vars...).
pub struct GuiViewState {
    pub query: String,
    pub last_query: String,
    pub active_tab: ActiveTab,
    pub cached_logs: Vec<LogEvent>,
    pub selected_log: Option<LogEvent>,
    pub total_matched: usize,
    pub is_auto_scroll: bool,
    pub request_scroll_to_bottom: bool,
    pub has_new_data: bool,
    pub prev_table_row_count: usize,
    pub last_processed_count: u64,
    pub last_search_time: Instant,
    pub autocomplete_state: AutocompleteState,
    pub history_state: SearchHistoryState,
    pub column_state: ColumnState,
    pub highlighted_row_ids: HashSet<u64>,
    pub highlighted_terms: HashSet<String>,
    pub inspector_width_ratio: f32,
    pub unfiltered_state: UnfilteredViewState,
    pub global_seen_at_pause: u64,
    pub filtered_seen_at_pause: usize,
    pub filtered_processed_at_pause: u64,
    pub paused_new_matched_count: usize,
    pub discovered_fields_cache: BTreeMap<String, FieldType>,
}

impl GuiViewState {
    pub fn new(engine: &Arc<SystemEngine>) -> Self {
        Self {
            query: String::new(),
            last_query: String::new(),
            active_tab: ActiveTab::Filtered,
            cached_logs: Vec::new(),
            selected_log: None,
            total_matched: 0,
            is_auto_scroll: true,
            request_scroll_to_bottom: false,
            has_new_data: false,
            prev_table_row_count: 0,
            last_processed_count: 0,
            last_search_time: Instant::now(),
            autocomplete_state: AutocompleteState::default(),
            history_state: SearchHistoryState::default(),
            column_state: ColumnState::default(),
            highlighted_row_ids: HashSet::new(),
            highlighted_terms: HashSet::new(),
            inspector_width_ratio: 0.35,
            unfiltered_state: UnfilteredViewState::default(),
            global_seen_at_pause: 0,
            filtered_seen_at_pause: 0,
            filtered_processed_at_pause: 0,
            paused_new_matched_count: 0,
            discovered_fields_cache: engine.get_schema_map().into_iter().collect(),
        }
    }
}

/// Thực thể đại diện cho một Workspace đang mở trong GUI.
/// Hợp nhất: `session` (SSOT cho Runtime/Engine/Process/Store) + `view` (Trạng thái UI thuần túy).
pub struct GuiSession {
    pub session: WorkspaceSession,
    pub view: GuiViewState,
}

impl GuiSession {
    pub fn new(session: WorkspaceSession) -> Self {
        let view = GuiViewState::new(&session.engine);
        Self { session, view }
    }

    pub fn from_workspace(ws: &Workspace, capacity: usize, display_limit: usize) -> Self {
        let session = WorkspaceSession::from_workspace(ws, capacity, display_limit);
        let mut view = GuiViewState::new(&session.engine);
        view.query = ws.last_query.clone();
        Self { session, view }
    }

    /// Xử lý cập nhật định kỳ cho từng session (incremental search, debounced history, latch scroll sync, unfiltered sync)
    pub fn tick(&mut self, now: Instant) {
        self.session.tick();
        let total_processed = self.session.engine.total_processed();

        let query_changed = self.query != self.last_query;
        if query_changed {
            self.trigger_full_search();
            if self.query.trim().is_empty() && self.active_tab == ActiveTab::Unfiltered {
                self.close_unfiltered_stream();
            }
        }

        let q = self.query.clone();
        self.history_state.try_debounced_record(&q, now);

        let new_logs_arrived = total_processed != self.last_processed_count
            && now.duration_since(self.last_search_time) > Duration::from_millis(150);

        self.has_new_data = false;
        let prev_processed = self.last_processed_count;

        if self.is_auto_scroll && new_logs_arrived && !query_changed {
            let (new_matched_count, new_matching_logs) = self
                .session
                .engine
                .filter_incremental(&self.query, prev_processed);

            if new_matched_count > 0 {
                self.total_matched += new_matched_count;
                self.sync_discovered_fields(&new_matching_logs);
                self.cached_logs.extend(new_matching_logs);

                if self.cached_logs.len() > self.session.display_limit {
                    let overflow = self.cached_logs.len() - self.session.display_limit;
                    self.cached_logs.drain(0..overflow);
                }
                self.has_new_data = true;
            }
        }

        if !self.is_auto_scroll && !self.query.trim().is_empty() && new_logs_arrived {
            let (new_matched, _) = self
                .session
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

        self.unfiltered_state.has_new_data = false;
        if self.unfiltered_state.is_open && self.unfiltered_state.is_live && new_logs_arrived {
            let (new_count, new_logs) = self.session.engine.filter_incremental("", prev_processed);
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

    pub fn trigger_full_search(&mut self) {
        let (matched, logs) = self
            .session
            .engine
            .search_with_count(&self.query, self.session.display_limit);
        self.total_matched = matched;
        self.sync_discovered_fields(&logs);
        self.cached_logs = logs;
        self.last_query = self.query.clone();
        self.last_processed_count = self.session.engine.total_processed();
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
        let total = self.session.engine.total_processed();
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

    pub fn sync_discovered_fields(&mut self, logs: &[LogEvent]) {
        self.column_state.sync_discovered_keys(logs);
        self.discovered_fields_cache = self.session.engine.get_schema_map().into_iter().collect();
    }

    pub fn get_available_log_fields(&self) -> Vec<(String, FieldType)> {
        self.discovered_fields_cache
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    pub fn apply_autocomplete_suggestion(&mut self, item: &SuggestionItem) {
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
            SuggestionKind::Key => {
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
            SuggestionKind::OperatorOrValue => {
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
        } else {
            format!("{}:{}", field, Self::format_selection_term(clean_val))
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
        let exclude_term = if term.starts_with('-') {
            term.to_string()
        } else {
            format!("-{term}")
        };
        self.apply_filter_term(&exclude_term);
    }

    pub fn open_unfiltered_stream(&mut self, target_id: Option<u64>) {
        self.unfiltered_state.is_open = true;
        self.unfiltered_state.target_id = target_id;
        self.unfiltered_state.is_live = target_id.is_none();
        self.refresh_unfiltered_snapshot();
        self.unfiltered_state.has_new_data = false;
        self.active_tab = ActiveTab::Unfiltered;
    }

    pub fn refresh_unfiltered_snapshot(&mut self) {
        self.unfiltered_state.snapshot_processed_count = self.session.engine.total_processed();
        let (target_idx, unfiltered) = self
            .session
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
            self.unfiltered_state.snapshot_processed_count = self.session.engine.total_processed();
        }
    }

    pub fn unlatch_unfiltered(&mut self) {
        if !self.unfiltered_state.is_live {
            return;
        }
        self.unfiltered_state.is_live = false;
        self.unfiltered_state.snapshot_processed_count = self.session.engine.total_processed();
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

    /// Xử lý các hành động tác động trực tiếp lên View State & Engine của Session
    pub fn handle_action(&mut self, action: &crate::app::AppAction) -> bool {
        match action {
            crate::app::AppAction::SelectLog(log) => {
                self.selected_log = log.clone();
                true
            }
            crate::app::AppAction::SwitchTab(tab) => {
                if *tab == ActiveTab::Unfiltered && !self.unfiltered_state.is_open {
                    self.open_unfiltered_stream(None);
                }
                self.active_tab = *tab;
                true
            }
            crate::app::AppAction::ApplyFilterTerm(term) => {
                self.apply_filter_term(term);
                true
            }
            crate::app::AppAction::ExcludeFilterTerm(term) => {
                self.exclude_filter_term(term);
                true
            }
            crate::app::AppAction::ClearQuery => {
                self.query.clear();
                self.autocomplete_state.is_open = false;
                self.history_state.close_popup();
                self.trigger_full_search();
                true
            }
            crate::app::AppAction::ToggleRowHighlight(id) => {
                self.toggle_row_highlight(*id);
                true
            }
            crate::app::AppAction::ToggleTermHighlight(term) => {
                self.toggle_term_highlight(term);
                true
            }
            crate::app::AppAction::ClearAllHighlights => {
                self.clear_all_highlights();
                true
            }
            crate::app::AppAction::ToggleLatch => {
                self.toggle_latch();
                true
            }
            crate::app::AppAction::ToggleUnfilteredLive => {
                self.toggle_unfiltered_live();
                true
            }
            crate::app::AppAction::RefreshUnfilteredSnapshot => {
                self.refresh_unfiltered_snapshot();
                true
            }
            crate::app::AppAction::OpenUnfilteredStream(id) => {
                self.open_unfiltered_stream(*id);
                true
            }
            crate::app::AppAction::CloseUnfilteredStream => {
                self.close_unfiltered_stream();
                true
            }
            crate::app::AppAction::FocusInMainAndClearFilter => {
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
