use crate::components::ui::{IconName, TextInput};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use eframe::egui;
#[cfg(target_os = "windows")]
use uwu_core_workspace::RemoteProject;
use uwu_core_workspace::{Workspace, WorkspaceStore};
use uwu_driver_transport::WslTransport;

use super::helpers::{
    anchor_cursor_to_end, create_remote_workspace, get_cached_or_read_directories,
    render_empty_state, render_section_title, step_selected_index, ListItemRow,
};
use super::types::{
    FolderPickerState, RemoteNavAction, RemoteServerKind, RemoteSubView, ServerOptionsState,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ServerListAction {
    ConnectSsh,
    ConnectDevContainer,
    AddWslDistro,
    OpenWorkspace(Box<Workspace>),
    OpenRemotePath { server: String, path: String },
    OpenFolder(String),
    ViewServerOptions(String),
}

/// Một mục hiển thị trong Server List
#[derive(Debug, Clone)]
pub struct ServerListItem {
    pub icon: IconName,
    pub label: String,
    pub tooltip: Option<String>,
    pub section_title: Option<String>,
    pub action: ServerListAction,
}

/// Thu thập danh sách các item hiển thị dựa trên search filter và workspace store
pub fn collect_server_list_items(filter: &str, _store: &WorkspaceStore) -> Vec<ServerListItem> {
    let mut items = Vec::new();
    let filter = filter.trim().to_lowercase();
    #[cfg(target_os = "windows")]
    let store = _store;

    // 1. 3 Nút Action trên cùng (Connect SSH, Dev Container, Add WSL Distro)
    if filter.is_empty() || "connect ssh server".contains(&filter) {
        items.push(ServerListItem {
            icon: IconName::Plus,
            label: "Connect SSH Server".to_string(),
            tooltip: Some("SSH connection coming soon".to_string()),
            section_title: None,
            action: ServerListAction::ConnectSsh,
        });
    }

    if filter.is_empty() || "connect dev container".contains(&filter) {
        items.push(ServerListItem {
            icon: IconName::Plus,
            label: "Connect Dev Container".to_string(),
            tooltip: Some("Dev Container connection coming soon".to_string()),
            section_title: None,
            action: ServerListAction::ConnectDevContainer,
        });
    }

    #[cfg(target_os = "windows")]
    if filter.is_empty() || "add wsl distro".contains(&filter) {
        items.push(ServerListItem {
            icon: IconName::Plus,
            label: "Add WSL Distro".to_string(),
            tooltip: Some("Detect and add a local WSL distribution".to_string()),
            section_title: None,
            action: ServerListAction::AddWslDistro,
        });
    }

    // 2. Hiển thị từng Server Connection và các Projects thuộc về nó (duyệt thẳng O(1), chuẩn Zed)
    #[cfg(target_os = "windows")]
    for server in &store.wsl_connections {
        let cluster_title = format!("WSL: {}", server.distro);

        let cluster_title_matches =
            !filter.is_empty() && cluster_title.to_lowercase().contains(&filter);
        let open_folder_matches =
            filter.is_empty() || "open folder".contains(&filter) || cluster_title_matches;
        let options_matches =
            filter.is_empty() || "view server options".contains(&filter) || cluster_title_matches;

        let matching_projects: Vec<&RemoteProject> = server
            .projects
            .iter()
            .filter(|p| {
                filter.is_empty()
                    || cluster_title_matches
                    || p.path.to_lowercase().contains(&filter)
            })
            .collect();

        if matching_projects.is_empty() && !open_folder_matches && !options_matches {
            continue;
        }

        let mut is_first = true;

        for proj in matching_projects {
            let section = if is_first {
                is_first = false;
                Some(cluster_title.clone())
            } else {
                None
            };

            items.push(ServerListItem {
                icon: IconName::Folder,
                label: proj.path.clone(),
                tooltip: Some(format!("Open project in {}", cluster_title)),
                section_title: section,
                action: ServerListAction::OpenRemotePath {
                    server: server.distro.clone(),
                    path: proj.path.clone(),
                },
            });
        }

        if open_folder_matches {
            let section = if is_first {
                is_first = false;
                Some(cluster_title.clone())
            } else {
                None
            };

            items.push(ServerListItem {
                icon: IconName::FolderOpen,
                label: "Open Folder".to_string(),
                tooltip: Some(format!("Open a directory path in {}", cluster_title)),
                section_title: section,
                action: ServerListAction::OpenFolder(server.distro.clone()),
            });
        }

        if options_matches {
            let section = if is_first {
                Some(cluster_title.clone())
            } else {
                None
            };

            items.push(ServerListItem {
                icon: IconName::Settings,
                label: "View Server Options".to_string(),
                tooltip: Some("View server options".to_string()),
                section_title: section,
                action: ServerListAction::ViewServerOptions(server.distro.clone()),
            });
        }
    }

    items
}

/// Subview 1: Danh sách tổng quan Remote Projects & Clusters
pub fn render_remote_list_subview(
    ui: &mut egui::Ui,
    search_query: &mut String,
    selected_index: &mut usize,
    keymap: &KeymapManager,
    store: &WorkspaceStore,
) -> (RemoteNavAction, Option<Workspace>) {
    let mut nav_action = RemoteNavAction::None;
    let mut selected_workspace = None;

    // Tiêu thụ Semantic Actions thông qua KeymapManager
    let action = keymap.consume_input(ui, KeyContext::RemoteServers);
    let key_down = action == Some(KeyAction::SelectNext);
    let key_up = action == Some(KeyAction::SelectPrev);
    let key_enter = action == Some(KeyAction::ConfirmSelection);
    let key_escape = action == Some(KeyAction::Back);

    if key_escape {
        return (RemoteNavAction::Back, None);
    }

    // 1. Thanh tìm kiếm trên cùng (Search remote projects...)
    let search_id = egui::Id::new("remote_projects_search_input");

    // Neo con trỏ text vào cuối chuỗi (chuẩn Zed)
    if key_down || key_up {
        anchor_cursor_to_end(ui.ctx(), search_id, search_query);
    }

    let prev_query = search_query.clone();
    let input_resp = TextInput::new(search_query)
        .id(search_id)
        .auto_focus(true)
        .hint_text("Search remote projects...")
        .transparent()
        .show(ui);

    // Luôn ưu tiên tuyệt đối cho việc gõ phím: duy trì focus ở ô input
    if !input_resp.has_focus() {
        input_resp.request_focus();
    }

    // Nếu query thay đổi (người dùng gõ hoặc xóa ký tự): LUÔN chọn item đầu tiên (index 0) giống folder picker
    if *search_query != prev_query {
        *selected_index = 0;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Thu thập danh sách item hiển thị
    let items = collect_server_list_items(search_query, store);
    let total_items = items.len();

    // Điều hướng cuộn vòng (wrap-around) khi tới cực hạn (chuẩn Zed)
    step_selected_index(selected_index, total_items, key_down, key_up);

    // Chỉ đổi selected_index theo chuột khi chuột THỰC SỰ DI CHUYỂN
    let mouse_moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    let mut triggered_action = None;

    // Phím Enter: kích hoạt item đang chọn
    if key_enter && total_items > 0 {
        if let Some(item) = items.get(*selected_index) {
            triggered_action = Some(item.action.clone());
        }
    }

    egui::ScrollArea::vertical()
        .id_salt("remote_projects_scroll_area")
        .max_height(380.0)
        .show(ui, |ui| {
            if items.is_empty() {
                render_empty_state(ui, "No matching remote projects");
            } else {
                for (idx, item) in items.iter().enumerate() {
                    if let Some(title) = &item.section_title {
                        render_section_title(ui, title);
                    }

                    let is_sel = *selected_index == idx;
                    let resp = ListItemRow::new(item.icon, &item.label)
                        .selected(is_sel)
                        .tooltip(item.tooltip.as_deref())
                        .show(ui);

                    if resp.hovered() && mouse_moved {
                        *selected_index = idx;
                    }
                    if is_sel && (key_down || key_up) {
                        resp.scroll_to_me(Some(egui::Align::Center));
                    }
                    if resp.clicked() {
                        triggered_action = Some(item.action.clone());
                    }
                    ui.add_space(1.0);
                }
            }
        });

    // Thực thi hành động được kích hoạt (bằng Enter hoặc Click chuột)
    if let Some(action) = triggered_action {
        match action {
            ServerListAction::ConnectSsh => {
                // Placeholder SSH
            }
            ServerListAction::ConnectDevContainer => {
                // Placeholder Dev Container
            }
            ServerListAction::AddWslDistro => {
                nav_action = RemoteNavAction::Navigate(RemoteSubView::WslPicker);
            }
            ServerListAction::OpenWorkspace(ws) => {
                selected_workspace = Some(*ws);
            }
            ServerListAction::OpenRemotePath { server, path } => {
                let ws = create_remote_workspace(&server, &path);
                selected_workspace = Some(ws);
            }
            ServerListAction::OpenFolder(distro) => {
                let home = WslTransport::resolve_home_dir(&distro);
                let entries = get_cached_or_read_directories(&distro, &home);
                nav_action = RemoteNavAction::Navigate(RemoteSubView::FolderPicker(
                    FolderPickerState::new(distro, home, entries),
                ));
            }
            ServerListAction::ViewServerOptions(distro) => {
                nav_action =
                    RemoteNavAction::Navigate(RemoteSubView::ServerOptions(ServerOptionsState {
                        server: RemoteServerKind::Wsl(distro),
                        selected_index: 0,
                        copied_flash_time: None,
                    }));
            }
        }
    }

    (nav_action, selected_workspace)
}
