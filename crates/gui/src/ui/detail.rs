use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Rounding, Stroke};
use uwu_schema::LogLevel;

pub fn render_detail(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("🔍 Log Inspector")
                .strong()
                .size(14.0)
                .color(theme::TEXT_KEY),
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

            if ui.add(close_btn).clicked() {
                app.selected_log = None;
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

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Metadata Card
                render_card(ui, "Metadata", |ui| {
                    render_meta_field(ui, "ID", &event.id.to_string(), theme::TEXT_PRIMARY, true);
                    ui.add_space(5.0);
                    render_meta_field(ui, "Timestamp", &event.timestamp, theme::TEXT_PRIMARY, true);
                    ui.add_space(5.0);
                    render_meta_field(ui, "Level", &event.level.to_string(), log_color, false);
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
                        for (i, (key, val)) in event.fields.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(6.0);
                            }
                            render_kv_field(ui, key, &val.to_string());
                        }
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

fn render_meta_field(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    val_color: egui::Color32,
    mono: bool,
) {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("{}: ", key),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(11.5),
            color: theme::TEXT_MUTED,
            ..Default::default()
        },
    );
    job.append(
        val,
        0.0,
        egui::TextFormat {
            font_id: if mono {
                egui::FontId::monospace(11.5)
            } else {
                egui::FontId::proportional(11.5)
            },
            color: val_color,
            ..Default::default()
        },
    );

    ui.add(
        egui::Label::new(job)
            .wrap_mode(egui::TextWrapMode::Wrap)
            .selectable(true),
    );
}

fn render_kv_field(ui: &mut egui::Ui, key: &str, val: &str) {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("{}: ", key),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::monospace(11.5),
            color: theme::TEXT_KEY,
            ..Default::default()
        },
    );
    job.append(
        val,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(11.5),
            color: theme::TEXT_PRIMARY,
            ..Default::default()
        },
    );

    ui.add(
        egui::Label::new(job)
            .wrap_mode(egui::TextWrapMode::Wrap)
            .selectable(true),
    );
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
            ui.set_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
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
