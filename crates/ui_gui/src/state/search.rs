use super::autocomplete::{
    generate_suggestions, AutocompleteState, FieldType, SuggestionItem, SuggestionKind,
};
use super::history::SearchHistoryState;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Trạng thái tìm kiếm, lọc truy vấn, gợi ý autocomplete và cache schema keys
pub struct SearchState {
    pub query: String,
    pub last_query: String,
    pub needs_search: bool,
    pub last_search_time: Instant,
    pub autocomplete: AutocompleteState,
    pub history: SearchHistoryState,
    pub schema_cache: BTreeMap<String, FieldType>,
    pub active_query_id: u64,
    /// Atomic token phục vụ cooperative early cancellation cho background search worker
    pub active_query_atomic: Arc<AtomicU64>,
    /// Cờ yêu cầu focus con trỏ vào ô nhập tìm kiếm khi người dùng nhấn Ctrl+F
    pub focus_requested: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            last_query: String::new(),
            needs_search: false,
            last_search_time: Instant::now(),
            autocomplete: AutocompleteState::default(),
            history: SearchHistoryState::default(),
            schema_cache: BTreeMap::new(),
            active_query_id: 0,
            active_query_atomic: Arc::new(AtomicU64::new(0)),
            focus_requested: false,
        }
    }
}

impl SearchState {
    /// Đánh dấu cần chạy lại worker search ngay cả khi chuỗi query không thay đổi (ví dụ: unlatch -> latch)
    #[inline]
    pub fn mark_needs_search(&mut self) {
        self.needs_search = true;
    }

    /// Tăng active_query_id và cập nhật atomic cancellation token đồng bộ
    #[inline]
    pub fn advance_query(&mut self) -> (u64, Arc<AtomicU64>) {
        self.active_query_id += 1;
        self.active_query_atomic
            .store(self.active_query_id, Ordering::Release);
        (self.active_query_id, Arc::clone(&self.active_query_atomic))
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema(schema: impl IntoIterator<Item = (String, FieldType)>) -> Self {
        let mut state = Self::default();
        state.sync_schema(schema);
        state
    }

    pub fn apply_filter_term(&mut self, term: &str) {
        let current = self.query.trim();
        if current.is_empty() {
            self.query = term.to_string();
        } else {
            let tokens: Vec<&str> = current.split_whitespace().collect();
            if !tokens.contains(&term) {
                self.query = format!("{current} {term}");
            }
        }
    }

    pub fn exclude_filter_term(&mut self, term: &str) {
        let exclude_term = if term.starts_with('-') {
            term.to_string()
        } else {
            format!("-{term}")
        };
        self.apply_filter_term(&exclude_term);
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.autocomplete.is_open = false;
        self.history.close_popup();
    }

    pub fn apply_autocomplete_suggestion(&mut self, item: &SuggestionItem) -> bool {
        let (start, end) = self.autocomplete.active_token_range;
        if start <= end && end <= self.query.len() {
            let mut new_query = String::new();
            new_query.push_str(&self.query[..start]);
            new_query.push_str(&item.insert_text);
            new_query.push_str(&self.query[end..]);
            self.query = new_query;
        } else {
            self.query = item.insert_text.clone();
        }

        self.autocomplete.just_applied = true;

        match item.kind {
            SuggestionKind::Key => {
                let available_fields = self.get_available_fields();
                let (suggestions, token_range) =
                    generate_suggestions(&self.query, &available_fields);
                if !suggestions.is_empty() {
                    self.autocomplete.suggestions = suggestions;
                    self.autocomplete.active_token_range = token_range;
                    self.autocomplete.selected_index = 0;
                    self.autocomplete.is_open = true;
                } else {
                    self.autocomplete.is_open = false;
                }
                false
            }
            SuggestionKind::OperatorOrValue => {
                self.autocomplete.is_open = false;
                true
            }
        }
    }

    pub fn sync_schema(&mut self, schema: impl IntoIterator<Item = (String, FieldType)>) {
        self.schema_cache = schema.into_iter().collect();
    }

    pub fn get_available_fields(&self) -> Vec<(String, FieldType)> {
        self.schema_cache
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    pub fn format_field_term(field: &str, val: &str) -> String {
        let clean_val = val.trim();
        if field.eq_ignore_ascii_case("level") {
            format!("level:{}", clean_val.to_lowercase())
        } else {
            format!("{}:{}", field, Self::format_selection_term(clean_val))
        }
    }

    pub fn format_selection_term(text: &str) -> String {
        let clean = text
            .replace(" ↵ ", " ")
            .replace('\n', " ")
            .replace('\r', "");
        let clean = clean.trim();
        if clean.contains(' ') || clean.contains('"') || clean.contains(':') {
            format!("\"{}\"", clean.replace('"', "\\\""))
        } else {
            clean.to_string()
        }
    }
}
