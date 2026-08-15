use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Id, Rounding, Stroke};
use std::time::Instant;

pub fn render_header(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.horizontal(|ui| {
        // App Title / Brand
        ui.label(
            egui::RichText::new("🐱 uwulog")
                .strong()
                .size(14.5)
                .color(theme::TEXT_KEY),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Search Input Box
        let search_id = Id::new("search_query_input");
        let search_response = ui.add(
            egui::TextEdit::singleline(&mut app.query)
                .id(search_id)
                .hint_text("🔍 Filter query (e.g. level:error, status:500, time:now..10m)...")
                .desired_width(500.0)
                .font(egui::TextStyle::Monospace)
                .margin(egui::Margin::symmetric(10.0, 6.0)),
        );

        // Giữ lại con trỏ chuột và focus vào ô input sau khi chọn gợi ý
        if app.autocomplete_state.just_applied {
            app.autocomplete_state.just_applied = false;
            ui.ctx().memory_mut(|m| m.request_focus(search_id));
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), search_id) {
                let char_count = app.query.chars().count();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(char_count),
                    )));
                state.store(ui.ctx(), search_id);
            }
        } else if search_response.changed() || search_response.gained_focus() {
            // Khi gõ chữ hoặc focus vào ô tìm kiếm: luôn ẩn menu lịch sử
            app.history_state.close_popup();

            let available_fields = app.get_available_log_fields();
            let (suggestions, token_range) =
                crate::ui::autocomplete::generate_suggestions(&app.query, &available_fields);
            app.autocomplete_state.suggestions = suggestions;
            app.autocomplete_state.active_token_range = token_range;
            app.autocomplete_state.selected_index = 0;
            app.autocomplete_state.is_open = !app.autocomplete_state.suggestions.is_empty();
            if search_response.changed() {
                // Reset debounce timer để tick() sẽ lưu lịch sử sau 500ms dừng gõ
                app.history_state.mark_query_changed(Instant::now());
                app.trigger_full_search();
            }
        }

        let search_rect = search_response.rect;

        // Lưu lịch sử khi người dùng nhấn Enter để hoàn tất tìm kiếm
        if (search_response.lost_focus() || search_response.has_focus())
            && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
        {
            app.history_state.record(&app.query.clone());
            app.history_state.mark_recorded();
        }

        // Quick Clear button if query is not empty
        if !app.query.is_empty()
            && ui
                .button(
                    egui::RichText::new("✖")
                        .size(11.0)
                        .color(theme::TEXT_PRIMARY),
                )
                .on_hover_text("Clear filter")
                .clicked()
        {
            app.query.clear();
            app.autocomplete_state.is_open = false;
            app.history_state.close_popup();
            app.trigger_full_search();
        }

        // Search History Toggle Button (⏱)
        let history_btn = egui::Button::new(egui::RichText::new("⏱").size(12.0).color(
            if app.history_state.is_open {
                theme::TEXT_KEY
            } else {
                theme::TEXT_MUTED
            },
        ))
        .fill(if app.history_state.is_open {
            theme::BG_SURFACE1
        } else {
            theme::BG_SURFACE0
        })
        .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
        .rounding(Rounding::same(4.0));

        if ui
            .add(history_btn)
            .on_hover_text("Search history")
            .clicked()
        {
            let opened = app.history_state.toggle_popup();
            if opened {
                app.autocomplete_state.is_open = false;
                app.autocomplete_state.suggestions.clear();
            }
        }

        // Render autocomplete popup dropdown below search box
        crate::ui::autocomplete::render_autocomplete_popup(ui.ctx(), app, search_rect);

        // Render search history popup dropdown below search box
        crate::ui::history::render_history_popup(ui.ctx(), app, search_rect);

        // Right-aligned Controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Source Parameters Modal Button
            let params_btn = egui::Button::new(
                egui::RichText::new("⚙ Params")
                    .strong()
                    .color(theme::TEXT_PRIMARY),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            if ui
                .add(params_btn)
                .on_hover_text("Configure engine buffer, display limits & sources")
                .clicked()
            {
                app.show_launch_modal = true;
            }

            ui.add_space(6.0);

            // Stop / Restart Source Button
            if app.is_source_running {
                let stop_btn = egui::Button::new(
                    egui::RichText::new("⏹ Stop")
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                )
                .fill(theme::BTN_STOP_BG)
                .stroke(Stroke::new(1.0, theme::BTN_STOP_BORDER))
                .rounding(Rounding::same(4.0));

                if ui
                    .add(stop_btn)
                    .on_hover_text("Stop running process source")
                    .clicked()
                {
                    app.stop_current_source();
                }
            } else {
                let restart_btn = egui::Button::new(
                    egui::RichText::new("🔄 Restart")
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                )
                .fill(theme::BTN_RESTART_BG)
                .stroke(Stroke::new(1.0, theme::BTN_RESTART_BORDER))
                .rounding(Rounding::same(4.0));

                if ui
                    .add(restart_btn)
                    .on_hover_text("Clear logs and restart source")
                    .clicked()
                {
                    app.restart_current_source();
                }
            }

            ui.add_space(6.0);

            // Latch / Auto-scroll State Toggle Button
            let (latch_text, latch_text_color, latch_bg, latch_border, latch_tooltip) =
                if app.is_auto_scroll {
                    (
                        "⚓ Live",
                        theme::COLOR_INFO,
                        theme::BTN_LATCHED_BG,
                        theme::BTN_LATCHED_BORDER,
                        "Auto-scroll: ON (Following tail)\n• Click to pause (Unlatch)\n• Scroll up or select a log to unlatch",
                    )
                } else {
                    (
                        "⏸ Paused",
                        theme::COLOR_WARN,
                        theme::BTN_UNLATCHED_BG,
                        theme::BTN_UNLATCHED_BORDER,
                        "Auto-scroll: PAUSED (View frozen)\n• Click to latch & scroll to bottom\n• Or press [End] / scroll to bottom",
                    )
                };

            let latch_btn = egui::Button::new(
                egui::RichText::new(latch_text)
                    .color(latch_text_color)
                    .strong(),
            )
            .fill(latch_bg)
            .stroke(Stroke::new(1.0, latch_border))
            .rounding(Rounding::same(4.0));

            if ui.add(latch_btn).on_hover_text(latch_tooltip).clicked() {
                app.toggle_latch();
            }

            ui.add_space(6.0);

            // RingBuffer Capacity & Overflow Progress Bar
            render_buffer_progress(ui, app);

            ui.add_space(6.0);

            // Log Count Indicator
            ui.label(
                egui::RichText::new(format!("{} logs", theme::format_number(app.cached_logs.len())))
                    .font(egui::FontId::monospace(11.5))
                    .color(theme::TEXT_MUTED),
            );
        });
    });
}

