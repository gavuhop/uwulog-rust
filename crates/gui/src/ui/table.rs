use crate::app::UwuGuiApp;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use uwu_schema::LogLevel;

pub fn render_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
    let row_count = app.cached_logs.len();

    // Nhận diện hướng cuộn chuột: cuộn LÊN (y > 0) -> Tắt Auto-Scroll
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

    // Nếu is_auto_scroll = true -> Tự động cuộn đến hàng cuối cùng (Align::Max)
    if app.is_auto_scroll && row_count > 0 {
        builder = builder.scroll_to_row(row_count - 1, Some(egui::Align::Max));
    }

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
                if row_index == row_count.saturating_sub(1) {
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

    // Khi hàng cuối cùng hiển thị trên màn hình và người dùng không cuộn ngược lên -> Tự động BẬT lại Auto-Scroll
    if last_row_visible && scroll_delta_y <= 0.0 {
        app.is_auto_scroll = true;
    }
}
