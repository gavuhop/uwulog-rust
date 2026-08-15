use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use uwu_schema::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
    let row_count = app.cached_logs.len();

    let not_focusing_text = !ui.memory(|m| m.focused().is_some());

    // Phím End: Latch auto-scroll và cuộn ngay xuống dòng mới nhất (khi không focus vào ô nhập text)
    let end_key_pressed = ui.input(|i| i.key_pressed(egui::Key::End)) && not_focusing_text;
    if end_key_pressed {
        app.latch();
    }

    // Các phím điều hướng cuộn lên (PageUp, Home, ArrowUp) khi không gõ text -> tự động Unlatch
    if not_focusing_text {
        let scroll_up_keys = ui.input(|i| {
            i.key_pressed(egui::Key::PageUp)
                || i.key_pressed(egui::Key::Home)
                || i.key_pressed(egui::Key::ArrowUp)
        });
        if scroll_up_keys {
            app.unlatch();
        }
    }

    // Phát hiện cuộn chuột LÊN → tắt auto-scroll (Unlatch)
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        app.unlatch();
    }

    ui.visuals_mut().selection.bg_fill = theme::BG_ROW_SELECTED;
    ui.visuals_mut().selection.stroke = egui::Stroke::NONE;

    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::initial(160.0).at_least(0.0).clip(true)) // Timestamp
        .column(Column::initial(70.0).at_least(0.0).clip(true)) // Level
        .column(Column::remainder()); // Message

    // Cuộn xuống dòng cuối khi:
    // 1. Có request cuộn ngay (bấm nút Latch hoặc phím End) HOẶC
    // 2. Auto-scroll đang bật VÀ có log mới đến (row_count tăng)
    let has_new_data = row_count > app.prev_table_row_count;
    let force_scroll = app.request_scroll_to_bottom;
    if (force_scroll || (app.is_auto_scroll && has_new_data)) && row_count > 0 {
        builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
        app.request_scroll_to_bottom = false;
    }
    app.prev_table_row_count = row_count;

    let mut last_row_visible = false;
    let mut newly_selected_event = None;

    builder
        .header(26.0, |mut header| {
            header.col(|ui| {
                ui.label(
                    egui::RichText::new("TIMESTAMP")
                        .font(egui::FontId::monospace(11.0))
                        .strong()
                        .color(theme::TEXT_MUTED),
                );
            });
            header.col(|ui| {
                ui.label(
                    egui::RichText::new("LEVEL")
                        .font(egui::FontId::monospace(11.0))
                        .strong()
                        .color(theme::TEXT_MUTED),
                );
            });
            header.col(|ui| {
                ui.label(
                    egui::RichText::new("MESSAGE")
                        .font(egui::FontId::monospace(11.0))
                        .strong()
                        .color(theme::TEXT_MUTED),
                );
            });
        })
        .body(|body| {
            body.rows(text_height + 8.0, row_count, |mut row| {
                let row_index = row.index();

                if row_count > 0 && row_index == row_count - 1 {
                    last_row_visible = true;
                }

                if let Some(event) = app.cached_logs.get(row_index) {
                    let is_selected = app.selected_log.as_ref().is_some_and(|s| s.id == event.id);
                    if is_selected {
                        row.set_selected(true);
                    }

                    // Độ tương phản mềm mại: Error -> Đỏ san hô, Warn -> Vàng hổ phách, Info -> Xanh lá pastel, Debug/Trace -> Xám dịu
                    let row_color = match event.level {
                        LogLevel::Error | LogLevel::Fatal => theme::COLOR_ERROR,
                        LogLevel::Warn => theme::COLOR_WARN,
                        LogLevel::Info => theme::COLOR_INFO,
                        _ => theme::TEXT_MUTED,
                    };

                    let timestamp_color = if matches!(
                        event.level,
                        LogLevel::Error | LogLevel::Fatal | LogLevel::Warn
                    ) {
                        row_color
                    } else {
                        theme::TEXT_MUTED
                    };

                    // Timestamp Column (Terminal monospace)
                    row.col(|ui| {
                        let time_str = event.timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(time_str)
                                    .font(egui::FontId::monospace(11.5))
                                    .color(timestamp_color),
                            )
                            .truncate(),
                        );
                        if resp.clicked() {
                            newly_selected_event = Some(event.clone());
                        }
                    });

                    // Level Column (Terminal monospace)
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(event.level.to_string())
                                    .font(egui::FontId::monospace(11.5))
                                    .color(row_color)
                                    .strong(),
                            )
                            .truncate(),
                        );
                        if resp.clicked() {
                            newly_selected_event = Some(event.clone());
                        }
                    });

                    // Message Column (Terminal monospace, single line truncate)
                    row.col(|ui| {
                        let clean_msg = event.message.replace('\n', " ↵ ");
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(clean_msg)
                                    .font(egui::FontId::monospace(12.0))
                                    .color(row_color),
                            )
                            .truncate(),
                        );
                        if resp.clicked() {
                            newly_selected_event = Some(event.clone());
                        }
                    });

                    // Nhận click bất kỳ vị trí nào trên hàng
                    if row.response().clicked() {
                        newly_selected_event = Some(event.clone());
                    }
                }
            });
        });

    if let Some(event) = newly_selected_event {
        app.selected_log = Some(event);
        // Khi user chọn xem một dòng log, unlatch để màn hình đứng yên giúp đọc chi tiết
        app.unlatch();
    }

    // Bật lại auto-scroll CHỈ KHI: user chủ động cuộn XUỐNG (scroll_delta_y < 0) VÀ đã chạm đáy
    if last_row_visible && scroll_delta_y < 0.0 {
        app.is_auto_scroll = true;
    }
}
