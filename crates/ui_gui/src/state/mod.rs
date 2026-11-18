pub mod autocomplete;
pub mod columns;
pub mod history;
pub mod inspector;
pub mod search;
pub mod viewport;

pub use autocomplete::{
    classify_field, generate_suggestions, AutocompleteState, FieldType, SuggestionItem,
    SuggestionKind,
};
pub use columns::{ColumnItem, ColumnState};
pub use history::SearchHistoryState;
pub use inspector::InspectorState;
pub use search::SearchState;
pub use viewport::{
    ActiveTab, LogStreamView, ScrollCommand, UnfilteredViewState, ViewportState, RAW_STREAM_LIMIT,
};

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
    pub buffer_overflow_dismissed: bool,
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

    /// Trả về view stream dùng chung cho tab tương ứng (`Filtered` hoặc `Unfiltered`).
    #[inline]
    pub fn stream(&self, tab: ActiveTab) -> &LogStreamView {
        match tab {
            ActiveTab::Filtered => &self.viewport.stream,
            ActiveTab::Unfiltered => &self.unfiltered.stream,
        }
    }

    /// Trả về view stream mutable dùng chung cho tab tương ứng (`Filtered` hoặc `Unfiltered`).
    #[inline]
    pub fn stream_mut(&mut self, tab: ActiveTab) -> &mut LogStreamView {
        match tab {
            ActiveTab::Filtered => &mut self.viewport.stream,
            ActiveTab::Unfiltered => &mut self.unfiltered.stream,
        }
    }

    /// Reset bộ đệm hiển thị khi khởi động lại nguồn log
    pub fn reset_stream_data(&mut self) {
        self.viewport.stream.reset();
        self.viewport.total_matched = 0;
        self.viewport.is_count_exact = true;
        self.viewport.last_processed_count = 0;
        self.inspector.selected_log = None;
        self.unfiltered.stream.reset();
        self.unfiltered.target_id = None;
        self.search.last_synced_schema_version = 0;
        self.search.autocomplete.is_open = false;
        self.search.autocomplete.suggestions.clear();
        self.buffer_overflow_dismissed = false;
    }
}
