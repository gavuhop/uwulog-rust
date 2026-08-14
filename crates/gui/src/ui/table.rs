use crate::app::UwuGuiApp;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use uwu_schema::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
    let row_count = app.cached_logs.len();

    // Phát hiện cuộn chuột LÊN → tắt auto-scroll
    let scroll_delta_y = ui.input(|i| i.raw_scroll_delta.y);
    if scroll_delta_y > 0.0 {
        app.is_auto_scroll = false;
    }

    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::initial(180.0).at_least(140.0)) // Timestamp
        .column(Column::initial(70.0).at_least(60.0)) // Level
        .column(Column::initial(120.0).at_least(80.0)) // Source
        .column(Column::remainder()); // Message

    // CHỈ gọi scroll_to_row khi CÓ DATA MỚI (row_count tăng), KHÔNG gọi mỗi frame.
    // Đây là khác biệt quan trọng: trước đây gọi mỗi frame → egui reset scroll offset
    // liên tục → giật. Bây giờ chỉ gọi 1 lần khi data mới đến → mượt mà.
    let has_new_data = row_count > app.prev_table_row_count;
    if app.is_auto_scroll && has_new_data && row_count > 0 {
        builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
    }
    app.prev_table_row_count = row_count;

    let mut last_row_visible = false;

    builder
        .header(22.0, |mut header| {
            header.col(|ui| {
                ui.strong("TIMESTAMP");
            });
            header.col(|ui| {
                ui.strong("LEVEL");
            });
            header.col(|ui| {
                ui.strong("SOURCE");
            });
            header.col(|ui| {
                ui.strong("MESSAGE");
            });
        })
        .body(|body| {
            body.rows(text_height + 6.0, row_count, |mut row| {
                let row_index = row.index();

                // Virtualized table chỉ gọi closure cho các row ĐANG HIỂN THỊ trên viewport.
                // Nếu row cuối cùng được render → nó đang visible trên màn hình.
                if row_count > 0 && row_index == row_count - 1 {
                    last_row_visible = true;
                }

                if let Some(event) = app.cached_logs.get(row_index) {
                    let is_selected = app.selected_log.as_ref().is_some_and(|s| s.id == event.id);

                    if is_selected {
                        row.set_selected(true);
                    }

                    row.col(|ui| {
                        let label = ui.selectable_label(
                            is_selected,
                            event.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
                        );
                        if label.clicked() {
                            app.selected_log = Some(event.clone());
                        }
                    });

                    row.col(|ui| {
                        let (color, text) = match event.level {
                            LogLevel::Error | LogLevel::Fatal => {
                                (egui::Color32::RED, event.level.to_string())
                            }
                            LogLevel::Warn => (egui::Color32::YELLOW, event.level.to_string()),
                            LogLevel::Info => (egui::Color32::LIGHT_GREEN, event.level.to_string()),
                            LogLevel::Debug => (egui::Color32::LIGHT_BLUE, event.level.to_string()),
                            LogLevel::Trace => (egui::Color32::GRAY, event.level.to_string()),
                            LogLevel::Unknown => {
                                (egui::Color32::LIGHT_GRAY, event.level.to_string())
                            }
                        };
                        let label = ui.colored_label(color, text);
                        if label.clicked() {
                            app.selected_log = Some(event.clone());
                        }
                    });

                    row.col(|ui| {
                        let label = ui.selectable_label(is_selected, &event.source_id);
                        if label.clicked() {
                            app.selected_log = Some(event.clone());
                        }
                    });

                    row.col(|ui| {
                        let label = ui.selectable_label(is_selected, &event.message);
                        if label.clicked() {
                            app.selected_log = Some(event.clone());
                        }
                    });
                }
            });
        });

    // Bật lại auto-scroll CHỈ KHI: user chủ động cuộn XUỐNG (scroll_delta_y < 0) VÀ đã chạm đáy
    if last_row_visible && scroll_delta_y < 0.0 {
        app.is_auto_scroll = true;
    }
}
