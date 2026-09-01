use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Color32, FontId, Id, Key, Order, Pos2, Rect, Rounding, Stroke};
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

pub fn render_history_popup(ctx: &egui::Context, app: &mut UwuGuiApp, input_rect: Rect) {
    if !app.history_state.is_open {
        return;
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.history_state.is_open = false;
        return;
    }

    let popup_pos = Pos2::new(input_rect.min.x, input_rect.max.y + 8.0);
    let popup_width = input_rect.width().max(420.0);
    let entry_count = app.history_state.entries.len().max(1);
    let approx_height = (entry_count as f32 * 28.0) + 48.0;
    let popup_rect = Rect::from_min_size(popup_pos, egui::vec2(popup_width, approx_height));

    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !input_rect.contains(pos) && !popup_rect.contains(pos) {
                app.history_state.is_open = false;
                return;
            }
        }
    }

    let mut selected_history_item = None;
    let mut clear_all_clicked = false;

    egui::Area::new(Id::new("search_history_dropdown_area"))
        .order(Order::Foreground)
        .fixed_pos(popup_pos)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::same(6.0))
                .show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header bar của History Popup
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("⏱ Search History")
                                .font(FontId::monospace(12.0))
                                .color(theme::TEXT_KEY)
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !app.history_state.entries.is_empty()
                                && ui
                                    .button(
                                        egui::RichText::new("Clear")
                                            .font(FontId::monospace(11.0))
                                            .color(theme::COLOR_WARN),
                                    )
                                    .on_hover_text("Clear all search history")
                                    .clicked()
                            {
                                clear_all_clicked = true;
                            }
                        });
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if app.history_state.entries.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("No search history yet.")
                                    .font(FontId::monospace(11.5))
                                    .color(theme::TEXT_MUTED)
                                    .italics(),
                            );
                        });
                        ui.add_space(2.0);
                    } else {
                        for hist_query in &app.history_state.entries {
                            let desired_size = egui::vec2(ui.available_width(), 26.0);
                            let (rect, resp) =
                                ui.allocate_exact_size(desired_size, egui::Sense::click());

                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            if resp.clicked() {
                                selected_history_item = Some(hist_query.clone());
                            }

                            let row_bg = if resp.hovered() {
                                theme::BG_ROW_SELECTED
                            } else {
                                Color32::TRANSPARENT
                            };

                            ui.painter().rect_filled(rect, Rounding::same(4.0), row_bg);

                            let center_y = rect.center().y;
                            let left_x = rect.min.x + 8.0;

                            // Icon tìm kiếm nhỏ
                            ui.painter().text(
                                Pos2::new(left_x, center_y),
                                egui::Align2::LEFT_CENTER,
                                "🔍",
                                FontId::monospace(10.5),
                                theme::TEXT_MUTED,
                            );

                            // Nội dung câu query
                            ui.painter().text(
                                Pos2::new(left_x + 20.0, center_y),
                                egui::Align2::LEFT_CENTER,
                                hist_query,
                                FontId::monospace(12.0),
                                theme::TEXT_PRIMARY,
                            );
                        }
                    }
                });
        });

    if clear_all_clicked {
        app.history_state.entries.clear();
        ctx.request_repaint();
    }

    if let Some(chosen_query) = selected_history_item {
        app.query = chosen_query.clone();
        app.history_state.apply_history_item(&chosen_query);
        app.autocomplete_state.is_open = false;
        app.autocomplete_state.suggestions.clear();
        app.autocomplete_state.just_applied = true;
        app.trigger_full_search();
        ctx.request_repaint();
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
