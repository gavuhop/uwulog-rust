use std::collections::HashSet;
use uwu_core_schema::LogEvent;
use uwu_core_workspace::SavedColumnConfig;

const DEFAULT_ROLES: [(&str, f32); 3] = [("timestamp", 150.0), ("level", 56.0), ("message", 350.0)];

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnItem {
    /// Tên key gốc của trường log (ví dụ: "timestamp", "level", "message", "source", "latency_ms", "user_id")
    pub name: String,
    /// Trạng thái bật/tắt hiển thị
    pub visible: bool,
    /// Độ rộng ban đầu
    pub width: f32,
}

/// Trạng thái nháp & kéo thả cục bộ khi mở `ColumnsModal`.
#[derive(Clone, Debug, Default)]
pub struct ColumnModalDraft {
    pub filter_query: String,
    pub columns: Vec<ColumnItem>,
    pub dragged_index: Option<usize>,
    pub animations: crate::animation::MoveAnimationManager,
    pub did_reset: bool,
}

/// Trạng thái kéo thả trực tiếp tiêu đề cột trên bảng log.
#[derive(Clone, Debug)]
pub struct HeaderDragState {
    pub name: String,
    pub width: f32,
    pub offset_x: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct ColumnState {
    pub columns: Vec<ColumnItem>,
    pub draft: Option<ColumnModalDraft>,
    pub header_drag: Option<HeaderDragState>,
    pub known_keys: HashSet<String>,
    pub seen_keys: HashSet<String>,
    pub table_animations: crate::animation::MoveAnimationManager,
    pub layout_version: u64,
    default_slot_keys: [String; 3],
    saved_layout: Vec<SavedColumnConfig>,
    columns_dirty_for_save: bool,
    pending_save_frames: Option<u8>,
}

impl Default for ColumnState {
    fn default() -> Self {
        let columns = Self::default_columns();
        let known_keys = columns.iter().map(|c| c.name.clone()).collect();
        Self {
            columns,
            draft: None,
            header_drag: None,
            known_keys,
            seen_keys: HashSet::new(),
            table_animations: crate::animation::MoveAnimationManager::default(),
            layout_version: 0,
            default_slot_keys: DEFAULT_ROLES.map(|(n, _)| n.to_string()),
            saved_layout: Vec::new(),
            columns_dirty_for_save: false,
            pending_save_frames: None,
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
        self.draft = Some(ColumnModalDraft {
            columns: self.columns.clone(),
            ..Default::default()
        });
    }

    pub fn close_modal(&mut self) {
        self.draft = None;
    }

    pub fn apply_modal(&mut self) {
        if let Some(draft) = self.draft.take() {
            self.columns = draft.columns;
            self.known_keys = self.columns.iter().map(|c| c.name.clone()).collect();
            if draft.did_reset {
                self.saved_layout.clear();
            }
            self.layout_version = self.layout_version.wrapping_add(1);
            self.columns_dirty_for_save = true;
        }
    }

    pub fn clear_modal_drag(&mut self) {
        if let Some(draft) = &mut self.draft {
            draft.dragged_index = None;
        }
    }

    pub fn clear_header_drag(&mut self) {
        self.header_drag = None;
    }

    pub fn default_columns() -> Vec<ColumnItem> {
        DEFAULT_ROLES
            .iter()
            .map(|&(name, width)| ColumnItem {
                name: name.to_string(),
                visible: true,
                width,
            })
            .collect()
    }

    fn reset_order_and_visibility(
        source_cols: &[ColumnItem],
        default_slots: &[String; 3],
    ) -> Vec<ColumnItem> {
        let mut new_cols = Vec::with_capacity(source_cols.len().max(3));
        for (slot_idx, slot_key) in default_slots.iter().enumerate() {
            let width = source_cols
                .iter()
                .find(|c| c.name == *slot_key)
                .map_or(DEFAULT_ROLES[slot_idx].1, |c| c.width);
            new_cols.push(ColumnItem {
                name: slot_key.clone(),
                visible: true,
                width,
            });
        }
        for existing in source_cols {
            let existing_std = uwu_core_schema::StandardField::from_alias(&existing.name);
            let covered_by_default = default_slots.iter().any(|k| {
                k == &existing.name
                    || existing_std
                        .is_some_and(|sf| uwu_core_schema::StandardField::from_alias(k) == Some(sf))
            });
            if !covered_by_default {
                new_cols.push(ColumnItem {
                    name: existing.name.clone(),
                    visible: false,
                    width: existing.width,
                });
            }
        }
        new_cols
    }

    pub fn merge_with_defaults(source_cols: &[ColumnItem]) -> Vec<ColumnItem> {
        Self::reset_order_and_visibility(source_cols, &DEFAULT_ROLES.map(|(n, _)| n.to_string()))
    }

    fn sanitize_width(width: f32, fallback: f32) -> f32 {
        if width.is_finite() && width >= 10.0 {
            width
        } else {
            fallback
        }
    }

    fn saved_rank(&self, name: &str) -> usize {
        self.saved_layout
            .iter()
            .position(|c| {
                c.name == name
                    || (self.default_slot_keys.iter().any(|k| k == name)
                        && c.default_role.as_deref() == Some(name))
            })
            .unwrap_or(usize::MAX)
    }

    pub fn reset_to_defaults(&mut self) {
        self.columns = Self::merge_with_defaults(&self.columns);
        self.clear_modal_drag();
        self.clear_header_drag();
        self.known_keys = self.columns.iter().map(|c| c.name.clone()).collect();
        self.seen_keys.clear();
        self.default_slot_keys = DEFAULT_ROLES.map(|(n, _)| n.to_string());
        self.saved_layout.clear();
        self.columns_dirty_for_save = true;
        self.layout_version = self.layout_version.wrapping_add(1);
    }

    pub fn reset_draft_to_defaults(&mut self) {
        let default_slots = self.default_slot_keys.clone();
        if let Some(draft) = &mut self.draft {
            draft.columns = Self::reset_order_and_visibility(&draft.columns, &default_slots);
            draft.did_reset = true;
            draft.dragged_index = None;
        } else {
            let mut draft = ColumnModalDraft {
                columns: Self::reset_order_and_visibility(&self.columns, &default_slots),
                did_reset: true,
                ..Default::default()
            };
            draft.dragged_index = None;
            self.draft = Some(draft);
        }
    }

    pub fn set_column_width(&mut self, name: &str, width: f32) {
        if width.is_finite() && width >= 10.0 {
            if let Some(col) = self.columns.iter_mut().find(|c| c.name == name) {
                if (col.width - width).abs() > 0.01 {
                    col.width = width;
                    self.columns_dirty_for_save = true;
                }
            }
        }
    }

    /// Hẹn kiểm tra lưu vào `workspaces.json` sau khi nhả chuột kéo/resize xong.
    pub fn schedule_save_after_drag(&mut self) {
        self.pending_save_frames = Some(2);
    }

    /// Trả về `true` đúng một lần sau khi đếm lùi kết thúc và có thay đổi cột cần lưu.
    pub fn poll_pending_save(&mut self) -> bool {
        let Some(rem) = self.pending_save_frames else {
            return false;
        };
        if rem > 0 {
            self.pending_save_frames = Some(rem - 1);
            return false;
        }
        self.pending_save_frames = None;
        std::mem::take(&mut self.columns_dirty_for_save)
    }

    /// Xóa hẹn lưu chờ và tiêu thụ cờ dirty ngay lập tức (dùng khi chuyển tab hoặc đóng session).
    pub fn take_dirty_for_save(&mut self) -> bool {
        self.pending_save_frames = None;
        std::mem::take(&mut self.columns_dirty_for_save)
    }

    /// Xuất các cột đã tuỳ chỉnh để lưu vào `workspaces.json` (bỏ cột custom ẩn độ rộng mặc định).
    pub fn to_saved_columns(&self) -> Vec<SavedColumnConfig> {
        let mut saved = Vec::new();
        let mut active_roles = HashSet::new();
        let mut active_names = HashSet::new();

        for col in &self.columns {
            let role_idx = self.default_slot_keys.iter().position(|k| k == &col.name);
            let role = role_idx.map(|i| DEFAULT_ROLES[i].0);
            let default_w = role_idx.map_or(120.0, |i| DEFAULT_ROLES[i].1);
            if role.is_none() && !col.visible && (col.width - default_w).abs() <= 0.5 {
                continue;
            }
            let pending_alias = if self.seen_keys.is_empty() {
                role.and_then(|r| {
                    self.saved_layout
                        .iter()
                        .find(|p| p.default_role.as_deref() == Some(r))
                })
            } else {
                None
            };
            let (name, width) = pending_alias.map_or_else(
                || (col.name.clone(), col.width),
                |p| (p.name.clone(), p.width),
            );
            if let Some(r) = role {
                active_roles.insert(r);
            }
            active_names.insert(name.clone());
            saved.push(SavedColumnConfig {
                name,
                visible: col.visible,
                width,
                default_role: role.map(str::to_string),
            });
        }

        // Giữ lại các cột chưa gặp lại trong phiên để không mất cấu hình của field ít xuất hiện
        for cfg in &self.saved_layout {
            let covered = cfg
                .default_role
                .as_deref()
                .is_some_and(|r| active_roles.contains(r))
                || uwu_core_schema::StandardField::from_alias(&cfg.name)
                    .is_some_and(|sf| active_roles.contains(sf.canonical_name()))
                || active_names.contains(&cfg.name);
            if !covered {
                let idx = self.saved_rank(&cfg.name);
                let pos = saved
                    .iter()
                    .position(|s| self.saved_rank(&s.name) > idx)
                    .unwrap_or(saved.len());
                saved.insert(pos, cfg.clone());
            }
        }
        saved
    }

    /// Khôi phục bố cục cột từ `workspaces.json`.
    /// Cột custom chỉ hiện và nhận lại độ rộng khi log mới khớp đúng key.
    pub fn restore_from_saved(&mut self, saved: &[SavedColumnConfig]) {
        if saved.is_empty() {
            return;
        }
        self.reset_to_defaults();
        self.saved_layout = saved.to_vec();

        let mut default_items: Vec<(usize, ColumnItem)> = DEFAULT_ROLES
            .iter()
            .enumerate()
            .map(|(fallback, &(canon, default_w))| {
                let matched = saved.iter().enumerate().find(|(_, c)| {
                    c.default_role.as_deref() == Some(canon)
                        || (c.default_role.is_none() && c.name == canon)
                });
                match matched {
                    Some((idx, cfg)) => (
                        idx,
                        ColumnItem {
                            name: canon.to_string(),
                            visible: cfg.visible,
                            width: if cfg.name == canon {
                                Self::sanitize_width(cfg.width, default_w)
                            } else {
                                default_w
                            },
                        },
                    ),
                    None => (
                        usize::MAX - 3 + fallback,
                        ColumnItem {
                            name: canon.to_string(),
                            visible: true,
                            width: default_w,
                        },
                    ),
                }
            })
            .collect();
        default_items.sort_by_key(|(idx, _)| *idx);
        self.columns = default_items.into_iter().map(|(_, item)| item).collect();
        self.columns_dirty_for_save = false;
    }

    pub fn reorder(&mut self, from_idx: usize, to_idx: usize) {
        if from_idx < self.columns.len() && to_idx < self.columns.len() && from_idx != to_idx {
            reorder_vec(&mut self.columns, from_idx, to_idx);
            self.columns_dirty_for_save = true;
            self.layout_version = self.layout_version.wrapping_add(1);
        }
    }

    /// Dịch chuyển cột `dragged` tới vị trí của `target`, kéo các cột nằm giữa dịch sang trái/phải.
    pub fn move_column(&mut self, dragged: &str, target: &str) {
        let Some(from) = self.columns.iter().position(|c| c.name == dragged) else {
            return;
        };
        let Some(to) = self.columns.iter().position(|c| c.name == target) else {
            return;
        };
        self.reorder(from, to);
    }

    pub fn reorder_draft(&mut self, from_idx: usize, to_idx: usize) {
        if let Some(draft) = &mut self.draft {
            reorder_vec(&mut draft.columns, from_idx, to_idx);
        }
    }

    pub fn sync_discovered_keys(&mut self, logs: &[LogEvent]) {
        for log in logs {
            for key in log.fields.keys() {
                if !self.seen_keys.insert(key.clone()) {
                    continue;
                }
                if self.columns.iter().any(|c| c.name == *key) {
                    self.known_keys.insert(key.clone());
                    continue;
                }

                let saved_match = self
                    .saved_layout
                    .iter()
                    .enumerate()
                    .find(|(_, c)| c.name == *key)
                    .map(|(rank, c)| (rank, c.clone()));

                // Mỗi StandardField (timestamp/level/message) chỉ ánh xạ vào 1 cột ngữ nghĩa duy nhất;
                // không tạo thêm cột trùng khi đã có cột đại diện (vd: "lv" và "level").
                if let Some(std_field) = uwu_core_schema::StandardField::from_alias(key) {
                    let canonical = std_field.canonical_name();
                    if let Some(idx) = self.columns.iter().position(|c| {
                        uwu_core_schema::StandardField::from_alias(&c.name) == Some(std_field)
                    }) {
                        // Chỉ đổi tên cột mặc định khi đang giữ tên canonical mặc định và tên canonical
                        // chưa từng xuất hiện thực sự trong log (vd: "level" -> "lv").
                        if self.columns[idx].name == canonical
                            && !self.seen_keys.contains(canonical)
                        {
                            self.known_keys.remove(canonical);
                            self.columns[idx].name = key.clone();
                            self.known_keys.insert(key.clone());
                            if let Some(slot) =
                                DEFAULT_ROLES.iter().position(|(n, _)| *n == canonical)
                            {
                                self.default_slot_keys[slot] = key.clone();
                            }
                            if let Some((_, cfg)) = saved_match {
                                self.columns[idx].visible = cfg.visible;
                                self.columns[idx].width =
                                    Self::sanitize_width(cfg.width, self.columns[idx].width);
                                self.layout_version = self.layout_version.wrapping_add(1);
                            }
                        }
                        continue;
                    }
                }

                if self.known_keys.insert(key.clone()) {
                    if let Some((rank, cfg)) = saved_match {
                        let idx = self
                            .columns
                            .iter()
                            .position(|c| self.saved_rank(&c.name) > rank)
                            .unwrap_or(self.columns.len());
                        self.columns.insert(
                            idx,
                            ColumnItem {
                                name: key.clone(),
                                visible: cfg.visible,
                                width: Self::sanitize_width(cfg.width, 120.0),
                            },
                        );
                        self.layout_version = self.layout_version.wrapping_add(1);
                    } else {
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
        fields.insert("id".to_string(), serde_json::json!(42));
        fields.insert("latency_ms".to_string(), serde_json::json!(123));
        fields.insert("user_id".to_string(), serde_json::json!("u42"));

        let log = LogEvent::new("2026-08-20T10:00:00Z", LogColor::Green, "msg", fields);

        state.sync_discovered_keys(&[log]);
        assert_eq!(state.columns.len(), 6);
        assert!(state.columns.iter().any(|c| c.name == "id" && !c.visible));
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

        state.open_modal();
        state.reorder_draft(1, 0);

        assert_eq!(state.columns, original_cols);

        state.apply_modal();
        assert_eq!(state.columns[0].name, "level");
        assert_eq!(state.columns[1].name, "timestamp");
    }

    #[test]
    fn test_column_draft_cancel_discards_changes() {
        let mut state = ColumnState::default();
        let original_cols = state.columns.clone();

        state.open_modal();
        state.draft.as_mut().unwrap().columns[0].visible = false;
        state.reorder_draft(2, 0);

        state.close_modal();

        assert_eq!(state.columns, original_cols);
        assert!(state.columns[0].visible);
        assert_eq!(state.columns[0].name, "timestamp");
    }

    #[test]
    fn test_reset_draft_to_defaults() {
        let mut state = ColumnState::default();
        let mut f = HashMap::new();
        f.insert("lvl".to_string(), serde_json::json!("INFO"));
        f.insert("custom".to_string(), serde_json::json!("val"));
        state.sync_discovered_keys(&[LogEvent::new("t1", LogColor::Green, "hi", f)]);
        state.set_column_width("timestamp", 220.0);
        state.set_column_width("lvl", 92.0);
        state.set_column_width("custom", 180.0);

        state.open_modal();
        if let Some(ref mut draft) = state.draft {
            if let Some(c) = draft.columns.iter_mut().find(|c| c.name == "custom") {
                c.visible = true;
            }
        }
        state.reorder_draft(1, 0);
        state.draft.as_mut().unwrap().columns[2].visible = false;

        state.reset_draft_to_defaults();
        let draft = &state.draft.as_ref().unwrap().columns;
        // Reset thứ tự và visible về 3 cột mặc định (giữ alias "lvl" và giữ nguyên width đã chỉnh)
        assert_eq!(draft[0].name, "timestamp");
        assert!(draft[0].visible);
        assert!((draft[0].width - 220.0).abs() < 0.01);

        assert_eq!(draft[1].name, "lvl");
        assert!(draft[1].visible);
        assert!((draft[1].width - 92.0).abs() < 0.01);

        assert_eq!(draft[2].name, "message");
        assert!(draft[2].visible);

        assert_eq!(draft[3].name, "custom");
        assert!(!draft[3].visible);
        assert!((draft[3].width - 180.0).abs() < 0.01);
    }

    #[test]
    fn test_move_column_shifts_affected_columns() {
        let mut state = ColumnState::default();
        state.columns.push(ColumnItem {
            name: "hidden".into(),
            visible: false,
            width: 80.0,
        });
        state.columns.push(ColumnItem {
            name: "extra".into(),
            visible: true,
            width: 100.0,
        });

        let vis = |s: &ColumnState| -> Vec<String> {
            s.columns
                .iter()
                .filter(|c| c.visible)
                .map(|c| c.name.clone())
                .collect()
        };

        // Kéo sang cột kế bên -> đổi chỗ 2 cột
        state.move_column("timestamp", "level");
        assert_eq!(vis(&state), ["level", "timestamp", "message", "extra"]);

        // Kéo sang phải qua nhiều cột -> các cột giữa dịch sang trái
        state.move_column("level", "extra");
        assert_eq!(vis(&state), ["timestamp", "message", "extra", "level"]);

        // Kéo sang trái qua nhiều cột -> các cột giữa dịch sang phải
        state.move_column("level", "timestamp");
        assert_eq!(vis(&state), ["level", "timestamp", "message", "extra"]);
    }

    #[test]
    fn test_move_column_preserves_resized_widths_and_bumps_layout_version() {
        let mut state = ColumnState::default();
        assert_eq!(state.layout_version, 0);

        state.set_column_width("timestamp", 225.0);
        state.set_column_width("level", 90.0);
        state.set_column_width("message", 180.0);

        state.move_column("timestamp", "message");
        assert_eq!(state.layout_version, 1);

        // Chiều rộng đã resize phải đi theo đúng từng cột, không reset về mặc định
        let width_of = |name: &str| {
            state
                .columns
                .iter()
                .find(|c| c.name == name)
                .map(|c| c.width)
        };
        assert_eq!(width_of("timestamp"), Some(225.0));
        assert_eq!(width_of("level"), Some(90.0));
        assert_eq!(width_of("message"), Some(180.0));

        // Di chuyển ngược lại vẫn tăng layout_version để TableBuilder không đọc cache cũ
        state.move_column("timestamp", "level");
        assert_eq!(state.layout_version, 2);
    }

    #[test]
    fn test_sync_discovered_keys_deduplicates_semantic_aliases() {
        let mut state = ColumnState::default();

        // Log 1 có đồng thời cả "level" và "lv" -> chỉ giữ 1 cột Level duy nhất ("level")
        let mut fields1 = HashMap::new();
        fields1.insert("level".to_string(), serde_json::json!("INFO"));
        fields1.insert("lv".to_string(), serde_json::json!("info"));
        fields1.insert("message".to_string(), serde_json::json!("hi"));
        let log1 = LogEvent::new("2026-08-20T10:00:00Z", LogColor::Green, "hi", fields1);

        // Log 2 xuất hiện thêm "severity" và "msg" -> không tạo cột trùng
        let mut fields2 = HashMap::new();
        fields2.insert("severity".to_string(), serde_json::json!("WARN"));
        fields2.insert("msg".to_string(), serde_json::json!("warn msg"));
        let log2 = LogEvent::new(
            "2026-08-20T10:00:01Z",
            LogColor::Yellow,
            "warn msg",
            fields2,
        );

        state.sync_discovered_keys(&[log1]);
        state.sync_discovered_keys(&[log2]);

        let names: Vec<&str> = state.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["timestamp", "level", "message"]);

        // Chiều ngược lại: "lv" tới trước -> đổi tên "level" thành "lv"; khi "level" tới sau không tạo cột trùng
        let mut reverse_state = ColumnState::default();
        let mut f_alias = HashMap::new();
        f_alias.insert("lv".to_string(), serde_json::json!("INFO"));
        let log_alias = LogEvent::new("2026-08-20T10:00:00Z", LogColor::Green, "hi", f_alias);

        let mut f_canon = HashMap::new();
        f_canon.insert("level".to_string(), serde_json::json!("WARN"));
        let log_canon = LogEvent::new("2026-08-20T10:00:01Z", LogColor::Yellow, "hi2", f_canon);

        reverse_state.sync_discovered_keys(&[log_alias]);
        reverse_state.sync_discovered_keys(&[log_canon]);

        let rev_names: Vec<&str> = reverse_state
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(rev_names, ["timestamp", "lv", "message"]);

        // reset_to_defaults đưa về "level" và không để lại cột "lv" ẩn dư thừa
        reverse_state.reset_to_defaults();
        let reset_names: Vec<&str> = reverse_state
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(reset_names, ["timestamp", "level", "message"]);
    }

    #[test]
    fn test_saved_alias_does_not_create_duplicate_column_when_canonical_seen_first() {
        let mut s1 = ColumnState::default();
        let mut f1 = HashMap::new();
        f1.insert("lv".to_string(), serde_json::json!("INFO"));
        s1.sync_discovered_keys(&[LogEvent::new("t1", LogColor::Green, "m", f1)]);
        s1.set_column_width("lv", 84.0);
        let saved = s1.to_saved_columns();

        // Mở lại với saved_layout có "lv" (visible: true), nhưng log đầu lại dùng "level", log sau dùng "lv"
        let mut s2 = ColumnState::default();
        s2.restore_from_saved(&saved);
        s2.sync_discovered_keys(&[
            LogEvent::new(
                "t2",
                LogColor::Green,
                "m1",
                HashMap::from([("level".to_string(), serde_json::json!("INFO"))]),
            ),
            LogEvent::new(
                "t3",
                LogColor::Green,
                "m2",
                HashMap::from([("lv".to_string(), serde_json::json!("WARN"))]),
            ),
        ]);

        let names: Vec<&str> = s2.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["timestamp", "level", "message"]);
    }

    #[test]
    fn test_saved_columns_exact_key_match_and_no_ghost_columns() {
        let mut s1 = ColumnState::default();
        let mut f1 = HashMap::new();
        f1.insert("aaa".to_string(), serde_json::json!("v1"));
        f1.insert("unused_hidden".to_string(), serde_json::json!("v2"));
        s1.sync_discovered_keys(&[LogEvent::new("t1", LogColor::Green, "m", f1)]);

        // Bật "aaa", chỉnh độ rộng thành 220.0 và đưa lên ngay trước "message"
        if let Some(col) = s1.columns.iter_mut().find(|c| c.name == "aaa") {
            col.visible = true;
            col.width = 220.0;
        }
        s1.move_column("aaa", "message");
        s1.set_column_width("timestamp", 190.0);

        let saved = s1.to_saved_columns();
        // Không lưu cột ẩn mặc định ("unused_hidden")
        assert!(!saved.iter().any(|c| c.name == "unused_hidden"));
        assert!(saved
            .iter()
            .any(|c| c.name == "aaa" && c.visible && (c.width - 220.0).abs() < 0.01));

        // Trường hợp 1: Mở lại với log CÓ đúng key "aaa" -> khôi phục đúng vị trí, visible và width 220.0
        let mut s2 = ColumnState::default();
        s2.restore_from_saved(&saved);
        // Trước khi log có "aaa" tới, không hiện cột ma "aaa"
        assert!(!s2.columns.iter().any(|c| c.name == "aaa"));

        let mut f2 = HashMap::new();
        f2.insert("bbb".to_string(), serde_json::json!("other"));
        f2.insert("aaa".to_string(), serde_json::json!("matched"));
        s2.sync_discovered_keys(&[LogEvent::new("t2", LogColor::Green, "m", f2)]);

        let vis_names: Vec<&str> = s2
            .columns
            .iter()
            .filter(|c| c.visible)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(vis_names, ["timestamp", "level", "aaa", "message"]);
        let aaa = s2.columns.iter().find(|c| c.name == "aaa").unwrap();
        assert!((aaa.width - 220.0).abs() < 0.01);
        // Key khác ("bbb") KHÔNG ăn nhầm width/visible của "aaa"
        let bbb = s2.columns.iter().find(|c| c.name == "bbb").unwrap();
        assert!(!bbb.visible);
        assert!((bbb.width - 120.0).abs() < 0.01);

        // Trường hợp 2: Mở lại nhưng dev đã bỏ "aaa", chỉ có "bbb" -> "aaa" không thành cột ma
        let mut s3 = ColumnState::default();
        s3.restore_from_saved(&saved);
        let mut f3 = HashMap::new();
        f3.insert("bbb".to_string(), serde_json::json!("only_b"));
        s3.sync_discovered_keys(&[LogEvent::new("t3", LogColor::Green, "m", f3)]);
        assert!(!s3.columns.iter().any(|c| c.name == "aaa"));
        let bbb3 = s3.columns.iter().find(|c| c.name == "bbb").unwrap();
        assert!(!bbb3.visible);
        assert!((bbb3.width - 120.0).abs() < 0.01);
    }

    #[test]
    fn test_saved_default_alias_only_applies_width_on_exact_key_match() {
        let mut s1 = ColumnState::default();
        let mut f1 = HashMap::new();
        f1.insert("lvl".to_string(), serde_json::json!("INFO"));
        s1.sync_discovered_keys(&[LogEvent::new("t1", LogColor::Green, "m", f1)]);
        s1.set_column_width("lvl", 88.0);
        let saved = s1.to_saved_columns();

        // Mở rồi đóng ngay khi chưa có log nào -> không làm mất cấu hình "lvl" (88.0)
        let mut empty_reopen = ColumnState::default();
        empty_reopen.restore_from_saved(&saved);
        let preserved = empty_reopen.to_saved_columns();
        assert!(preserved
            .iter()
            .any(|c| c.name == "lvl" && (c.width - 88.0).abs() < 0.01));

        // Dev đổi logger từ "lvl" -> "level": "level" KHÔNG ăn nhầm width 88.0 của "lvl"
        let mut s_canon = ColumnState::default();
        s_canon.restore_from_saved(&saved);
        let mut f_canon = HashMap::new();
        f_canon.insert("level".to_string(), serde_json::json!("INFO"));
        s_canon.sync_discovered_keys(&[LogEvent::new("t2", LogColor::Green, "m", f_canon)]);
        let level_col = s_canon.columns.iter().find(|c| c.name == "level").unwrap();
        assert!((level_col.width - 56.0).abs() < 0.01);

        // Nếu log vẫn dùng "lvl": khớp đúng key "lvl" -> áp dụng lại width 88.0
        let mut s_alias = ColumnState::default();
        s_alias.restore_from_saved(&saved);
        let mut f_alias = HashMap::new();
        f_alias.insert("lvl".to_string(), serde_json::json!("INFO"));
        s_alias.sync_discovered_keys(&[LogEvent::new("t3", LogColor::Green, "m", f_alias)]);
        let lvl_col = s_alias.columns.iter().find(|c| c.name == "lvl").unwrap();
        assert!((lvl_col.width - 88.0).abs() < 0.01);
    }
}
