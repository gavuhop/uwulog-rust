use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::overlay::OverlayLayer;
use crate::state::ActiveTab;
use std::collections::HashMap;
use uwu_core_schema::{LogColor, LogEvent};
use uwu_core_workspace::{SourceType, Workspace, WorkspaceLocation};

#[tokio::test]
async fn test_zed_style_draft_isolation_and_cancel() {
    let mut app = create_test_app();
    app.active_session_mut().session.source_config.source_type = SourceType::Process;
    app.active_session_mut().session.source_config.command_str = "cargo run".to_string();

    // Open launch modal creates draft from live session
    app.dispatch_action(AppAction::OpenLaunchModal);
    assert!(app.is_overlay_open(OverlayLayer::LaunchModal));
    assert!(app.overlays.launch_modal_draft.is_some());

    // Modify draft in form (e.g. user toggles to File source and types a path)
    if let Some(ref mut draft) = app.overlays.launch_modal_draft {
        draft.source_type = SourceType::File;
        draft.file_path = "/var/log/test.log".to_string();
    }

    // Live session must remain completely untouched while modal is uncommitted
    assert_eq!(
        app.active_session().session.source_config.source_type,
        SourceType::Process
    );
    assert_eq!(
        app.active_session().session.source_config.command_str,
        "cargo run"
    );
    assert_eq!(app.active_session().session.source_config.file_path, "");

    // User cancels modal
    app.dispatch_action(AppAction::CloseLaunchModal);
    assert!(!app.is_overlay_open(OverlayLayer::LaunchModal));
    assert!(app.overlays.launch_modal_draft.is_none());

    // Live session remains intact
    assert_eq!(
        app.active_session().session.source_config.source_type,
        SourceType::Process
    );
    assert_eq!(
        app.active_session().session.source_config.command_str,
        "cargo run"
    );
}

#[tokio::test]
async fn test_zed_style_draft_apply() {
    let mut app = create_test_app();
    app.active_session_mut().session.source_config.source_type = SourceType::Process;
    app.active_session_mut().session.source_config.command_str = "cargo run".to_string();

    // Open modal
    app.dispatch_action(AppAction::OpenLaunchModal);
    if let Some(ref mut draft) = app.overlays.launch_modal_draft {
        draft.source_type = SourceType::File;
        draft.file_path = "C:\\logs\\app.log".to_string();
        draft.capacity = 50_000;
    }

    // Apply
    app.dispatch_action(AppAction::ApplyLaunchModal);
    assert!(!app.is_overlay_open(OverlayLayer::LaunchModal));
    assert!(app.overlays.launch_modal_draft.is_none());

    // Live session has received the new config
    assert_eq!(
        app.active_session().session.source_config.source_type,
        SourceType::File
    );
    assert_eq!(
        app.active_session().session.source_config.file_path,
        "C:\\logs\\app.log"
    );
    assert_eq!(app.active_session().session.source_config.capacity, 50_000);
}

#[tokio::test]
async fn test_zed_style_app_action_dispatch() {
    let mut app = create_test_app();
    let ws1 = Workspace::new(
        "Service A",
        WorkspaceLocation::local("C:\\projects\\service_a"),
        SourceType::Process,
    );
    let ws2 = Workspace::new(
        "Service B",
        WorkspaceLocation::local("C:\\projects\\service_b"),
        SourceType::Process,
    );

    app.dispatch_action(AppAction::OpenWorkspace(ws1));
    assert_eq!(app.workspaces.sessions.len(), 2);
    assert_eq!(app.workspaces.active_index, 1);

    app.dispatch_action(AppAction::OpenWorkspace(ws2));
    assert_eq!(app.workspaces.sessions.len(), 3);
    assert_eq!(app.workspaces.active_index, 2);

    // Cycle backwards
    app.dispatch_action(AppAction::CycleSession(false));
    assert_eq!(app.workspaces.active_index, 1);

    // Cycle forwards
    app.dispatch_action(AppAction::CycleSession(true));
    assert_eq!(app.workspaces.active_index, 2);

    // Switch directly
    app.dispatch_action(AppAction::SwitchSession(0));
    assert_eq!(app.workspaces.active_index, 0);

    // Close session
    app.dispatch_action(AppAction::CloseSession(1));
    assert_eq!(app.workspaces.sessions.len(), 2);
}

