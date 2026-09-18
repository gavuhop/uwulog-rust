use crate::components::ui::{AppButton, IconName};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui;
use uwu_core_workspace::WorkspaceStore;
use uwu_driver_transport::WslTransport;

use super::helpers::{calculate_adaptive_scroll_height, step_selected_index, ListItemRow};
use super::types::RemoteNavAction;

/// Subview 2: Chọn WSL Distribution để thêm
pub fn render_wsl_picker_subview(
    ui: &mut egui::Ui,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
) -> RemoteNavAction {
    let theme = ui.app_theme();
    let mut nav_action = RemoteNavAction::None;

    ui.horizontal(|ui| {
        if AppButton::new()
            .label("Back")
            .icon(IconName::ArrowLeft)
            .variant(crate::components::ui::ButtonVariant::Ghost)
            .show(ui)
            .clicked()
        {
            nav_action = RemoteNavAction::Back;
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Select WSL Distribution")
                .strong()
                .size(13.0)
                .color(theme.text.primary),
        );
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    // Tiêu thụ Semantic Actions thông qua KeymapManager
    let action = keymap.consume_input(ui, KeyContext::RemoteServers);
    let key_down = action == Some(KeyAction::SelectNext);
    let key_up = action == Some(KeyAction::SelectPrev);
    let key_enter = action == Some(KeyAction::ConfirmSelection);
    let key_escape = action == Some(KeyAction::Back);

    if key_escape {
        return RemoteNavAction::Back;
    }

    let detected = WslTransport::detect_distros();

    if detected.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("No WSL distributions found.")
                    .size(12.0)
                    .color(theme.text.muted),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Make sure WSL is installed on Windows.")
                    .size(11.0)
                    .color(theme.text.muted),
            );
        });
    } else {
        let sel_id = ui.id().with("wsl_picker_selected_index");
        let mut selected_index: usize = ui.data(|d| d.get_temp(sel_id)).unwrap_or(0);

        step_selected_index(&mut selected_index, detected.len(), key_down, key_up);

        let mut chosen_distro = None;
        if key_enter {
            chosen_distro = detected.get(selected_index).cloned();
        }

        let mouse_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        let wsl_list_height = calculate_adaptive_scroll_height(detected.len(), 30.0, 340.0);

        egui::ScrollArea::vertical()
            .id_salt("wsl_distros_scroll")
            .max_height(wsl_list_height)
            .show(ui, |ui| {
                for (idx, distro) in detected.iter().enumerate() {
                    let is_sel = selected_index == idx;
                    let resp = ListItemRow::new(IconName::Linux, distro)
                        .selected(is_sel)
                        .tooltip(Some("Add this distro to remote server list"))
                        .show(ui);

                    if resp.hovered() && mouse_moved {
                        selected_index = idx;
                    }
                    if is_sel && (key_down || key_up) {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }
                    if resp.clicked() {
                        chosen_distro = Some(distro.clone());
                    }
                    ui.add_space(2.0);
                }
            });

        ui.data_mut(|d| d.insert_temp(sel_id, selected_index));

        if let Some(distro) = chosen_distro {
            store.ensure_wsl_connection(&distro);
            // Sau khi thêm distro, quay trở về danh sách server chính (pop màn hình WslPicker khỏi history)
            nav_action = RemoteNavAction::Back;
        }
    }

    nav_action
}
