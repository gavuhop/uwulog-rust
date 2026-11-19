use uwu_core_schema::LogEvent;

pub const RAW_STREAM_LIMIT: usize = 500;

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, Hash)]
pub enum ActiveTab {
    #[default]
    Filtered,
    Unfiltered,
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

/// Chế độ lọc/nguồn dữ liệu riêng của từng `LogTab` (Main hoặc Raw).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabFilter {
    Filtered {
        total_matched: usize,
        is_count_exact: bool,
        last_processed_count: u64,
    },
    Unfiltered {
        target_id: Option<u64>,
    },
}

/// Thực thể tab hiển thị log thống nhất dùng chung cho cả tab Main (`Filtered`) và Raw (`Unfiltered`).
#[derive(Clone, Debug)]
pub struct LogTab {
    pub kind: ActiveTab,
    pub filter: TabFilter,
    pub stream: LogStreamView,
    pub hscroll_offset: f32,
    pub vscroll_offset: f32,
}

impl LogTab {
    pub fn new_filtered() -> Self {
        Self {
            kind: ActiveTab::Filtered,
            filter: TabFilter::Filtered {
                total_matched: 0,
                is_count_exact: true,
                last_processed_count: 0,
            },
            stream: LogStreamView::new(true),
            hscroll_offset: 0.0,
            vscroll_offset: 0.0,
        }
    }

    pub fn new_unfiltered(target_id: Option<u64>) -> Self {
        Self {
            kind: ActiveTab::Unfiltered,
            filter: TabFilter::Unfiltered { target_id },
            stream: LogStreamView::new(target_id.is_none()),
            hscroll_offset: 0.0,
            vscroll_offset: 0.0,
        }
    }

    #[inline]
    pub fn total_matched(&self) -> usize {
        match self.filter {
            TabFilter::Filtered { total_matched, .. } => total_matched,
            TabFilter::Unfiltered { .. } => self.stream.cached_logs.len(),
        }
    }

    #[inline]
    pub fn is_count_exact(&self) -> bool {
        match self.filter {
            TabFilter::Filtered { is_count_exact, .. } => is_count_exact,
            TabFilter::Unfiltered { .. } => true,
        }
    }

    #[inline]
    pub fn last_processed_count(&self) -> u64 {
        match self.filter {
            TabFilter::Filtered {
                last_processed_count,
                ..
            } => last_processed_count,
            TabFilter::Unfiltered { .. } => 0,
        }
    }

    #[inline]
    pub fn set_last_processed_count(&mut self, count: u64) {
        if let TabFilter::Filtered {
            ref mut last_processed_count,
            ..
        } = self.filter
        {
            *last_processed_count = count;
        }
    }

    #[inline]
    pub fn set_search_counts(&mut self, matched: usize, exact: bool) {
        if let TabFilter::Filtered {
            ref mut total_matched,
            ref mut is_count_exact,
            ..
        } = self.filter
        {
            *total_matched = matched;
            *is_count_exact = exact;
        }
    }

    #[inline]
    pub fn add_matched(&mut self, delta: usize) {
        if let TabFilter::Filtered {
            ref mut total_matched,
            ..
        } = self.filter
        {
            *total_matched += delta;
        }
    }

    #[inline]
    pub fn target_id(&self) -> Option<u64> {
        match self.filter {
            TabFilter::Unfiltered { target_id } => target_id,
            TabFilter::Filtered { .. } => None,
        }
    }

    #[inline]
    pub fn set_target_id(&mut self, id: Option<u64>) {
        if let TabFilter::Unfiltered { ref mut target_id } = self.filter {
            *target_id = id;
        }
    }

    pub fn reset(&mut self) {
        self.stream.reset();
        self.hscroll_offset = 0.0;
        self.vscroll_offset = 0.0;
        match self.filter {
            TabFilter::Filtered {
                ref mut total_matched,
                ref mut is_count_exact,
                ref mut last_processed_count,
            } => {
                *total_matched = 0;
                *is_count_exact = true;
                *last_processed_count = 0;
            }
            TabFilter::Unfiltered { ref mut target_id } => {
                *target_id = None;
            }
        }
    }
}
