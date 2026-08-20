use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use uwu_core::LogLevel;

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

    let visible_cols: Vec<crate::ui::columns_modal::ColumnItem> = app
        .column_state
        .columns
        .iter()
        .filter(|c| c.visible)
        .cloned()
        .collect();

    let mut builder = TableBuilder::new(ui).striped(true).resizable(true);

    let remainder_col_name = if visible_cols.iter().any(|c| c.name == "message") {
        "message".to_string()
    } else if let Some(last) = visible_cols.last() {
        last.name.clone()
    } else {
        String::new()
    };

    for col in &visible_cols {
        if col.name == remainder_col_name {
            builder = builder.column(Column::remainder());
        } else {
            builder = builder.column(Column::initial(col.width.max(40.0)).at_least(0.0).clip(true));
        }
    }

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
            for col in &visible_cols {
                header.col(|ui| {
                    ui.label(
                        egui::RichText::new(&col.name)
                            .font(egui::FontId::monospace(11.0))
                            .strong()
                            .color(theme::TEXT_MUTED),
                    );
                });
            }
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

                    for col in &visible_cols {
                        row.col(|ui| {
                            let (cell_text, is_bold) = match col.name.as_str() {
                                "timestamp" => (event.timestamp.clone(), false),
                                "level" => (event.level.to_string(), true),
                                "message" => (event.message.replace('\n', " ↵ "), false),
                                custom_key => {
                                    let val_str = if let Some(val) = event.fields.get(custom_key) {
                                        match val {
                                            serde_json::Value::String(s) => s.clone(),
                                            _ => val.to_string(),
                                        }
                                    } else {
                                        "-".to_string()
                                    };
                                    (val_str, false)
                                }
                            };

                            let mut rich = egui::RichText::new(cell_text)
                                .font(egui::FontId::monospace(11.5))
                                .color(row_color);
                            if is_bold {
                                rich = rich.strong();
                            }

                            let resp = ui.add(
                                egui::Label::new(rich).truncate(),
                            );
                            if resp.clicked() {
                                newly_selected_event = Some(event.clone());
                            }
                        });
                    }

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
