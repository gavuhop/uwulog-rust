use crate::actions::AppAction;
use crate::session::GuiSession;
use crate::state::generate_suggestions;
use crate::theme;
use eframe::egui::{self, Id, Stroke};
use std::time::Instant;

pub fn render_search_bar(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    let button_extras = if !session.view.search.query.is_empty() {
        52.0
    } else {
        28.0
    };
    let available_w = ui.available_width();
    let search_box_width = (available_w - button_extras - 6.0).max(40.0);

    let search_id = Id::new("search_query_input");
    let search_response = ui.add(
        egui::TextEdit::singleline(&mut session.view.search.query)
            .id(search_id)
            .hint_text(
                egui::RichText::new(
                    "🔍 Filter query (e.g. level:error, status:500, time:now..10m)...",
                )
                .color(theme::TEXT_PLACEHOLDER),
            )
            .desired_width(search_box_width)
            .font(egui::TextStyle::Monospace)
            .margin(egui::Margin::symmetric(8.0, 4.0)),
    );

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
    if !session.view.search.query.is_empty()
        && ui
            .button(
                egui::RichText::new("✖")
                    .size(10.5)
                    .color(theme::TEXT_PRIMARY),
            )
            .on_hover_text("Clear filter")
            .clicked()
    {
        dispatch(AppAction::ClearQuery);
    }

    // Search History Toggle Button (⏱)
    let history_btn = egui::Button::new(egui::RichText::new("⏱").size(11.0).color(
        if session.view.search.history.is_open {
            theme::TEXT_KEY
        } else {
            theme::TEXT_MUTED
        },
    ))
    .fill(if session.view.search.history.is_open {
        theme::BG_SURFACE1
    } else {
        theme::BG_SURFACE0
    })
    .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
    .rounding(egui::Rounding::same(4.0));

    if ui
        .add(history_btn)
        .on_hover_text("Search history")
        .clicked()
    {
        let opened = session.view.search.history.toggle_popup();
        if opened {
            session.view.search.autocomplete.is_open = false;
            session.view.search.autocomplete.suggestions.clear();
        }
    }

    // Render autocomplete popup dropdown below search box
    crate::components::render_autocomplete_popup(ui.ctx(), session, search_rect);

    // Render search history popup dropdown below search box
    crate::components::render_history_popup(ui.ctx(), session, search_rect);
}
