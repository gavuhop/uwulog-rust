use crate::app::{SourceType, UwuGuiApp};
use crate::ui::theme;
use eframe::egui::{self, Color32, Id, Key, Order, Pos2, Rect, Rounding, Stroke};
use uwu_core_workspace::WorkspaceLocation;

pub fn render_project_picker_popup(ctx: &egui::Context, app: &mut UwuGuiApp, trigger_rect: Rect) {
    if !app.project_picker_open {
        return;
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.project_picker_open = false;
        return;
    }

    let popup_pos = Pos2::new(trigger_rect.min.x, trigger_rect.max.y + 6.0);
    let popup_width = 300.0;
    let popup_rect = Rect::from_min_size(popup_pos, egui::vec2(popup_width, 360.0));

    // Đóng popup nếu click ra ngoài
    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !trigger_rect.contains(pos) && !popup_rect.contains(pos) {
                app.project_picker_open = false;
                return;
            }
        }
    }

    let mut project_to_launch = None;
    let mut project_to_delete = None;
    let mut open_local_folder_clicked = false;
    let mut open_wsl_modal_clicked = false;

    egui::Area::new(Id::new("zed_project_picker_popup_area"))
        .order(Order::Foreground)
        .fixed_pos(popup_pos)
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::same(8.0))
                .show(ui, |ui| {
                    ui.set_width(popup_width);

                    // 1. Search Box
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("🔍")
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                        let search_edit = egui::TextEdit::singleline(&mut app.project_search_query)
                            .hint_text("Search projects...")
                            .desired_width(popup_width - 36.0)
                            .margin(egui::Margin::symmetric(4.0, 3.0));
                        ui.add(search_edit);
                    });

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    let search_filter = app.project_search_query.trim().to_lowercase();

                    // 2. Section: This Window (Active Project)
                    ui.label(
                        egui::RichText::new("This Window")
                            .size(11.0)
                            .strong()
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(2.0);

                    let active_name = if app.project_name_input.is_empty() {
                        "Workspace".to_string()
                    } else {
                        app.project_name_input.clone()
                    };

                    let active_icon = match app.source_config.source_type {
                        SourceType::Wsl => "🐧",
                        _ => "🖥",
                    };

                    egui::Frame::none()
                        .fill(theme::BG_SURFACE0)
                        .rounding(Rounding::same(4.0))
                        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(active_icon).size(13.0));
                                ui.label(
                                    egui::RichText::new(&active_name)
                                        .strong()
                                        .size(12.0)
                                        .color(theme::TEXT_PRIMARY),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("✓")
                                                .strong()
                                                .size(12.0)
                                                .color(theme::COLOR_INFO),
                                        );
                                    },
                                );
                            });
                        });

                    ui.add_space(6.0);

                    // 3. Section: Recent Projects
                    ui.label(
                        egui::RichText::new("Recent Projects")
                            .size(11.0)
                            .strong()
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(2.0);

                    let filtered_recent: Vec<_> = app
                        .workspace_store
                        .recent_workspaces
                        .iter()
                        .filter(|ws| {
                            if search_filter.is_empty() {
                                true
                            } else {
                                let (name, distro, dir) = match &ws.location {
                                    WorkspaceLocation::Wsl {
                                        distro,
                                        working_dir,
                                    } => (&ws.name, distro.as_str(), working_dir.as_str()),
                                    WorkspaceLocation::Local { working_dir } => {
                                        (&ws.name, "", working_dir.as_str())
                                    }
                                };
                                name.to_lowercase().contains(&search_filter)
                                    || distro.to_lowercase().contains(&search_filter)
                                    || dir.to_lowercase().contains(&search_filter)
                            }
                        })
                        .cloned()
                        .collect();

                    if filtered_recent.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("No matching projects")
                                .italics()
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(4.0);
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                for ws in &filtered_recent {
                                    let is_current = ws.name == app.project_name_input;
                                    let (icon, label_text, tooltip_path) = match &ws.location {
                                        WorkspaceLocation::Wsl {
                                            distro,
                                            working_dir,
                                        } => {
                                            let full = format!("{} ({})", working_dir, distro);
                                            ("🐧", format!("{} ({})", ws.name, distro), full)
                                        }
                                        WorkspaceLocation::Local { working_dir } => {
                                            ("🖥", ws.name.clone(), working_dir.clone())
                                        }
                                    };

                                    let mut frame = egui::Frame::none()
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(egui::Margin::symmetric(6.0, 3.0));

                                    if is_current {
                                        frame = frame.fill(theme::BG_SURFACE0);
                                    }

                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(icon).size(12.5));

                                            let name_resp = ui.selectable_label(
                                                is_current,
                                                egui::RichText::new(&label_text).size(12.0).color(
                                                    if is_current {
                                                        theme::TEXT_KEY
                                                    } else {
                                                        theme::TEXT_PRIMARY
                                                    },
                                                ),
                                            );

                                            if name_resp.clicked() {
                                                project_to_launch = Some(ws.clone());
                                            }

                                            if !tooltip_path.is_empty() {
                                                name_resp.on_hover_text(format!(
                                                    "Open Project in:\n{}",
                                                    tooltip_path
                                                ));
                                            }

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // Delete button (✕)
                                                    let del_btn = egui::Button::new(
                                                        egui::RichText::new("✕")
                                                            .size(10.5)
                                                            .color(theme::TEXT_MUTED),
                                                    )
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false);

                                                    if ui
                                                        .add(del_btn)
                                                        .on_hover_text("Remove from recent list")
                                                        .clicked()
                                                    {
                                                        project_to_delete = Some(ws.id);
                                                    }

                                                    // Open button (↗)
                                                    let open_btn = egui::Button::new(
                                                        egui::RichText::new("↗")
                                                            .size(11.5)
                                                            .color(theme::TEXT_PRIMARY),
                                                    )
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false);

                                                    if ui
                                                        .add(open_btn)
                                                        .on_hover_text(
                                                            "Switch to and launch this project",
                                                        )
                                                        .clicked()
                                                    {
                                                        project_to_launch = Some(ws.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                }
                            });
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 4. Quick Actions
                    let local_btn = egui::Button::new(
                        egui::RichText::new("📂 Open Local Folder...")
                            .size(11.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(Color32::TRANSPARENT)
                    .frame(false);

                    if ui.add(local_btn).clicked() {
                        open_local_folder_clicked = true;
                    }

                    let wsl_btn = egui::Button::new(
                        egui::RichText::new("🐧 Open WSL Folder / Config...")
                            .size(11.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(Color32::TRANSPARENT)
                    .frame(false);

                    if ui.add(wsl_btn).clicked() {
                        open_wsl_modal_clicked = true;
                    }
                });
        });

    if let Some(id) = project_to_delete {
        app.workspace_store.remove(id);
    }

    if let Some(ws) = project_to_launch {
        app.load_workspace(&ws);
        app.save_current_workspace();
        app.restart_current_source();
        app.project_picker_open = false;
    }

    if open_local_folder_clicked {
        app.project_picker_open = false;
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let path_str = folder.to_string_lossy().to_string();
            let _ = std::env::set_current_dir(&folder);
            let folder_name = folder
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Workspace".to_string());
            app.project_name_input = folder_name;
            app.source_config.source_type = SourceType::Process;
            app.source_config.working_dir = path_str;
            app.source_config.command_str.clear();
            app.save_current_workspace();
            app.restart_current_source();
        }
    }

    if open_wsl_modal_clicked {
        app.project_picker_open = false;
        app.source_config.source_type = SourceType::Wsl;
        app.show_launch_modal = true;
    }
}
