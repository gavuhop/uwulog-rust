use crate::components::ui::TextInput;
use eframe::egui::{self, Key};
use std::collections::BTreeSet;
use uwu_core_workspace::{Workspace, WorkspaceStore};
use uwu_driver_transport::WslTransport;

use super::helpers::{
    get_cached_or_read_directories, render_empty_state, render_section_title, ListItemRow,
};
use super::types::{
    FolderPickerState, RemoteNavAction, RemoteServerKind, RemoteSubView, ServerOptionsState,
};

/// Hành động của từng mục trong Server List
#[derive(Debug, Clone, PartialEq)]
pub enum ServerListAction {
    ConnectSsh,
    ConnectDevContainer,
    AddWslDistro,
    OpenWorkspace(Box<Workspace>),
    OpenFolder(String),
    ViewServerOptions(String),
}

/// Một mục hiển thị trong Server List
#[derive(Debug, Clone)]
pub struct ServerListItem {
    pub icon: &'static str,
    pub label: String,
    pub tooltip: Option<String>,
    pub section_title: Option<String>,
    pub action: ServerListAction,
}

/// Thu thập danh sách các item hiển thị dựa trên search filter và workspace store
pub fn collect_server_list_items(filter: &str, store: &WorkspaceStore) -> Vec<ServerListItem> {
    let mut items = Vec::new();
    let filter = filter.trim().to_lowercase();

    // 1. 3 Nút Action trên cùng (Connect SSH, Dev Container, Add WSL Distro)
    if filter.is_empty() || "connect ssh server".contains(&filter) {
        items.push(ServerListItem {
            icon: "+",
            label: "Connect SSH Server".to_string(),
            tooltip: Some("SSH connection coming soon".to_string()),
            section_title: None,
            action: ServerListAction::ConnectSsh,
        });
    }

    if filter.is_empty() || "connect dev container".contains(&filter) {
        items.push(ServerListItem {
            icon: "+",
            label: "Connect Dev Container".to_string(),
            tooltip: Some("Dev Container connection coming soon".to_string()),
            section_title: None,
            action: ServerListAction::ConnectDevContainer,
        });
    }

    if filter.is_empty() || "add wsl distro".contains(&filter) {
        items.push(ServerListItem {
            icon: "+",
            label: "Add WSL Distro".to_string(),
            tooltip: Some("Detect and add a local WSL distribution".to_string()),
            section_title: None,
            action: ServerListAction::AddWslDistro,
        });
    }

    // 2. Thu thập danh sách các Remote Clusters đã kết nối
    let mut cluster_distros: BTreeSet<String> = BTreeSet::new();
    for d in &store.known_wsl_distros {
        cluster_distros.insert(d.clone());
    }
    for ws in &store.recent_workspaces {
        if let Some(remote) = ws.location.as_remote() {
            cluster_distros.insert(remote.display_name().to_string());
        }
    }

    // 3. Hiển thị từng cụm Pack (Cluster)
    for distro in &cluster_distros {
        let cluster_title = format!("WSL: {}", distro);

        let cluster_workspaces: Vec<_> = store
            .recent_workspaces
            .iter()
            .filter(|ws| {
                ws.location
                    .as_remote()
                    .map(|r| r.display_name() == distro)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        let cluster_title_matches =
            !filter.is_empty() && cluster_title.to_lowercase().contains(&filter);
        let open_folder_matches =
            filter.is_empty() || "open folder".contains(&filter) || cluster_title_matches;
        let options_matches =
            filter.is_empty() || "view server options".contains(&filter) || cluster_title_matches;

        let matching_workspaces: Vec<_> = cluster_workspaces
            .into_iter()
            .filter(|ws| {
                filter.is_empty()
                    || cluster_title_matches
                    || ws.location.working_dir().to_lowercase().contains(&filter)
            })
            .collect();

        if matching_workspaces.is_empty() && !open_folder_matches && !options_matches {
            continue;
        }

        let mut is_first = true;

        for ws in matching_workspaces {
            let work_dir = ws.location.working_dir();
            let section = if is_first {
                is_first = false;
                Some(cluster_title.clone())
            } else {
                None
            };

            items.push(ServerListItem {
                icon: "📁",
                label: work_dir.to_string(),
                tooltip: Some(format!("Open project in {}", cluster_title)),
                section_title: section,
                action: ServerListAction::OpenWorkspace(Box::new(ws)),
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
                icon: "+",
                label: "Open Folder".to_string(),
                tooltip: Some(format!("Open a directory path in {}", cluster_title)),
                section_title: section,
                action: ServerListAction::OpenFolder(distro.clone()),
            });
        }

        if options_matches {
            let section = if is_first {
                Some(cluster_title.clone())
            } else {
                None
            };

            items.push(ServerListItem {
                icon: "⚙",
                label: "View Server Options".to_string(),
                tooltip: Some("View server options".to_string()),
                section_title: section,
                action: ServerListAction::ViewServerOptions(distro.clone()),
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
    store: &WorkspaceStore,
) -> (RemoteNavAction, Option<Workspace>) {
    let mut nav_action = RemoteNavAction::None;
    let mut selected_workspace = None;

    // Tiêu thụ các phím điều hướng TRƯỚC TIÊN để TextInput không nhận ArrowUp/ArrowDown làm con trỏ nhảy về pos 0
    let key_down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
    let key_up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
    let key_enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
    let key_escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));

    if key_escape {
        return (RemoteNavAction::Back, None);
    }

    // 1. Thanh tìm kiếm trên cùng (Search remote projects...)
    let search_id = egui::Id::new("remote_projects_search_input");

    // Neo con trỏ text vào cuối chuỗi (chuẩn Zed)
    if key_down || key_up {
        let mut text_state =
            egui::text_edit::TextEditState::load(ui.ctx(), search_id).unwrap_or_default();
        let char_count = search_query.chars().count();
        text_state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(char_count),
            )));
        text_state.store(ui.ctx(), search_id);
        ui.ctx().memory_mut(|m| m.request_focus(search_id));
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

    // Đảm bảo selected_index hợp lệ
    if total_items > 0 && *selected_index >= total_items {
        *selected_index = 0;
    }

    // Điều hướng cuộn vòng (wrap-around) khi tới cực hạn (chuẩn Zed)
    if total_items > 0 {
        if key_down {
            if *selected_index + 1 >= total_items {
                *selected_index = 0;
            } else {
                *selected_index += 1;
            }
        }
        if key_up {
            if *selected_index == 0 {
                *selected_index = total_items - 1;
            } else {
                *selected_index -= 1;
            }
        }
    }

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
            ServerListAction::OpenFolder(distro) => {
                let home = WslTransport::resolve_home_dir(&distro);
                let entries = get_cached_or_read_directories(&distro, &home);
                nav_action =
                    RemoteNavAction::Navigate(RemoteSubView::FolderPicker(FolderPickerState {
                        distro,
                        path_query: home.clone(),
                        current_dir: home,
                        entries,
                        selected_index: 0,
                        focus_input: true,
                        error: None,
                    }));
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
