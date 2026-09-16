use std::collections::HashSet;
use uwu_core_schema::LogEvent;

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnItem {
    /// Tên key gốc của trường log (ví dụ: "timestamp", "level", "message", "source", "latency_ms", "user_id")
    pub name: String,
    /// Trạng thái bật/tắt hiển thị
    pub visible: bool,
    /// Độ rộng ban đầu
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct ColumnState {
    pub is_modal_open: bool,
    pub filter_query: String,
    pub columns: Vec<ColumnItem>,
    pub draft_columns: Option<Vec<ColumnItem>>,
    pub dragged_index: Option<usize>,
    pub header_dragged_name: Option<String>,
    pub header_dragged_width: Option<f32>,
    pub header_drag_offset_x: Option<f32>,
    pub known_keys: HashSet<String>,
    pub table_animations: crate::animation::MoveAnimationManager,
    pub modal_animations: crate::animation::MoveAnimationManager,
}

impl Default for ColumnState {
    fn default() -> Self {
        let columns = Self::default_columns();
        let known_keys = columns.iter().map(|c| c.name.clone()).collect();
        Self {
            is_modal_open: false,
            filter_query: String::new(),
            columns,
            draft_columns: None,
            dragged_index: None,
            header_dragged_name: None,
            header_dragged_width: None,
            header_drag_offset_x: None,
            known_keys,
            table_animations: crate::animation::MoveAnimationManager::default(),
            modal_animations: crate::animation::MoveAnimationManager::default(),
        }
    }
}

fn reorder_vec(list: &mut Vec<ColumnItem>, from_idx: usize, to_idx: usize) {
    if from_idx < list.len() && to_idx < list.len() && from_idx != to_idx {
        let item = list.remove(from_idx);
        list.insert(to_idx, item);
    }
}

impl ColumnState {
    pub fn open_modal(&mut self) {
        self.draft_columns = Some(self.columns.clone());
        self.is_modal_open = true;
    }

    pub fn close_modal(&mut self) {
        self.is_modal_open = false;
        self.draft_columns = None;
        self.clear_modal_drag();
    }

    pub fn apply_modal(&mut self) {
        if let Some(draft) = self.draft_columns.take() {
            self.columns = draft;
            self.known_keys = self.columns.iter().map(|c| c.name.clone()).collect();
        }
        self.is_modal_open = false;
        self.clear_modal_drag();
    }

    pub fn clear_modal_drag(&mut self) {
        self.dragged_index = None;
    }

    pub fn clear_header_drag(&mut self) {
        self.header_dragged_name = None;
        self.header_dragged_width = None;
        self.header_drag_offset_x = None;
    }

    pub fn default_columns() -> Vec<ColumnItem> {
        uwu_core_schema::StandardField::default_columns()
            .iter()
            .map(|f| {
                let name = f.canonical_name().to_string();
                let width = match f {
                    uwu_core_schema::StandardField::Timestamp => 150.0,
                    uwu_core_schema::StandardField::Level => 56.0,
                    uwu_core_schema::StandardField::Message => 350.0,
                    _ => 120.0,
                };
                ColumnItem {
                    name,
                    visible: true,
                    width,
                }
            })
            .collect()
    }

    pub fn merge_with_defaults(source_cols: &[ColumnItem]) -> Vec<ColumnItem> {
        let mut new_cols = Self::default_columns();
        for existing in source_cols {
            if !new_cols.iter().any(|c| c.name == existing.name) {
                new_cols.push(ColumnItem {
                    name: existing.name.clone(),
                    visible: false,
                    width: 120.0,
                });
            }
        }
        new_cols
    }

    pub fn reset_to_defaults(&mut self) {
        self.columns = Self::merge_with_defaults(&self.columns);
        self.clear_modal_drag();
        self.clear_header_drag();
        self.known_keys = self.columns.iter().map(|c| c.name.clone()).collect();
    }

    pub fn reset_draft_to_defaults(&mut self) {
        let source = self.draft_columns.as_deref().unwrap_or(&self.columns);
        self.draft_columns = Some(Self::merge_with_defaults(source));
        self.clear_modal_drag();
    }

    pub fn reorder(&mut self, from_idx: usize, to_idx: usize) {
        reorder_vec(&mut self.columns, from_idx, to_idx);
    }

    pub fn swap_columns(&mut self, col_a: &str, col_b: &str) {
        let pos_a = self.columns.iter().position(|c| c.name == col_a);
        let pos_b = self.columns.iter().position(|c| c.name == col_b);
        if let (Some(a), Some(b)) = (pos_a, pos_b) {
            if a != b {
                self.columns.swap(a, b);
            }
        }
    }

    pub fn reorder_draft(&mut self, from_idx: usize, to_idx: usize) {
        if let Some(draft) = &mut self.draft_columns {
            reorder_vec(draft, from_idx, to_idx);
        }
    }

    pub fn sync_discovered_keys(&mut self, logs: &[LogEvent]) {
        for log in logs {
            for key in log.fields.keys() {
                if let Some(std_field) = uwu_core_schema::StandardField::from_alias(key) {
                    let found_idx = self.columns.iter().position(|c| {
                        uwu_core_schema::StandardField::from_alias(&c.name) == Some(std_field)
                    });

                    if let Some(idx) = found_idx {
                        if self.columns[idx].name != *key {
                            let old_name = self.columns[idx].name.clone();
                            self.known_keys.remove(&old_name);
                            self.columns[idx].name = key.clone();
                            self.known_keys.insert(key.clone());
                        }
                    }
                } else if self.known_keys.insert(key.clone()) {
                    self.columns.push(ColumnItem {
                        name: key.clone(),
                        visible: false,
                        width: 120.0,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uwu_core_schema::LogColor;

    #[test]
    fn test_default_columns() {
        let cols = ColumnState::default_columns();
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0].name, "timestamp");
        assert_eq!(cols[1].name, "level");
        assert_eq!(cols[2].name, "message");
        assert!(cols.iter().all(|c| c.visible));
    }

    #[test]
    fn test_reorder_columns() {
        let mut state = ColumnState::default();
        state.reorder(0, 2);
        assert_eq!(state.columns[0].name, "level");
        assert_eq!(state.columns[1].name, "message");
        assert_eq!(state.columns[2].name, "timestamp");
    }

    #[test]
    fn test_sync_discovered_keys() {
        let mut state = ColumnState::default();
        let mut fields = HashMap::new();
        fields.insert("latency_ms".to_string(), serde_json::json!(123));
        fields.insert("user_id".to_string(), serde_json::json!("u42"));

        let log = LogEvent::new("2026-08-20T10:00:00Z", LogColor::Green, "msg", fields);

        state.sync_discovered_keys(&[log]);
        assert_eq!(state.columns.len(), 5);
        assert!(state.columns.iter().any(|c| c.name == "latency_ms"));
        assert!(state.columns.iter().any(|c| c.name == "user_id"));
    }

    #[test]
    fn test_sync_discovered_keys_replaces_default_names() {
        let mut state = ColumnState::default();
        let mut fields = HashMap::new();
        fields.insert("ts".to_string(), serde_json::json!("2026-08-20T10:00:00Z"));
        fields.insert("lvl".to_string(), serde_json::json!("INFO"));
        fields.insert("msg".to_string(), serde_json::json!("hello"));
        fields.insert("user_id".to_string(), serde_json::json!("u42"));

        let log = LogEvent::new("2026-08-20T10:00:00Z", LogColor::Green, "hello", fields);

        state.sync_discovered_keys(&[log]);
        assert_eq!(state.columns.len(), 4);
        assert!(state.columns.iter().any(|c| c.name == "ts" && c.visible));
        assert!(state.columns.iter().any(|c| c.name == "lvl" && c.visible));
        assert!(state.columns.iter().any(|c| c.name == "msg" && c.visible));
        assert!(state
            .columns
            .iter()
            .any(|c| c.name == "user_id" && !c.visible));
        assert!(!state.columns.iter().any(|c| c.name == "timestamp"));
        assert!(!state.columns.iter().any(|c| c.name == "level"));
        assert!(!state.columns.iter().any(|c| c.name == "message"));
    }

    #[test]
    fn test_reset_to_defaults() {
        let mut state = ColumnState::default();
        state.reorder(1, 0);
        state.columns[2].visible = false;

        state.reset_to_defaults();
        assert_eq!(state.columns[0].name, "timestamp");
        assert!(state.columns[0].visible);
        assert_eq!(state.columns[1].name, "level");
        assert!(state.columns[1].visible);
        assert_eq!(state.columns[2].name, "message");
        assert!(state.columns[2].visible);
    }

    #[test]
    fn test_column_draft_isolation_and_commit() {
        let mut state = ColumnState::default();
        let original_cols = state.columns.clone();

        state.draft_columns = Some(state.columns.clone());
        state.reorder_draft(1, 0);

        assert_eq!(state.columns, original_cols);

        state.columns = state.draft_columns.take().unwrap();
        assert_eq!(state.columns[0].name, "level");
        assert_eq!(state.columns[1].name, "timestamp");
    }

    #[test]
    fn test_column_draft_cancel_discards_changes() {
        let mut state = ColumnState::default();
        let original_cols = state.columns.clone();

        state.draft_columns = Some(state.columns.clone());
        state.draft_columns.as_mut().unwrap()[0].visible = false;
        state.reorder_draft(2, 0);

        state.draft_columns = None;

        assert_eq!(state.columns, original_cols);
        assert!(state.columns[0].visible);
        assert_eq!(state.columns[0].name, "timestamp");
    }

    #[test]
    fn test_reset_draft_to_defaults() {
        let mut state = ColumnState::default();
        state.draft_columns = Some(state.columns.clone());
        state.reorder_draft(1, 0);
        state.draft_columns.as_mut().unwrap()[2].visible = false;

        state.reset_draft_to_defaults();
        let draft = state.draft_columns.unwrap();
        assert_eq!(draft[0].name, "timestamp");
        assert!(draft[0].visible);
        assert_eq!(draft[1].name, "level");
        assert!(draft[1].visible);
        assert_eq!(draft[2].name, "message");
        assert!(draft[2].visible);
    }

    #[test]
    fn test_swap_columns() {
        let mut state = ColumnState::default();
        // Ban đầu: ["timestamp", "level", "message"]
        state.swap_columns("timestamp", "level");
        assert_eq!(state.columns[0].name, "level");
        assert_eq!(state.columns[1].name, "timestamp");
        assert_eq!(state.columns[2].name, "message");

        // Swap lại về cũ
        state.swap_columns("level", "timestamp");
        assert_eq!(state.columns[0].name, "timestamp");
        assert_eq!(state.columns[1].name, "level");
    }
}
