use std::collections::HashSet;
use uwu_core_schema::LogEvent;

/// Trạng thái thanh xem chi tiết log (Inspector) và quản lý Highlight
pub struct InspectorState {
    pub selected_log: Option<LogEvent>,
    pub width_ratio: f32,
    pub highlighted_row_ids: HashSet<u64>,
    pub highlighted_terms: HashSet<String>,
    pub active_editing_cell: Option<eframe::egui::Id>,
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
            active_editing_cell: None,
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
