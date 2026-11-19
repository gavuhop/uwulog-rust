pub mod autocomplete;
pub mod columns;
pub mod history;
pub mod inspector;
pub mod search;
pub mod viewport;
pub mod warnings;

pub use autocomplete::{
    classify_field, generate_suggestions, AutocompleteState, FieldType, SuggestionItem,
    SuggestionKind,
};
pub use columns::{ColumnItem, ColumnState};
pub use history::SearchHistoryState;
pub use inspector::InspectorState;
pub use search::SearchState;
pub use viewport::{ActiveTab, LogStreamView, LogTab, ScrollCommand, TabFilter, RAW_STREAM_LIMIT};
pub use warnings::FieldWarningState;

/// Trạng thái UI dùng chung giữa các tab (`Main` và `Raw`) trong cùng một `LogViewer`.
#[derive(Default)]
pub struct SharedViewState {
    pub columns: ColumnState,
    pub inspector: InspectorState,
}

/// Lưu trữ trạng thái hiển thị giao diện của từng workspace session (`LogViewer`).
/// Phần dùng chung (`shared`) tách biệt tường minh với danh sách `tabs` (`Main` và tùy chọn `Raw`).
pub struct GuiViewState {
    pub shared: SharedViewState,
    pub search: SearchState,
    pub tabs: Vec<LogTab>,
    pub warnings: FieldWarningState,
    pub active_tab: ActiveTab,
    pub buffer_overflow_dismissed: bool,
}

impl Default for GuiViewState {
    fn default() -> Self {
        Self {
            shared: SharedViewState::default(),
            search: SearchState::default(),
            tabs: vec![LogTab::new_filtered()],
            warnings: FieldWarningState::default(),
            active_tab: ActiveTab::Filtered,
            buffer_overflow_dismissed: false,
        }
    }
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

    #[inline]
    pub fn main_tab(&self) -> &LogTab {
        debug_assert!(!self.tabs.is_empty() && self.tabs[0].kind == ActiveTab::Filtered);
        &self.tabs[0]
    }

    #[inline]
    pub fn main_tab_mut(&mut self) -> &mut LogTab {
        if self.tabs.is_empty() || self.tabs[0].kind != ActiveTab::Filtered {
            self.tabs.insert(0, LogTab::new_filtered());
        }
        &mut self.tabs[0]
    }

    #[inline]
    pub fn raw_tab(&self) -> Option<&LogTab> {
        self.tabs.iter().find(|t| t.kind == ActiveTab::Unfiltered)
    }

    #[inline]
    pub fn raw_tab_mut(&mut self) -> Option<&mut LogTab> {
        self.tabs
            .iter_mut()
            .find(|t| t.kind == ActiveTab::Unfiltered)
    }

    #[inline]
    pub fn has_raw_tab(&self) -> bool {
        self.raw_tab().is_some()
    }

    /// Mở (hoặc cập nhật) tab Raw trong `tabs` với `target_id` tùy chọn.
    pub fn open_raw_tab(&mut self, target_id: Option<u64>) -> &mut LogTab {
        if let Some(pos) = self
            .tabs
            .iter()
            .position(|t| t.kind == ActiveTab::Unfiltered)
        {
            let tab = &mut self.tabs[pos];
            tab.set_target_id(target_id);
            tab.stream.is_auto_scroll = target_id.is_none();
            return &mut self.tabs[pos];
        }
        self.tabs.push(LogTab::new_unfiltered(target_id));
        let last = self.tabs.len() - 1;
        &mut self.tabs[last]
    }

    /// Đóng tab Raw và gỡ khỏi `tabs`.
    pub fn close_raw_tab(&mut self) {
        self.tabs.retain(|t| t.kind != ActiveTab::Unfiltered);
    }

    #[inline]
    pub fn tab(&self, kind: ActiveTab) -> Option<&LogTab> {
        self.tabs.iter().find(|t| t.kind == kind)
    }

    #[inline]
    pub fn tab_mut(&mut self, kind: ActiveTab) -> Option<&mut LogTab> {
        self.tabs.iter_mut().find(|t| t.kind == kind)
    }

    /// Trả về `LogTab` tương ứng `kind`, fallback về `main_tab` nếu tab Raw chưa mở.
    #[inline]
    pub fn tab_or_main(&self, kind: ActiveTab) -> &LogTab {
        self.tab(kind).unwrap_or_else(|| self.main_tab())
    }

    /// Trả về `LogTab` mutable tương ứng `kind`, fallback về `main_tab_mut` nếu tab Raw chưa mở.
    #[inline]
    pub fn tab_or_main_mut(&mut self, kind: ActiveTab) -> &mut LogTab {
        if let Some(pos) = self.tabs.iter().position(|t| t.kind == kind) {
            &mut self.tabs[pos]
        } else {
            self.main_tab_mut()
        }
    }

    /// Trả về view stream dùng chung cho tab tương ứng (`Filtered` hoặc `Unfiltered`).
    #[inline]
    pub fn stream(&self, tab: ActiveTab) -> &LogStreamView {
        &self.tab_or_main(tab).stream
    }

    /// Trả về view stream mutable dùng chung cho tab tương ứng (`Filtered` hoặc `Unfiltered`).
    #[inline]
    pub fn stream_mut(&mut self, tab: ActiveTab) -> &mut LogStreamView {
        &mut self.tab_or_main_mut(tab).stream
    }

    /// Reset bộ đệm hiển thị khi khởi động lại nguồn log
    pub fn reset_stream_data(&mut self) {
        for tab in &mut self.tabs {
            tab.reset();
        }
        self.shared.inspector.selected_log = None;
        self.search.last_synced_schema_version = 0;
        self.search.autocomplete.is_open = false;
        self.search.autocomplete.suggestions.clear();
        self.buffer_overflow_dismissed = false;
        self.warnings.reset();
    }
}
