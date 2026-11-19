use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::app::WorkspaceManager;
use crate::overlay::{OverlayLayer, RemoteModalPlacement};
use crate::session::GuiSession;
use uwu_core_workspace::{
    extract_project_name, SourceType, Workspace, WorkspaceLocation, WorkspaceStore,
};

#[tokio::test]
async fn test_multi_project_switch_and_close() {
    let mut app = create_test_app();
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);

    app.active_session_mut().view.search.query = "level:error".to_string();

    let ws2 = Workspace::new(
        "Project B",
        WorkspaceLocation::local("D:\\test\\proj_b"),
        SourceType::Process,
    );
    app.open_or_switch_workspace(&ws2);

    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);
    assert_eq!(app.active_session().session.name, "Project B");
    assert_eq!(app.active_session().view.search.query, "");

    app.active_session_mut().view.search.query = "tag:Audio".to_string();

    app.switch_session(0);
    assert_eq!(app.workspaces.active_index, 0);
    assert_eq!(app.active_session().session.name, "Test Project");
    assert_eq!(app.active_session().view.search.query, "level:error");

    app.switch_session(1);
    assert_eq!(app.workspaces.active_index, 1);
    assert_eq!(app.active_session().session.name, "Project B");
    assert_eq!(app.active_session().view.search.query, "tag:Audio");

    app.close_session(1);
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);
    assert_eq!(app.active_session().session.name, "Test Project");
    assert_eq!(app.active_session().view.search.query, "level:error");
}

#[tokio::test]
async fn test_remote_workspace_save_and_reload() {
    let mut app = create_test_app();

    // 1. Cấu hình Remote workspace trong session hiện tại
    app.active_session_mut().session.location = WorkspaceLocation::remote(
        uwu_core_workspace::WslConnectionOptions::new("Ubuntu", "/home/user/backend"),
    );
    app.active_session_mut().session.source_config.source_type = SourceType::Process;
    app.active_session_mut().session.source_config.command_str = "python3 app.py".to_string();
    app.active_session_mut().session.name = "Remote-Backend".to_string();

    // 2. Lưu workspace hiện tại
    app.save_current_workspace();

    // 3. Kiểm tra xem workspace được lưu vào store đúng chưa
    let ws = app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .find(|w| w.name == "Remote-Backend")
        .cloned()
        .expect("Remote-Backend must be saved in store");

    assert_eq!(ws.source_type, SourceType::Process);
    assert_eq!(ws.command_str, "python3 app.py");
    let remote = ws.location.as_remote().expect("Expected remote location");
    assert_eq!(remote.display_name(), "Ubuntu");
    assert_eq!(remote.working_dir(), "/home/user/backend");

    // 4. Mở lại workspace Remote qua open_or_switch_workspace
    app.open_or_switch_workspace(&ws);
    assert_eq!(
        app.active_session().session.source_config.source_type,
        SourceType::Process
    );
    assert_eq!(
        app.active_session().session.source_config.command_str,
        "python3 app.py"
    );
    let remote = app
        .active_session()
        .session
        .location
        .as_remote()
        .expect("Expected remote location");
    assert_eq!(remote.display_name(), "Ubuntu");
    assert_eq!(remote.working_dir(), "/home/user/backend");
}

#[tokio::test]
async fn test_open_or_switch_workspace_slash_normalization() {
    let mut app = create_test_app();
    let ws_forward = Workspace::new(
        "test-slash",
        WorkspaceLocation::local("D:/Learn/Go/uwulog-rust"),
        SourceType::Process,
    );
    app.open_or_switch_workspace(&ws_forward);
    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);

    // Try opening the same folder with backslashes
    let ws_backward = Workspace::new(
        "test-slash-alt",
        WorkspaceLocation::local("D:\\Learn\\Go\\uwulog-rust\\"),
        SourceType::Process,
    );
    app.open_or_switch_workspace(&ws_backward);

    // Should NOT create a duplicate session, should switch to session index 1
    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);
}

#[test]
fn test_extract_project_name() {
    assert_eq!(
        extract_project_name("D:\\Learn\\Go\\uwulog-rust"),
        "uwulog-rust"
    );
    assert_eq!(
        extract_project_name("D:/Learn/Go/uwulog-rust/"),
        "uwulog-rust"
    );
    assert_eq!(extract_project_name("/home/user/backend"), "backend");
    assert_eq!(extract_project_name(""), "Workspace");
}

