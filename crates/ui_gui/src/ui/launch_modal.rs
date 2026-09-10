use crate::app::{AppAction, SourceType, UwuGuiApp};
use crate::ui::card::render_card;
use crate::ui::theme;
use eframe::egui::{self, Rounding, Stroke};

pub fn render_launch_modal(ctx: &egui::Context, app: &mut UwuGuiApp) {
    if !app.show_launch_modal {
        return;
    }

    if app.launch_modal_draft.is_none() {
        app.launch_modal_draft = Some(app.session.source_config.clone());
    }

    let mut action_to_dispatch: Option<AppAction> = None;

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
                egui::RichText::new("Settings & Launch Parameters")
                    .size(14.0)
                    .strong()
                    .color(theme::TEXT_KEY),
            );
            ui.separator();
            ui.add_space(6.0);

            let draft = app.launch_modal_draft.as_mut().unwrap();

            // Engine Performance Card
            render_card(ui, "System Engine Performance", |ui| {
                egui::Grid::new("engine_params_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new("RingBuffer Capacity (-cap):")
                                    .color(theme::TEXT_MUTED),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        ui.add(
                            egui::DragValue::new(&mut draft.capacity)
                                .range(1_000..=1_000_000)
                                .speed(5000),
                        );
                        ui.end_row();

                        ui.add(
                            egui::Label::new(
                                egui::RichText::new("Display Limit (-n):").color(theme::TEXT_MUTED),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        ui.add(
                            egui::DragValue::new(&mut draft.display_limit)
                                .range(100..=50_000)
                                .speed(500),
                        );
                        ui.end_row();
                    });
            });

            ui.add_space(8.0);

            // Log Source Selection Card
            render_card(ui, "Log Source Selection", |ui| {
                ui.radio_value(
                    &mut draft.source_type,
                    SourceType::Process,
                    egui::RichText::new("🚀 Command").color(theme::TEXT_PRIMARY),
                );
                if draft.source_type == SourceType::Process {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Command:").color(theme::TEXT_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.command_str)
                                .hint_text("e.g. go run gen_logs.go")
                                .font(egui::TextStyle::Monospace)
                                .desired_width(320.0)
                                .margin(egui::Margin::symmetric(8.0, 4.0)),
                        );
                    });
                }

                ui.add_space(6.0);

                ui.radio_value(
                    &mut draft.source_type,
                    SourceType::File,
                    egui::RichText::new("📁 Log File (File Tailer)").color(theme::TEXT_PRIMARY),
                );
                if draft.source_type == SourceType::File {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("File Path:").color(theme::TEXT_MUTED));
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.file_path)
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
                                draft.file_path = path.display().to_string();
                            }
                        }
                    });
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
                    action_to_dispatch = Some(AppAction::ApplyLaunchModal);
                }

                let cancel_btn =
                    egui::Button::new(egui::RichText::new("Cancel").color(theme::TEXT_PRIMARY))
                        .fill(theme::BG_SURFACE0)
                        .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                        .rounding(Rounding::same(4.0));

                if ui.add(cancel_btn).clicked() {
                    action_to_dispatch = Some(AppAction::CloseLaunchModal);
                }
            });
        });

    if let Some(action) = action_to_dispatch {
        app.dispatch_action(action);
    }
}
