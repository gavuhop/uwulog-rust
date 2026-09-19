use crate::components::ui::{IconName, TextInput};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use eframe::egui;
use std::collections::BTreeSet;
#[cfg(target_os = "windows")]
use uwu_core_workspace::WslConnection;
use uwu_core_workspace::{RemoteProject, SshConnection, Workspace, WorkspaceStore};
use uwu_driver_transport::{load_system_and_user_ssh_hosts, SshTransport, WslTransport};

use super::helpers::{
    anchor_cursor_to_end, create_server_workspace, get_cached_or_read_directories,
    render_empty_state, render_section_title, step_selected_index, ListItemRow,
};
use super::types::{
    FolderPickerState, RemoteNavAction, RemoteServerKind, RemoteSubView, ServerOptionsState,
    SshPickerState,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ServerListAction {
    ConnectSsh,
    ConnectDevContainer,
    AddWslDistro,
    OpenWorkspace(Box<Workspace>),
    OpenRemotePath {
        server: String,
        path: String,
    },
    OpenSshPath {
        host: String,
        nickname: Option<String>,
        path: String,
    },
    OpenFolder(String),
    OpenFolderSsh(String),
    ViewServerOptions(String),
    ViewServerOptionsKind(RemoteServerKind),
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
pub fn collect_server_list_items(filter: &str, store: &WorkspaceStore) -> Vec<ServerListItem> {
    let mut items = Vec::new();
    let filter = filter.trim().to_lowercase();

    // 1. 3 Nút Action trên cùng (Connect SSH, Dev Container, Add WSL Distro)
    if filter.is_empty() || "connect ssh server".contains(&filter) {
        items.push(ServerListItem {
            icon: IconName::Plus,
            label: "Connect SSH Server".to_string(),
            tooltip: Some("Connect to a remote server over SSH".to_string()),
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

    // 2. Gom tất cả các server connection vào biến `connections` và lặp qua một luồng duy nhất
    let connections: Vec<Connection> = {
        let mut list = Vec::new();
        #[cfg(target_os = "windows")]
        list.extend(store.wsl_connections.iter().map(Connection::Wsl));
        list.extend(store.ssh_connections.iter().map(Connection::Ssh));
        list
    };

    for conn in &connections {
        append_connection_items(&mut items, &filter, conn);
    }

    items
}

/// Đại diện bọc cho một kết nối Server (WSL hoặc SSH)
enum Connection<'a> {
    #[cfg(target_os = "windows")]
    Wsl(&'a WslConnection),
    Ssh(&'a SshConnection),
}

impl<'a> Connection<'a> {
    fn title(&self) -> String {
        match self {
            #[cfg(target_os = "windows")]
            Self::Wsl(s) => format!("WSL: {}", s.distro),
            Self::Ssh(s) => format!("SSH: {}", s.display_name()),
        }
    }

    fn projects(&self) -> &'a BTreeSet<RemoteProject> {
        match self {
            #[cfg(target_os = "windows")]
            Self::Wsl(s) => &s.projects,
            Self::Ssh(s) => &s.projects,
        }
    }

    fn project_action(&self, path: &str) -> ServerListAction {
        match self {
            #[cfg(target_os = "windows")]
            Self::Wsl(s) => ServerListAction::OpenRemotePath {
                server: s.distro.clone(),
                path: path.to_string(),
            },
            Self::Ssh(s) => ServerListAction::OpenSshPath {
                host: s.host.clone(),
                nickname: s.nickname.clone(),
                path: path.to_string(),
            },
        }
    }

    fn open_folder_action(&self) -> ServerListAction {
        match self {
            #[cfg(target_os = "windows")]
            Self::Wsl(s) => ServerListAction::OpenFolder(s.distro.clone()),
            Self::Ssh(s) => ServerListAction::OpenFolderSsh(s.host.clone()),
        }
    }

    fn options_action(&self) -> ServerListAction {
        match self {
            #[cfg(target_os = "windows")]
            Self::Wsl(s) => ServerListAction::ViewServerOptions(s.distro.clone()),
            Self::Ssh(s) => ServerListAction::ViewServerOptionsKind(RemoteServerKind::Ssh {
                host: s.host.clone(),
                nickname: s.nickname.clone(),
            }),
        }
    }
}

/// Helper trích xuất và hiển thị danh sách item cho một kết nối (WSL hoặc SSH)
fn append_connection_items(items: &mut Vec<ServerListItem>, filter: &str, conn: &Connection) {
    let title = conn.title();
    let title_matches = !filter.is_empty() && title.to_lowercase().contains(filter);
    let open_folder_matches = filter.is_empty() || "open folder".contains(filter) || title_matches;
    let options_matches =
        filter.is_empty() || "view server options".contains(filter) || title_matches;

    let matching_projects: Vec<&RemoteProject> = conn
        .projects()
        .iter()
        .filter(|p| filter.is_empty() || title_matches || p.path.to_lowercase().contains(filter))
        .collect();

    if matching_projects.is_empty() && !open_folder_matches && !options_matches {
        return;
    }

    let mut is_first = true;
    let mut take_section = || {
        if is_first {
            is_first = false;
            Some(title.clone())
        } else {
            None
        }
    };

    for proj in matching_projects {
        items.push(ServerListItem {
            icon: IconName::Folder,
            label: proj.path.clone(),
            tooltip: Some(format!("Open project in {}", title)),
            section_title: take_section(),
            action: conn.project_action(&proj.path),
        });
    }

    if open_folder_matches {
        items.push(ServerListItem {
            icon: IconName::FolderOpen,
            label: "Open Folder".to_string(),
            tooltip: Some(format!("Open a directory path in {}", title)),
            section_title: take_section(),
            action: conn.open_folder_action(),
        });
    }

    if options_matches {
        items.push(ServerListItem {
            icon: IconName::Settings,
            label: "View Server Options".to_string(),
            tooltip: Some("View server options".to_string()),
            section_title: take_section(),
            action: conn.options_action(),
        });
    }
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
                let hosts = load_system_and_user_ssh_hosts().into_iter().collect();
                nav_action =
                    RemoteNavAction::navigate(RemoteSubView::SshPicker(SshPickerState::new(hosts)));
            }
            ServerListAction::ConnectDevContainer => {
                // Placeholder Dev Container
            }
            ServerListAction::AddWslDistro => {
                nav_action = RemoteNavAction::navigate(RemoteSubView::WslPicker);
            }
            ServerListAction::OpenWorkspace(ws) => {
                selected_workspace = Some(*ws);
            }
            ServerListAction::OpenRemotePath { server, path } => {
                let ws = create_server_workspace(&RemoteServerKind::Wsl(server), &path);
                selected_workspace = Some(ws);
            }
            ServerListAction::OpenSshPath {
                host,
                nickname,
                path,
            } => {
                let ws = create_server_workspace(&RemoteServerKind::Ssh { host, nickname }, &path);
                selected_workspace = Some(ws);
            }
            ServerListAction::OpenFolder(distro) => {
                let home = WslTransport::resolve_home_dir(&distro);
                let entries =
                    get_cached_or_read_directories(&RemoteServerKind::Wsl(distro.clone()), &home);
                nav_action = RemoteNavAction::navigate(RemoteSubView::FolderPicker(
                    FolderPickerState::new(RemoteServerKind::Wsl(distro), home, entries),
                ));
            }
            ServerListAction::OpenFolderSsh(host) => {
                let conn = store.find_ssh_connection(&host);
                let user = conn.and_then(|c| c.username.as_deref());
                let port = conn.and_then(|c| c.port);
                let args = conn.and_then(|c| c.args.as_deref());
                let nickname = conn.and_then(|c| c.nickname.clone());
                let home = SshTransport::resolve_home_dir(&host, user, port, args);
                let entries = SshTransport::list_remote_directories(&host, &home, user, port, args)
                    .unwrap_or_default();
                nav_action = RemoteNavAction::navigate(RemoteSubView::FolderPicker(
                    FolderPickerState::new(RemoteServerKind::Ssh { host, nickname }, home, entries),
                ));
            }
            ServerListAction::ViewServerOptions(distro) => {
                nav_action =
                    RemoteNavAction::navigate(RemoteSubView::ServerOptions(ServerOptionsState {
                        server: RemoteServerKind::Wsl(distro),
                        selected_index: 0,
                        copied_flash_time: None,
                    }));
            }
            ServerListAction::ViewServerOptionsKind(kind) => {
                nav_action =
                    RemoteNavAction::navigate(RemoteSubView::ServerOptions(ServerOptionsState {
                        server: kind,
                        selected_index: 0,
                        copied_flash_time: None,
                    }));
            }
        }
    }

    (nav_action, selected_workspace)
}