#[tokio::test]
async fn test_app_action_project_picker() {
    let mut app = create_test_app();

    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    app.dispatch_action(AppAction::ToggleProjectPicker);
    assert!(app.is_overlay_open(OverlayLayer::ProjectPicker));

    app.overlays.project_search_query = "search_test".to_string();
    app.dispatch_action(AppAction::CloseProjectPicker);
    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    assert!(app.overlays.project_search_query.is_empty());
}

#[tokio::test]
async fn test_app_action_remote_servers_modal() {
    let mut app = create_test_app();

    assert!(!app.is_overlay_open(OverlayLayer::RemoteServersModal));

    // Mở Project Picker trước
    app.dispatch_action(AppAction::ToggleProjectPicker);
    assert!(app.is_overlay_open(OverlayLayer::ProjectPicker));

    // Mở Remote Servers modal -> tự động đóng Project Picker và mở RemoteServersModal
    app.dispatch_action(AppAction::OpenRemoteServersModal);
    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    assert!(app.is_overlay_open(OverlayLayer::RemoteServersModal));
    assert_eq!(
        app.overlays.remote_placement,
        RemoteModalPlacement::TopCenter
    );

    // Đóng Remote Servers modal
    app.dispatch_action(AppAction::CloseRemoteServersModal);
    assert!(!app.is_overlay_open(OverlayLayer::RemoteServersModal));

    // Mở Remote Servers modal từ Server Picker trên header -> placement là TopLeft
    app.dispatch_action(AppAction::ToggleRemoteServersModal);
    assert!(app.is_overlay_open(OverlayLayer::RemoteServersModal));
    assert_eq!(app.overlays.remote_placement, RemoteModalPlacement::TopLeft);
}

#[tokio::test]
async fn test_remote_servers_modal_escape_back_navigation() {
    use crate::app::overlay_manager::RemoteServersOverlayState;
    use crate::views::remote_servers::render_remote_servers_modal;
    use crate::views::remote_servers::types::{
        FolderPickerState, RemoteModalState, RemoteSubView, ServerOptionsState,
    };
    use uwu_core_workspace::ServerConnection;

    let rt_handle = tokio::runtime::Handle::current();
    let ctx = egui::Context::default();
    let mut store = uwu_core_workspace::WorkspaceStore::default();
    let keymap = crate::keymap::KeymapManager::new();

    // 1. Khởi tạo state đang ở WslPicker (đã navigate từ List)
    let mut overlay_state = RemoteServersOverlayState::default();
    overlay_state.modal.navigate(RemoteSubView::WslPicker);

    // 2. Mô phỏng phím Escape khi đang ở WslPicker
    let mut esc_input = egui::RawInput::default();
    esc_input.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    });

    let mut closed_dispatched = false;
    let mut out1 = ctx.run_ui(esc_input, |ui| {
        render_remote_servers_modal(
            ui.ctx(),
            true,
            RemoteModalPlacement::TopCenter,
            &mut overlay_state,
            &rt_handle,
            &keymap,
            &mut store,
            None,
            &mut |action| {
                if matches!(action, AppAction::CloseRemoteServersModal) {
                    closed_dispatched = true;
                }
            },
        );
    });
    out1.textures_delta.clear();

    // Modal KHÔNG được đóng, mà phải quay về List!
    assert!(!closed_dispatched);
    assert_eq!(overlay_state.modal.subview, RemoteSubView::List);
    assert!(overlay_state.modal.history.is_empty());

    // 3. Khởi tạo state đang ở FolderPicker (đã navigate từ List)
    overlay_state.modal = RemoteModalState::default();
    overlay_state
        .modal
        .navigate(RemoteSubView::FolderPicker(FolderPickerState::new(
            ServerConnection::wsl("Ubuntu"),
            "/home",
            vec![],
        )));

    let mut esc_input = egui::RawInput::default();
    esc_input.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    });

    let mut closed_dispatched = false;
    let mut out2 = ctx.run_ui(esc_input, |ui| {
        render_remote_servers_modal(
            ui.ctx(),
            true,
            RemoteModalPlacement::TopCenter,
            &mut overlay_state,
            &rt_handle,
            &keymap,
            &mut store,
            None,
            &mut |action| {
                if matches!(action, AppAction::CloseRemoteServersModal) {
                    closed_dispatched = true;
                }
            },
        );
    });
    out2.textures_delta.clear();

    // Modal KHÔNG được đóng, mà phải quay về List!
    assert!(!closed_dispatched);
    assert_eq!(overlay_state.modal.subview, RemoteSubView::List);

    // 4. Khởi tạo state đang ở ServerOptions (đã navigate từ List)
    overlay_state.modal = RemoteModalState::default();
    overlay_state
        .modal
        .navigate(RemoteSubView::ServerOptions(ServerOptionsState {
            server: ServerConnection::wsl("Ubuntu"),
            selected_index: 0,
            copied_flash_time: None,
        }));

    let mut esc_input = egui::RawInput::default();
    esc_input.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    });

    let mut closed_dispatched = false;
    let mut out3 = ctx.run_ui(esc_input, |ui| {
        render_remote_servers_modal(
            ui.ctx(),
            true,
            RemoteModalPlacement::TopCenter,
            &mut overlay_state,
            &rt_handle,
            &keymap,
            &mut store,
            None,
            &mut |action| {
                if matches!(action, AppAction::CloseRemoteServersModal) {
                    closed_dispatched = true;
                }
            },
        );
    });
    out3.textures_delta.clear();

    // Modal KHÔNG được đóng, mà phải quay về List!
    assert!(!closed_dispatched);
    assert_eq!(overlay_state.modal.subview, RemoteSubView::List);

    // 5. Khi đang ở List (history rỗng), nhấn Escape -> phải dispatch CloseRemoteServersModal!
    let mut esc_input = egui::RawInput::default();
    esc_input.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    });

    let mut closed_dispatched = false;
    let mut out4 = ctx.run_ui(esc_input, |ui| {
        render_remote_servers_modal(
            ui.ctx(),
            true,
            RemoteModalPlacement::TopCenter,
            &mut overlay_state,
            &rt_handle,
            &keymap,
            &mut store,
            None,
            &mut |action| {
                if matches!(action, AppAction::CloseRemoteServersModal) {
                    closed_dispatched = true;
                }
            },
        );
    });
    out4.textures_delta.clear();

    assert!(closed_dispatched);
}

