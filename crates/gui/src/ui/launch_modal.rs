use crate::app::{SourceType, UwuGuiApp};
use crate::ui::theme;
use eframe::egui::{self, Rounding, Stroke};

pub fn render_launch_modal(ctx: &egui::Context, app: &mut UwuGuiApp) {
    if !app.show_launch_modal {
        return;
    }

    egui::Window::new("⚙️ Launch & Source Parameters")
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .inner_margin(egui::Margin::same(14.0))
                .rounding(Rounding::same(6.0)),
        )
        .collapsible(false)
        .resizable(true)
        .default_width(500.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("Engine & Source Configuration")
                    .size(14.0)
                    .strong()
                    .color(theme::TEXT_KEY),
            );
            ui.separator();
            ui.add_space(6.0);

            // Engine Performance Card
            render_modal_card(ui, "System Engine Performance", |ui| {
                egui::Grid::new("engine_params_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("RingBuffer Capacity (-cap):")
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add(
                            egui::DragValue::new(&mut app.source_config.capacity)
                                .range(1_000..=1_000_000)
                                .speed(5000),
                        );
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Display Limit (-n):").color(theme::TEXT_MUTED),
                        );
                        ui.add(
                            egui::DragValue::new(&mut app.source_config.display_limit)
                                .range(100..=50_000)
                                .speed(500),
                        );
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            // Log Source Selection Card
            render_modal_card(ui, "Log Source Selection", |ui| {
                ui.radio_value(
                    &mut app.source_config.source_type,
                    SourceType::Process,
                    egui::RichText::new("🚀 Command / Process Output").color(theme::TEXT_PRIMARY),
                );
                if app.source_config.source_type == SourceType::Process {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Command:").color(theme::TEXT_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut app.source_config.command_str)
                                .hint_text("e.g. go run gen_logs.go")
                                .font(egui::TextStyle::Monospace)
                                .desired_width(320.0)
                                .margin(egui::Margin::symmetric(8.0, 4.0)),
                        );
                    });
                }

                ui.add_space(6.0);

                ui.radio_value(
                    &mut app.source_config.source_type,
                    SourceType::File,
                    egui::RichText::new("📁 Log File (File Tailer)").color(theme::TEXT_PRIMARY),
                );
                if app.source_config.source_type == SourceType::File {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("File Path:").color(theme::TEXT_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut app.source_config.file_path)
                                .desired_width(260.0)
                                .margin(egui::Margin::symmetric(8.0, 4.0)),
                        );

                        let browse_btn = egui::Button::new(
                            egui::RichText::new("Browse...").color(theme::TEXT_PRIMARY),
                        )
                        .fill(theme::BG_SURFACE0)
                        .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                        .rounding(Rounding::same(4.0));

                        if ui.add(browse_btn).clicked() {
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
                        egui::RichText::new("🪟 Windows Event Log").color(theme::TEXT_PRIMARY),
                    );
                    if app.source_config.source_type == SourceType::WinEvent {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Channel:").color(theme::TEXT_MUTED));
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
            ui.add_space(6.0);

            // Action Buttons
            ui.horizontal(|ui| {
                let apply_btn = egui::Button::new(
                    egui::RichText::new("🚀 Apply & Restart")
                        .strong()
                        .color(theme::TEXT_PRIMARY),
                )
                .fill(theme::BTN_RESTART_BG)
                .stroke(Stroke::new(1.0, theme::BTN_RESTART_BORDER))
                .rounding(Rounding::same(4.0));

                if ui.add(apply_btn).clicked() {
                    app.restart_current_source();
                    app.show_launch_modal = false;
                }

                let cancel_btn =
                    egui::Button::new(egui::RichText::new("Cancel").color(theme::TEXT_PRIMARY))
                        .fill(theme::BG_SURFACE0)
                        .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                        .rounding(Rounding::same(4.0));

                if ui.add(cancel_btn).clicked() {
                    app.show_launch_modal = false;
                }
            });
        });
}

fn render_modal_card<R>(
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
