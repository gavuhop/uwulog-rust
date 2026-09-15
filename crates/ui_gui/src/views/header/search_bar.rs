use crate::actions::AppAction;
use crate::session::GuiSession;
use crate::state::generate_suggestions;
use eframe::egui::{self, Id};
use std::time::Instant;

pub fn render_search_bar(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    let button_extras = if !session.view.search.query.is_empty() {
        56.0
    } else {
        28.0
    };
    let available_w = ui.available_width();
    let search_box_width = (available_w - button_extras - 6.0).max(40.0);

    let search_id = Id::new("search_query_input");
    let search_response = crate::components::ui::TextInput::new(&mut session.view.search.query)
        .id(search_id)
        .hint_text("Filter query (e.g. level:error, status:500, time:now..10m)...")
        .width(search_box_width)
        .show(ui);

    // Giữ lại con trỏ chuột và focus vào ô input sau khi chọn gợi ý
    if session.view.search.autocomplete.just_applied {
        session.view.search.autocomplete.just_applied = false;
        ui.ctx().memory_mut(|m| m.request_focus(search_id));
        if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), search_id) {
            let char_count = session.view.search.query.chars().count();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(char_count),
                )));
            state.store(ui.ctx(), search_id);
        }
    } else if search_response.changed() || search_response.gained_focus() {
        // Khi gõ chữ hoặc focus vào ô tìm kiếm: luôn ẩn menu lịch sử
        session.view.search.history.close_popup();

        let available_fields = session.get_available_log_fields();
        let (suggestions, token_range) =
            generate_suggestions(&session.view.search.query, &available_fields);
        session.view.search.autocomplete.suggestions = suggestions;
        session.view.search.autocomplete.active_token_range = token_range;
        session.view.search.autocomplete.selected_index = 0;
        session.view.search.autocomplete.is_open =
            !session.view.search.autocomplete.suggestions.is_empty();
        if search_response.changed() {
            // Reset debounce timer để tick() sẽ lưu lịch sử sau 500ms dừng gõ
            session
                .view
                .search
                .history
                .mark_query_changed(Instant::now());
            session.trigger_full_search();
        }
    }

    let search_rect = search_response.rect;

    // Lưu lịch sử khi người dùng nhấn Enter để hoàn tất tìm kiếm
    if (search_response.lost_focus() || search_response.has_focus())
        && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
    {
        let q = session.view.search.query.clone();
        session.view.search.history.record(&q);
        session.view.search.history.mark_recorded();
    }

    // Quick Clear button if query is not empty
    let clear_resp = if !session.view.search.query.is_empty() {
        Some(
            crate::components::ui::IconButton::new(crate::components::ui::IconName::Close)
                .size(24.0)
                .tooltip("Clear filter")
                .show(ui),
        )
    } else {
        None
    };
    if let Some(resp) = &clear_resp {
        if resp.clicked() {
            dispatch(AppAction::ClearQuery);
        }
    }

    // Search History Toggle Button
    let history_resp =
        crate::components::ui::IconButton::new(crate::components::ui::IconName::History)
            .size(24.0)
            .selected(session.view.search.history.is_open)
            .tooltip("Search history")
            .show(ui);
    if history_resp.clicked() {
        let opened = session.view.search.history.toggle_popup();
        if opened {
            session.view.search.autocomplete.is_open = false;
            session.view.search.autocomplete.suggestions.clear();
        }
    }

    // Render autocomplete popup dropdown below search box
    crate::components::render_autocomplete_popup(ui.ctx(), session, search_rect);

    // Render search history popup dropdown below search box
    let combined_trigger_rect = search_rect.union(history_resp.rect);
    crate::components::render_history_popup(ui.ctx(), session, search_rect, combined_trigger_rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_core_workspace::{SourceConfig, SourceType, WorkspaceLocation, WorkspaceSession};

    #[test]
    fn test_render_search_bar_layout_and_popups() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let ctx = egui::Context::default();
        let raw_input = egui::RawInput::default();
        let source_config = SourceConfig {
            source_type: SourceType::Process,
            command_str: String::new(),
            file_path: String::new(),
            working_dir: String::new(),
            capacity: 100,
            display_limit: 50,
        };
        let ws = WorkspaceSession::new(
            "Test".to_string(),
            WorkspaceLocation::local(""),
            source_config,
        );
        let mut session = GuiSession::new(ws);
        session.view.search.query = "level:error".to_string();

        let mut output = ctx.run_ui(raw_input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut dispatch = |_| {};
                render_search_bar(ui, &mut session, &mut dispatch);
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn test_search_history_popup_stays_open_on_trigger() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let ctx = egui::Context::default();
        let source_config = SourceConfig {
            source_type: SourceType::Process,
            command_str: String::new(),
            file_path: String::new(),
            working_dir: String::new(),
            capacity: 100,
            display_limit: 50,
        };
        let ws = WorkspaceSession::new(
            "Test".to_string(),
            WorkspaceLocation::local(""),
            source_config,
        );
        let mut session = GuiSession::new(ws);
        session.view.search.history.is_open = true;
        session.view.search.history.entries =
            vec!["level:error".to_string(), "status:500".to_string()];

        let raw_input = egui::RawInput::default();
        let mut output = ctx.run_ui(raw_input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut dispatch = |_| {};
                render_search_bar(ui, &mut session, &mut dispatch);
            });
        });
        output.textures_delta.clear();

        // History popup should remain open because no outside click occurred
        assert!(session.view.search.history.is_open);
    }
}