#[tokio::test]
async fn test_close_and_switch_session_behaviors() {
    let mut app = create_test_app();
    let ws1 = Workspace::new(
        "P1",
        WorkspaceLocation::local("D:\\test\\p1"),
        SourceType::Process,
    );
    let ws2 = Workspace::new(
        "P2",
        WorkspaceLocation::local("D:\\test\\p2"),
        SourceType::Process,
    );

    app.open_or_switch_workspace(&ws1);
    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);
    // Kiểm tra kế thừa capacity và display_limit từ active session
    assert_eq!(app.active_session().session.capacity(), 100);
    assert_eq!(app.active_session().session.display_limit(), 50);

    app.open_or_switch_workspace(&ws2);
    assert_eq!(app.workspaces.sessions.len(), 3);
    assert_eq!(app.workspaces.active_index, 2);

    // 1. switch_session vào chính index hiện tại là no-op
    app.switch_session(2);
    assert_eq!(app.workspaces.active_index, 2);

    // 2. close_session tab trước active_index sẽ giảm active_index đi 1
    app.close_session(0);
    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);
    assert_eq!(app.active_session().session.name, "P2");

    // 3. Đóng hết các session -> chuyển sang trạng thái 0 session (màn hình Welcome, không fallback current_dir)
    app.close_session(1);
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);

    app.close_session(0);
    assert_eq!(app.workspaces.sessions.len(), 0);
    assert!(!app.workspaces.has_active_session());
    assert!(app.workspaces.store.active_workspace_id.is_none());
    assert!(app.workspaces.store.open_workspace_ids.is_empty());
}

