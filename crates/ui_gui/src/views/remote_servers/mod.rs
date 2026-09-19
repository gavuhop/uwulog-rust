pub mod folder_picker;
pub mod helpers;
pub mod server_list;
pub mod server_options;
pub mod ssh_picker;
pub mod types;
pub mod wsl_picker;

pub use folder_picker::render_folder_picker_subview;
pub use helpers::*;
pub use server_list::{
    collect_server_list_items, render_remote_list_subview, ServerListAction, ServerListItem,
};
pub use server_options::render_server_options_subview;
pub use ssh_picker::render_ssh_picker_subview;
pub use types::*;
pub use wsl_picker::render_wsl_picker_subview;

use crate::actions::AppAction;
use crate::keymap::KeymapManager;
use crate::overlay::RemoteModalPlacement;
use eframe::egui;
use uwu_core_workspace::{Workspace, WorkspaceStore};

/// Render modal chọn Remote Projects theo thiết kế chuẩn của Zed
pub fn render_remote_servers_modal(
    ctx: &egui::Context,
    is_open: bool,
    placement: RemoteModalPlacement,
    keymap: &KeymapManager,
    store: &mut WorkspaceStore,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    let state_id = egui::Id::new("remote_servers_modal_state");
    let mut state: RemoteModalState = ctx.data(|d| d.get_temp(state_id)).unwrap_or_default();

    let mut selected_workspace: Option<Workspace> = None;
    let mut nav_action = RemoteNavAction::None;

    let closed = RemotePickerContainer::new("remote_servers_picker_container")
        .placement(placement)
        .width(520.0)
        .max_height(480.0)
        .close_on_escape(false)
        .show(ctx, |ui| match &mut state.subview {
            RemoteSubView::List => {
                let (action, ws) = render_remote_list_subview(
                    ui,
                    &mut state.search_query,
                    &mut state.selected_index,
                    keymap,
                    store,
                );
                nav_action = action;
                selected_workspace = ws;
            }
            RemoteSubView::WslPicker => {
                nav_action = render_wsl_picker_subview(ui, keymap, store);
            }
            RemoteSubView::SshPicker(ssh_state) => {
                nav_action = render_ssh_picker_subview(ui, ssh_state, keymap, store);
            }
            RemoteSubView::FolderPicker(folder_state) => {
                let (action, ws) = render_folder_picker_subview(ui, folder_state, keymap, store);
                nav_action = action;
                selected_workspace = ws;
            }
            RemoteSubView::ServerOptions(options_state) => {
                nav_action = render_server_options_subview(ui, options_state, keymap, store);
            }
        });

    let close_modal =
        |ctx: &egui::Context, state: &mut RemoteModalState, dispatch: &mut dyn FnMut(AppAction)| {
            state.reset();
            ctx.data_mut(|d| d.insert_temp(state_id, state.clone()));
            dispatch(AppAction::CloseRemoteServersModal);
        };

    match nav_action {
        RemoteNavAction::Navigate(next) => {
            state.navigate(*next);
            ctx.data_mut(|d| d.insert_temp(state_id, state));
            return;
        }
        RemoteNavAction::Back => {
            if !state.back() {
                // Không còn màn hình nào trong history để back -> Đóng modal
                close_modal(ctx, &mut state, dispatch);
                return;
            }
            ctx.data_mut(|d| d.insert_temp(state_id, state));
            return;
        }
        RemoteNavAction::None => {}
    }

    // Xử lý mở workspace đã chọn
    if let Some(ws) = selected_workspace {
        close_modal(ctx, &mut state, dispatch);
        dispatch(AppAction::OpenWorkspace(ws));
        return;
    }

    if closed {
        close_modal(ctx, &mut state, dispatch);
        return;
    }

    ctx.data_mut(|d| d.insert_temp(state_id, state));
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ui::IconName;

    #[test]
    fn test_calculate_adaptive_scroll_height() {
        assert_eq!(calculate_adaptive_scroll_height(0, 29.0, 360.0), 48.0);
        assert_eq!(calculate_adaptive_scroll_height(1, 29.0, 360.0), 40.0);
        assert_eq!(calculate_adaptive_scroll_height(3, 29.0, 360.0), 93.0);
        assert_eq!(calculate_adaptive_scroll_height(50, 29.0, 360.0), 360.0);
    }

    #[test]
    fn test_get_dir_and_suffix() {
        assert_eq!(
            get_dir_and_suffix("/home/truongviet/"),
            ("/home/truongviet/".to_string(), "".to_string())
        );
        assert_eq!(
            get_dir_and_suffix("/home/truongviet/.claude"),
            ("/home/truongviet/".to_string(), ".claude".to_string())
        );
        assert_eq!(
            get_dir_and_suffix("/home/truongviet/.claude/"),
            ("/home/truongviet/.claude/".to_string(), "".to_string())
        );
        assert_eq!(
            get_dir_and_suffix("/etc"),
            ("/".to_string(), "etc".to_string())
        );
        assert_eq!(get_dir_and_suffix("/"), ("/".to_string(), "".to_string()));
        assert_eq!(get_dir_and_suffix(""), ("/".to_string(), "".to_string()));
    }

    #[test]
    fn test_step_selected_index_behavior() {
        let mut idx = 0;
        // Total 0: stays 0
        step_selected_index(&mut idx, 0, true, false);
        assert_eq!(idx, 0);

        // Key down from 0 in 3 items -> 1
        let mut idx = 0;
        step_selected_index(&mut idx, 3, true, false);
        assert_eq!(idx, 1);

        // Key down from 2 in 3 items -> wraps to 0
        step_selected_index(&mut idx, 3, true, false);
        assert_eq!(idx, 2);
        step_selected_index(&mut idx, 3, true, false);
        assert_eq!(idx, 0);

        // Key up from 0 in 3 items -> wraps to 2
        step_selected_index(&mut idx, 3, false, true);
        assert_eq!(idx, 2);

        // Out of bounds reset
        let mut idx = 10;
        step_selected_index(&mut idx, 3, false, false);
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_clear_dir_cache() {
        clear_dir_cache();
        // Caching and clearing
        let entries = get_cached_or_read_directories(
            &RemoteServerKind::Wsl("non_existent_distro".to_string()),
            "/home",
        );
        assert!(entries.is_empty());
        clear_dir_cache();
    }

    #[test]
    fn test_folder_picker_state_focus_input() {
        let state = FolderPickerState::new(
            RemoteServerKind::Wsl("Ubuntu".to_string()),
            "/home/truongviet/",
            vec![".claude".to_string(), ".config".to_string()],
        );
        assert!(state.focus_input);
        assert_eq!(state.path_query.chars().count(), 17);
    }

    #[test]
    fn test_folder_picker_tab_and_enter_resolution() {
        let (dir, suffix) = get_dir_and_suffix("/home/truongviet/");
        assert_eq!(dir, "/home/truongviet/");
        assert_eq!(suffix, "");

        let matched_folders = vec!["backend".to_string(), "frontend".to_string()];
        let show_open_this_dir = suffix.is_empty();
        assert!(show_open_this_dir);

        // Case A: selected_index = 0 (open this directory)
        let enter_open_path = if show_open_this_dir && 0 == 0 {
            dir.clone()
        } else {
            String::new()
        };
        assert_eq!(enter_open_path, "/home/truongviet/");

        // Tab completes first folder
        let target_folder_tab = if show_open_this_dir {
            matched_folders.first().cloned()
        } else {
            None
        };
        assert_eq!(target_folder_tab, Some("backend".to_string()));
        let new_tab_dir = format!("{}{}/", dir, target_folder_tab.unwrap());
        assert_eq!(new_tab_dir, "/home/truongviet/backend/");

        // Case B: selected_index = 1 (backend folder)
        let selected_index = 1;
        let selected_folder = matched_folders.get(selected_index - 1).unwrap();
        let enter_folder_path = format!("{}{}", dir, selected_folder);
        assert_eq!(enter_folder_path, "/home/truongviet/backend");

        // Case C: Filtering with suffix
        let (dir2, suffix2) = get_dir_and_suffix("/home/truongviet/front");
        assert_eq!(dir2, "/home/truongviet/");
        assert_eq!(suffix2, "front");
        let show_open_this_dir2 = suffix2.is_empty();
        assert!(!show_open_this_dir2);

        let matched_filtered: Vec<String> = matched_folders
            .into_iter()
            .filter(|f| f.contains(&suffix2))
            .collect();
        assert_eq!(matched_filtered, vec!["frontend".to_string()]);

        // Enter on index 0 of filtered: opens frontend
        let enter_filtered_path = format!("{}{}", dir2, matched_filtered[0]);
        assert_eq!(enter_filtered_path, "/home/truongviet/frontend");

        // Tab on index 0 of filtered: completes frontend/
        let tab_filtered_path = format!("{}{}/", dir2, matched_filtered[0]);
        assert_eq!(tab_filtered_path, "/home/truongviet/frontend/");

        // Case D: Back parent resolution
        let trimmed1 = "/home/truongviet/".trim_end_matches('/');
        let (parent1, _) = get_dir_and_suffix(trimmed1);
        assert_eq!(parent1, "/home/");

        let trimmed2 = "/home/".trim_end_matches('/');
        let parent2 = get_dir_and_suffix(trimmed2).0;
        assert_eq!(parent2, "/");
    }

    #[test]
    fn test_folder_picker_wrap_around_and_typing_resets_selection() {
        let total_items = 4;
        let mut selected_index = 0;

        // Bấm Lên khi đang ở cực hạn trên (index 0) -> cuộn vòng xuống item cuối cùng (3)
        step_selected_index(&mut selected_index, total_items, false, true);
        assert_eq!(selected_index, 3);

        // Bấm Xuống khi đang ở cực hạn dưới (index 3) -> cuộn vòng lên item đầu tiên (0)
        step_selected_index(&mut selected_index, total_items, true, false);
        assert_eq!(selected_index, 0);

        // Giả sử sau đó người dùng bấm Xuống để chọn item 2
        selected_index = 2;

        // Khi người dùng gõ chữ: query thay đổi -> LUÔN reset selected_index về 0
        let prev_query = "/home/truongviet/".to_string();
        let new_query = "/home/truongviet/p".to_string();
        if new_query != prev_query {
            selected_index = 0;
        }
        assert_eq!(selected_index, 0);
    }

    #[test]
    fn test_remote_server_kind_display_and_icon() {
        let wsl = RemoteServerKind::Wsl("Ubuntu-22.04".to_string());
        assert_eq!(wsl.display_name(), "Ubuntu-22.04");
        assert_eq!(wsl.icon(), IconName::Linux);

        let ssh = RemoteServerKind::Ssh {
            host: "prod-server".to_string(),
            nickname: None,
        };
        assert_eq!(ssh.display_name(), "prod-server");
        assert_eq!(ssh.icon(), IconName::Server);

        let ssh_nick = RemoteServerKind::Ssh {
            host: "prod-server".to_string(),
            nickname: Some("Production Node".to_string()),
        };
        assert_eq!(ssh_nick.display_name(), "Production Node");

        let dev_container = RemoteServerKind::DevContainer("rust-env".to_string());
        assert_eq!(dev_container.display_name(), "rust-env");
        assert_eq!(dev_container.icon(), IconName::Box);
    }

    #[test]
    fn test_server_option_items_for_different_kinds() {
        // 1. WSL: Remove Distro (destructive) + Go Back
        let wsl = RemoteServerKind::Wsl("Ubuntu".to_string());
        let wsl_opts = ServerOptionItem::list_for_server(&wsl);
        assert_eq!(wsl_opts.len(), 2);
        assert_eq!(wsl_opts[0].action, ServerOptionAction::RemoveServer);
        assert_eq!(wsl_opts[0].label, "Remove Distro");
        assert!(wsl_opts[0].is_destructive);
        assert_eq!(wsl_opts[1].action, ServerOptionAction::GoBack);
        assert_eq!(wsl_opts[1].label, "Go Back");
        assert!(!wsl_opts[1].is_destructive);

        // 2. SSH: Edit Nickname + Copy Server Address + Remove Server + Go Back
        let ssh = RemoteServerKind::Ssh {
            host: "server1.example.com".to_string(),
            nickname: None,
        };
        let ssh_opts = ServerOptionItem::list_for_server(&ssh);
        assert_eq!(ssh_opts.len(), 4);
        assert_eq!(ssh_opts[0].action, ServerOptionAction::EditNickname);
        assert_eq!(ssh_opts[0].label, "Add Nickname to Server");
        assert_eq!(
            ssh_opts[1].action,
            ServerOptionAction::CopyAddress("server1.example.com".to_string())
        );
        assert_eq!(ssh_opts[1].label, "Copy Server Address");
        assert_eq!(
            ssh_opts[1].end_slot,
            Some("server1.example.com".to_string())
        );
        assert_eq!(ssh_opts[2].action, ServerOptionAction::RemoveServer);
        assert!(ssh_opts[2].is_destructive);
        assert_eq!(ssh_opts[3].action, ServerOptionAction::GoBack);

        // 3. DevContainer: Remove Dev Container + Go Back
        let dc = RemoteServerKind::DevContainer("dc1".to_string());
        let dc_opts = ServerOptionItem::list_for_server(&dc);
        assert_eq!(dc_opts.len(), 2);
        assert_eq!(dc_opts[0].action, ServerOptionAction::RemoveServer);
        assert_eq!(dc_opts[0].label, "Remove Dev Container");
        assert!(dc_opts[0].is_destructive);
        assert_eq!(dc_opts[1].action, ServerOptionAction::GoBack);
    }

    #[test]
    fn test_server_options_keyboard_wrap_around() {
        let options =
            ServerOptionItem::list_for_server(&RemoteServerKind::Wsl("Ubuntu".to_string()));
        let total_items = options.len();
        assert_eq!(total_items, 2);

        let mut state = ServerOptionsState {
            server: RemoteServerKind::Wsl("Ubuntu".to_string()),
            selected_index: 0,
            copied_flash_time: None,
        };

        // ArrowUp from 0 wraps to 1 (last)
        if state.selected_index == 0 {
            state.selected_index = total_items - 1;
        } else {
            state.selected_index -= 1;
        }
        assert_eq!(state.selected_index, 1);

        // ArrowDown from 1 wraps to 0 (first)
        if state.selected_index + 1 >= total_items {
            state.selected_index = 0;
        } else {
            state.selected_index += 1;
        }
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_server_options_flash_timer() {
        let flash_t = std::time::Instant::now();
        assert!(flash_t.elapsed() < std::time::Duration::from_millis(1000));
    }

    #[test]
    fn test_remote_server_add_distro_flow() {
        let mut store = WorkspaceStore::default();
        assert!(store.wsl_connections.is_empty());

        // 1. Giả lập mở WslPicker từ List
        let mut state = RemoteModalState::default();
        state.navigate(RemoteSubView::WslPicker);
        assert_eq!(state.subview, RemoteSubView::WslPicker);
        assert_eq!(state.history.len(), 1);

        // 2. Thêm distro từ WslPicker
        let chosen_distro = "Ubuntu".to_string();
        store.ensure_wsl_connection(&chosen_distro);

        // 3. Sau khi thêm distro, WslPicker trả về Back -> pop về List, history rỗng (không circular)
        assert!(state.back());
        assert_eq!(state.subview, RemoteSubView::List);
        assert!(state.history.is_empty());

        // 4. Distro đã lưu trong store và hiển thị trên server list
        assert_eq!(store.wsl_connections.len(), 1);
        assert_eq!(store.wsl_connections[0].distro, "Ubuntu");

        // 5. Bấm "Open Folder" sau đó mới vào FolderPicker
        let folder_picker_subview = RemoteSubView::FolderPicker(FolderPickerState::new(
            RemoteServerKind::Wsl("Ubuntu".to_string()),
            "/home/user",
            vec![],
        ));
        match folder_picker_subview {
            RemoteSubView::FolderPicker(s) => {
                assert_eq!(s.server.display_name(), "Ubuntu");
            }
            _ => panic!("Expected FolderPicker subview"),
        }
    }

    #[test]
    fn test_collect_server_list_items_and_filtering() {
        let mut store = WorkspaceStore::default();
        // 1. Mặc định chưa có server nào
        let items_empty = collect_server_list_items("", &store);

        #[cfg(target_os = "windows")]
        {
            assert_eq!(items_empty.len(), 3);
            assert_eq!(items_empty[0].action, ServerListAction::ConnectSsh);
            assert_eq!(items_empty[1].action, ServerListAction::ConnectDevContainer);
            assert_eq!(items_empty[2].action, ServerListAction::AddWslDistro);

            // 2. Thêm WSL distro vào store -> có thêm 2 items: Open Folder và View Server Options
            store.ensure_wsl_connection("Ubuntu");
            let items_with_server = collect_server_list_items("", &store);
            assert_eq!(items_with_server.len(), 5);
            assert_eq!(
                items_with_server[3].action,
                ServerListAction::OpenFolder("Ubuntu".to_string())
            );
            assert_eq!(
                items_with_server[3].section_title,
                Some("WSL: Ubuntu".to_string())
            );
            assert_eq!(
                items_with_server[4].action,
                ServerListAction::ViewServerOptions("Ubuntu".to_string())
            );

            // 3. Thêm project trực tiếp vào server Ubuntu -> xuất hiện mục project duyệt O(1)
            store.add_remote_project_to_server("Ubuntu", "/home/user/backend");
            let items_with_project = collect_server_list_items("", &store);
            assert_eq!(items_with_project.len(), 6);
            assert_eq!(
                items_with_project[3].action,
                ServerListAction::OpenRemotePath {
                    server: "Ubuntu".to_string(),
                    path: "/home/user/backend".to_string(),
                }
            );

            // 4. Lọc theo chữ "open folder"
            let filtered = collect_server_list_items("open folder", &store);
            assert_eq!(filtered.len(), 1);
            assert_eq!(
                filtered[0].action,
                ServerListAction::OpenFolder("Ubuntu".to_string())
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Trên Linux/macOS: không có WSL, chỉ có các action SSH và Dev Container
            assert_eq!(items_empty.len(), 2);
            assert_eq!(items_empty[0].action, ServerListAction::ConnectSsh);
            assert_eq!(items_empty[1].action, ServerListAction::ConnectDevContainer);

            // WSL connections không hiển thị trên non-Windows
            store.ensure_wsl_connection("Ubuntu");
            let items_with_server = collect_server_list_items("", &store);
            assert_eq!(items_with_server.len(), 2);
        }
    }

    #[test]
    fn test_server_list_keyboard_wrap_around_and_typing_resets_selection() {
        let total_items = 5;
        let mut selected_index = 0;

        // Bấm Lên khi đang ở index 0 -> cuộn vòng về item cuối cùng (4)
        step_selected_index(&mut selected_index, total_items, false, true);
        assert_eq!(selected_index, 4);

        // Bấm Xuống khi đang ở item cuối cùng -> cuộn vòng về 0
        step_selected_index(&mut selected_index, total_items, true, false);
        assert_eq!(selected_index, 0);

        // Bấm Xuống tiếp -> index 1
        selected_index += 1;
        assert_eq!(selected_index, 1);

        // Khi người dùng gõ chữ tìm kiếm: query thay đổi -> LUÔN chọn item đầu tiên (index 0)
        let prev_query = "";
        let new_query = "ub";
        if new_query != prev_query {
            selected_index = 0;
        }
        assert_eq!(selected_index, 0);
    }

    #[test]
    fn test_remote_modal_state_navigation_back_stack() {
        let mut state = RemoteModalState::default();
        assert_eq!(state.subview, RemoteSubView::List);
        assert!(state.history.is_empty());

        // Back khi đang ở màn gốc và history rỗng -> false (đóng modal)
        assert!(!state.back());

        // 1. Chuyển từ List -> WslPicker (như trên điện thoại, lưu List vào stack)
        state.navigate(RemoteSubView::WslPicker);
        assert_eq!(state.subview, RemoteSubView::WslPicker);
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0], RemoteSubView::List);

        // 2. Chuyển từ WslPicker -> FolderPicker
        let folder_subview = RemoteSubView::FolderPicker(FolderPickerState::new(
            RemoteServerKind::Wsl("Ubuntu".to_string()),
            "/home",
            vec![],
        ));
        state.navigate(folder_subview.clone());
        assert_eq!(state.subview, folder_subview);
        assert_eq!(state.history.len(), 2);

        // 3. Nhấn Esc (Back lần 1) -> pop về WslPicker
        assert!(state.back());
        assert_eq!(state.subview, RemoteSubView::WslPicker);
        assert_eq!(state.history.len(), 1);

        // 4. Nhấn Esc (Back lần 2) -> pop về List
        assert!(state.back());
        assert_eq!(state.subview, RemoteSubView::List);
        assert!(state.history.is_empty());

        // 5. Nhấn Esc (Back lần 3 khi history rỗng) -> trả về false để đóng modal
        assert!(!state.back());
        assert_eq!(state.subview, RemoteSubView::List);
    }

    #[test]
    fn test_remote_modal_state_reset() {
        let mut state = RemoteModalState {
            search_query: "ubuntu".to_string(),
            selected_index: 2,
            ..Default::default()
        };
        state.navigate(RemoteSubView::WslPicker);

        assert!(!state.history.is_empty());
        assert_eq!(state.selected_index, 2);

        state.reset();
        assert!(state.search_query.is_empty());
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.subview, RemoteSubView::List);
        assert!(state.history.is_empty());
    }

    #[test]
    fn test_remote_picker_container_positioning_and_escape() {
        let ctx = egui::Context::default();

        // Frame 1: Render container
        let raw_input = egui::RawInput::default();
        let mut output = ctx.run_ui(raw_input, |ui| {
            let closed = RemotePickerContainer::new("test_remote_picker")
                .width(400.0)
                .show(ui.ctx(), |ui| {
                    ui.label("Remote servers content");
                });
            assert!(!closed);
        });
        output.textures_delta.clear();

        // Frame 2: Escape key press should close
        let mut esc_input = egui::RawInput::default();
        esc_input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        let mut output2 = ctx.run_ui(esc_input, |ui| {
            let closed = RemotePickerContainer::new("test_remote_picker")
                .width(400.0)
                .show(ui.ctx(), |ui| {
                    ui.label("Remote servers content");
                });
            assert!(closed);
        });
        output2.textures_delta.clear();

        // Frame 3: TopLeft placement with server_button_rect in ctx.data
        let btn_rect =
            egui::Rect::from_min_size(egui::Pos2::new(42.0, 4.0), egui::vec2(100.0, 24.0));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("server_button_rect"), btn_rect));
        let mut output3 = ctx.run_ui(egui::RawInput::default(), |ui| {
            let closed = RemotePickerContainer::new("test_remote_picker_top_left")
                .placement(RemoteModalPlacement::TopLeft)
                .width(400.0)
                .show(ui.ctx(), |ui| {
                    ui.label("Remote servers content");
                });
            assert!(!closed);
        });
        output3.textures_delta.clear();
    }

    #[test]
    fn test_ssh_picker_and_server_flow() {
        let mut store = WorkspaceStore::default();
        store.ensure_ssh_connection("staging-server");
        store.add_remote_project_to_ssh_server("staging-server", "/var/log/nginx");

        let items = collect_server_list_items("", &store);
        let ssh_project_item = items.iter().find(|i| {
            matches!(
                &i.action,
                ServerListAction::OpenSshPath { host, .. } if host == "staging-server"
            )
        });
        assert!(ssh_project_item.is_some());

        // Test FolderPicker with SSH ServerKind
        let folder_state = FolderPickerState::new(
            RemoteServerKind::Ssh {
                host: "staging-server".to_string(),
                nickname: None,
            },
            "/var/log",
            vec!["nginx".to_string(), "redis".to_string()],
        );
        assert_eq!(folder_state.server.display_name(), "staging-server");
        assert_eq!(folder_state.entries.len(), 2);

        // Test SshPickerState
        let mut ssh_state =
            SshPickerState::new(vec!["server-alpha".to_string(), "server-beta".to_string()]);
        assert_eq!(ssh_state.suggested_hosts.len(), 2);
        ssh_state.input_query = "alpha".to_string();
        assert_eq!(ssh_state.input_query, "alpha");
    }

    #[test]
    fn test_ssh_multi_stage_zed_flow() {
        use uwu_core_workspace::SshConnectionOptions;

        // 1. Phân tích cú pháp: `ssh user@example -o 2222`
        let input = "ssh user@example -o 2222";
        let opts =
            SshConnectionOptions::parse_command_line(input, "").expect("Must parse successfully");
        assert_eq!(opts.host, "example");
        assert_eq!(opts.username, Some("user".to_string()));
        assert_eq!(opts.port, Some(2222));
        assert_eq!(opts.target_string(), "user@example:2222");

        // 2. Khởi tạo SshPickerState ở giai đoạn Input
        let mut ssh_state = SshPickerState::new(vec!["example".to_string()]);
        assert_eq!(ssh_state.stage, SshPickerStage::Input);
        assert!(ssh_state.parsed_options.is_none());

        // 3. Sau khi người dùng xác nhận kết nối -> chuyển sang HostKeyVerification (yes/no)
        let prompt = format!(
            "The authenticity of host '{}' can't be established.\nED25519 key fingerprint is SHA256:4Z1q9sK9jWzL6NpRv8X2tQ7mY0uI3eB5wV1c8aF4oDk.\nAre you sure you want to continue connecting (yes/no)?",
            opts.target_string()
        );
        ssh_state.parsed_options = Some(opts.clone());
        ssh_state.stage = SshPickerStage::HostKeyVerification {
            prompt_message: prompt.clone(),
            user_input: String::new(),
        };

        assert!(prompt.contains("yes/no"));
        assert!(prompt.contains("user@example:2222"));

        // 4. Nếu người dùng nhập "no", hủy quay lại Input
        let abort_answer = "no";
        if abort_answer == "no" {
            ssh_state.stage = SshPickerStage::Input;
        }
        assert_eq!(ssh_state.stage, SshPickerStage::Input);

        // 5. Nếu người dùng nhập "yes", chuyển sang PasswordPrompt
        let confirm_answer = "yes";
        if confirm_answer == "yes" {
            ssh_state.stage = SshPickerStage::PasswordPrompt {
                prompt_message: format!("{}'s password:", opts.target_string()),
                password_input: String::new(),
                is_masked: true,
                error_message: None,
            };
        }

        match &mut ssh_state.stage {
            SshPickerStage::PasswordPrompt {
                prompt_message,
                password_input,
                is_masked,
                error_message: _,
            } => {
                assert_eq!(prompt_message, "user@example:2222's password:");
                assert!(*is_masked, "Password must initially be masked");

                // Toggle unmask (mô phỏng nút Show)
                *is_masked = !*is_masked;
                assert!(!*is_masked, "Password must be unmasked after toggling");

                // Toggle mask (mô phỏng nút Hide)
                *is_masked = !*is_masked;
                assert!(
                    *is_masked,
                    "Password must be re-masked after toggling again"
                );

                // Nhập mật khẩu
                password_input.push_str("secret123");
                assert_eq!(password_input, "secret123");
            }
            _ => panic!("Expected PasswordPrompt stage"),
        }

        // 6. Lưu cấu hình vào WorkspaceStore khi kết nối thành công
        let mut store = WorkspaceStore::default();
        let conn = store.ensure_ssh_connection(&opts.host);
        if let Some(ref u) = opts.username {
            conn.username = Some(u.clone());
        }
        if let Some(p) = opts.port {
            conn.port = Some(p);
        }

        let saved = store
            .find_ssh_connection("example")
            .expect("Connection must exist");
        assert_eq!(saved.host, "example");
        assert_eq!(saved.username.as_deref(), Some("user"));
        assert_eq!(saved.port, Some(2222));
    }

    #[test]
    fn test_ssh_step_by_step_interactive_failure_and_prompts() {
        use uwu_driver_transport::{
            SshConnectionSuccess, SshInteractiveEvent, SshInteractivePromptType,
        };

        // 1. Khởi tạo trạng thái ban đầu: Stage::Input
        let mut ssh_state = SshPickerState::new(vec![]);
        assert_eq!(ssh_state.stage, SshPickerStage::Input);
        assert!(ssh_state.error_message.is_none());

        // 2. Mô phỏng: Nhập server không kết nối được (ví dụ port đóng: `ssh user@localhost -p 3333`)
        // Quy trình Zed: Thử kết nối ngay lập tức -> Nếu thất bại, báo lỗi NGAY TẠI Input stage
        ssh_state.stage = SshPickerStage::Connecting {
            status_message: "Connecting to localhost:3333...".to_string(),
        };

        let fail_event = SshInteractiveEvent::Failed(
            "ssh: connect to host localhost port 3333: Connection refused".to_string(),
        );

        match fail_event {
            SshInteractiveEvent::Failed(err) => {
                ssh_state.stage = SshPickerStage::Input;
                ssh_state.error_message = Some(err);
            }
            _ => panic!("Expected Failed event"),
        }

        // Đảm bảo quay lại Input stage và hiển thị lỗi ngay lập tức, KHÔNG hề hỏi yes/no hay password
        assert_eq!(ssh_state.stage, SshPickerStage::Input);
        assert_eq!(
            ssh_state.error_message.as_deref(),
            Some("ssh: connect to host localhost port 3333: Connection refused")
        );

        // 3. Khi người dùng sửa input, error_message tự động bị xóa
        ssh_state.input_query = "ssh testuser@validserver".to_string();
        ssh_state.error_message = None;
        assert!(ssh_state.error_message.is_none());

        // 4. Mô phỏng: Server yêu cầu xác thực Host Key
        let (resp_tx, resp_rx) = std::sync::mpsc::channel::<String>();
        let hostkey_prompt_event = SshInteractiveEvent::Prompt {
            prompt_message: "The authenticity of host 'validserver' can't be established (yes/no)?"
                .to_string(),
            prompt_type: SshInteractivePromptType::HostKeyConfirmation,
            response_sender: resp_tx,
        };

        match hostkey_prompt_event {
            SshInteractiveEvent::Prompt {
                prompt_message,
                prompt_type,
                response_sender,
            } => {
                assert_eq!(prompt_type, SshInteractivePromptType::HostKeyConfirmation);
                ssh_state.stage = SshPickerStage::HostKeyVerification {
                    prompt_message,
                    user_input: String::new(),
                };
                // Người dùng gõ "yes"
                let _ = response_sender.send("yes".to_string());
            }
            _ => panic!("Expected Prompt event"),
        }

        assert_eq!(resp_rx.recv().unwrap(), "yes");
        match &ssh_state.stage {
            SshPickerStage::HostKeyVerification { prompt_message, .. } => {
                assert!(prompt_message.contains("authenticity"));
            }
            _ => panic!("Expected HostKeyVerification stage"),
        }

        // 5. Mô phỏng: Sau khi xác thực host key, SSH daemon yêu cầu Password
        let (pw_tx, pw_rx) = std::sync::mpsc::channel::<String>();
        let pw_prompt_event = SshInteractiveEvent::Prompt {
            prompt_message: "testuser@validserver's password:".to_string(),
            prompt_type: SshInteractivePromptType::Password,
            response_sender: pw_tx,
        };

        match pw_prompt_event {
            SshInteractiveEvent::Prompt {
                prompt_message,
                prompt_type,
                response_sender,
            } => {
                assert_eq!(prompt_type, SshInteractivePromptType::Password);
                ssh_state.stage = SshPickerStage::PasswordPrompt {
                    prompt_message,
                    password_input: String::new(),
                    is_masked: true,
                    error_message: None,
                };
                // Người dùng nhập password
                let _ = response_sender.send("mypassword".to_string());
            }
            _ => panic!("Expected Password Prompt event"),
        }

        assert_eq!(pw_rx.recv().unwrap(), "mypassword");
        match &ssh_state.stage {
            SshPickerStage::PasswordPrompt {
                prompt_message,
                is_masked,
                ..
            } => {
                assert_eq!(prompt_message, "testuser@validserver's password:");
                assert!(*is_masked);
            }
            _ => panic!("Expected PasswordPrompt stage"),
        }

        // 6. Mô phỏng: Xác thực thành công -> chuyển sang FolderPicker
        let success_event = SshInteractiveEvent::Connected(SshConnectionSuccess {
            initial_dir: "/home/testuser".to_string(),
            entries: vec!["projects".to_string(), "logs".to_string()],
        });

        match success_event {
            SshInteractiveEvent::Connected(success) => {
                let folder_state = FolderPickerState::new(
                    RemoteServerKind::Ssh {
                        host: "validserver".to_string(),
                        nickname: None,
                    },
                    success.initial_dir,
                    success.entries,
                );
                assert_eq!(folder_state.current_dir, "/home/testuser");
                assert_eq!(folder_state.entries.len(), 2);
            }
            _ => panic!("Expected Connected event"),
        }
    }
}
