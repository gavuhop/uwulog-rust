use crate::app::{AppAction, SourceType, UwuGuiApp, WslSubMode};
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
    let available_distros = app.available_wsl_distros.clone();

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

            // Zed-Style Recent Projects Section
            ui.label(
                egui::RichText::new("Recent Projects")
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(3.0);

            egui::Frame::none()
                .fill(theme::BG_BASE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                .show(ui, |ui| {
                    if app.store.recent_workspaces.is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "No recent projects yet. Configure below and save.",
                            )
                            .italics()
                            .size(11.5)
                            .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(6.0);
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for ws in &app.store.recent_workspaces {
                                    let is_active = ws.name == app.session.name;
                                    let bg_color = if is_active {
                                        theme::BG_SURFACE0
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    let icon = ws.icon();
                                    let label_text = ws.display_label();
                                    let subtitle = ws.target_summary();

                                    egui::Frame::none()
                                        .fill(bg_color)
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(egui::Margin::symmetric(6.0, 3.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(icon).size(13.0));

                                                let name_resp = ui.selectable_label(
                                                    is_active,
                                                    egui::RichText::new(&label_text)
                                                        .size(12.5)
                                                        .strong()
                                                        .color(if is_active {
                                                            theme::TEXT_KEY
                                                        } else {
                                                            theme::TEXT_PRIMARY
                                                        }),
                                                );

                                                if name_resp.clicked() {
                                                    action_to_dispatch =
                                                        Some(AppAction::LoadWorkspace(ws.clone()));
                                                }
                                                if !subtitle.is_empty() {
                                                    name_resp.on_hover_text(format!(
                                                        "Path: {}",
                                                        subtitle
                                                    ));
                                                }

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        // Delete button (✕)
                                                        let del_btn = egui::Button::new(
                                                            egui::RichText::new("✕")
                                                                .size(11.0)
                                                                .color(theme::TEXT_MUTED),
                                                        )
                                                        .fill(egui::Color32::TRANSPARENT)
                                                        .frame(false);

                                                        if ui
                                                            .add(del_btn)
                                                            .on_hover_text(
                                                                "Remove from recent list",
                                                            )
                                                            .clicked()
                                                        {
                                                            action_to_dispatch = Some(
                                                                AppAction::DeleteWorkspace(ws.id),
                                                            );
                                                        }

                                                        // Open button (↗)
                                                        let open_btn = egui::Button::new(
                                                            egui::RichText::new("↗")
                                                                .size(12.0)
                                                                .color(theme::TEXT_PRIMARY),
                                                        )
                                                        .fill(egui::Color32::TRANSPARENT)
                                                        .frame(false);

                                                        if ui
                                                            .add(open_btn)
                                                            .on_hover_text(
                                                                "Open and launch this project",
                                                            )
                                                            .clicked()
                                                        {
                                                            action_to_dispatch =
                                                                Some(AppAction::OpenWorkspace(
                                                                    ws.clone(),
                                                                ));
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                }
                            });
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            let draft = app.launch_modal_draft.as_mut().unwrap();

            // Engine Performance Card
            render_card(ui, "System Engine Performance", |ui| {
                egui::Grid::new("engine_params_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("RingBuffer Capacity (-cap):")
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add(
                            egui::DragValue::new(&mut draft.capacity)
                                .range(1_000..=1_000_000)
                                .speed(5000),
                        );
                        ui.end_row();

                        ui.label(
                            egui::RichText::new("Display Limit (-n):").color(theme::TEXT_MUTED),
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
                    egui::RichText::new("🚀 Command / Process Output").color(theme::TEXT_PRIMARY),
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

                ui.add_space(6.0);

                ui.radio_value(
                    &mut draft.source_type,
                    SourceType::Wsl,
                    egui::RichText::new("🐧 WSL (Windows Subsystem for Linux)")
                        .color(theme::TEXT_PRIMARY),
                );
                if draft.source_type == SourceType::Wsl {
                    egui::Frame::none()
                        .fill(theme::BG_CRUST)
                        .rounding(Rounding::same(4.0))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Distro:").color(theme::TEXT_MUTED));
                                if !available_distros.is_empty() {
                                    egui::ComboBox::from_id_salt("wsl_distro_combo")
                                        .selected_text(if draft.wsl_config.distro.is_empty() {
                                            "Select Distro"
                                        } else {
                                            &draft.wsl_config.distro
                                        })
                                        .show_ui(ui, |ui| {
                                            for d in &available_distros {
                                                ui.selectable_value(
                                                    &mut draft.wsl_config.distro,
                                                    d.clone(),
                                                    d,
                                                );
                                            }
                                        });
                                } else {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut draft.wsl_config.distro)
                                            .hint_text("Ubuntu")
                                            .desired_width(120.0),
                                    );
                                }
                            });

                            ui.add_space(4.0);

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Workdir:").color(theme::TEXT_MUTED));
                                ui.add(
                                    egui::TextEdit::singleline(&mut draft.wsl_config.working_dir)
                                        .hint_text("e.g. /home/user/project (Optional)")
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(280.0),
                                );
                            });

                            ui.add_space(4.0);

                            ui.horizontal(|ui| {
                                ui.radio_value(
                                    &mut draft.wsl_config.sub_mode,
                                    WslSubMode::Command,
                                    egui::RichText::new("🚀 Cmd")
                                        .size(11.5)
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.radio_value(
                                    &mut draft.wsl_config.sub_mode,
                                    WslSubMode::File,
                                    egui::RichText::new("📁 File")
                                        .size(11.5)
                                        .color(theme::TEXT_PRIMARY),
                                );
                            });

                            ui.add_space(4.0);

                            match draft.wsl_config.sub_mode {
                                WslSubMode::Command => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Command:")
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut draft.wsl_config.command_str,
                                            )
                                            .hint_text("e.g. python3 app.py or cargo run")
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(280.0),
                                        );
                                    });
                                }
                                WslSubMode::File => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("File Path:")
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut draft.wsl_config.file_path,
                                            )
                                            .hint_text("/var/log/app.log")
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(280.0),
                                        );
                                    });
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