#[tokio::test]
async fn test_on_exit_saves_current_workspace() {
    use eframe::App;
    let mut app = create_test_app();
    app.workspaces.sessions[0].session.name = "ExitTestProj".to_string();
    app.workspaces.sessions[0].view.search.query = "error_query".to_string();
    app.workspaces.sessions[0]
        .view
        .search
        .history
        .record("error_query");

    app.on_exit();

    let ws = app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .find(|w| w.name == "ExitTestProj")
        .cloned();
    assert!(ws.is_some());
    let ws = ws.unwrap();
    let key = ws.viewer_key();
    let prefs = app
        .workspaces
        .store
        .get_viewer_prefs(&key)
        .expect("saved viewer prefs must exist");
    assert_eq!(prefs.filter_history, vec!["error_query"]);

    // Mở lại session từ ViewerPrefs đã lưu -> KHÔNG dính lại filter cũ trong ô search
    let mut reopened = GuiSession::from_workspace(&ws, 1000, 100);
    reopened.view.search.query = "stale_query".to_string();
    reopened.apply_viewer_prefs(prefs);
    assert!(reopened.view.search.query.is_empty());
    assert!(reopened.view.search.last_query.is_empty());
    assert_eq!(reopened.view.search.history.entries, vec!["error_query"]);
}

