use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Id, Rounding, Stroke};
use std::time::Instant;

pub fn render_header(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.horizontal(|ui| {
        // 1. App Title / Brand (hỗ trợ kéo di chuyển cửa sổ & double click maximize)
        let brand_response = ui.add(
            egui::Label::new(
                egui::RichText::new("🐱 uwulog")
                    .strong()
                    .size(13.5)
                    .color(theme::TEXT_KEY),
            )
            .sense(egui::Sense::click_and_drag()),
        );
        if brand_response.dragged_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if brand_response.double_clicked() {
            let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // 2. Stream Tabs: Main / Filtered and Raw Stream
        let is_filtered_tab = app.active_tab == crate::app::ActiveTab::Filtered;
        let is_unfiltered_tab = app.active_tab == crate::app::ActiveTab::Unfiltered;
        let is_filtering = !app.query.trim().is_empty();

        let filtered_tab_text = if is_filtering {
            "🔍 Filtered".to_string()
        } else {
            "🔍 Main".to_string()
        };

        let tab_filtered_btn = egui::Button::new(
            egui::RichText::new(filtered_tab_text)
                .size(11.5)
                .strong()
                .color(if is_filtered_tab {
                    theme::TEXT_PRIMARY
                } else {
                    theme::TEXT_MUTED
                }),
        )
        .fill(if is_filtered_tab {
            theme::BG_SURFACE1
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(Stroke::new(
            1.0,
            if is_filtered_tab {
                theme::TEXT_KEY
            } else {
                theme::BG_SURFACE0
            },
        ))
        .rounding(Rounding::same(4.0));

        if ui
            .add(tab_filtered_btn)
            .on_hover_text("Switch to Filtered Logs view")
            .clicked()
        {
            app.active_tab = crate::app::ActiveTab::Filtered;
        }

        // Tab 2: Raw Stream (Chỉ xuất hiện khi người dùng đang có bộ lọc tìm kiếm hoặc đang mở tab Raw!)
        if is_filtering || is_unfiltered_tab {
            ui.add_space(2.0);

            let tab_unfil_btn = egui::Button::new(
                egui::RichText::new("📄 Raw")
                    .size(11.5)
                    .strong()
                    .color(if is_unfiltered_tab {
                        theme::TEXT_PRIMARY
                    } else {
                        theme::TEXT_MUTED
                    }),
            )
            .fill(if is_unfiltered_tab {
                theme::BG_SURFACE1
            } else {
                egui::Color32::TRANSPARENT
            })
            .stroke(Stroke::new(
                1.0,
                if is_unfiltered_tab {
                    theme::TEXT_KEY
                } else {
                    theme::BG_SURFACE0
                },
            ))
            .rounding(Rounding::same(4.0));

            if ui
                .add(tab_unfil_btn)
                .on_hover_text("Switch to Raw Stream view (500 logs buffer)")
                .clicked()
            {
                if !app.unfiltered_state.is_open {
                    app.open_unfiltered_stream(None);
                }
                app.active_tab = crate::app::ActiveTab::Unfiltered;
            }
        }

        // 3. Phía bên phải: Window Controls & Navigation Toolbar
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Bộ 3 nút điều khiển cửa sổ chuẩn (—, 🗖/🗗, ✕)
            render_window_controls(ui);

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Table Columns & Ordering Modal Button
            let visible_count = app.column_state.columns.iter().filter(|c| c.visible).count();
            let columns_btn = egui::Button::new(
                egui::RichText::new(format!("📊 ({visible_count})"))
                    .size(11.5)
                    .strong()
                    .color(theme::TEXT_PRIMARY),
            )
            .fill(if app.column_state.is_modal_open {
                theme::BG_SURFACE1
            } else {
                theme::BG_SURFACE0
            })
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            if ui
                .add(columns_btn)
                .on_hover_text("Configure visible columns and adjust their display order")
                .clicked()
            {
                app.column_state.is_modal_open = true;
            }

            ui.add_space(2.0);

            // Source Parameters Modal Button
            let params_btn = egui::Button::new(
                egui::RichText::new("⚙")
                    .size(11.5)
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

            ui.add_space(2.0);

            // Stop / Restart Source Button
            if app.is_source_running {
                let stop_btn = egui::Button::new(
                    egui::RichText::new("⏹")
                        .size(11.5)
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
                    egui::RichText::new("🔄")
                        .size(11.5)
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

            ui.add_space(4.0);

            // Snapshot Button (Chỉ hiển thị khi đang xem Tab Raw Stream)
            if app.active_tab == crate::app::ActiveTab::Unfiltered {
                let snapshot_tooltip = if app.unfiltered_state.is_live {
                    "Freeze current Raw Stream into a fixed snapshot at this moment"
                } else {
                    "Re-capture the latest surrounding context snapshot from buffer"
                };

                let snapshot_btn = egui::Button::new(
                    egui::RichText::new("📸")
                        .size(11.0)
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                )
                .fill(theme::BG_SURFACE0)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                .rounding(Rounding::same(4.0));

                if ui.add(snapshot_btn).on_hover_text(snapshot_tooltip).clicked() {
                    if app.unfiltered_state.is_live {
                        app.toggle_unfiltered_live();
                    } else {
                        app.refresh_unfiltered_snapshot();
                    }
                }

                ui.add_space(4.0);
            }

            // Latch / Live / Paused State Toggle Button
            let (is_live, toggle_tooltip) = match app.active_tab {
                crate::app::ActiveTab::Filtered => (
                    app.is_auto_scroll,
                    if app.is_auto_scroll {
                        "Main Stream: LIVE (Following tail)\n• Click to pause (Unlatch)\n• Scroll up or select a log to unlatch"
                    } else {
                        "Main Stream: PAUSED (View frozen)\n• Click to live stream & scroll to bottom"
                    },
                ),
                crate::app::ActiveTab::Unfiltered => (
                    app.unfiltered_state.is_live,
                    if app.unfiltered_state.is_live {
                        "Raw Stream: LIVE (Following real-time stream)\n• Click to pause / freeze snapshot"
                    } else {
                        "Raw Stream: PAUSED (Snapshot frozen)\n• Click to follow live real-time stream"
                    },
                ),
            };

            let (latch_text, latch_text_color, latch_bg, latch_border) = if is_live {
                (
                    "⚓ Live",
                    theme::COLOR_INFO,
                    theme::BTN_LATCHED_BG,
                    theme::BTN_LATCHED_BORDER,
                )
            } else {
                (
                    "⏸ Paused",
                    theme::COLOR_WARN,
                    theme::BTN_UNLATCHED_BG,
                    theme::BTN_UNLATCHED_BORDER,
                )
            };

            let latch_btn = egui::Button::new(
                egui::RichText::new(latch_text)
                    .size(11.5)
                    .color(latch_text_color)
                    .strong(),
            )
            .fill(latch_bg)
            .stroke(Stroke::new(1.0, latch_border))
            .rounding(Rounding::same(4.0));

            if ui.add(latch_btn).on_hover_text(toggle_tooltip).clicked() {
                match app.active_tab {
                    crate::app::ActiveTab::Filtered => app.toggle_latch(),
                    crate::app::ActiveTab::Unfiltered => app.toggle_unfiltered_live(),
                }
            }

            ui.add_space(4.0);

            // Log Count / Filter Matched Indicator
            render_log_counter(ui, app);

            ui.add_space(6.0);

            // 4. Ở giữa: Search Box Command Palette Style tự động co dãn theo khoảng trống còn lại
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let button_extras = if !app.query.is_empty() { 52.0 } else { 28.0 };
                let available_w = ui.available_width();
                let search_box_width = (available_w - button_extras - 6.0).max(40.0);

                let search_id = Id::new("search_query_input");
                let search_response = ui.add(
                    egui::TextEdit::singleline(&mut app.query)
                        .id(search_id)
                        .hint_text(
                            egui::RichText::new("🔍 Filter query (e.g. level:error, status:500, time:now..10m)...")
                                .color(theme::TEXT_PLACEHOLDER),
                        )
                        .desired_width(search_box_width)
                        .font(egui::TextStyle::Monospace)
                        .margin(egui::Margin::symmetric(8.0, 4.0)),
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
                                .size(10.5)
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
                let history_btn = egui::Button::new(egui::RichText::new("⏱").size(11.0).color(
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
            });
        });
    });
}

/// Nút điều khiển cửa sổ vector chuẩn Windows (Ẩn / Thu nhỏ, Phóng to / Khôi phục, Đóng)
fn render_window_controls(ui: &mut egui::Ui) {
    let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    let btn_size = egui::vec2(32.0, 22.0);

    // 1. Nút Đóng (✕)
    let (close_rect, close_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if close_resp.hovered() {
        ui.painter().rect_filled(
            close_rect,
            Rounding::same(3.0),
            egui::Color32::from_rgb(0xe8, 0x11, 0x23),
        );
    }
    let close_color = if close_resp.hovered() {
        egui::Color32::WHITE
    } else {
        theme::TEXT_MUTED
    };
    let center = close_rect.center();
    let d = 4.5;
    ui.painter().line_segment(
        [
            egui::pos2(center.x - d, center.y - d),
            egui::pos2(center.x + d, center.y + d),
        ],
        Stroke::new(1.1, close_color),
    );
    ui.painter().line_segment(
        [
            egui::pos2(center.x + d, center.y - d),
            egui::pos2(center.x - d, center.y + d),
        ],
        Stroke::new(1.1, close_color),
    );
    if close_resp.clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }

    // 2. Nút Phóng to / Khôi phục (🗖 / 🗗)
    let (max_rect, max_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if max_resp.hovered() {
        ui.painter()
            .rect_filled(max_rect, Rounding::same(3.0), theme::BG_SURFACE1);
    }
    let max_color = if max_resp.hovered() {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    let center = max_rect.center();

    if is_maximized {
        // Biểu tượng Restore (2 ô vuông lồng nhau)
        // Ô vuông phía sau (chỉ vẽ cạnh trên và cạnh phải)
        let s = 4.0;
        let p_top_left = egui::pos2(center.x - 2.0, center.y - s);
        let p_top_right = egui::pos2(center.x + s, center.y - s);
        let p_bottom_right = egui::pos2(center.x + s, center.y + 2.0);
        ui.painter()
            .line_segment([p_top_left, p_top_right], Stroke::new(1.0, max_color));
        ui.painter()
            .line_segment([p_top_right, p_bottom_right], Stroke::new(1.0, max_color));

        // Ô vuông phía trước
        let front_rect = egui::Rect::from_min_max(
            egui::pos2(center.x - s, center.y - 2.0),
            egui::pos2(center.x + 2.0, center.y + s),
        );
        ui.painter()
            .rect_stroke(front_rect, Rounding::ZERO, Stroke::new(1.0, max_color));
    } else {
        // Biểu tượng Maximize (1 ô vuông đơn)
        let s = 4.5;
        let square_rect = egui::Rect::from_min_max(
            egui::pos2(center.x - s, center.y - s),
            egui::pos2(center.x + s, center.y + s),
        );
        ui.painter()
            .rect_stroke(square_rect, Rounding::ZERO, Stroke::new(1.0, max_color));
    }

    if max_resp.clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
    }

    // 3. Nút Thu nhỏ (—)
    let (min_rect, min_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if min_resp.hovered() {
        ui.painter()
            .rect_filled(min_rect, Rounding::same(3.0), theme::BG_SURFACE1);
    }
    let min_color = if min_resp.hovered() {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_MUTED
    };
    let center = min_rect.center();
    let d = 5.0;
    ui.painter().line_segment(
        [
            egui::pos2(center.x - d, center.y + 3.5),
            egui::pos2(center.x + d, center.y + 3.5),
        ],
        Stroke::new(1.1, min_color),
    );
    if min_resp.clicked() {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

/// Hiển thị bộ đếm số lượng log theo từng trạng thái chuẩn hóa trong detail task.md:
/// - main (chưa lọc): Live -> số log hiện tại     | Pause -> số log mới đến / số log tại pause
/// - main (đã lọc):   Live -> số log khớp         | Pause -> số log khớp mới đến / số log khớp tại pause
/// - Raw:             Live -> số log hiện tại     | Pause -> số log mới đến / số log tại pause
pub fn render_log_counter(ui: &mut egui::Ui, app: &UwuGuiApp) {
    let is_filtering = !app.query.trim().is_empty();

    let (count_text, count_color, count_tooltip) = match app.active_tab {
        crate::app::ActiveTab::Unfiltered => {
            let total_now = app.engine.total_processed() as usize;

            if app.unfiltered_state.is_live {
                // Live: số log hiện tại
                let text = theme::format_number(total_now);
                let tooltip = format!(
                    "Raw Stream (Live)\n• Total Ingested / Seen: {}\n• In-Memory Buffer: {} / {}\n• Buffer Limit: 500",
                    theme::format_number(total_now),
                    theme::format_number(app.engine.total_logs()),
                    theme::format_number(app.engine.max_capacity()),
                );
                (text, theme::TEXT_MUTED, tooltip)
            } else {
                // Pause: số log mới đến / số log tại pause
                let seen_at_pause = app.unfiltered_state.snapshot_processed_count as usize;
                let new_incoming = total_now.saturating_sub(seen_at_pause);
                let text = format_fraction(new_incoming, seen_at_pause);
                let tooltip = format!(
                    "Raw Stream (Paused / Frozen Snapshot)\n• New Logs Since Pause: {}\n• Total Logs at Pause: {}\n• Total Ingested: {}",
                    theme::format_number(new_incoming),
                    theme::format_number(seen_at_pause),
                    theme::format_number(total_now),
                );
                (text, theme::TEXT_MUTED, tooltip)
            }
        }
        crate::app::ActiveTab::Filtered => {
            if is_filtering {
                if app.is_auto_scroll {
                    // Live: số log khớp
                    let text = if app.total_matched > app.cached_logs.len() {
                        format!(
                            "{}/{}",
                            theme::format_number(app.cached_logs.len()),
                            theme::format_number(app.total_matched)
                        )
                    } else {
                        theme::format_number(app.total_matched)
                    };
                    let tooltip = format!(
                        "Filter Query: \"{}\" (Live)\n• Total Matched: {}\n• Displayed: {} (Limit: {})",
                        app.query.trim(),
                        theme::format_number(app.total_matched),
                        theme::format_number(app.cached_logs.len()),
                        theme::format_number(app.display_limit),
                    );
                    (text, theme::TEXT_KEY, tooltip)
                } else {
                    // Pause: số log khớp mới đến / số log khớp tại pause
                    let seen_matched_at_pause = app.filtered_seen_at_pause;
                    let new_matched = app.paused_new_matched_count;
                    let text = format_fraction(new_matched, seen_matched_at_pause);
                    let tooltip = format!(
                        "Filter Query: \"{}\" (Paused)\n• New Matched Logs Since Pause: {}\n• Matched at Pause: {}\n• Displayed: {}",
                        app.query.trim(),
                        theme::format_number(new_matched),
                        theme::format_number(seen_matched_at_pause),
                        theme::format_number(app.cached_logs.len()),
                    );
                    (text, theme::TEXT_KEY, tooltip)
                }
            } else {
                let total_now = app.engine.total_processed() as usize;

                if app.is_auto_scroll {
                    // Live: số log hiện tại
                    let text = theme::format_number(total_now);
                    let tooltip = format!(
                        "Main Stream (Live)\n• Total Ingested: {}\n• In-Memory Buffer: {} / {}\n• Displayed: {}",
                        theme::format_number(total_now),
                        theme::format_number(app.engine.total_logs()),
                        theme::format_number(app.engine.max_capacity()),
                        theme::format_number(app.cached_logs.len()),
                    );
                    (text, theme::TEXT_MUTED, tooltip)
                } else {
                    // Pause: số log mới đến / số log tại pause
                    let seen_at_pause = app.global_seen_at_pause as usize;
                    let new_incoming = total_now.saturating_sub(seen_at_pause);
                    let text = format_fraction(new_incoming, seen_at_pause);
                    let tooltip = format!(
                        "Main Stream (Paused)\n• New Logs Since Pause: {}\n• Total Logs at Pause: {}\n• Total Ingested: {}",
                        theme::format_number(new_incoming),
                        theme::format_number(seen_at_pause),
                        theme::format_number(total_now),
                    );
                    (text, theme::TEXT_MUTED, tooltip)
                }
            }
        }
    };

    ui.add(egui::Label::new(
        egui::RichText::new(count_text)
            .font(egui::FontId::monospace(11.5))
            .color(count_color),
    ))
    .on_hover_text(count_tooltip);
}

#[inline]
fn format_fraction(numerator: usize, denominator: usize) -> String {
    format!(
        "{}/{}",
        theme::format_number(numerator),
        theme::format_number(denominator)
    )
}
