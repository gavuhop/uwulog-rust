use crate::app::{SourceType, UwuGuiApp, WslSubMode};
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
                egui::RichText::new("Settings & Launch Parameters")
                    .size(14.0)
                    .strong()
                    .color(theme::TEXT_KEY),
            );
            ui.separator();
            ui.add_space(6.0);

            // Project & Workspace Card
            render_modal_card(ui, "📁 Project & Workspace", |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Recent:").color(theme::TEXT_MUTED));
                    let current_name = if app.project_name_input.is_empty() {
                        "Select Workspace".to_string()
                    } else {
                        app.project_name_input.clone()
                    };

                    let mut selected_workspace: Option<uwu_core::Workspace> = None;

                    egui::ComboBox::from_id_salt("workspace_recent_combo")
                        .selected_text(
                            egui::RichText::new(&current_name)
                                .strong()
                                .color(theme::TEXT_PRIMARY),
                        )
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for ws in &app.workspace_store.recent_workspaces {
                                let label = match &ws.location {
                                    uwu_core::WorkspaceLocation::Wsl { distro, .. } => {
                                        format!("🐧 [{}] {}", distro, ws.name)
                                    }
                                    _ => format!("🪟 {}", ws.name),
                                };
                                if ui
                                    .selectable_label(ws.name == app.project_name_input, label)
                                    .clicked()
                                {
                                    selected_workspace = Some(ws.clone());
                                }
                            }
                        });

                    if let Some(ws) = selected_workspace {
                        app.load_workspace(&ws);
                    }
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Name:").color(theme::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.project_name_input)
                            .hint_text("Project Name")
                            .desired_width(170.0),
                    );

                    let save_btn = egui::Button::new(
                        egui::RichText::new("💾 Save").color(theme::TEXT_PRIMARY),
                    )
                    .fill(theme::BG_SURFACE0)
                    .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                    .rounding(Rounding::same(4.0));

                    if ui
                        .add(save_btn)
                        .on_hover_text("Save current configuration as project")
                        .clicked()
                    {
                        app.save_current_workspace();
                    }

                    if let Some(active) = app.workspace_store.get_active() {
                        let active_id = active.id;
                        let del_btn =
                            egui::Button::new(egui::RichText::new("🗑").color(theme::COLOR_ERROR))
                                .fill(theme::BG_SURFACE0)
                                .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                                .rounding(Rounding::same(4.0));

                        if ui
                            .add(del_btn)
                            .on_hover_text("Delete current project from history")
                            .clicked()
                        {
                            app.workspace_store.remove(active_id);
                        }
                    }
                });
            });

            ui.add_space(8.0);

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

                ui.radio_value(
                    &mut app.source_config.source_type,
                    SourceType::Wsl,
                    egui::RichText::new("🐧 WSL (Windows Subsystem for Linux)")
                        .color(theme::TEXT_PRIMARY),
                );
                if app.source_config.source_type == SourceType::Wsl {
                    egui::Frame::none()
                        .fill(theme::BG_CRUST)
                        .rounding(Rounding::same(4.0))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Distro:").color(theme::TEXT_MUTED));
                                if !app.available_wsl_distros.is_empty() {
                                    egui::ComboBox::from_id_salt("wsl_distro_combo")
                                        .selected_text(
                                            if app.source_config.wsl_config.distro.is_empty() {
                                                "Select Distro"
                                            } else {
                                                &app.source_config.wsl_config.distro
                                            },
                                        )
                                        .show_ui(ui, |ui| {
                                            for d in &app.available_wsl_distros {
                                                ui.selectable_value(
                                                    &mut app.source_config.wsl_config.distro,
                                                    d.clone(),
                                                    d,
                                                );
                                            }
                                        });
                                } else {
                                    ui.add(
                                        egui::TextEdit::singleline(
                                            &mut app.source_config.wsl_config.distro,
                                        )
                                        .hint_text("Ubuntu")
                                        .desired_width(120.0),
                                    );
                                }
                            });

                            ui.add_space(4.0);

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Workdir:").color(theme::TEXT_MUTED));
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut app.source_config.wsl_config.working_dir,
                                    )
                                    .hint_text("e.g. /home/user/project (Optional)")
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(280.0),
                                );
                            });

                            ui.add_space(4.0);

                            ui.horizontal(|ui| {
                                ui.radio_value(
                                    &mut app.source_config.wsl_config.sub_mode,
                                    WslSubMode::Command,
                                    egui::RichText::new("🚀 Cmd")
                                        .size(11.5)
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.radio_value(
                                    &mut app.source_config.wsl_config.sub_mode,
                                    WslSubMode::File,
                                    egui::RichText::new("📁 File")
                                        .size(11.5)
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.radio_value(
                                    &mut app.source_config.wsl_config.sub_mode,
                                    WslSubMode::Journald,
                                    egui::RichText::new("📜 Journald")
                                        .size(11.5)
                                        .color(theme::TEXT_PRIMARY),
                                );
                            });

                            ui.add_space(4.0);

                            match app.source_config.wsl_config.sub_mode {
                                WslSubMode::Command => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Command:")
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut app.source_config.wsl_config.command_str,
                                            )
                                            .hint_text(
                                                "e.g. journalctl -f -o json or python3 app.py",
                                            )
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
                                                &mut app.source_config.wsl_config.file_path,
                                            )
                                            .hint_text("/var/log/syslog")
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(280.0),
                                        );
                                    });
                                }
                                WslSubMode::Journald => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Unit (opt):")
                                                .color(theme::TEXT_MUTED),
                                        );
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut app.source_config.wsl_config.journald_unit,
                                            )
                                            .hint_text("e.g. nginx.service (leave blank for all)")
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(240.0),
                                        );
                                    });
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
                    app.save_current_workspace();
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
