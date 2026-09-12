use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SearchHistoryState {
    pub entries: Vec<String>,
    pub is_open: bool,
    pub last_change_time: Instant,
    pub pending_record: bool,
}

impl Default for SearchHistoryState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            is_open: false,
            last_change_time: Instant::now(),
            pending_record: false,
        }
    }
}

impl SearchHistoryState {
    pub fn extract_query_keys(query: &str) -> Vec<String> {
        let mut keys: Vec<String> = query
            .split_whitespace()
            .filter_map(|token| {
                let sep = token.find(':').or_else(|| token.find('='));
                sep.map(|pos| token[..pos].to_lowercase())
            })
            .collect();
        keys.sort();
        keys
    }

    pub fn record(&mut self, query: &str) {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return;
        }
        // Don't save queries ending with incomplete operators
        if trimmed.ends_with(':')
            || trimmed.ends_with(":-")
            || trimmed.ends_with(":-~")
            || trimmed.ends_with(":~")
            || trimmed.ends_with(":<=")
            || trimmed.ends_with(":>=")
            || trimmed.ends_with(":<")
            || trimmed.ends_with(":>")
            || trimmed.ends_with('=')
        {
            return;
        }
        let new_keys = Self::extract_query_keys(trimmed);
        self.entries.retain(|q| {
            if q == trimmed || trimmed.starts_with(q) {
                return false;
            }
            let existing_keys = Self::extract_query_keys(q);
            if !existing_keys.is_empty() && existing_keys == new_keys {
                return false;
            }
            true
        });
        self.entries.insert(0, trimmed.to_string());
        if self.entries.len() > 10 {
            self.entries.truncate(10);
        }
    }

    pub fn mark_query_changed(&mut self, now: Instant) {
        self.last_change_time = now;
        self.pending_record = true;
    }

    pub fn try_debounced_record(&mut self, query: &str, now: Instant) {
        if self.pending_record
            && now.duration_since(self.last_change_time) > Duration::from_millis(500)
        {
            self.record(query);
            self.pending_record = false;
        }
    }

    pub fn toggle_popup(&mut self) -> bool {
        self.is_open = !self.is_open;
        self.is_open
    }

    pub fn close_popup(&mut self) {
        self.is_open = false;
    }

    pub fn apply_history_item(&mut self, chosen_query: &str) {
        self.record(chosen_query);
        self.close_popup();
    }

    pub fn mark_recorded(&mut self) {
        self.pending_record = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_search_history_dedup_exact() {
        let mut state = SearchHistoryState::default();

        state.record("level:error");
        state.record("msg:auth");
        state.record("level:error");

        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.entries[0], "level:error");
        assert_eq!(state.entries[1], "msg:auth");
    }

    #[test]
    fn test_record_search_history_dedup_same_key_different_value() {
        let mut state = SearchHistoryState::default();

        state.record("level:-222");
        state.record("level:-aaa");

        // Cùng key "level" → chỉ giữ mới nhất
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0], "level:-aaa");
    }

    #[test]
    fn test_record_search_history_different_keys_kept() {
        let mut state = SearchHistoryState::default();

        state.record("level:error");
        state.record("msg:auth");

        // Key khác nhau → giữ cả hai
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.entries[0], "msg:auth");
        assert_eq!(state.entries[1], "level:error");
    }

    #[test]
    fn test_record_search_history_multi_key_dedup() {
        let mut state = SearchHistoryState::default();

        state.record("level:error msg:auth");
        state.record("level:warn msg:timeout");

        // Cùng bộ key ["level", "msg"] → thay thế
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0], "level:warn msg:timeout");
    }

    #[test]
    fn test_record_search_history_replaces_prefix() {
        let mut state = SearchHistoryState::default();

        state.record("lev");
        state.record("level:error");

        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0], "level:error");
    }

    #[test]
    fn test_record_search_history_max_10() {
        let mut state = SearchHistoryState::default();

        for i in 1..=15 {
            state.record(&format!("key{i}:val{i}"));
        }

        assert_eq!(state.entries.len(), 10);
        assert_eq!(state.entries[0], "key15:val15");
    }

    #[test]
    fn test_record_search_history_ignores_incomplete_operators() {
        let mut state = SearchHistoryState::default();

        state.record("level:");
        state.record("level:-");
        state.record("level:~");
        assert!(state.entries.is_empty());

        state.record("level:error");
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0], "level:error");
    }
}
