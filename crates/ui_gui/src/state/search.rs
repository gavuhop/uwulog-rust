use super::autocomplete::{
    generate_suggestions, AutocompleteState, FieldType, SuggestionItem, SuggestionKind,
};
use super::history::SearchHistoryState;
use std::collections::BTreeMap;
use std::time::Instant;

/// Trạng thái tìm kiếm, lọc truy vấn, gợi ý autocomplete và cache schema keys
pub struct SearchState {
    pub query: String,
    pub last_query: String,
    pub last_search_time: Instant,
    pub autocomplete: AutocompleteState,
    pub history: SearchHistoryState,
    pub schema_cache: BTreeMap<String, FieldType>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            last_query: String::new(),
            last_search_time: Instant::now(),
            autocomplete: AutocompleteState::default(),
            history: SearchHistoryState::default(),
            schema_cache: BTreeMap::new(),
        }
    }
}

impl SearchState {
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

    pub fn sync_discovered_fields(
        &mut self,
        schema: impl IntoIterator<Item = (String, FieldType)>,
    ) {
        self.sync_schema(schema);
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
