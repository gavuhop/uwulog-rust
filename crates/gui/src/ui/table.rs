use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use uwu_schema::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
    let row_count = app.cached_logs.len();

    // Phím End: Latch auto-scroll và cuộn ngay xuống dòng mới nhất (khi không focus vào ô nhập text)
    let end_key_pressed =
        ui.input(|i| i.key_pressed(egui::Key::End)) && !ui.memory(|m| m.focused().is_some());
    if end_key_pressed {
        app.is_auto_scroll = true;
    }

    // Phát hiện cuộn chuột LÊN → tắt auto-scroll
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        app.is_auto_scroll = false;
    }

    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::initial(160.0).at_least(140.0)) // Timestamp
        .column(Column::initial(70.0).at_least(60.0)) // Level
        .column(Column::initial(120.0).at_least(80.0)) // Source
        .column(Column::remainder()); // Message

    // Cuộn xuống dòng cuối khi:
    // 1. Vừa bấm phím End HOẶC
    // 2. Auto-scroll đang bật VÀ có log mới đến (row_count tăng)
    let has_new_data = row_count > app.prev_table_row_count;
    if (end_key_pressed || (app.is_auto_scroll && has_new_data)) && row_count > 0 {
        builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
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
                    egui::RichText::new("SOURCE")
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

                    // Độ tương phản mềm mại: Error -> Đỏ san hô, Warn -> Vàng hổ phách, Info -> Xám ấm
                    let (row_color, is_highlighted) = match event.level {
                        LogLevel::Error | LogLevel::Fatal => (theme::COLOR_ERROR, true),
                        LogLevel::Warn => (theme::COLOR_WARN, true),
                        _ => (theme::TEXT_PRIMARY, false),
                    };

                    let timestamp_color = if is_highlighted {
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

                    // Source Column (Terminal monospace)
                    row.col(|ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(&event.source_id)
                                    .font(egui::FontId::monospace(11.5))
                                    .color(row_color),
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
    }

    // Bật lại auto-scroll CHỈ KHI: user chủ động cuộn XUỐNG (scroll_delta_y < 0) VÀ đã chạm đáy
    if last_row_visible && scroll_delta_y < 0.0 {
        app.is_auto_scroll = true;
    }
}
