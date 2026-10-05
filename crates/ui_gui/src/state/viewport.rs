use uwu_core_schema::LogEvent;

/// Trạng thái đếm số log khi người dùng dừng cuộn (Pause streaming)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PauseSnapshot {
    pub global_seen: u64,
    pub filtered_seen: usize,
    pub filtered_processed: u64,
    pub paused_new_matched_count: usize,
}

/// Trạng thái hiển thị luồng log, bộ đệm cuộn và latch auto-scroll
pub struct ViewportState {
    pub cached_logs: Vec<LogEvent>,
    pub total_matched: usize,
    pub is_auto_scroll: bool,
    pub request_scroll_to_bottom: bool,
    pub request_maintain_scroll_offset: Option<usize>,
    pub first_visible_row: Option<usize>,
    pub request_scroll_to_row: Option<(usize, Option<eframe::egui::Align>)>,
    pub request_horizontal_scroll: Option<f32>,
    pub reached_oldest: bool,
    pub is_loading_older: bool,
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
            request_maintain_scroll_offset: None,
            first_visible_row: None,
            request_scroll_to_row: None,
            request_horizontal_scroll: None,
            reached_oldest: false,
            is_loading_older: false,
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
        self.pause_snapshot.global_seen = total_processed;
        self.pause_snapshot.filtered_seen = self.total_matched;
        self.pause_snapshot.filtered_processed = total_processed;
        self.pause_snapshot.paused_new_matched_count = 0;
    }
}