#[tokio::test]
async fn test_app_action_columns_modal_flow() {
    let mut app = create_test_app();
    let initial_cols = app.active_session().columns.columns.clone();

    // 1. Open columns modal
    app.dispatch_action(AppAction::OpenColumnsModal);
    assert!(app.active_session().columns.is_modal_open);
    assert!(app.active_session().columns.draft_columns.is_some());

    // 2. Modify draft
    if let Some(ref mut draft) = app.active_session_mut().columns.draft_columns {
        draft[0].visible = false;
    }

    // Live columns should still be unchanged (Live vs Draft separation)
    assert_eq!(app.active_session().columns.columns, initial_cols);
    assert!(app.active_session().columns.columns[0].visible);

    // 3. Cancel / Close modal
    app.dispatch_action(AppAction::CloseColumnsModal);
    assert!(!app.active_session().columns.is_modal_open);
    assert!(app.active_session().columns.draft_columns.is_none());
    assert_eq!(app.active_session().columns.columns, initial_cols);

    // 4. Open again, modify and Apply
    app.dispatch_action(AppAction::OpenColumnsModal);
    if let Some(ref mut draft) = app.active_session_mut().columns.draft_columns {
        draft[0].visible = false;
    }
    app.dispatch_action(AppAction::ApplyColumnsModal);
    assert!(!app.active_session().columns.is_modal_open);
    assert!(!app.active_session().columns.columns[0].visible);
}

#[tokio::test]
async fn test_columns_all_hidden_safe_and_reset() {
    let mut app = create_test_app();

    // 1. Open modal and hide ALL columns
    app.dispatch_action(AppAction::OpenColumnsModal);
    if let Some(ref mut draft) = app.active_session_mut().columns.draft_columns {
        for col in draft.iter_mut() {
            col.visible = false;
        }
    }
    assert!(app
        .active_session()
        .columns
        .draft_columns
        .as_ref()
        .unwrap()
        .iter()
        .all(|c| !c.visible));

    // 2. Apply all hidden columns
    app.dispatch_action(AppAction::ApplyColumnsModal);
    assert!(!app.active_session().columns.is_modal_open);
    assert!(app
        .active_session()
        .columns
        .columns
        .iter()
        .all(|c| !c.visible));

    // 3. Reset to defaults restores visibility
    app.active_session_mut().columns.reset_to_defaults();
    let visible_count = app
        .active_session()
        .columns
        .columns
        .iter()
        .filter(|c| c.visible)
        .count();
    assert!(visible_count >= 3);
}

#[tokio::test]
async fn test_app_action_select_log_and_switch_tab() {
    let mut app = create_test_app();

    let event = LogEvent::new(
        "2026-08-20T10:00:00Z",
        uwu_core_schema::LogColor::Green,
        "test message",
        std::collections::HashMap::new(),
    );

    app.dispatch_action(AppAction::SelectLog(Some(event.clone())));
    assert!(app.active_session().inspector.selected_log.is_some());
    assert_eq!(
        app.active_session()
            .inspector
            .selected_log
            .as_ref()
            .unwrap()
            .message,
        "test message"
    );

    app.dispatch_action(AppAction::SelectLog(None));
    assert!(app.active_session().inspector.selected_log.is_none());

    // Không có filter -> không thể đổi sang tab Unfiltered
    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Unfiltered));
    assert_eq!(app.active_session().active_tab, ActiveTab::Filtered);

    // Có filter -> đổi tab bình thường
    app.active_session_mut().view.search.query = "error".to_string();
    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Unfiltered));
    assert_eq!(app.active_session().active_tab, ActiveTab::Unfiltered);

    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Filtered));
    assert_eq!(app.active_session().active_tab, ActiveTab::Filtered);
}

#[tokio::test]
async fn test_app_action_dismiss_top_layer() {
    let mut app = create_test_app();

    // 1. When selected_log is present
    let test_log = LogEvent::new(
        "2026-09-10 12:00:00",
        LogColor::Green,
        "msg",
        HashMap::new(),
    )
    .with_id(123);
    app.active_session_mut().inspector.selected_log = Some(test_log);
    assert!(app.active_session().inspector.selected_log.is_some());

    // 2. Add unfiltered stream on top
    app.active_session_mut().open_unfiltered_stream(None);
    assert_eq!(app.active_session().active_tab, ActiveTab::Unfiltered);

    // 3. Add launch modal on top (stack)
    app.push_overlay(OverlayLayer::LaunchModal);

    // 4. Add columns modal on top (stack)
    app.active_session_mut().columns.open_modal();
    app.push_overlay(OverlayLayer::ColumnsModal);

    // 5. Add project picker on top (stack)
    app.push_overlay(OverlayLayer::ProjectPicker);

    // Popping order verification:
    // Pop 1: Project picker
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    assert!(app.is_overlay_open(OverlayLayer::ColumnsModal));

    // Pop 2: Columns modal
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(!app.is_overlay_open(OverlayLayer::ColumnsModal));
    assert!(app.is_overlay_open(OverlayLayer::LaunchModal));

    // Pop 3: Launch modal
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(!app.is_overlay_open(OverlayLayer::LaunchModal));
    assert_eq!(app.active_session().active_tab, ActiveTab::Unfiltered);

    // Pop 4: Unfiltered stream tab
    app.dispatch_action(AppAction::DismissTopLayer);
    assert_eq!(app.active_session().active_tab, ActiveTab::Filtered);
    assert!(app.active_session().inspector.selected_log.is_some());

    // Pop 5: Selected log inspector
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(app.active_session().inspector.selected_log.is_none());

    // Pop 6: Nothing left to pop
    assert!(!app.dismiss_top_layer());
}