fn render_buffer_progress(ui: &mut egui::Ui, app: &UwuGuiApp) {
    let total_in_buffer = app.engine.total_logs();
    let capacity = app.engine.max_capacity();
    let total_processed = app.engine.total_processed();
    let ratio = (total_in_buffer as f32 / capacity.max(1) as f32).clamp(0.0, 1.0);
    let is_overflow = total_processed > capacity as u64;
    let health_color = theme::buffer_health_color(ratio);

    let percent = ratio * 100.0;
    let bar_text = if is_overflow {
        "BUF: 100% (OVF)".to_string()
    } else {
        format!("BUF: {:.0}%", percent)
    };

    let (status_icon, status_desc) = if is_overflow {
        (
            "🔴",
            "OVERFLOW - Buffer is full, oldest logs are rotating & being evicted",
        )
    } else if ratio >= 0.8 {
        ("🟡", "WARNING - Buffer is nearing capacity limit")
    } else {
        ("🟢", "HEALTHY - Operating normally within buffer limits")
    };

    let evicted = total_processed.saturating_sub(total_in_buffer as u64);

    let tooltip = format!(
        "RingBuffer Memory & Capacity Monitor:\n\
         • Stored in Memory: {} / {} logs ({:.1}%)\n\
         • Total Ingested: {} logs\n\
         • Evicted (Overwritten): {} logs\n\
         • Health Status: {} {}\n\n\
         Tip: To change capacity, click '⚙ Params'.",
        theme::format_number(total_in_buffer),
        theme::format_number(capacity),
        percent,
        theme::format_number(total_processed as usize),
        theme::format_number(evicted as usize),
        status_icon,
        status_desc
    );

    let progress_bar = egui::ProgressBar::new(ratio)
        .desired_width(100.0)
        .desired_height(18.0)
        .fill(health_color)
        .rounding(Rounding::same(3.0))
        .text(
            egui::RichText::new(bar_text)
                .font(egui::FontId::monospace(10.5))
                .strong()
                .color(theme::TEXT_PRIMARY),
        );

    ui.add(progress_bar).on_hover_text(tooltip);
}
