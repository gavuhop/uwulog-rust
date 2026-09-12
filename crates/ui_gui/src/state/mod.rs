pub mod autocomplete;
pub mod columns;
pub mod history;
pub mod inspector;
pub mod search;
pub mod unfiltered;
pub mod viewport;

pub use autocomplete::{
    classify_field, generate_suggestions, AutocompleteState, FieldType, SuggestionItem,
    SuggestionKind,
};
pub use columns::{ColumnItem, ColumnState};
pub use history::SearchHistoryState;
pub use inspector::InspectorState;
pub use search::SearchState;
pub use unfiltered::{ActiveTab, UnfilteredViewState, RAW_STREAM_LIMIT};
pub use viewport::{PauseSnapshot, ViewportState};

/// Lưu trữ trạng thái hiển thị giao diện của từng workspace session.
/// Gom nhóm rõ ràng theo các Sub-Models chuyên trách.
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

    /// Reset bộ đệm hiển thị khi khởi động lại nguồn log
    pub fn reset_stream_data(&mut self) {
        self.viewport.cached_logs.clear();
        self.viewport.total_matched = 0;
        self.viewport.last_processed_count = 0;
        self.inspector.selected_log = None;
        self.unfiltered.cached_unfiltered.clear();
    }
}
