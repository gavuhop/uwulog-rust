use crate::app::UwuGuiApp;
use eframe::egui;

pub fn render_detail(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("🔍 Log Inspector");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("✖ Close").clicked() {
                app.selected_log = None;
            }
        });
    });

    ui.separator();

    if let Some(event) = &app.selected_log {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Metadata").strong());
                ui.separator();

                egui::Grid::new("log_meta_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("ID:").strong());
                        ui.label(event.id.to_string());
                        ui.end_row();

                        ui.label(egui::RichText::new("Timestamp:").strong());
                        ui.label(event.timestamp.to_rfc3339());
                        ui.end_row();

                        ui.label(egui::RichText::new("Level:").strong());
                        ui.label(event.level.to_string());
                        ui.end_row();

                        ui.label(egui::RichText::new("Source:").strong());
                        ui.label(&event.source_id);
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Message").strong());
                ui.separator();
                ui.add(
                    egui::TextEdit::multiline(&mut event.message.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3),
                );
            });

            ui.add_space(8.0);

            if !event.fields.is_empty() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Parsed Fields").strong());
                    ui.separator();

                    egui::Grid::new("log_fields_grid")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (key, val) in &event.fields {
                                ui.label(egui::RichText::new(key).monospace());
                                ui.label(val.to_string());
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(8.0);
            }

            ui.group(|ui| {
                ui.label(egui::RichText::new("Raw Event Payload").strong());
                ui.separator();
                ui.add(
                    egui::TextEdit::multiline(&mut event.raw.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4),
                );
            });
        });
    }
}
