use crate::app::{ActiveTab, UnfilteredViewState};
use crate::ui::autocomplete::{AutocompleteState, FieldType};
use crate::ui::columns_modal::ColumnState;
use crate::ui::history::SearchHistoryState;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use uwu_core_engine::SystemEngine;
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{Workspace, WorkspaceSession};

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
