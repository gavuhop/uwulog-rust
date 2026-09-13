use crate::actions::AppAction;
use crate::components::render_card;
use crate::theme;
use eframe::egui;
use uwu_core_workspace::{SourceConfig, SourceType};

pub fn render_launch_modal(
    ctx: &egui::Context,
    is_open: bool,
    draft: &mut Option<SourceConfig>,
    current_config: &SourceConfig,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    if draft.is_none() {
        *draft = Some(current_config.clone());
    }

    let mut action_to_dispatch: Option<AppAction> = None;

    let resp = crate::components::ui::ModalContainer::new(
        "launch_modal_window",
        "Settings & Launch Parameters",
    )
    .subtitle("Configure engine buffer, display limits & sources")
    .width(460.0)
    .show(
        ctx,
        |ui| {
            let draft = draft.as_mut().unwrap();

            // Engine Performance Card
            render_card(ui, "System Engine Performance", |ui| {
                egui::Grid::new("engine_params_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new("RingBuffer Capacity (-C / --capacity):")
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

            // Source Selector Card
            render_card(ui, "Data Ingestion Source", |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut draft.source_type,
                        SourceType::Process,
                        "Process Exec (-c)",
                    );
                    ui.selectable_value(&mut draft.source_type, SourceType::File, "File Tail (-f)");
                });

                ui.add_space(6.0);

                if draft.source_type == SourceType::Process {
                    ui.label(
                        egui::RichText::new("Command string (executed via sh -c or cmd /C):")
                            .color(theme::TEXT_MUTED)
                            .size(11.0),
                    );
                    let cmd_edit = egui::TextEdit::singleline(&mut draft.command_str)
                        .hint_text("e.g. go run main.go or ping 127.0.0.1")
                        .font(egui::TextStyle::Monospace)
                        .margin(egui::Margin::symmetric(8.0, 5.0));
                    ui.add_sized([ui.available_width(), 24.0], cmd_edit);

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Working Directory (optional):")
                            .color(theme::TEXT_MUTED)
                            .size(11.0),
                    );
                    ui.horizontal(|ui| {
                        let browse_width = 80.0;
                        let text_width = (ui.available_width() - browse_width - 8.0).max(100.0);

                        let dir_edit = egui::TextEdit::singleline(&mut draft.working_dir)
                            .hint_text("e.g. D:\\projects\\backend")
                            .font(egui::TextStyle::Monospace)
                            .margin(egui::Margin::symmetric(8.0, 5.0));
                        ui.add_sized([text_width, 24.0], dir_edit);

                        let browse_clicked = crate::components::ui::AppButton::new()
                            .label("Browse...")
                            .min_size(egui::vec2(browse_width, 22.0))
                            .show(ui)
                            .clicked();

                        if browse_clicked {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                draft.working_dir = path.display().to_string();
                            }
                        }
                    });
                } else {
                    ui.label(
                        egui::RichText::new("Log file path:")
                            .color(theme::TEXT_MUTED)
                            .size(11.0),
                    );
                    ui.horizontal(|ui| {
                        let browse_width = 80.0;
                        let text_width = (ui.available_width() - browse_width - 8.0).max(100.0);

                        let path_edit = egui::TextEdit::singleline(&mut draft.file_path)
                            .hint_text("e.g. /var/log/app.log or C:\\logs\\app.log")
                            .font(egui::TextStyle::Monospace)
                            .margin(egui::Margin::symmetric(8.0, 5.0));
                        ui.add_sized([text_width, 24.0], path_edit);

                        let browse_clicked = crate::components::ui::AppButton::new()
                            .label("Browse...")
                            .min_size(egui::vec2(browse_width, 22.0))
                            .show(ui)
                            .clicked();

                        if browse_clicked {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                draft.file_path = path.display().to_string();
                            }
                        }
                    });
                }
            });
        },
        Some(|ui: &mut egui::Ui, close_req: &mut bool| {
            if crate::components::ui::AppButton::new()
                .label("Apply & Restart")
                .icon("🚀")
                .variant(crate::components::ui::ButtonVariant::Success)
                .show(ui)
                .clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                action_to_dispatch = Some(AppAction::ApplyLaunchModal);
            }

            ui.add_space(4.0);

            if crate::components::ui::AppButton::new()
                .label("Cancel")
                .show(ui)
                .clicked()
            {
                *close_req = true;
            }
        }),
    );

    if resp.closed {
        action_to_dispatch = Some(AppAction::CloseLaunchModal);
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}
