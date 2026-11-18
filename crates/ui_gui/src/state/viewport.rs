use std::ops::{Deref, DerefMut};
use uwu_core_schema::LogEvent;

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, Hash)]
pub enum ActiveTab {
    #[default]
    Filtered,
    Unfiltered,
}

macro_rules! impl_deref_stream {
    ($ty:ty) => {
        impl Deref for $ty {
            type Target = LogStreamView;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.stream
            }
        }
        impl DerefMut for $ty {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.stream
            }
        }
    };
}

/// Trạng thái dùng chung cho một bảng hiển thị luồng log (cả tab Filtered và Unfiltered).
#[derive(Clone, Debug)]
pub struct LogStreamView {
    pub cached_logs: Vec<LogEvent>,
    pub selected_log: Option<LogEvent>,
    pub is_auto_scroll: bool,
    pub has_vertical_overflow: bool,
    pub first_visible_row: Option<usize>,
    pub reached_oldest: bool,
    pub is_loading_older: bool,
    pub request_scroll_to_bottom: bool,
    pub request_maintain_scroll_offset: Option<usize>,
    pub request_scroll_to_row: Option<(usize, Option<eframe::egui::Align>)>,
    pub request_horizontal_scroll: Option<f32>,
}

impl LogStreamView {
    pub fn new(is_auto_scroll: bool) -> Self {
        Self {
            cached_logs: Vec::new(),
            selected_log: None,
            is_auto_scroll,
            has_vertical_overflow: false,
            first_visible_row: None,
            reached_oldest: false,
            is_loading_older: false,
            request_scroll_to_bottom: false,
            request_maintain_scroll_offset: None,
            request_scroll_to_row: None,
            request_horizontal_scroll: None,
        }
    }

    /// Cho phép append thêm log mới nếu đang Live hoặc bảng chưa tràn chiều dọc viewport.
    #[inline]
    pub fn should_accept_incremental(&self) -> bool {
        self.is_auto_scroll || !self.has_vertical_overflow
    }

    /// Nối thêm log mới: khi Live thì giữ tối đa `limit` dòng mới nhất (drain đầu);
    /// khi Paused & chưa tràn màn hình thì chỉ điền thêm tối đa tới `limit` để không xê dịch dòng đầu.
    pub fn append_logs(&mut self, logs: Vec<LogEvent>, limit: usize) -> usize {
        if logs.is_empty() {
            return 0;
        }
        if self.is_auto_scroll {
            let count = logs.len();
            self.cached_logs.extend(logs);
            if self.cached_logs.len() > limit {
                let overflow = self.cached_logs.len() - limit;
                self.cached_logs.drain(0..overflow);
            }
            count
        } else {
            let remaining = limit.saturating_sub(self.cached_logs.len());
            let added = logs.len().min(remaining);
            if added > 0 {
                self.cached_logs.extend(logs.into_iter().take(added));
            }
            added
        }
    }

    /// Chèn trang log cũ hơn lên đầu cache khi Reverse Pagination hoàn tất.
    pub fn prepend_older_logs(&mut self, older: Vec<LogEvent>, reached_oldest: bool) {
        self.is_loading_older = false;
        self.reached_oldest = reached_oldest;
        let count = older.len();
        if count > 0 {
            let mut new_cache = older;
            new_cache.extend(std::mem::take(&mut self.cached_logs));
            self.cached_logs = new_cache;
            self.request_maintain_scroll_offset = Some(count);
        }
    }

    /// Tiêu thụ các yêu cầu cuộn chung (`request_scroll_to_row`, `request_maintain_scroll_offset`, bottom/auto-scroll).
    pub fn consume_common_scroll_request(
        &mut self,
        row_count: usize,
    ) -> Option<(usize, Option<eframe::egui::Align>)> {
        if let Some(target) = self.request_scroll_to_row.take() {
            Some(target)
        } else if let Some(offset) = self.request_maintain_scroll_offset.take() {
            Some((offset, Some(eframe::egui::Align::Min)))
        } else if row_count > 0 && (self.request_scroll_to_bottom || self.is_auto_scroll) {
            self.request_scroll_to_bottom = false;
            Some((row_count - 1, Some(eframe::egui::Align::Max)))
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.cached_logs.clear();
        self.selected_log = None;
        self.has_vertical_overflow = false;
        self.first_visible_row = None;
        self.reached_oldest = false;
        self.is_loading_older = false;
        self.request_maintain_scroll_offset = None;
        self.request_scroll_to_row = None;
    }
}

impl Default for LogStreamView {
    fn default() -> Self {
        Self::new(true)
    }
}

/// Trạng thái đếm số log khi người dùng dừng cuộn (Pause streaming)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PauseSnapshot {
    pub processed_at_pause: u64,
    pub filtered_seen: usize,
    pub paused_new_matched_count: usize,
}

/// Trạng thái hiển thị luồng log chính (Filtered) và bộ đếm Pause/Match
pub struct ViewportState {
    pub stream: LogStreamView,
    pub total_matched: usize,
    pub is_count_exact: bool,
    pub last_processed_count: u64,
    pub pause_snapshot: PauseSnapshot,
}

impl_deref_stream!(ViewportState);

impl Default for ViewportState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportState {
    pub fn new() -> Self {
        Self {
            stream: LogStreamView::new(true),
            total_matched: 0,
            is_count_exact: true,
            last_processed_count: 0,
            pause_snapshot: PauseSnapshot::default(),
        }
    }

    pub fn latch(&mut self) -> bool {
        let was_unlatched = !self.is_auto_scroll;
        self.is_auto_scroll = true;
        self.request_scroll_to_bottom = true;
        self.reached_oldest = false;
        self.request_maintain_scroll_offset = None;
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
        self.pause_snapshot.processed_at_pause = total_processed;
        self.pause_snapshot.filtered_seen = self.total_matched;
        self.pause_snapshot.paused_new_matched_count = 0;
    }
}

/// Trạng thái bổ sung cho tab Raw/Unfiltered (giữ lại ngữ cảnh "View in Raw Log" song song với tab Main).
#[derive(Clone, Debug)]
pub struct UnfilteredViewState {
    pub stream: LogStreamView,
    pub is_open: bool,
    pub target_id: Option<u64>,
    pub target_index: Option<usize>,
    pub request_scroll_to_target: bool,
    pub snapshot_processed_count: u64,
}

impl_deref_stream!(UnfilteredViewState);

impl Default for UnfilteredViewState {
    fn default() -> Self {
        Self {
            stream: LogStreamView::new(false),
            is_open: false,
            target_id: None,
            target_index: None,
            request_scroll_to_target: false,
            snapshot_processed_count: 0,
        }
    }
}
