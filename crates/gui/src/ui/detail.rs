use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Rounding, Stroke};
use uwu_schema::LogLevel;

pub fn render_detail(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("🔍 Inspector")
                .strong()
                .size(13.5)
                .color(theme::TEXT_KEY),
        );

        ui.label(
            egui::RichText::new("⏸ Frozen")
                .size(11.0)
                .color(theme::COLOR_WARN),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close_btn = egui::Button::new(
                egui::RichText::new("✖ Close")
                    .size(11.5)
                    .color(theme::TEXT_PRIMARY),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            if ui
                .add(close_btn)
                .on_hover_text("Close Inspector [Esc]")
                .clicked()
            {
                app.selected_log = None;
            }

            ui.add_space(4.0);

            let next_btn = egui::Button::new(
                egui::RichText::new("⏭")
                    .size(11.0)
                    .color(theme::TEXT_PRIMARY),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            if ui.add(next_btn).on_hover_text("Next Log [Down]").clicked() {
                app.select_next_log();
            }

            let prev_btn = egui::Button::new(
                egui::RichText::new("⏮")
                    .size(11.0)
                    .color(theme::TEXT_PRIMARY),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            if ui
                .add(prev_btn)
                .on_hover_text("Previous Log [Up]")
                .clicked()
            {
                app.select_prev_log();
            }
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);

    if let Some(event) = &app.selected_log {
        let (log_color, is_highlighted) = match event.level {
            LogLevel::Error | LogLevel::Fatal => (theme::COLOR_ERROR, true),
            LogLevel::Warn => (theme::COLOR_WARN, true),
            LogLevel::Info => (theme::COLOR_INFO, true),
            _ => (theme::TEXT_MUTED, false),
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Metadata Card
            render_card(ui, "Metadata", |ui| {
                egui::Grid::new("log_meta_grid")
                    .num_columns(2)
                    .spacing([14.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("ID:")
                                .size(11.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(event.id.to_string())
                                .size(11.5)
                                .monospace()
                                .color(theme::TEXT_PRIMARY),
                        );
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Timestamp:")
                                .size(11.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(event.timestamp.to_rfc3339())
                                .size(11.5)
                                .monospace()
                                .color(theme::TEXT_PRIMARY),
                        );
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Level:")
                                .size(11.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(event.level.to_string())
                                .size(11.5)
                                .color(log_color)
                                .strong(),
                        );
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Source:")
                                .size(11.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(&event.source_id)
                                .size(11.5)
                                .color(theme::TEXT_PRIMARY),
                        );
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            // Message Card (Nền đồng bộ hoàn toàn với Metadata)
            render_card(ui, "Message", |ui| {
                let mut msg = event.message.clone();
                let msg_color = if is_highlighted {
                    log_color
                } else {
                    theme::TEXT_PRIMARY
                };
                ui.add(
                    egui::TextEdit::multiline(&mut msg)
                        .font(egui::TextStyle::Monospace)
                        .text_color(msg_color)
                        .desired_width(f32::INFINITY)
                        .frame(false)
                        .desired_rows(3),
                );
            });

            ui.add_space(8.0);

            // Parsed JSON / K-V Fields Card (Nền đồng bộ)
            if !event.fields.is_empty() {
                render_card(ui, "Parsed Fields", |ui| {
                    egui::Grid::new("log_fields_grid")
                        .num_columns(2)
                        .spacing([14.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (key, val) in &event.fields {
                                ui.label(
                                    egui::RichText::new(key)
                                        .size(11.5)
                                        .monospace()
                                        .color(theme::TEXT_KEY)
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(val.to_string())
                                        .size(11.5)
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(8.0);
            }

            // Raw Payload Card with Copy button (Nền đồng bộ)
            render_card(ui, "Raw Payload", |ui| {
                ui.horizontal(|ui| {
                    let copy_btn = egui::Button::new(
                        egui::RichText::new("📋 Copy Raw")
                            .size(11.0)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(theme::BG_SURFACE0)
                    .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                    .rounding(Rounding::same(4.0));

                    if ui
                        .add(copy_btn)
                        .on_hover_text("Copy raw payload to clipboard")
                        .clicked()
                    {
                        ui.ctx().output_mut(|o| o.copied_text = event.raw.clone());
                    }
                });
                ui.add_space(4.0);

                let mut raw = event.raw.clone();
                ui.add(
                    egui::TextEdit::multiline(&mut raw)
                        .font(egui::TextStyle::Monospace)
                        .text_color(theme::TEXT_PRIMARY)
                        .desired_width(f32::INFINITY)
                        .frame(false)
                        .desired_rows(4),
                );
            });
        });
    }
}

fn render_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let frame = egui::Frame::default()
        .fill(theme::BG_BASE)
        .rounding(Rounding::same(4.0))
        .inner_margin(egui::Margin::same(10.0))
        .stroke(Stroke::new(1.0, theme::BG_SURFACE0));

    frame
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_KEY),
            );
            ui.add_space(4.0);
            add_contents(ui);
        })
        .response
}
