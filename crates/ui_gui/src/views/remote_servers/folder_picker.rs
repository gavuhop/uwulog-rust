use crate::components::ui::{IconName, TextInput};
use crate::theme::ActiveTheme;
use eframe::egui::{self, Key};
use uwu_core_workspace::{
    extract_project_name, SourceType, Workspace, WorkspaceLocation, WorkspaceStore,
    WslConnectionOptions,
};

use super::helpers::{
    calculate_adaptive_scroll_height, get_cached_or_read_directories, get_dir_and_suffix,
    render_empty_state, ListItemRow,
};
use super::types::{FolderPickerState, RemoteNavAction};

/// Subview 3: Chọn Thư Mục Remote Chuẩn Zed (Bàn phím ưu tiên, Tab drill-down, Enter mở project)
pub fn render_folder_picker_subview(
    ui: &mut egui::Ui,
    folder_state: &mut FolderPickerState,
    store: &mut WorkspaceStore,
) -> (RemoteNavAction, Option<Workspace>) {
    let theme = ui.app_theme();
    let mut nav_action = RemoteNavAction::None;
    let mut selected_workspace = None;

    // 1. Header: Linux <distro_name>
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let (icon_r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        IconName::Linux.paint(ui.painter(), icon_r, theme.text.primary);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(&folder_state.distro)
                .strong()
                .size(13.0)
                .color(theme.text.primary),
        );
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Tiêu thụ các phím điều hướng TRƯỚC HẾT để TextInput không nhận ArrowUp/ArrowDown làm con trỏ nhảy về pos 0
    let key_down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
    let key_up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
    let key_tab = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Tab));
    let key_enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    let key_escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));

    // 2. Thanh nhập đường dẫn hiện tại
    let input_id = egui::Id::new("remote_folder_picker_path_input");

    // Neo con trỏ text vào cuối chuỗi (chuẩn Zed)
    if folder_state.focus_input || key_down || key_up {
        folder_state.focus_input = false;
        let mut text_state =
            egui::text_edit::TextEditState::load(ui.ctx(), input_id).unwrap_or_default();
        let char_count = folder_state.path_query.chars().count();
        text_state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(char_count),
            )));
        text_state.store(ui.ctx(), input_id);
        ui.ctx().memory_mut(|m| m.request_focus(input_id));
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
            folder_state.entries = get_cached_or_read_directories(&folder_state.distro, &dir);
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

    if total_items > 0 && folder_state.selected_index >= total_items {
        folder_state.selected_index = 0;
    }

    let mut action_open_dir: Option<String> = None;
    let mut action_enter_folder: Option<String> = None;

    // Điều hướng cuộn vòng (wrap-around) khi tới cực hạn (chuẩn Zed)
    if total_items > 0 {
        if key_down {
            if folder_state.selected_index + 1 >= total_items {
                folder_state.selected_index = 0;
            } else {
                folder_state.selected_index += 1;
            }
        }
        if key_up {
            if folder_state.selected_index == 0 {
                folder_state.selected_index = total_items - 1;
            } else {
                folder_state.selected_index -= 1;
            }
        }
    }

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
                let full_path = if dir.ends_with('/') {
                    format!("{}{}", dir, folder)
                } else {
                    format!("{}/{}", dir, folder)
                };
                action_open_dir = Some(full_path);
            }
        } else if let Some(folder) = matched_folders.get(folder_state.selected_index) {
            let full_path = if dir.ends_with('/') {
                format!("{}{}", dir, folder)
            } else {
                format!("{}/{}", dir, folder)
            };
            action_open_dir = Some(full_path);
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
                    let full_path = if dir.ends_with('/') {
                        format!("{}{}", dir, folder)
                    } else {
                        format!("{}/{}", dir, folder)
                    };
                    action_open_dir = Some(full_path);
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
        let clean_path = if target_dir.trim().is_empty() {
            "/".to_string()
        } else {
            target_dir.trim().to_string()
        };

        let folder_name = extract_project_name(&clean_path);
        let display_title = if clean_path == "/" || clean_path == "~" {
            format!("WSL ({})", folder_state.distro)
        } else {
            format!("{} ({})", folder_name, folder_state.distro)
        };

        let ws = Workspace::new(
            display_title,
            WorkspaceLocation::remote(WslConnectionOptions::new(
                folder_state.distro.clone(),
                clean_path,
            )),
            SourceType::Process,
        );

        store.add_known_wsl_distro(&folder_state.distro);
        store.add_or_update(ws.clone());
        selected_workspace = Some(ws);
    } else if let Some(folder) = action_enter_folder {
        let new_dir = if dir.ends_with('/') {
            format!("{}{}/", dir, folder)
        } else {
            format!("{}/{}/", dir, folder)
        };
        folder_state.current_dir = new_dir.clone();
        folder_state.path_query = new_dir.clone();
        folder_state.entries = get_cached_or_read_directories(&folder_state.distro, &new_dir);
        folder_state.selected_index = 0;
        folder_state.focus_input = true;
    }

    (nav_action, selected_workspace)
}
