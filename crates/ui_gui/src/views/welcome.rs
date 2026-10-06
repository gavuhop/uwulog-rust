use crate::actions::AppAction;
use crate::components::ui::IconName;
use crate::theme::ActiveTheme;
use crate::views::modals::project_picker::{
    format_project_display_label, render_project_row, ProjectRowAction, ProjectRowConfig,
};
use eframe::egui::{self, CornerRadius};
use uwu_core_workspace::{SourceType, Workspace, WorkspaceLocation, WorkspaceStore};

/// Render màn hình Welcome / Empty State khi ứng dụng chưa mở dự án nào
pub fn render_welcome_view(
    ui: &mut egui::Ui,
    store: &mut WorkspaceStore,
    dispatch: &mut impl FnMut(AppAction),
) {
    let theme = ui.app_theme();
    let has_recents = !store.recent_workspaces.is_empty();

    let mut project_to_open = None;
    let mut project_to_delete = None;
    let mut open_local_clicked = false;
    let mut open_remote_clicked = false;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(theme.surfaces.base)
                .inner_margin(egui::Margin::symmetric(24, 32)),
        )
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.1);

                // 1. Logo & App Title
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                IconName::Screen.paint(ui.painter(), icon_rect, theme.text.accent);

                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Uwu Log")
                        .size(22.0)
                        .strong()
                        .color(theme.text.primary),
                );

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("High-performance log viewer & workspace monitor")
                        .size(13.0)
                        .color(theme.text.muted),
                );

                ui.add_space(28.0);

                let container_width = 380.0_f32.min(ui.available_width() - 32.0);

                if has_recents {
                    // 2. Recent Projects Section
                    egui::Frame::default()
                        .fill(theme.surfaces.mantle)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(container_width);

                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("RECENT PROJECTS")
                                        .size(10.5)
                                        .strong()
                                        .color(theme.text.muted),
                                );
                            });

                            ui.add_space(6.0);

                            egui::ScrollArea::vertical()
                                .id_salt("welcome_recent_projects_scroll")
                                .max_height(240.0)
                                .show(ui, |ui| {
                                    for ws in &store.recent_workspaces {
                                        let summary = ws.target_summary();
                                        let tooltip_loc = if summary.is_empty() {
                                            None
                                        } else {
                                            Some(summary.as_str())
                                        };
                                        let ws_icon = ws.icon();
                                        let label = format_project_display_label(
                                            &ws.name,
                                            ws.server_name(),
                                        );

                                        let action = render_project_row(
                                            ui,
                                            ProjectRowConfig {
                                                icon: ws_icon,
                                                label: &label,
                                                is_active: false,
                                                location_tooltip: tooltip_loc,
                                                action_icon: None,
                                                action_tooltip: None,
                                                close_tooltip: "Remove from recent list",
                                            },
                                        );

                                        match action {
                                            ProjectRowAction::Select
                                            | ProjectRowAction::OpenInNewWindow => {
                                                project_to_open = Some(ws.clone())
                                            }
                                            ProjectRowAction::Close => {
                                                project_to_delete = Some(ws.id)
                                            }
                                            ProjectRowAction::None => {}
                                        }
                                    }
                                });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Quick Action Buttons
                            if crate::components::ui::AppButton::new()
                                .label("Open Local Folder")
                                .icon(IconName::Folder)
                                .variant(crate::components::ui::ButtonVariant::Ghost)
                                .align_left()
                                .full_width()
                                .show(ui)
                                .clicked()
                            {
                                open_local_clicked = true;
                            }

                            ui.add_space(4.0);

                            if crate::components::ui::AppButton::new()
                                .label("Open Remote Folder")
                                .icon(IconName::Screen)
                                .variant(crate::components::ui::ButtonVariant::Ghost)
                                .align_left()
                                .full_width()
                                .show(ui)
                                .clicked()
                            {
                                open_remote_clicked = true;
                            }
                        });
                } else {
                    // 3. Minimal Empty State: Only Open Local and Open Remote buttons
                    egui::Frame::default()
                        .fill(theme.surfaces.mantle)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(16, 20))
                        .show(ui, |ui| {
                            ui.set_width(320.0_f32.min(ui.available_width() - 32.0));

                            if crate::components::ui::AppButton::new()
                                .label("Open Local Folder")
                                .icon(IconName::Folder)
                                .variant(crate::components::ui::ButtonVariant::Default)
                                .full_width()
                                .show(ui)
                                .clicked()
                            {
                                open_local_clicked = true;
                            }

                            ui.add_space(10.0);

                            if crate::components::ui::AppButton::new()
                                .label("Open Remote Folder")
                                .icon(IconName::Screen)
                                .variant(crate::components::ui::ButtonVariant::Outline)
                                .full_width()
                                .show(ui)
                                .clicked()
                            {
                                open_remote_clicked = true;
                            }
                        });
                }
            });
        });

    if let Some(ws) = project_to_open {
        dispatch(AppAction::OpenWorkspace(ws));
    }

    if let Some(id) = project_to_delete {
        dispatch(AppAction::DeleteWorkspace(id));
    }

    if open_local_clicked {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let path_str = uwu_core_workspace::clean_path(&folder.to_string_lossy());
            let folder_name = uwu_core_workspace::extract_project_name(&path_str);
            let ws = Workspace::new(
                folder_name,
                WorkspaceLocation::local(path_str),
                SourceType::Process,
            );
            dispatch(AppAction::OpenWorkspace(ws));
        }
    }

    if open_remote_clicked {
        dispatch(AppAction::OpenRemoteServersModal);
    }
}
