use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use uwu_schema::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
    let row_count = app.cached_logs.len();

    let not_in_input = !ui.memory(|m| m.focused().is_some());

    // Phím Space hoặc 'p': Bật/Tắt Live Auto-scroll & Đóng băng quan sát
    if not_in_input
        && (ui.input(|i| i.key_pressed(egui::Key::Space))
            || ui.input(|i| i.key_pressed(egui::Key::P)))
    {
        app.toggle_live();
    }

    // Phím End: Latch auto-scroll và cuộn ngay xuống dòng mới nhất
    let end_key_pressed = ui.input(|i| i.key_pressed(egui::Key::End)) && not_in_input;
    if end_key_pressed {
        app.resume_live();
    }

    // Phím Home: Đóng băng và cuộn lên đầu
    let home_key_pressed = ui.input(|i| i.key_pressed(egui::Key::Home)) && not_in_input;
    if home_key_pressed {
        app.pause_live();
    }

    // Phím Mũi tên Lên / Xuống khi đang xem Log Inspector
    if not_in_input && app.selected_log.is_some() {
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            app.select_prev_log();
        } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            app.select_next_log();
        }
    }

    // Phát hiện cuộn chuột LÊN → đóng băng auto-scroll
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        app.pause_live();
    }

    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::initial(160.0).at_least(0.0).clip(true)) // Timestamp
        .column(Column::initial(70.0).at_least(0.0).clip(true)) // Level
        .column(Column::remainder()); // Message

    if (end_key_pressed || app.is_auto_scroll) && row_count > 0 {
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
        app.pause_live(); // Đóng băng live streaming khi chọn 1 log để đọc chi tiết
    }

    // Bật lại live auto-scroll khi user cuộn XUỐNG chạm đáy và không đang mở inspector
    if last_row_visible && scroll_delta_y < 0.0 && !app.is_auto_scroll && app.selected_log.is_none()
    {
        app.resume_live();
    }
}
