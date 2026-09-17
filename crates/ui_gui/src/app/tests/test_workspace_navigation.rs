use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::overlay::OverlayLayer;
use uwu_core_workspace::{extract_project_name, SourceType, Workspace, WorkspaceLocation};

#[tokio::test]
async fn test_multi_project_switch_and_close() {
    let mut app = create_test_app();
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);

    app.active_session_mut().search.query = "level:error".to_string();

    let ws2 = Workspace::new(
        "Project B",
        WorkspaceLocation::local("D:\\test\\proj_b"),
        SourceType::Process,
    );
    app.open_or_switch_workspace(&ws2);

    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);
    assert_eq!(app.active_session().session.name, "Project B");
    assert_eq!(app.active_session().search.query, "");

    app.active_session_mut().search.query = "tag:Audio".to_string();

    app.switch_session(0);
    assert_eq!(app.workspaces.active_index, 0);
    assert_eq!(app.active_session().session.name, "Test Project");
    assert_eq!(app.active_session().search.query, "level:error");

    app.switch_session(1);
    assert_eq!(app.workspaces.active_index, 1);
    assert_eq!(app.active_session().session.name, "Project B");
    assert_eq!(app.active_session().search.query, "tag:Audio");

    app.close_session(1);
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);
    assert_eq!(app.active_session().session.name, "Test Project");
    assert_eq!(app.active_session().search.query, "level:error");
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

    // Đóng Remote Servers modal
    app.dispatch_action(AppAction::CloseRemoteServersModal);
    assert!(!app.is_overlay_open(OverlayLayer::RemoteServersModal));
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
    assert_eq!(app.active_session().session.source_config.capacity, 100);
    assert_eq!(app.active_session().session.display_limit, 50);

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

    // 3. Đóng hết các session -> tạo session mặc định và kế thừa capacity
    app.close_session(1);
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);

    app.close_session(0);
    assert_eq!(app.workspaces.sessions.len(), 1);
    assert_eq!(app.workspaces.active_index, 0);
    assert_eq!(app.active_session().session.source_config.capacity, 100);
    assert_eq!(app.active_session().session.display_limit, 50);
}

#[tokio::test]
async fn test_on_exit_saves_current_workspace() {
    use eframe::App;
    let mut app = create_test_app();
    app.workspaces.sessions[0].session.name = "ExitTestProj".to_string();
    app.workspaces.sessions[0].view.search.query = "error_query".to_string();

    app.on_exit();

    let ws = app
        .workspaces
        .store
        .recent_workspaces
        .iter()
        .find(|w| w.name == "ExitTestProj");
    assert!(ws.is_some());
    assert_eq!(ws.unwrap().last_query, "error_query");
}
