use crate::theme::ActiveTheme;
use eframe::egui::{self, Key};
use uwu_core_workspace::WorkspaceStore;

use super::helpers::{step_selected_index, ListItemRow};
use super::types::{
    RemoteNavAction, RemoteServerKind, ServerOptionAction, ServerOptionItem, ServerOptionsState,
};

/// Subview 4: Danh sách các Option của Server (chuẩn Zed: Remove Distro, Go Back, mở rộng SSH...)
pub fn render_server_options_subview(
    ui: &mut egui::Ui,
    options_state: &mut ServerOptionsState,
    store: &mut WorkspaceStore,
) -> RemoteNavAction {
    let theme = ui.app_theme();
    let mut nav_action = RemoteNavAction::None;

    // 1. Header: [Icon] [Server Name] (chuẩn Zed: ví dụ Linux Ubuntu, Server SSH)
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let (icon_r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        options_state
            .server
            .icon()
            .paint(ui.painter(), icon_r, theme.text.primary);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(options_state.server.display_name())
                .strong()
                .size(13.0)
                .color(theme.text.primary),
        );
    });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    let options = ServerOptionItem::list_for_server(&options_state.server);
    let total_items = options.len();

    // Tiêu thụ phím điều hướng bàn phím chuẩn Zed
    let key_down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
    let key_up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
    let key_enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    let key_escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));

    if key_escape {
        return RemoteNavAction::Back;
    }

    step_selected_index(
        &mut options_state.selected_index,
        total_items,
        key_down,
        key_up,
    );

    let mouse_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    let mut triggered_action = None;

    if key_enter {
        triggered_action = options
            .get(options_state.selected_index)
            .map(|opt| opt.action.clone());
    }

    // Xử lý hiệu ứng chớp đổi màu khi copy (1 giây)
    let is_currently_flashing = if let Some(flash_t) = options_state.copied_flash_time {
        if flash_t.elapsed() < std::time::Duration::from_millis(1000) {
            ui.ctx().request_repaint();
            true
        } else {
            options_state.copied_flash_time = None;
            false
        }
    } else {
        false
    };

    for (idx, item) in options.iter().enumerate() {
        let is_sel = options_state.selected_index == idx;

        let is_success =
            is_currently_flashing && matches!(item.action, ServerOptionAction::CopyAddress(_));

        let resp = ListItemRow::new(item.icon, &item.label)
            .end_slot(item.end_slot.as_deref())
            .destructive(item.is_destructive)
            .success(is_success)
            .selected(is_sel)
            .show(ui);
        if resp.hovered() && mouse_moved {
            options_state.selected_index = idx;
        }
        if resp.clicked() {
            triggered_action = Some(item.action.clone());
        }
        ui.add_space(2.0);
    }

    if let Some(action) = triggered_action {
        match action {
            ServerOptionAction::RemoveServer => {
                let (title, message) = match &options_state.server {
                    RemoteServerKind::Wsl(distro) => (
                        "Remove WSL Distro",
                        format!("Remove WSL distro `{}`?", distro),
                    ),
                    RemoteServerKind::Ssh { host, nickname } => {
                        let name = nickname.as_deref().unwrap_or(host);
                        (
                            "Remove SSH Server",
                            format!("Remove SSH server `{}`?", name),
                        )
                    }
                    RemoteServerKind::DevContainer(name) => (
                        "Remove Dev Container",
                        format!("Remove Dev Container `{}`?", name),
                    ),
                };

                let confirmed = rfd::MessageDialog::new()
                    .set_title(title)
                    .set_description(&message)
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .set_level(rfd::MessageLevel::Warning)
                    .show();

                if confirmed == rfd::MessageDialogResult::Yes {
                    match &options_state.server {
                        RemoteServerKind::Wsl(distro) => {
                            store.remove_wsl_connection(distro);
                        }
                        RemoteServerKind::Ssh { .. } => {}
                        RemoteServerKind::DevContainer(_) => {}
                    }
                    nav_action = RemoteNavAction::Back;
                }
            }
            ServerOptionAction::CopyAddress(addr) => {
                ui.ctx().copy_text(addr);
                options_state.copied_flash_time = Some(std::time::Instant::now());
                ui.ctx().request_repaint();
            }
            ServerOptionAction::EditNickname => {
                // Placeholder cho trình chỉnh sửa nickname SSH sau này
            }
            ServerOptionAction::GoBack => {
                nav_action = RemoteNavAction::Back;
            }
        }
    }

    nav_action
}
