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

/// Yêu cầu cuộn bảng một lần ở frame tiếp theo (thay thế nhiều cờ boolean rời rạc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollCommand {
    Bottom,
    Row(usize, Option<eframe::egui::Align>),
    MaintainOffset(usize),
}

/// Trạng thái dùng chung cho một bảng hiển thị luồng log (cả tab Filtered và Unfiltered).
#[derive(Clone, Debug)]
pub struct LogStreamView {
    pub cached_logs: Vec<LogEvent>,
    pub selected_log: Option<LogEvent>,
    pub is_auto_scroll: bool,
    pub first_visible_row: Option<usize>,
    pub reached_oldest: bool,
    pub is_loading_older: bool,
    pub pending_scroll: Option<ScrollCommand>,
    pub request_horizontal_scroll: Option<f32>,
}

impl LogStreamView {
    pub fn new(is_auto_scroll: bool) -> Self {
        Self {
            cached_logs: Vec::new(),
            selected_log: None,
            is_auto_scroll,
            first_visible_row: None,
            reached_oldest: false,
            is_loading_older: false,
            pending_scroll: None,
            request_horizontal_scroll: None,
        }
    }

    /// Nối thêm log mới: khi Live thì giữ tối đa `limit` dòng mới nhất (drain đầu);
    /// khi Paused thì chỉ điền thêm tối đa tới `limit` để không xê dịch dòng đầu đang đọc.
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
            self.pending_scroll = Some(ScrollCommand::MaintainOffset(count));
        }
    }

    /// Tiêu thụ yêu cầu cuộn ở frame hiện tại.
    pub fn consume_scroll_request(
        &mut self,
        row_count: usize,
    ) -> Option<(usize, Option<eframe::egui::Align>)> {
        match self.pending_scroll.take() {
            Some(ScrollCommand::Row(row, align)) => Some((row, align)),
            Some(ScrollCommand::MaintainOffset(offset)) => {
                Some((offset, Some(eframe::egui::Align::Min)))
            }
            Some(ScrollCommand::Bottom) if row_count > 0 => {
                Some((row_count - 1, Some(eframe::egui::Align::Max)))
            }
            _ if self.is_auto_scroll && row_count > 0 => {
                Some((row_count - 1, Some(eframe::egui::Align::Max)))
            }
            _ => None,
        }
    }

    pub fn latch(&mut self) -> bool {
        let was_unlatched = !self.is_auto_scroll;
        self.is_auto_scroll = true;
        self.reached_oldest = false;
        if !matches!(self.pending_scroll, Some(ScrollCommand::Row(_, _))) {
            self.pending_scroll = Some(ScrollCommand::Bottom);
        }
        was_unlatched
    }

    pub fn unlatch(&mut self) {
        self.is_auto_scroll = false;
    }

    pub fn toggle_latch(&mut self) -> bool {
        if self.is_auto_scroll {
            self.unlatch();
            false
        } else {
            self.latch();
            true
        }
    }

    pub fn reset(&mut self) {
        self.cached_logs.clear();
        self.selected_log = None;
        self.first_visible_row = None;
        self.reached_oldest = false;
        self.is_loading_older = false;
        self.pending_scroll = None;
    }
}

impl Default for LogStreamView {
    fn default() -> Self {
        Self::new(true)
    }
}

/// Trạng thái hiển thị luồng log chính (Filtered)
pub struct ViewportState {
    pub stream: LogStreamView,
    pub total_matched: usize,
    pub is_count_exact: bool,
    pub last_processed_count: u64,
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
        }
    }
}

/// Trạng thái bổ sung cho tab Raw/Unfiltered (giữ lại ngữ cảnh "View in Raw Log" song song với tab Main).
#[derive(Clone, Debug)]
pub struct UnfilteredViewState {
    pub stream: LogStreamView,
    pub is_open: bool,
    pub target_id: Option<u64>,
}

impl_deref_stream!(UnfilteredViewState);

impl Default for UnfilteredViewState {
    fn default() -> Self {
        Self {
            stream: LogStreamView::new(false),
            is_open: false,
            target_id: None,
        }
    }
}
