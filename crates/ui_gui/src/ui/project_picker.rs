use crate::app::{AppAction, SourceType, UwuGuiApp};
use crate::ui::theme;
use eframe::egui::{self, Color32, Id, Key, Order, Pos2, Rect, Rounding, Stroke};
use std::collections::HashSet;
use uwu_core_workspace::{Workspace, WorkspaceLocation};

pub fn render_project_picker_popup(ctx: &egui::Context, app: &mut UwuGuiApp, trigger_rect: Rect) {
    if !app.project_picker_open {
        return;
    }

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.dispatch_action(AppAction::CloseProjectPicker);
        return;
    }

    let popup_pos = Pos2::new(trigger_rect.min.x, trigger_rect.max.y + 6.0);
    let popup_width = 300.0;
    let popup_rect = Rect::from_min_size(popup_pos, egui::vec2(popup_width, 420.0));

    // Đóng popup nếu click ra ngoài
    if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if !trigger_rect.contains(pos) && !popup_rect.contains(pos) {
                app.dispatch_action(AppAction::CloseProjectPicker);
                return;
            }
        }
    }

    let mut session_to_switch = None;
    let mut session_to_close = None;
    let mut project_to_open = None;
    let mut project_to_delete = None;
    let mut open_local_folder_clicked = false;

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

                    egui::ScrollArea::vertical()
                        .id_salt("this_window_scroll")
                        .max_height(140.0)
                        .show(ui, |ui| {
                            for (ix, session) in app.sessions.iter().enumerate() {
                                let is_active = ix == app.active_index;
                                let name = if session.session.name.is_empty() {
                                    "Workspace".to_string()
                                } else {
                                    session.session.name.clone()
                                };

                                if !search_filter.is_empty()
                                    && !name.to_lowercase().contains(&search_filter)
                                {
                                    continue;
                                }

                                let icon = session.session.icon();
                                let tooltip_path = session.session.target_summary();
                                let mut frame = egui::Frame::none()
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(egui::Margin::symmetric(6.0, 4.0));

                                if is_active {
                                    frame = frame.fill(theme::BG_SURFACE0);
                                }

                                frame.show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(icon).size(12.0));

                                        let label_color = if is_active {
                                            theme::TEXT_KEY
                                        } else {
                                            theme::TEXT_PRIMARY
                                        };

                                        let name_resp = ui.selectable_label(
                                            is_active,
                                            egui::RichText::new(&name)
                                                .strong()
                                                .size(12.0)
                                                .color(label_color),
                                        );

                                        if name_resp.clicked() {
                                            session_to_switch = Some(ix);
                                        }

                                        if !tooltip_path.is_empty() {
                                            name_resp.on_hover_text(format!(
                                                "{}\nLocation: {}",
                                                name, tooltip_path
                                            ));
                                        }

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // Nút Close '✕' để đóng project khỏi window
                                                let close_btn = egui::Button::new(
                                                    egui::RichText::new("✕")
                                                        .size(11.0)
                                                        .color(theme::TEXT_MUTED),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .frame(false);

                                                if ui
                                                    .add(close_btn)
                                                    .on_hover_text(
                                                        "Close and stop project from this window",
                                                    )
                                                    .clicked()
                                                {
                                                    session_to_close = Some(ix);
                                                }

                                                if is_active {
                                                    ui.label(
                                                        egui::RichText::new("✓")
                                                            .strong()
                                                            .size(12.0)
                                                            .color(theme::COLOR_INFO),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                });
                            }
                        });

                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 3. Section: Recent Projects (Chỉ hiển thị những project chưa mở trong window này)
                    ui.label(
                        egui::RichText::new("Recent Projects")
                            .size(11.0)
                            .strong()
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(2.0);

                    let open_ids: HashSet<_> = app.sessions.iter().map(|s| s.session.id).collect();
                    let open_dirs: HashSet<_> = app
                        .sessions
                        .iter()
                        .map(|s| s.session.location.normalized_dir())
                        .collect();

                    let filtered_recent: Vec<_> = app
                        .store
                        .recent_workspaces
                        .iter()
                        .filter(|ws| {
                            // Bỏ qua nếu đã mở trong window hiện tại
                            if open_ids.contains(&ws.id) {
                                return false;
                            }
                            let ws_dir = ws.location.normalized_dir();
                            if !ws_dir.is_empty() && open_dirs.contains(&ws_dir) {
                                return false;
                            }

                            if search_filter.is_empty() {
                                true
                            } else {
                                let (name, distro, dir) = match &ws.location {
                                    WorkspaceLocation::Wsl {
                                        distro,
                                        working_dir,
                                    } => (ws.name.as_str(), distro.as_str(), working_dir.as_str()),
                                    WorkspaceLocation::Local { working_dir } => {
                                        (ws.name.as_str(), "", working_dir.as_str())
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
                            egui::RichText::new("No other recent projects")
                                .italics()
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(4.0);
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("recent_projects_scroll")
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for ws in &filtered_recent {
                                    let icon = ws.icon();
                                    let label_text = ws.display_label();
                                    let tooltip_path = ws.target_summary();

                                    let frame = egui::Frame::none()
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(egui::Margin::symmetric(6.0, 3.0));

                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(icon).size(12.5));

                                            let name_resp = ui.selectable_label(
                                                false,
                                                egui::RichText::new(&label_text)
                                                    .size(12.0)
                                                    .color(theme::TEXT_PRIMARY),
                                            );

                                            if name_resp.clicked() {
                                                project_to_open = Some(ws.clone());
                                            }

                                            if !tooltip_path.is_empty() {
                                                name_resp.on_hover_text(format!(
                                                    "Open Project in This Window:\n{}",
                                                    tooltip_path
                                                ));
                                            }

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // Nút Delete khỏi Recent
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

                                                    // Nút Open '↗'
                                                    let open_btn = egui::Button::new(
                                                        egui::RichText::new("↗")
                                                            .size(11.5)
                                                            .color(theme::TEXT_PRIMARY),
                                                    )
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false);

                                                    if ui
                                                        .add(open_btn)
                                                        .on_hover_text("Open in This Window")
                                                        .clicked()
                                                    {
                                                        project_to_open = Some(ws.clone());
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
                        egui::RichText::new("📂 Open Local Folder")
                            .size(11.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(Color32::TRANSPARENT)
                    .frame(false);

                    if ui.add(local_btn).clicked() {
                        open_local_folder_clicked = true;
                    }

                    let remote_btn = egui::Button::new(
                        egui::RichText::new("🌐 Open Remote Folder")
                            .size(11.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(Color32::TRANSPARENT)
                    .frame(false);

                    ui.add_enabled(false, remote_btn);
                });
        });

    if let Some(ix) = session_to_switch {
        app.dispatch_action(AppAction::SwitchSession(ix));
    }

    if let Some(ix) = session_to_close {
        app.dispatch_action(AppAction::CloseSession(ix));
    }

    if let Some(id) = project_to_delete {
        app.dispatch_action(AppAction::DeleteWorkspace(id));
    }

    if let Some(ws) = project_to_open {
        app.dispatch_action(AppAction::OpenWorkspace(ws));
    }

    if open_local_folder_clicked {
        app.dispatch_action(AppAction::CloseProjectPicker);
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let path_str = uwu_core_workspace::clean_path(&folder.to_string_lossy());
            let _ = std::env::set_current_dir(&folder);
            let folder_name = uwu_core_workspace::extract_project_name(&path_str);

            let ws = Workspace::new(
                folder_name,
                WorkspaceLocation::Local {
                    working_dir: path_str,
                },
                SourceType::Process,
            );
            app.dispatch_action(AppAction::OpenWorkspace(ws));
        }
    }
}
