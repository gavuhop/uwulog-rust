use crate::ui::autocomplete::{AutocompleteState, FieldType, SuggestionItem, SuggestionKind};
use crate::ui::columns_modal::ColumnState;
use crate::ui::history::SearchHistoryState;
use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{Workspace, WorkspaceSession};

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum ActiveTab {
    #[default]
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

/// Trạng thái đếm số log khi người dùng dừng cuộn (Pause streaming)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PauseSnapshot {
    pub global_seen: u64,
    pub filtered_seen: usize,
    pub filtered_processed: u64,
    pub paused_new_matched_count: usize,
}

/// Trạng thái tìm kiếm, lọc truy vấn, gợi ý autocomplete và cache schema keys
pub struct SearchState {
    pub query: String,
    pub last_query: String,
    pub last_search_time: Instant,
    pub autocomplete: AutocompleteState,
    pub history: SearchHistoryState,
    pub schema_cache: BTreeMap<String, FieldType>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            last_query: String::new(),
            last_search_time: Instant::now(),
            autocomplete: AutocompleteState::default(),
            history: SearchHistoryState::default(),
            schema_cache: BTreeMap::new(),
        }
    }
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema(schema: impl IntoIterator<Item = (String, FieldType)>) -> Self {
        let mut state = Self::default();
        state.sync_schema(schema);
        state
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
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        let exclude_term = if term.starts_with('-') {
            term.to_string()
        } else {
            format!("-{term}")
        };
        self.apply_filter_term(&exclude_term);
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.autocomplete.is_open = false;
        self.history.close_popup();
    }

    pub fn apply_autocomplete_suggestion(&mut self, item: &SuggestionItem) -> bool {
        let (start, end) = self.autocomplete.active_token_range;
        if start <= end && end <= self.query.len() {
            let mut new_query = String::new();
            new_query.push_str(&self.query[..start]);
            new_query.push_str(&item.insert_text);
            new_query.push_str(&self.query[end..]);
            self.query = new_query;
        } else {
            self.query = item.insert_text.clone();
        }

        self.autocomplete.just_applied = true;

        match item.kind {
            SuggestionKind::Key => {
                let available_fields = self.get_available_fields();
                let (suggestions, token_range) =
                    crate::ui::autocomplete::generate_suggestions(&self.query, &available_fields);
                if !suggestions.is_empty() {
                    self.autocomplete.suggestions = suggestions;
                    self.autocomplete.active_token_range = token_range;
                    self.autocomplete.selected_index = 0;
                    self.autocomplete.is_open = true;
                } else {
                    self.autocomplete.is_open = false;
                }
                false
            }
            SuggestionKind::OperatorOrValue => {
                self.autocomplete.is_open = false;
                true
            }
        }
    }

    pub fn sync_schema(&mut self, schema: impl IntoIterator<Item = (String, FieldType)>) {
        self.schema_cache = schema.into_iter().collect();
    }

    pub fn sync_discovered_fields(
        &mut self,
        schema: impl IntoIterator<Item = (String, FieldType)>,
    ) {
        self.sync_schema(schema);
    }

    pub fn get_available_fields(&self) -> Vec<(String, FieldType)> {
        self.schema_cache
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
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
}

/// Trạng thái hiển thị luồng log, bộ đệm cuộn và latch auto-scroll
pub struct ViewportState {
    pub cached_logs: Vec<LogEvent>,
    pub total_matched: usize,
    pub is_auto_scroll: bool,
    pub request_scroll_to_bottom: bool,
    pub has_new_data: bool,
    pub prev_table_row_count: usize,
    pub last_processed_count: u64,
    pub pause_snapshot: PauseSnapshot,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportState {
    pub fn new() -> Self {
        Self {
            cached_logs: Vec::new(),
            total_matched: 0,
            is_auto_scroll: true,
            request_scroll_to_bottom: false,
            has_new_data: false,
            prev_table_row_count: 0,
            last_processed_count: 0,
            pause_snapshot: PauseSnapshot::default(),
        }
    }

    pub fn latch(&mut self) -> bool {
        let was_unlatched = !self.is_auto_scroll;
        self.is_auto_scroll = true;
        self.request_scroll_to_bottom = true;
        was_unlatched
    }

    pub fn unlatch(&mut self, total_processed: u64) {
        if !self.is_auto_scroll {
            return;
        }
        self.is_auto_scroll = false;
        self.record_pause(total_processed);
    }

    pub fn toggle_latch(&mut self, total_processed: u64) -> bool {
        if self.is_auto_scroll {
            self.unlatch(total_processed);
            false
        } else {
            self.latch();
            true
        }
    }

    pub fn record_pause(&mut self, total_processed: u64) {
        self.pause_snapshot.global_seen = total_processed;
        self.pause_snapshot.filtered_seen = self.total_matched;
        self.pause_snapshot.filtered_processed = total_processed;
        self.pause_snapshot.paused_new_matched_count = 0;
    }
}

/// Trạng thái thanh xem chi tiết log (Inspector) và quản lý Highlight
pub struct InspectorState {
    pub selected_log: Option<LogEvent>,
    pub width_ratio: f32,
    pub highlighted_row_ids: HashSet<u64>,
    pub highlighted_terms: HashSet<String>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorState {
    pub fn new() -> Self {
        Self {
            selected_log: None,
            width_ratio: 0.35,
            highlighted_row_ids: HashSet::new(),
            highlighted_terms: HashSet::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.selected_log.is_some()
    }

    pub fn close(&mut self) {
        self.selected_log = None;
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
}

/// Lưu trữ trạng thái hiển thị giao diện của từng workspace session.
/// Gom nhóm rõ ràng theo 4 Sub-Models: Search, Viewport, Inspector, và Columns.
#[derive(Default)]
pub struct GuiViewState {
    pub search: SearchState,
    pub viewport: ViewportState,
    pub inspector: InspectorState,
    pub columns: ColumnState,
    pub unfiltered: UnfilteredViewState,
    pub active_tab: ActiveTab,
}

impl GuiViewState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema(schema: impl IntoIterator<Item = (String, FieldType)>) -> Self {
        let mut view = Self::default();
        view.search.sync_schema(schema);
        view
    }
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
        self.trigger_full_search();
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        self.view.search.exclude_filter_term(term);
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
    pub fn handle_action(&mut self, action: &crate::app::AppAction) -> bool {
        match action {
            crate::app::AppAction::SelectLog(log) => {
                self.view.inspector.selected_log = log.clone();
                true
            }
            crate::app::AppAction::SwitchTab(tab) => {
                if *tab == ActiveTab::Unfiltered && !self.view.unfiltered.is_open {
                    self.open_unfiltered_stream(None);
                }
                self.view.active_tab = *tab;
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
                self.view.search.clear();
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