#[tokio::test]
async fn test_workspace_persists_and_restores_columns_only_on_matching_keys() {
    use eframe::App;
    use std::collections::HashMap;
    use uwu_core_schema::{LogColor, LogEvent};

    let temp_dir =
        std::env::temp_dir().join(format!("uwulog_cols_persist_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let store_path = temp_dir.join("workspaces.json");

    let mut app = create_test_app();
    app.workspaces.store.set_storage_path(&store_path);
    app.workspaces.sessions[0].session.name = "ColsPersistProj".to_string();
    app.workspaces.sessions[0].sync_discovered_fields(&[LogEvent::new(
        "2026-11-18T00:00:00Z",
        LogColor::Green,
        "m1",
        HashMap::from([
            ("aaa".to_string(), serde_json::json!("x")),
            ("hidden_col".to_string(), serde_json::json!("y")),
        ]),
    )]);
    app.active_session_mut().view.shared.columns.open_modal();
    if let Some(ref mut draft) = app.active_session_mut().view.shared.columns.draft {
        if let Some(c) = draft.columns.iter_mut().find(|c| c.name == "aaa") {
            c.visible = true;
            c.width = 240.0;
        }
    }
    app.dispatch_action(AppAction::ApplyColumnsModal);
    app.on_exit();

    // Đọc lại từ JSON trên đĩa -> mở WorkspaceSession mới và nạp ViewerPrefs theo ViewerKey
    let reloaded_store = uwu_core_workspace::WorkspaceStore::load_from_path(&store_path);
    let saved_ws = reloaded_store
        .recent_workspaces
        .iter()
        .find(|w| w.name == "ColsPersistProj")
        .expect("saved workspace must exist")
        .clone();
    let saved_prefs = reloaded_store
        .get_viewer_prefs(&saved_ws.viewer_key())
        .expect("saved viewer prefs must exist");
    assert!(!saved_prefs.columns.iter().any(|c| c.name == "hidden_col"));

    let mut restored_session = GuiSession::from_workspace(&saved_ws, 1000, 100);
    restored_session.apply_viewer_prefs(saved_prefs);
    // Nếu log mới không có "aaa" (chỉ có "bbb") -> không hiện cột ma "aaa", "bbb" dùng mặc định 120.0
    restored_session.sync_discovered_fields(&[LogEvent::new(
        "2026-11-18T00:00:01Z",
        LogColor::Green,
        "m2",
        HashMap::from([("bbb".to_string(), serde_json::json!("only_b"))]),
    )]);
    assert!(!restored_session
        .view
        .shared
        .columns
        .columns
        .iter()
        .any(|c| c.name == "aaa"));
    let bbb = restored_session
        .view
        .shared
        .columns
        .columns
        .iter()
        .find(|c| c.name == "bbb")
        .unwrap();
    assert!(!bbb.visible);
    assert!((bbb.width - 120.0).abs() < 0.01);

    // Khi log xuất hiện đúng key "aaa" -> khôi phục lại visible: true và width 240.0
    restored_session.sync_discovered_fields(&[LogEvent::new(
        "2026-11-18T00:00:02Z",
        LogColor::Green,
        "m3",
        HashMap::from([("aaa".to_string(), serde_json::json!("matched"))]),
    )]);
    let aaa = restored_session
        .view
        .shared
        .columns
        .columns
        .iter()
        .find(|c| c.name == "aaa")
        .unwrap();
    assert!(aaa.visible);
    assert!((aaa.width - 240.0).abs() < 0.01);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
#[cfg(target_os = "windows")]
async fn test_project_picker_and_server_list_consistency() {
    use uwu_core_workspace::ServerConnection;

    let mut app = create_test_app();

    // 1. Thêm một project trực tiếp vào server connection (wsl_connections)
    app.workspaces
        .store
        .add_remote_project_to_server("Ubuntu", "/home/user/backend");

    // Server list có project này
    let server_items = crate::views::remote_servers::server_list::collect_server_list_items(
        "",
        &app.workspaces.store,
    );
    assert!(server_items
        .iter()
        .any(|it| it.label == "/home/user/backend"));

    // 2. Mở project này qua open_or_switch_workspace (như khi chọn từ server list)
    let ws = crate::views::remote_servers::helpers::create_server_workspace(
        &ServerConnection::wsl("Ubuntu"),
        "/home/user/backend",
    );
    app.open_or_switch_workspace(&ws);

    // Kiểm tra đã được lưu ngay vào recent_workspaces
    assert!(app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .any(|w| w.location.working_dir() == "/home/user/backend"));

    // 3. Đóng session này
    let active_idx = app.workspaces.active_index;
    app.close_session(active_idx);

    // Project vẫn còn lưu trong recent_workspaces sau khi đóng session
    let saved_ws = app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .find(|w| w.location.working_dir() == "/home/user/backend")
        .cloned();
    assert!(saved_ws.is_some());

    // 4. Xóa project khỏi store (như khi bấm X trong Project Picker)
    let ws_id = saved_ws.unwrap().id;
    app.workspaces.store.remove(ws_id);

    // Đảm bảo đã đồng bộ xóa khỏi cả recent_workspaces lẫn wsl_connections
    assert!(!app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .any(|w| w.location.working_dir() == "/home/user/backend"));

    let server_items_after = crate::views::remote_servers::server_list::collect_server_list_items(
        "",
        &app.workspaces.store,
    );
    assert!(!server_items_after
        .iter()
        .any(|it| it.label == "/home/user/backend"));
}

#[tokio::test]
async fn test_start_at_shutdown_save_and_restore() {
    use eframe::App;
    let mut app = create_test_app();
    let temp_dir1 = std::env::temp_dir().join(format!("uwu_test_dir1_{}", uuid::Uuid::new_v4()));
    let temp_dir2 = std::env::temp_dir().join(format!("uwu_test_dir2_{}", uuid::Uuid::new_v4()));
    let _ = std::fs::create_dir_all(&temp_dir1);
    let _ = std::fs::create_dir_all(&temp_dir2);

    let ws1 = Workspace::new(
        "Project-1",
        WorkspaceLocation::local(temp_dir1.to_string_lossy().to_string()),
        SourceType::Process,
    );
    let ws2 = Workspace::new(
        "Project-2",
        WorkspaceLocation::local(temp_dir2.to_string_lossy().to_string()),
        SourceType::Process,
    );
    let id1 = ws1.id;
    let id2 = ws2.id;

    // Đóng session mặc định ban đầu
    app.close_session(0);
    assert_eq!(app.workspaces.sessions.len(), 0);

    // Mở 2 projects
    app.open_or_switch_workspace(&ws1);
    app.open_or_switch_workspace(&ws2);
    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);

    // Shutdown / exit
    app.on_exit();

    let store = &app.workspaces.store;
    assert_eq!(store.open_workspace_ids.len(), 2);
    assert_eq!(store.open_workspace_ids[0], id1);
    assert_eq!(store.open_workspace_ids[1], id2);
    assert_eq!(store.active_workspace_id, Some(id2));

    // Khôi phục lại từ store như lúc khởi chạy ứng dụng không có CLI target
    let rt = tokio::runtime::Handle::current();
    let mut restored_sessions = Vec::new();
    let mut active_index = 0;
    for id in &store.open_workspace_ids {
        if let Some(ws) = store
            .recent_workspaces
            .iter()
            .find(|w| w.id == *id)
            .cloned()
        {
            if ws.is_valid_to_open() {
                let mut gui_session = GuiSession::from_workspace(&ws, 100, 50);
                gui_session.session.spawn_load_environment(&rt);
                restored_sessions.push(gui_session);
            }
        }
    }
    if !restored_sessions.is_empty() {
        if let Some(act_id) = store.active_workspace_id {
            if let Some(pos) = restored_sessions
                .iter()
                .position(|s| s.session.id == act_id)
            {
                active_index = pos;
            }
        }
    }
    let restored_mgr =
        WorkspaceManager::with_sessions(restored_sessions, active_index, store.clone());

    assert_eq!(restored_mgr.sessions.len(), 2);
    assert_eq!(restored_mgr.active_index, 1);
    assert_eq!(restored_mgr.sessions[0].session.name, "Project-1");
    assert_eq!(restored_mgr.sessions[1].session.name, "Project-2");

    let _ = std::fs::remove_dir_all(&temp_dir1);
    let _ = std::fs::remove_dir_all(&temp_dir2);
}

#[tokio::test]
async fn test_reopen_failure_keeps_in_recent() {
    let mut store = WorkspaceStore::default();
    let dead_path = "/non/existent/path/uwulog/dead_project_12345";
    let ws = Workspace::new(
        "DeadProject",
        WorkspaceLocation::local(dead_path),
        SourceType::Process,
    );
    let ws_id = ws.id;
    store.add_or_update(ws);
    store.open_workspace_ids = vec![ws_id];
    store.active_workspace_id = Some(ws_id);

    // Xác nhận is_valid_to_open trả về false vì đường dẫn không tồn tại
    let saved_ws = store
        .recent_workspaces
        .iter()
        .find(|w| w.id == ws_id)
        .unwrap();
    assert!(!saved_ws.is_valid_to_open());

    // Khởi chạy khôi phục
    let rt = tokio::runtime::Handle::current();
    let mut sessions = Vec::new();
    for id in &store.open_workspace_ids {
        if let Some(w) = store
            .recent_workspaces
            .iter()
            .find(|w| w.id == *id)
            .cloned()
        {
            if w.is_valid_to_open() {
                let mut gui_session = GuiSession::from_workspace(&w, 100, 50);
                gui_session.session.spawn_load_environment(&rt);
                sessions.push(gui_session);
            }
            // Không mở lại nhưng không xóa khỏi store
        }
    }

    // Sessions rỗng vì mở lại fail
    assert_eq!(sessions.len(), 0);

    // VẪN CÒN trong recent_workspaces!
    assert!(store.recent_workspaces.iter().any(|w| w.id == ws_id));
}

#[tokio::test]
async fn test_no_project_open_does_not_use_current_dir() {
    let mut store = WorkspaceStore::default();
    store.open_workspace_ids.clear();
    store.active_workspace_id = None;

    let mgr = WorkspaceManager::with_sessions(Vec::new(), 0, store);
    assert!(!mgr.has_active_session());
    assert_eq!(mgr.sessions.len(), 0);
    assert!(mgr.active_session_opt().is_none());

    // Đảm bảo không có project nào trỏ vào current_dir
    let current_dir = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(!mgr
        .store
        .recent_workspaces
        .iter()
        .any(|w| w.location.working_dir() == current_dir));
}

#[tokio::test]
async fn test_open_normal_workspace_does_not_auto_run_while_container_log_tab_streams() {
    let mut app = create_test_app();

    // 1. Dự án bình thường (File / Run Command) khi mở qua OpenWorkspace:
    // KHÔNG tự động chạy lệnh/nguồn, nhưng trạng thái hiển thị vẫn ở Live (is_auto_scroll = true)
    let log_dir = std::env::temp_dir();
    let log_file = log_dir.join(format!("uwu_test_normal_{}.log", uuid::Uuid::new_v4()));
    std::fs::write(&log_file, "hello log\n").unwrap();
    let mut file_ws = Workspace::new(
        "local-log",
        WorkspaceLocation::local(log_dir.to_string_lossy().to_string()),
        SourceType::File,
    );
    file_ws.file_path = log_file.to_string_lossy().to_string();
    app.dispatch_action(AppAction::OpenWorkspace(Box::new(file_ws)));

    assert!(!app.active_session().has_auto_started_source);
    assert!(!app.active_session().session.is_source_running);
    assert!(app.active_session().view.main_tab().stream.is_auto_scroll);

    // 2. App Container (Dev Container chính mở dự án): cũng KHÔNG tự động chạy
    let app_container_ws = Workspace::new(
        "psittacus",
        WorkspaceLocation::remote(
            uwu_core_workspace::DockerConnectionOptions::new("psittacus-app-1", "/workspaces/p")
                .with_compose_project("psittacus_devcontainer")
                .with_compose_service("app")
                .with_devcontainer(true),
        ),
        SourceType::Process,
    );
    app.dispatch_action(AppAction::OpenWorkspace(Box::new(app_container_ws)));

    assert!(!app.active_session().session.is_following_container_logs());
    assert!(!app.active_session().has_auto_started_source);
    assert!(!app.active_session().session.is_source_running);
    assert!(app.active_session().view.main_tab().stream.is_auto_scroll);

    // 3. Mở container log tab khác (vd: `db` với follow_container_logs = true):
    // Tự động lấy log từ Docker và bật Live
    let svc_container_ws = Workspace::new(
        "psittacus",
        WorkspaceLocation::remote(
            uwu_core_workspace::DockerConnectionOptions::new("psittacus-db-1", "/workspaces/p")
                .with_compose_project("psittacus_devcontainer")
                .with_compose_service("db")
                .with_follow_container_logs(true),
        ),
        SourceType::Process,
    );
    app.dispatch_action(AppAction::OpenContainerLogSession(Box::new(
        svc_container_ws,
    )));

    assert!(app.active_session().session.is_following_container_logs());
    assert!(app.active_session().has_auto_started_source);
    assert!(app.active_session().session.is_source_running);
    assert!(app.active_session().view.main_tab().stream.is_auto_scroll);

    // Tab container phụ (`follow_container_logs`) không được lưu trùng vào recent_workspaces / open_workspace_ids
    app.workspaces.save_all_open_workspaces();
    let docker_recent_count = app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .filter(|w| w.name == "psittacus")
        .count();
    assert_eq!(docker_recent_count, 1);

    // 4. Nếu user bấm Stop thủ công rồi chuyển tab qua lại -> không tự khởi chạy đè
    let svc_idx = app.workspaces.active_index;
    app.dispatch_action(AppAction::StopSource);
    assert!(app.active_session().user_stopped_source);
    assert!(!app.active_session().session.is_source_running);

    app.dispatch_action(AppAction::SwitchSession(svc_idx - 1));
    app.dispatch_action(AppAction::SwitchSession(svc_idx));
    assert!(!app.active_session().session.is_source_running);

    let _ = std::fs::remove_file(&log_file);
}

#[tokio::test]
async fn test_devcontainer_tab_switching_and_reopen_preserves_state() {
    let mut app = create_test_app();
    let storage_path =
        std::env::temp_dir().join(format!("uwu_test_devc_state_{}.json", uuid::Uuid::new_v4()));
    app.workspaces.store.set_storage_path(&storage_path);

    let app_ws = Workspace::new(
        "psittacus",
        WorkspaceLocation::remote(
            uwu_core_workspace::DockerConnectionOptions::new("psittacus-app-1", "/workspaces/p")
                .with_compose_project("psittacus_devcontainer")
                .with_compose_service("app")
                .with_devcontainer(true),
        ),
        SourceType::Process,
    );
    app.dispatch_action(AppAction::OpenWorkspace(Box::new(app_ws)));
    let app_idx = app.workspaces.active_index;

    let make_db_ws = || {
        Workspace::new(
            "psittacus",
            WorkspaceLocation::remote(
                uwu_core_workspace::DockerConnectionOptions::new("psittacus-db-1", "/workspaces/p")
                    .with_compose_project("psittacus_devcontainer")
                    .with_compose_service("db")
                    .with_follow_container_logs(true),
            ),
            SourceType::Process,
        )
    };
    app.dispatch_action(AppAction::OpenContainerLogSession(Box::new(make_db_ws())));

    let db_idx = app.workspaces.active_index;
    assert_ne!(app_idx, db_idx);

    // Chỉnh trạng thái riêng trên tab `db`: cột, query, inspector, unlatch
    {
        let db = app.active_session_mut();
        db.view.shared.columns.set_column_width("timestamp", 185.0);
        db.view.shared.columns.schedule_save_after_drag();
        db.view.search.query = "db_slow".into();
        db.view.search.last_query = "db_slow".into();
        db.view.search.needs_search = false;
        db.view.shared.inspector.width_ratio = 0.28;
        let main = db.view.main_tab_mut();
        main.stream.is_auto_scroll = false;
        main.set_last_processed_count(1);
        main.stream.cached_logs.push(uwu_core_schema::LogEvent::new(
            "12:00:00",
            uwu_core_schema::LogColor::Default,
            "db line",
            uwu_core_schema::LogFields::default(),
        ));
    }

    // Chuyển sang `app`, chỉnh trạng thái riêng trên `app`
    app.dispatch_action(AppAction::SwitchSession(app_idx));
    {
        let main_app = app.active_session_mut();
        main_app
            .view
            .shared
            .columns
            .set_column_width("timestamp", 245.0);
        main_app.view.shared.columns.schedule_save_after_drag();
        main_app.view.search.query = "level:error".into();
        main_app.view.search.last_query = "level:error".into();
        main_app.view.search.needs_search = false;
        main_app.view.shared.inspector.width_ratio = 0.42;
        let main = main_app.view.main_tab_mut();
        main.stream.is_auto_scroll = false;
        main.set_last_processed_count(1);
        main.stream.cached_logs.push(uwu_core_schema::LogEvent::new(
            "12:00:01",
            uwu_core_schema::LogColor::Default,
            "app line",
            uwu_core_schema::LogFields::default(),
        ));
    }

    // Đổi qua lại giữa `db` và `app`: cả hai phải giữ nguyên độ rộng cột, query, inspector, cache
    app.dispatch_action(AppAction::SwitchSession(db_idx));
    assert!(
        (app.active_session().view.shared.columns.columns[0].width - 185.0).abs() < 0.1,
        "db timestamp width lost"
    );
    assert_eq!(app.active_session().view.search.query, "db_slow");
    assert!((app.active_session().view.shared.inspector.width_ratio - 0.28).abs() < 0.01);
    assert!(!app.active_session().view.main_tab().stream.is_auto_scroll);
    assert!(!app.active_session().view.search.needs_search);
    assert_eq!(
        app.active_session()
            .view
            .main_tab()
            .stream
            .cached_logs
            .len(),
        1
    );

    app.dispatch_action(AppAction::SwitchSession(app_idx));
    assert!(
        (app.active_session().view.shared.columns.columns[0].width - 245.0).abs() < 0.1,
        "app timestamp width lost"
    );
    assert_eq!(app.active_session().view.search.query, "level:error");
    assert!((app.active_session().view.shared.inspector.width_ratio - 0.42).abs() < 0.01);
    assert!(!app.active_session().view.main_tab().stream.is_auto_scroll);
    assert!(!app.active_session().view.search.needs_search);
    assert_eq!(
        app.active_session()
            .view
            .main_tab()
            .stream
            .cached_logs
            .len(),
        1
    );

    // Đóng tab `db`, reload store từ đĩa (mô phỏng tắt app mở lại) và mở lại `db`:
    // không tạo duplicate trong `recent_workspaces`, khôi phục layout cột & inspector nhưng KHÔNG dính lại filter cũ.
    app.dispatch_action(AppAction::CloseSession(db_idx));
    let docker_recent_count = app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .filter(|w| w.name == "psittacus")
        .count();
    assert_eq!(docker_recent_count, 1);

    app.workspaces.store = WorkspaceStore::load_from_path(&storage_path);
    app.dispatch_action(AppAction::OpenContainerLogSession(Box::new(make_db_ws())));
    assert!(
        (app.active_session().view.shared.columns.columns[0].width - 185.0).abs() < 0.1,
        "reopened db tab should restore saved timestamp width"
    );
    assert_eq!(app.active_session().view.search.query, "");
    assert!((app.active_session().view.shared.inspector.width_ratio - 0.28).abs() < 0.01);

    app.dispatch_action(AppAction::CloseSession(db_idx));
    app.dispatch_action(AppAction::CloseSession(app_idx));
    let _ = std::fs::remove_file(&storage_path);
}
