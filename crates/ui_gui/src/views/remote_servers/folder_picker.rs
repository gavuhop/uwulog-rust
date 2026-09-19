use crate::components::ui::{IconName, TextInput};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui;
use uwu_core_workspace::{Workspace, WorkspaceStore};

use super::helpers::{
    anchor_cursor_to_end, calculate_adaptive_scroll_height, create_server_workspace,
    get_cached_or_read_directories, get_dir_and_suffix, join_unix_dir, join_unix_path,
    render_empty_state, step_selected_index, ListItemRow,
};
use super::types::{FolderPickerState, RemoteNavAction, RemoteServerKind};

/// Subview 3: Chọn Thư Mục Remote Chuẩn Zed (Bàn phím ưu tiên, Tab drill-down, Enter mở project)
pub fn render_folder_picker_subview(
    ui: &mut egui::Ui,
    folder_state: &mut FolderPickerState,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
) -> (RemoteNavAction, Option<Workspace>) {
    let theme = ui.app_theme();
    let mut nav_action = RemoteNavAction::None;
    let mut selected_workspace = None;

    // 1. Header: [Icon] [Server Name] (chuẩn Zed: Linux Ubuntu hoặc Server SSH)
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let (icon_r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        folder_state
            .server
            .icon()
            .paint(ui.painter(), icon_r, theme.text.primary);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(folder_state.server.display_name())
                .strong()
                .size(13.0)
                .color(theme.text.primary),
        );
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Tiêu thụ Semantic Actions thông qua KeymapManager
    let action = keymap.consume_input(ui, KeyContext::RemoteServers);
    let key_down = action == Some(KeyAction::SelectNext);
    let key_up = action == Some(KeyAction::SelectPrev);
    let key_tab = action == Some(KeyAction::TabComplete);
    let key_enter = action == Some(KeyAction::ConfirmSelection);
    let key_escape = action == Some(KeyAction::Back);

    // 2. Thanh nhập đường dẫn hiện tại
    let input_id = egui::Id::new("remote_folder_picker_path_input");

    // Neo con trỏ text vào cuối chuỗi (chuẩn Zed)
    if folder_state.focus_input || key_down || key_up {
        folder_state.focus_input = false;
        anchor_cursor_to_end(ui.ctx(), input_id, &folder_state.path_query);
    }

    let prev_query = folder_state.path_query.clone();
    let input_resp = TextInput::new(&mut folder_state.path_query)
        .id(input_id)
        .auto_focus(true)
        .hint_text("e.g. /home/username/project")
        .transparent()
        .show(ui);

    // Luôn ưu tiên tuyệt đối cho việc gõ phím: duy trì focus ở ô input
    if !input_resp.has_focus() {
        input_resp.request_focus();
    }

    // Nếu query thay đổi (người dùng gõ hoặc xóa ký tự)
    if folder_state.path_query != prev_query {
        let (dir, _) = get_dir_and_suffix(&folder_state.path_query);
        if dir != folder_state.current_dir {
            folder_state.current_dir = dir.clone();
            folder_state.entries = get_cached_or_read_directories(&folder_state.server, &dir);
        }
        // Khi gõ chữ: LUÔN pick item đầu tiên (index 0)
        folder_state.selected_index = 0;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);

    // 3. Phân tách (dir, suffix) để lọc ứng viên
    let (dir, suffix) = get_dir_and_suffix(&folder_state.path_query);
    let filter = suffix.to_lowercase();
    let matched_folders: Vec<String> = folder_state
        .entries
        .iter()
        .filter(|e| filter.is_empty() || e.to_lowercase().contains(&filter))
        .cloned()
        .collect();

    let show_open_this_dir = filter.is_empty();
    let total_items = if show_open_this_dir {
        1 + matched_folders.len()
    } else {
        matched_folders.len()
    };

    step_selected_index(
        &mut folder_state.selected_index,
        total_items,
        key_down,
        key_up,
    );

    let mut action_open_dir: Option<String> = None;
    let mut action_enter_folder: Option<String> = None;

    // Phím Tab: Điền thư mục đang chọn vào đường dẫn (chuẩn Zed)
    if key_tab {
        let target_folder = if show_open_this_dir {
            if folder_state.selected_index > 0 {
                matched_folders.get(folder_state.selected_index - 1)
            } else {
                matched_folders.first()
            }
        } else {
            matched_folders.get(folder_state.selected_index)
        };
        if let Some(folder) = target_folder {
            action_enter_folder = Some(folder.clone());
        }
    }

    // Phím Enter: Mở mục đang chọn để xem log workspace (chuẩn Zed)
    if key_enter {
        if show_open_this_dir {
            if folder_state.selected_index == 0 {
                action_open_dir = Some(dir.clone());
            } else if let Some(folder) = matched_folders.get(folder_state.selected_index - 1) {
                action_open_dir = Some(join_unix_path(&dir, folder));
            }
        } else if let Some(folder) = matched_folders.get(folder_state.selected_index) {
            action_open_dir = Some(join_unix_path(&dir, folder));
        } else if !folder_state.path_query.trim().is_empty() {
            action_open_dir = Some(folder_state.path_query.trim().to_string());
        }
    }

    if key_escape {
        nav_action = RemoteNavAction::Back;
    }

    // Chỉ đổi selected_index theo chuột khi chuột THỰC SỰ DI CHUYỂN
    let mouse_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);

    // 4. Danh sách thư mục: Tính toán chiều cao ScrollArea tự co giãn theo số lượng item
    let folder_list_height = calculate_adaptive_scroll_height(total_items, 29.0, 360.0);

    egui::ScrollArea::vertical()
        .id_salt("remote_folder_picker_scroll")
        .max_height(folder_list_height)
        .show(ui, |ui| {
            let mut current_item_ix = 0;

            // Mục đầu tiên: ↪ open this directory
            if show_open_this_dir {
                let is_sel = folder_state.selected_index == current_item_ix;
                let resp = ListItemRow::new(IconName::Return, "open this directory")
                    .selected(is_sel)
                    .tooltip(Some(&format!("Open {} as project workspace", dir)))
                    .show(ui);
                if resp.hovered() && mouse_moved {
                    folder_state.selected_index = current_item_ix;
                }
                if is_sel && (key_down || key_up) {
                    resp.scroll_to_me(Some(egui::Align::Center));
                }
                if resp.clicked() {
                    action_open_dir = Some(dir.clone());
                }
                current_item_ix += 1;
                ui.add_space(1.0);
            }

            // Các mục thư mục con: 📁 <folder>
            for folder in &matched_folders {
                let is_sel = folder_state.selected_index == current_item_ix;
                let resp = ListItemRow::new(IconName::Folder, folder)
                    .selected(is_sel)
                    .tooltip(Some(&format!(
                        "Open {} as project (Press Tab to browse subfolders)",
                        folder
                    )))
                    .show(ui);
                if resp.hovered() && mouse_moved {
                    folder_state.selected_index = current_item_ix;
                }
                if is_sel && (key_down || key_up) {
                    resp.scroll_to_me(Some(egui::Align::Center));
                }
                if resp.clicked() {
                    action_open_dir = Some(join_unix_path(&dir, folder));
                }
                current_item_ix += 1;
                ui.add_space(1.0);
            }

            if total_items == 0 {
                render_empty_state(ui, "No matching folders");
            }
        });

    // Thực thi mở thư mục được chọn
    if let Some(target_dir) = action_open_dir {
        match &folder_state.server {
            RemoteServerKind::Wsl(distro) | RemoteServerKind::DevContainer(distro) => {
                store.add_remote_project_to_server(distro, &target_dir);
            }
            RemoteServerKind::Ssh { host, .. } => {
                store.add_remote_project_to_ssh_server(host, &target_dir);
            }
        }
        let ws = create_server_workspace(&folder_state.server, &target_dir);
        store.add_or_update(ws.clone());
        selected_workspace = Some(ws);
    } else if let Some(folder) = action_enter_folder {
        let new_dir = join_unix_dir(&dir, &folder);
        folder_state.current_dir = new_dir.clone();
        folder_state.path_query = new_dir.clone();
        folder_state.entries = get_cached_or_read_directories(&folder_state.server, &new_dir);
        folder_state.selected_index = 0;
        folder_state.focus_input = true;
    }

    (nav_action, selected_workspace)
}
