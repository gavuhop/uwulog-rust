use crate::app::{SourceType, UwuGuiApp};
use eframe::egui;

pub fn render_launch_modal(ctx: &egui::Context, app: &mut UwuGuiApp) {
    if !app.show_launch_modal {
        return;
    }

    egui::Window::new("⚙️ Launch & Source Parameters")
        .collapsible(false)
        .resizable(true)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.heading("Engine & Source Configuration");
            ui.separator();
            ui.add_space(4.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("System Engine Performance").strong());
                ui.separator();

                egui::Grid::new("engine_params_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("RingBuffer Capacity (-cap):");
                        ui.add(
                            egui::DragValue::new(&mut app.source_config.capacity)
                                .range(1_000..=500_000)
                                .speed(1000),
                        );
                        ui.end_row();

                        ui.label("Display Limit (-n):");
                        ui.add(
                            egui::DragValue::new(&mut app.source_config.display_limit)
                                .range(100..=50_000)
                                .speed(500),
                        );
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Log Source Selection").strong());
                ui.separator();

                ui.radio_value(
                    &mut app.source_config.source_type,
                    SourceType::Process,
                    "🚀 Command / Process Output",
                );
                if app.source_config.source_type == SourceType::Process {
                    ui.horizontal(|ui| {
                        ui.label("Command:");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.source_config.command_str)
                                .hint_text("e.g. go run gen_logs.go"),
                        );
                    });
                }

                ui.add_space(6.0);

                ui.radio_value(
                    &mut app.source_config.source_type,
                    SourceType::File,
                    "📁 Log File (File Tailer)",
                );
                if app.source_config.source_type == SourceType::File {
                    ui.horizontal(|ui| {
                        ui.label("File Path:");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.source_config.file_path)
                                .desired_width(260.0),
                        );
                        if ui.button("Browse...").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                app.source_config.file_path = path.display().to_string();
                            }
                        }
                    });
                }

                ui.add_space(6.0);

                #[cfg(target_os = "windows")]
                {
                    ui.radio_value(
                        &mut app.source_config.source_type,
                        SourceType::WinEvent,
                        "🪟 Windows Event Log",
                    );
                    if app.source_config.source_type == SourceType::WinEvent {
                        ui.horizontal(|ui| {
                            ui.label("Channel:");
                            egui::ComboBox::from_id_salt("win_channel_combo")
                                .selected_text(&app.source_config.win_channel)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut app.source_config.win_channel,
                                        "System".to_string(),
                                        "System",
                                    );
                                    ui.selectable_value(
                                        &mut app.source_config.win_channel,
                                        "Application".to_string(),
                                        "Application",
                                    );
                                    ui.selectable_value(
                                        &mut app.source_config.win_channel,
                                        "Security".to_string(),
                                        "Security",
                                    );
                                });
                        });
                    }
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new("🚀 Apply & Restart").strong())
                    .clicked()
                {
                    app.restart_current_source();
                    app.show_launch_modal = false;
                }

                if ui.button("Cancel").clicked() {
                    app.show_launch_modal = false;
                }
            });
        });
}
