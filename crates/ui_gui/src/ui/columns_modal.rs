use crate::app::{AppAction, UwuGuiApp};
use crate::ui::card::render_card;
use crate::ui::theme;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Rounding, Stroke};
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
    pub header_drop_target: Option<String>,
    pub known_keys: std::collections::HashSet<String>,
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
            header_drop_target: None,
            known_keys,
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
    }

    pub fn apply_modal(&mut self) {
        if let Some(draft) = self.draft_columns.take() {
            self.columns = draft;
            self.known_keys = self.columns.iter().map(|c| c.name.clone()).collect();
        }
        self.is_modal_open = false;
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
        self.dragged_index = None;
        self.header_dragged_name = None;
        self.header_drop_target = None;
        self.known_keys = self.columns.iter().map(|c| c.name.clone()).collect();
    }

    pub fn reset_draft_to_defaults(&mut self) {
        let source = self.draft_columns.as_deref().unwrap_or(&self.columns);
        self.draft_columns = Some(Self::merge_with_defaults(source));
        self.dragged_index = None;
    }

    pub fn reorder(&mut self, from_idx: usize, to_idx: usize) {
        reorder_vec(&mut self.columns, from_idx, to_idx);
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

pub fn render_columns_modal(ctx: &egui::Context, app: &mut UwuGuiApp) {
    if !app.column_state.is_modal_open {
        return;
    }

    if app.column_state.draft_columns.is_none() {
        app.column_state.draft_columns = Some(app.column_state.columns.clone());
    }

    let mut action_to_dispatch: Option<AppAction> = None;

    egui::Window::new("📊 Table Columns & Ordering")
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .inner_margin(egui::Margin::same(14.0))
                .rounding(Rounding::same(6.0)),
        )
        .collapsible(false)
        .resizable(false)
        .min_width(520.0)
        .max_width(520.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Columns Configuration & Drag-to-Reorder")
                    .size(14.0)
                    .strong()
                    .color(theme::TEXT_KEY),
            );
            ui.label(
                egui::RichText::new(
                    "Click & drag ⠿ items up or down to reorder columns. Toggle checkboxes to show/hide.",
                )
                .size(11.5)
                .color(theme::TEXT_MUTED),
            );
            ui.separator();
            ui.add_space(6.0);

            // Filter search box
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔍").size(12.0).color(theme::TEXT_MUTED));
                let avail_w = ui.available_width();
                let clear_btn_w = if app.column_state.filter_query.is_empty() { 0.0 } else { 28.0 };
                let text_w = (avail_w - clear_btn_w - 6.0).max(100.0);

                ui.add_sized(
                    [text_w, 22.0],
                    egui::TextEdit::singleline(&mut app.column_state.filter_query)
                        .hint_text("Filter column keys...")
                        .font(egui::TextStyle::Monospace)
                        .margin(egui::Margin::symmetric(8.0, 4.0)),
                );
                if !app.column_state.filter_query.is_empty() && ui.button("✖").clicked() {
                    app.column_state.filter_query.clear();
                }
            });

            ui.add_space(8.0);

            // Columns Drag & Drop List Card
            render_card(ui, "Columns List (Drag to Reorder)", |ui| {
                let filter_lower = app.column_state.filter_query.trim().to_lowercase();
                let total_cols = app.column_state.draft_columns.as_ref().map_or(0, |c| c.len());

                let pointer_pos = ui.ctx().pointer_latest_pos();
                let pointer_released = ui.input(|i| i.pointer.any_released());

                if pointer_released {
                    app.column_state.dragged_index = None;
                }

                egui::ScrollArea::vertical()
                    .id_salt("columns_modal_scroll_area")
                    .max_height(340.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut new_drag_source = None;
                        let mut target_drop = None;
                        let mut toggle_vis = None;

                        for idx in 0..total_cols {
                            let (col_name, col_visible) = {
                                let item = &app.column_state.draft_columns.as_ref().unwrap()[idx];
                                (item.name.clone(), item.visible)
                            };

                            if !filter_lower.is_empty()
                                && !col_name.to_lowercase().contains(&filter_lower)
                            {
                                continue;
                            }

                            let is_dragging_this = app.column_state.dragged_index == Some(idx);
                            let desired_size = egui::vec2(ui.available_width(), 30.0);
                            let (rect, resp) = ui.allocate_exact_size(
                                desired_size,
                                egui::Sense::click_and_drag(),
                            );

                            // Detect drag started
                            if resp.drag_started() {
                                new_drag_source = Some(idx);
                            }

                            // Detect drop target while dragging
                            if let Some(dragged_idx) = app.column_state.dragged_index {
                                if dragged_idx != idx {
                                    if let Some(pos) = pointer_pos {
                                        if rect.contains(pos) {
                                            target_drop = Some((dragged_idx, idx));
                                        }
                                    }
                                }
                            }

                            // Cursor icon
                            if resp.hovered() || is_dragging_this {
                                if is_dragging_this {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                } else {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                }
                            }

                            // Background and border styling
                            let bg_color = if is_dragging_this {
                                theme::BG_ROW_SELECTED
                            } else if resp.hovered() {
                                theme::BG_ROW_HOVER
                            } else {
                                theme::BG_BASE
                            };

                            let border_stroke = if is_dragging_this {
                                Stroke::new(1.5, theme::TEXT_KEY)
                            } else if resp.hovered() {
                                Stroke::new(1.0, theme::BG_SURFACE1)
                            } else {
                                Stroke::new(1.0, theme::BG_SURFACE0)
                            };

                            ui.painter().rect(
                                rect,
                                Rounding::same(4.0),
                                bg_color,
                                border_stroke,
                            );

                            let center_y = rect.center().y;

                            // 1. Drag Grip Icon (⠿)
                            let grip_x = rect.min.x + 10.0;
                            ui.painter().text(
                                Pos2::new(grip_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                "⠿",
                                FontId::monospace(14.0),
                                if is_dragging_this || resp.hovered() {
                                    theme::TEXT_KEY
                                } else {
                                    theme::TEXT_MUTED
                                },
                            );

                            // 2. Custom Checkbox
                            let checkbox_x = grip_x + 22.0;
                            let check_rect = Rect::from_center_size(
                                Pos2::new(checkbox_x + 8.0, center_y),
                                egui::vec2(16.0, 16.0),
                            );

                            let check_resp = ui.interact(
                                check_rect,
                                ui.make_persistent_id(format!("chk_{idx}_{col_name}")),
                                egui::Sense::click(),
                            );

                            if check_resp.clicked() {
                                toggle_vis = Some((idx, !col_visible));
                            }

                            let check_bg = if col_visible {
                                theme::TEXT_KEY
                            } else {
                                Color32::TRANSPARENT
                            };
                            let check_stroke = Stroke::new(
                                1.0,
                                if col_visible {
                                    theme::TEXT_KEY
                                } else {
                                    theme::TEXT_MUTED
                                },
                            );

                            ui.painter().rect(
                                check_rect,
                                Rounding::same(3.0),
                                check_bg,
                                check_stroke,
                            );

                            if col_visible {
                                ui.painter().text(
                                    check_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "✓",
                                    FontId::monospace(11.0),
                                    theme::BG_BASE,
                                );
                            }

                            // 3. Raw Key Name
                            let text_x = checkbox_x + 24.0;
                            let text_color = if col_visible {
                                theme::TEXT_PRIMARY
                            } else {
                                theme::TEXT_MUTED
                            };
                            ui.painter().text(
                                Pos2::new(text_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                &col_name,
                                FontId::monospace(12.0),
                                text_color,
                            );

                            // Also toggle visibility on clicking name area if not dragging
                            if resp.clicked() && !check_resp.clicked() {
                                toggle_vis = Some((idx, !col_visible));
                            }

                            // 4. Position / Status badge on the right
                            let right_x = rect.max.x - 10.0;
                            if col_visible {
                                let pos_text = format!("Pos #{}", idx + 1);
                                ui.painter().text(
                                    Pos2::new(right_x, center_y),
                                    egui::Align2::RIGHT_CENTER,
                                    pos_text,
                                    FontId::monospace(11.0),
                                    theme::COLOR_INFO,
                                );
                            } else {
                                ui.painter().text(
                                    Pos2::new(right_x, center_y),
                                    egui::Align2::RIGHT_CENTER,
                                    "Hidden",
                                    FontId::monospace(11.0),
                                    theme::TEXT_MUTED,
                                );
                            }

                            ui.add_space(4.0);
                        }

                        if let Some(idx) = new_drag_source {
                            app.column_state.dragged_index = Some(idx);
                        }

                        if let Some((from, to)) = target_drop {
                            app.column_state.reorder_draft(from, to);
                            app.column_state.dragged_index = Some(to);
                            ui.ctx().request_repaint();
                        }

                        if let Some((idx, new_vis)) = toggle_vis {
                            if let Some(draft) = &mut app.column_state.draft_columns {
                                draft[idx].visible = new_vis;
                            }
                        }
                    });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            // Action buttons
            ui.horizontal(|ui| {
                let reset_btn = egui::Button::new(
                    egui::RichText::new("🔄 Reset Defaults").color(theme::TEXT_PRIMARY),
                )
                .fill(theme::BG_SURFACE0)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                .rounding(Rounding::same(4.0));

                if ui
                    .add(reset_btn)
                    .on_hover_text("Reset draft column order and visibility to default")
                    .clicked()
                {
                    app.column_state.reset_draft_to_defaults();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let done_btn = egui::Button::new(
                        egui::RichText::new("✔ Apply & Done")
                            .strong()
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(theme::BTN_RESTART_BG)
                    .stroke(Stroke::new(1.0, theme::BTN_RESTART_BORDER))
                    .rounding(Rounding::same(4.0));

                    if ui.add(done_btn).clicked() {
                        action_to_dispatch = Some(AppAction::ApplyColumnsModal);
                    }

                    ui.add_space(6.0);

                    let cancel_btn = egui::Button::new(
                        egui::RichText::new("Cancel").color(theme::TEXT_MUTED),
                    )
                    .fill(theme::BG_SURFACE0)
                    .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                    .rounding(Rounding::same(4.0));

                    if ui.add(cancel_btn).clicked() {
                        action_to_dispatch = Some(AppAction::CloseColumnsModal);
                    }
                });
            });
        });

    if let Some(action) = action_to_dispatch {
        app.dispatch_action(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uwu_core_schema::LogColor;

    #[test]
    fn test_default_columns() {
        let state = ColumnState::default();
        assert_eq!(state.columns.len(), 3);
        assert_eq!(state.columns[0].name, "timestamp");
        assert!(state.columns[0].visible);
        assert_eq!(state.columns[1].name, "level");
        assert!(state.columns[1].visible);
        assert_eq!(state.columns[2].name, "message");
        assert!(state.columns[2].visible);
    }

    #[test]
    fn test_reorder_columns() {
        let mut state = ColumnState::default();
        // Reorder "level" (idx 1) to front (idx 0)
        state.reorder(1, 0);
        assert_eq!(state.columns[0].name, "level");
        assert_eq!(state.columns[1].name, "timestamp");
        assert_eq!(state.columns[2].name, "message");

        // Reorder "level" (idx 0) to idx 2
        state.reorder(0, 2);
        assert_eq!(state.columns[0].name, "timestamp");
        assert_eq!(state.columns[1].name, "message");
        assert_eq!(state.columns[2].name, "level");
    }

    #[test]
    fn test_sync_discovered_keys() {
        let mut state = ColumnState::default();
        let mut fields = HashMap::new();
        fields.insert("latency_ms".to_string(), serde_json::json!(150));
        fields.insert("user_id".to_string(), serde_json::json!("u42"));

        let log = LogEvent::new("2026-08-20T10:00:00Z", LogColor::Green, "msg", fields);

        state.sync_discovered_keys(&[log]);
        assert_eq!(state.columns.len(), 5);
        assert!(state
            .columns
            .iter()
            .any(|c| c.name == "latency_ms" && !c.visible));
        assert!(state
            .columns
            .iter()
            .any(|c| c.name == "user_id" && !c.visible));
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
        state.reorder(1, 0); // Swap level & timestamp
        state.columns[2].visible = false; // Turn off message

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

        // Initialize draft
        state.draft_columns = Some(state.columns.clone());
        state.reorder_draft(1, 0); // Swap level & timestamp in draft

        // Original columns should remain untouched (Live vs Draft isolation)
        assert_eq!(state.columns, original_cols);

        // Commit draft
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

        // Discard draft (Cancel action)
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
}
