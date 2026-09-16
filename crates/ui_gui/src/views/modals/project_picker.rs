use crate::actions::AppAction;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, Rect};
use std::collections::HashSet;
use uwu_core_workspace::{SourceType, Workspace, WorkspaceLocation, WorkspaceStore};

#[derive(Debug, Clone)]
pub struct ProjectPickerSessionInfo {
    pub id: uuid::Uuid,
    pub name: String,
    pub icon: &'static str,
    pub target_summary: String,
    pub normalized_dir: String,
}

pub struct ProjectPickerArgs<'a> {
    pub is_open: bool,
    pub store: &'a WorkspaceStore,
    pub sessions: &'a [ProjectPickerSessionInfo],
    pub active_index: usize,
    pub project_search_query: &'a mut String,
    pub trigger_rect: Rect,
}

/// Hành động phát sinh khi tương tác với một hàng project trong Project Picker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectRowAction {
    None,
    Select,
    Close,
}

/// Tham số cấu hình hiển thị cho một hàng project
pub struct ProjectRowConfig<'a> {
    pub icon: &'a str,
    pub label: &'a str,
    pub is_active: bool,
    pub location_tooltip: Option<&'a str>,
    pub action_tooltip: &'a str,
    pub close_tooltip: &'a str,
}

/// Render một hàng project (dùng chung cho cả This Window và Recent Projects)
pub fn render_project_row(ui: &mut egui::Ui, config: ProjectRowConfig<'_>) -> ProjectRowAction {
    let theme = ui.app_theme();
    let height = crate::components::ui::button::BUTTON_HEIGHT_NORMAL;
    let row_size = egui::vec2(ui.available_width(), height);
    let (row_rect, mut row_resp) = ui.allocate_exact_size(row_size, egui::Sense::click());
    row_resp = row_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

    let is_hovered = ui.rect_contains_pointer(row_rect);
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let bg_color = if is_hovered {
        theme.log.row_hover
    } else {
        Color32::TRANSPARENT
    };

    if bg_color != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(row_rect, CornerRadius::same(4), bg_color);
    }

    let content_rect = row_rect.shrink2(egui::vec2(6.0, 0.0));
    let mut close_clicked = false;
    let mut close_hovered = false;
    let mut action_clicked = false;
    let mut action_hovered = false;

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.label(
                egui::RichText::new(config.icon)
                    .size(12.0)
                    .color(theme.text.muted),
            );

            ui.label(
                egui::RichText::new(config.label)
                    .size(12.0)
                    .color(theme.text.primary),
            );

            if config.is_active {
                let (check_r, _) =
                    ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                crate::components::ui::IconName::Check.paint(
                    ui.painter(),
                    check_r,
                    theme.text.accent,
                );
            }

            if is_hovered {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                    // Nút Close (đóng / xóa project)
                    let (close_rect, close_resp) =
                        ui.allocate_exact_size(egui::vec2(22.0, height), egui::Sense::click());
                    let close_resp = close_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    close_hovered = close_resp.hovered();
                    if close_resp.clicked() {
                        close_clicked = true;
                    }
                    close_resp.on_hover_text(config.close_tooltip);

                    let close_color = if close_hovered {
                        theme.text.primary
                    } else {
                        theme.text.muted
                    };
                    let close_icon_r =
                        egui::Rect::from_center_size(close_rect.center(), egui::vec2(12.0, 12.0));
                    crate::components::ui::IconName::Close.paint(
                        ui.painter(),
                        close_icon_r,
                        close_color,
                    );

                    // Nút Action (switch / open project)
                    let (act_rect, act_resp) =
                        ui.allocate_exact_size(egui::vec2(22.0, height), egui::Sense::click());
                    let act_resp = act_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    action_hovered = act_resp.hovered();
                    if act_resp.clicked() {
                        action_clicked = true;
                    }
                    act_resp.on_hover_text(config.action_tooltip);

                    let act_color = if action_hovered {
                        theme.text.primary
                    } else {
                        theme.text.muted
                    };
                    let act_icon_r =
                        egui::Rect::from_center_size(act_rect.center(), egui::vec2(12.0, 12.0));
                    crate::components::ui::IconName::ExternalLink.paint(
                        ui.painter(),
                        act_icon_r,
                        act_color,
                    );
                });
            }
        },
    );

    if is_hovered || close_hovered || action_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let row_clicked =
        row_resp.clicked() || (is_hovered && ui.input(|i| i.pointer.primary_clicked()));

    if let Some(tooltip) = config.location_tooltip {
        if !tooltip.is_empty() && !close_hovered && !action_hovered {
            row_resp.on_hover_text(format!("{}\nLocation: {}", config.label, tooltip));
        }
    }

    if close_clicked {
        ProjectRowAction::Close
    } else if action_clicked || row_clicked {
        ProjectRowAction::Select
    } else {
        ProjectRowAction::None
    }
}

pub fn render_project_picker_popup(
    ctx: &egui::Context,
    args: ProjectPickerArgs<'_>,
    dispatch: &mut impl FnMut(AppAction),
) {
    let ProjectPickerArgs {
        is_open,
        store,
        sessions,
        active_index,
        project_search_query,
        trigger_rect,
    } = args;

    if !is_open {
        return;
    }

    let popup_width = 300.0;
    let mut session_to_switch = None;
    let mut session_to_close = None;
    let mut project_to_open = None;
    let mut project_to_delete = None;
    let mut open_local_folder_clicked = false;

    let resp =
        crate::components::ui::PopoverContainer::new("zed_project_picker_popup", trigger_rect)
            .width(popup_width)
            .max_height(420.0)
            .show(ctx, |ui| {
                let theme = ui.app_theme();
                // 1. Search Box (No stroke, height = BUTTON_HEIGHT_NORMAL, auto-focus on open)
                let search_id = egui::Id::new("project_picker_search_input");
                crate::components::ui::TextInput::new(project_search_query)
                    .id(search_id)
                    .auto_focus(true)
                    .hint_text("Search projects...")
                    .transparent()
                    .show(ui);

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(4.0);

                let search_filter = project_search_query.trim().to_lowercase();

                // 2. Section: This Window (Active Project)
                ui.label(
                    egui::RichText::new("This Window")
                        .size(11.0)
                        .strong()
                        .color(theme.text.muted),
                );
                ui.add_space(2.0);

                egui::ScrollArea::vertical()
                    .id_salt("this_window_scroll")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for (ix, session) in sessions.iter().enumerate() {
                            let is_active = ix == active_index;
                            let name = if session.name.is_empty() {
                                "Workspace".to_string()
                            } else {
                                session.name.clone()
                            };

                            if !search_filter.is_empty()
                                && !name.to_lowercase().contains(&search_filter)
                            {
                                continue;
                            }

                            let tooltip_loc = if session.target_summary.is_empty() {
                                None
                            } else {
                                Some(session.target_summary.as_str())
                            };

                            let action = render_project_row(
                                ui,
                                ProjectRowConfig {
                                    icon: session.icon,
                                    label: &name,
                                    is_active,
                                    location_tooltip: tooltip_loc,
                                    action_tooltip: "Switch to this project",
                                    close_tooltip: "Close and stop project from this window",
                                },
                            );

                            match action {
                                ProjectRowAction::Select => session_to_switch = Some(ix),
                                ProjectRowAction::Close => session_to_close = Some(ix),
                                ProjectRowAction::None => {}
                            }
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
                        .color(theme.text.muted),
                );
                ui.add_space(2.0);

                let open_ids: HashSet<_> = sessions.iter().map(|s| s.id).collect();
                let open_dirs: HashSet<_> =
                    sessions.iter().map(|s| s.normalized_dir.clone()).collect();

                let filtered_recent: Vec<_> = store
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
                            let name = ws.name.as_str();
                            let remote_info = ws
                                .location
                                .as_remote()
                                .map(|r| r.display_name())
                                .unwrap_or("");
                            let dir = ws.location.working_dir();
                            name.to_lowercase().contains(&search_filter)
                                || remote_info.to_lowercase().contains(&search_filter)
                                || dir.to_lowercase().contains(&search_filter)
                        }
                    })
                    .cloned()
                    .collect();

                if filtered_recent.is_empty() {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("No other recent projects")
                                .italics()
                                .size(11.0)
                                .color(theme.text.muted),
                        );
                    });
                    ui.add_space(3.0);
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("recent_projects_scroll")
                        .max_height(140.0)
                        .show(ui, |ui| {
                            for ws in &filtered_recent {
                                let summary = ws.target_summary();
                                let tooltip_loc = if summary.is_empty() {
                                    None
                                } else {
                                    Some(summary.as_str())
                                };

                                let action = render_project_row(
                                    ui,
                                    ProjectRowConfig {
                                        icon: ws.icon(),
                                        label: &ws.display_label(),
                                        is_active: false,
                                        location_tooltip: tooltip_loc,
                                        action_tooltip: "Open in This Window",
                                        close_tooltip: "Remove from recent list",
                                    },
                                );

                                match action {
                                    ProjectRowAction::Select => project_to_open = Some(ws.clone()),
                                    ProjectRowAction::Close => project_to_delete = Some(ws.id),
                                    ProjectRowAction::None => {}
                                }
                            }
                        });
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 4. Quick Actions
                if crate::components::ui::AppButton::new()
                    .label("Open Local Folder")
                    .icon("📂")
                    .variant(crate::components::ui::ButtonVariant::Ghost)
                    .align_left()
                    .full_width()
                    .show(ui)
                    .clicked()
                {
                    open_local_folder_clicked = true;
                }

                ui.add_space(2.0);

                crate::components::ui::AppButton::new()
                    .label("Open Remote Folder")
                    .icon("🌐")
                    .variant(crate::components::ui::ButtonVariant::Ghost)
                    .align_left()
                    .full_width()
                    .show(ui);
            });

    if resp.closed {
        dispatch(AppAction::CloseProjectPicker);
    }

    if let Some(ix) = session_to_switch {
        dispatch(AppAction::SwitchSession(ix));
    }

    if let Some(ix) = session_to_close {
        dispatch(AppAction::CloseSession(ix));
    }

    if let Some(id) = project_to_delete {
        dispatch(AppAction::DeleteWorkspace(id));
    }

    if let Some(ws) = project_to_open {
        dispatch(AppAction::OpenWorkspace(ws));
    }

    if open_local_folder_clicked {
        dispatch(AppAction::CloseProjectPicker);
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let path_str = uwu_core_workspace::clean_path(&folder.to_string_lossy());
            let _ = std::env::set_current_dir(&folder);
            let folder_name = uwu_core_workspace::extract_project_name(&path_str);

            let ws = Workspace::new(
                folder_name,
                WorkspaceLocation::local(path_str),
                SourceType::Process,
            );
            dispatch(AppAction::OpenWorkspace(ws));
        }
    }
}
