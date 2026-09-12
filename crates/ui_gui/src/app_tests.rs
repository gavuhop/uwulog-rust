use super::*;
use crate::ui::autocomplete::{SuggestionItem, SuggestionKind};
use std::collections::HashMap;
use uwu_core_schema::{LogColor, LogEvent, RawLogEntry, RawPayload};
use uwu_core_workspace::WorkspaceLocation;

fn create_test_app() -> UwuGuiApp {
    let rt = tokio::runtime::Handle::current();

    let source_config = SourceConfig {
        source_type: SourceType::Process,
        command_str: String::new(),
        file_path: String::new(),
        working_dir: String::new(),
        capacity: 100,
        display_limit: 50,
    };

    let session = WorkspaceSession::new(
        "Test Project".to_string(),
        WorkspaceLocation::local(""),
        source_config,
    );

    let gui_session = GuiSession::new(session);
    let store = WorkspaceStore::default();

    UwuGuiApp {
        sessions: vec![gui_session],
        active_index: 0,
        store,
        rt,
        launch_modal_draft: None,
        project_search_query: String::new(),
        prev_screen_width: 0.0,
        overlay_stack: OverlayStack::new(),
        should_quit: false,
    }
}

#[tokio::test]
async fn test_latch_toggle() {
    let mut app = create_test_app();
    assert!(app.viewport.is_auto_scroll);

    app.unlatch();
    assert!(!app.viewport.is_auto_scroll);

    app.latch();
    assert!(app.viewport.is_auto_scroll);
    assert!(app.viewport.request_scroll_to_bottom);

    app.toggle_latch();
    assert!(!app.viewport.is_auto_scroll);

    app.toggle_latch();
    assert!(app.viewport.is_auto_scroll);
}

#[tokio::test]
async fn test_highlight_toggle_and_clear() {
    let mut app = create_test_app();
    let id1 = 1001u64;
    let id2 = 1002u64;

    assert!(!app.is_row_highlighted(&id1));
    assert!(!app.has_any_highlights());

    app.toggle_row_highlight(id1);
    assert!(app.is_row_highlighted(&id1));
    assert!(app.has_any_highlights());

    app.toggle_row_highlight(id2);
    assert!(app.is_row_highlighted(&id2));

    // Term highlight
    assert!(!app.is_term_highlighted("timeout"));
    app.toggle_term_highlight("timeout");
    assert!(app.is_term_highlighted("timeout"));
    assert!(app.is_term_highlighted("TIMEOUT")); // Case-insensitive
    assert!(app.has_any_highlights());

    app.toggle_term_highlight("timeout");
    assert!(!app.is_term_highlighted("timeout"));

    // Clear all
    app.toggle_term_highlight("error");
    app.clear_all_highlights();
    assert!(!app.has_any_highlights());
    assert!(!app.is_row_highlighted(&id1));
    assert!(!app.is_term_highlighted("error"));
}

#[tokio::test]
async fn test_format_field_and_selection_term() {
    assert_eq!(
        UwuGuiApp::format_field_term("level", "ERROR"),
        "level:error"
    );
    assert_eq!(
        UwuGuiApp::format_field_term("source", "auth-service"),
        "source:auth-service"
    );
    assert_eq!(
        UwuGuiApp::format_field_term("message", "connection refused"),
        "message:\"connection refused\""
    );

    assert_eq!(
        UwuGuiApp::format_selection_term("connection refused"),
        "\"connection refused\""
    );
    assert_eq!(UwuGuiApp::format_selection_term("simple"), "simple");
}

#[tokio::test]
async fn test_filter_and_exclude_term() {
    let mut app = create_test_app();

    app.apply_filter_term("level:error");
    assert_eq!(app.search.query, "level:error");

    app.apply_filter_term("tag:Auth");
    assert_eq!(app.search.query, "level:error tag:Auth");

    app.exclude_filter_term("healthcheck");
    assert_eq!(app.search.query, "level:error tag:Auth -healthcheck");

    app.exclude_filter_term("-already_negated");
    assert_eq!(
        app.search.query,
        "level:error tag:Auth -healthcheck -already_negated"
    );
}

#[tokio::test]
async fn test_apply_autocomplete_suggestion() {
    let mut app = create_test_app();

    let item_key = SuggestionItem {
        kind: SuggestionKind::Key,
        op_symbol: "",
        action_name: "level".to_string(),
        example_syntax: "level:".to_string(),
        insert_text: "level:".to_string(),
    };
    app.search.autocomplete.active_token_range = (0, 0);
    app.apply_autocomplete_suggestion(&item_key);
    assert_eq!(app.search.query, "level:");

    let item_val = SuggestionItem {
        kind: SuggestionKind::OperatorOrValue,
        op_symbol: "",
        action_name: "error".to_string(),
        example_syntax: "error".to_string(),
        insert_text: "error".to_string(),
    };
    app.search.autocomplete.active_token_range = (6, 6);
    app.apply_autocomplete_suggestion(&item_val);
    assert_eq!(app.search.query, "level:error");
    assert!(!app.search.autocomplete.is_open);
}

#[tokio::test]
async fn test_get_available_log_fields_inference() {
    let app = create_test_app();
    let fields = app.get_available_log_fields();
    assert!(!fields.is_empty());
    assert!(fields.iter().any(|(k, _)| k == "level"));
    assert!(fields.iter().any(|(k, _)| k == "timestamp"));
}

#[tokio::test]
async fn test_unfiltered_stream_open_close_and_focus() {
    let mut app = create_test_app();
    assert_eq!(app.active_tab, ActiveTab::Filtered);

    app.open_unfiltered_stream(Some(12345));
    assert_eq!(app.active_tab, ActiveTab::Unfiltered);
    assert!(app.unfiltered.is_open);
    assert_eq!(app.unfiltered.target_id, Some(12345));
    assert!(!app.unfiltered.is_live);

    app.close_unfiltered_stream();
    assert_eq!(app.active_tab, ActiveTab::Filtered);
    assert!(!app.unfiltered.is_open);
    assert!(app.unfiltered.target_id.is_none());
}

#[tokio::test]
async fn test_sync_discovered_fields_replaces_aliases() {
    let mut app = create_test_app();
    let log = LogEvent::new(
        "2026-08-20T10:00:00Z",
        LogColor::Green,
        "msg",
        HashMap::from([("custom_field".to_string(), serde_json::json!("val"))]),
    );

    app.sync_discovered_fields(&[log]);
    let fields = app.get_available_log_fields();
    assert!(fields.iter().any(|(k, _)| k == "level"));
}

#[tokio::test]
async fn test_unlatch_unfiltered_idempotency() {
    let mut app = create_test_app();
    app.open_unfiltered_stream(None);
    assert!(app.unfiltered.is_live);

    app.unlatch_unfiltered();
    assert!(!app.unfiltered.is_live);

    app.unlatch_unfiltered();
    assert!(!app.unfiltered.is_live);
}

#[tokio::test]
async fn test_unfiltered_live_tick_sync_both_branches() {
    let mut app = create_test_app();
    let tx = app.session.engine.get_channel();

    // 1. Initial 5 logs
    for i in 0..5 {
        let log = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message {}", i)),
        };
        tx.send(log).await.unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    app.trigger_full_search();
    assert_eq!(app.viewport.cached_logs.len(), 5);

    // 2. Open unfiltered stream in LIVE mode
    app.open_unfiltered_stream(None);
    assert!(app.unfiltered.is_live);
    assert_eq!(app.unfiltered.cached_unfiltered.len(), 5);

    // 3. Ingest 5 more logs
    for i in 5..10 {
        let log = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message {}", i)),
        };
        tx.send(log).await.unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Force last_search_time backward to satisfy 150ms debounce in tick()
    app.search.last_search_time = Instant::now() - Duration::from_millis(200);
    app.tick();

    // Both filtered and unfiltered live buffers must have received all 10 logs
    assert_eq!(app.viewport.cached_logs.len(), 10);
    assert_eq!(app.unfiltered.cached_unfiltered.len(), 10);
}

#[tokio::test]
async fn test_unfiltered_frozen_snapshot_no_drift() {
    let mut app = create_test_app();
    let tx = app.session.engine.get_channel();

    for i in 0..5 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("{{\"level\":\"INFO\",\"message\":\"msg {}\"}}", i)),
        };
        let _ = tx.send(entry).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    app.open_unfiltered_stream(Some(2));
    assert!(!app.unfiltered.is_live);
    let initial_count = app.unfiltered.cached_unfiltered.len();

    let new_entry = RawLogEntry {
        payload: RawPayload::Text("{\"level\":\"INFO\",\"message\":\"msg 99\"}".to_string()),
    };
    let _ = tx.send(new_entry).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    app.tick();
    assert_eq!(app.unfiltered.cached_unfiltered.len(), initial_count);
}

#[tokio::test]
async fn test_repeated_unlatch_idempotency_preserves_pause_state() {
    let mut app = create_test_app();
    let tx = app.session.engine.get_channel();

    for i in 0..10 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("{{\"level\":\"INFO\",\"message\":\"msg {}\"}}", i)),
        };
        let _ = tx.send(entry).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.tick();

    app.unlatch();
    assert!(!app.viewport.is_auto_scroll);
    let paused_count = app.viewport.pause_snapshot.filtered_seen;

    app.unlatch();
    assert_eq!(app.viewport.pause_snapshot.filtered_seen, paused_count);
}

#[tokio::test]
async fn test_spawn_load_environment_and_tick() {
    let mut app = create_test_app();
    app.spawn_load_environment();

    tokio::time::sleep(Duration::from_millis(350)).await;
    app.tick();

    assert!(matches!(
        app.session.env_status,
        uwu_core_workspace::EnvLoadStatus::Ready { .. }
    ));
}

#[tokio::test]
async fn test_source_running_state_transitions_to_stopped() {
    let mut app = create_test_app();
    assert!(!app.session.is_source_running);

    #[cfg(target_os = "windows")]
    {
        app.session.source_config.command_str = "cmd /c echo test".to_string();
    }
    #[cfg(not(target_os = "windows"))]
    {
        app.session.source_config.command_str = "echo test".to_string();
    }

    app.start_configured_source();
    assert!(app.session.is_source_running);

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        app.tick();
        if !app.session.is_source_running {
            break;
        }
    }

    assert!(!app.session.is_source_running);
}

#[tokio::test]
async fn test_close_unfiltered_stream_frees_ram() {
    let mut app = create_test_app();
    app.open_unfiltered_stream(None);
    app.close_unfiltered_stream();
    assert!(app.unfiltered.cached_unfiltered.is_empty());
}

#[tokio::test]
async fn test_multi_project_switch_and_close() {
    let mut app = create_test_app();
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.active_index, 0);

    app.search.query = "level:error".to_string();

    let ws2 = Workspace::new(
        "Project B",
        WorkspaceLocation::local("D:\\test\\proj_b"),
        SourceType::Process,
    );
    app.open_or_switch_workspace(&ws2);

    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_index, 1);
    assert_eq!(app.session.name, "Project B");
    assert_eq!(app.search.query, "");

    app.search.query = "tag:Audio".to_string();

    app.switch_session(0);
    assert_eq!(app.active_index, 0);
    assert_eq!(app.session.name, "Test Project");
    assert_eq!(app.search.query, "level:error");

    app.switch_session(1);
    assert_eq!(app.active_index, 1);
    assert_eq!(app.session.name, "Project B");
    assert_eq!(app.search.query, "tag:Audio");

    app.close_session(1);
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.active_index, 0);
    assert_eq!(app.session.name, "Test Project");
    assert_eq!(app.search.query, "level:error");
}

#[tokio::test]
async fn test_remote_workspace_save_and_reload() {
    let mut app = create_test_app();

    // 1. Cấu hình Remote workspace trong session hiện tại
    app.session.location = WorkspaceLocation::remote(
        uwu_core_workspace::WslConnectionOptions::new("Ubuntu", "/home/user/backend"),
    );
    app.session.source_config.source_type = SourceType::Process;
    app.session.source_config.command_str = "python3 app.py".to_string();
    app.session.name = "Remote-Backend".to_string();

    // 2. Lưu workspace hiện tại
    app.save_current_workspace();

    // 3. Kiểm tra xem workspace được lưu vào store đúng chưa
    let ws = app
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
    assert_eq!(app.session.source_config.source_type, SourceType::Process);
    assert_eq!(app.session.source_config.command_str, "python3 app.py");
    let remote = app
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
    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_index, 1);

    // Try opening the same folder with backslashes
    let ws_backward = Workspace::new(
        "test-slash-alt",
        WorkspaceLocation::local("D:\\Learn\\Go\\uwulog-rust\\"),
        SourceType::Process,
    );
    app.open_or_switch_workspace(&ws_backward);

    // Should NOT create a duplicate session, should switch to session index 1
    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_index, 1);
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
async fn test_zed_style_draft_isolation_and_cancel() {
    let mut app = create_test_app();
    app.session.source_config.source_type = SourceType::Process;
    app.session.source_config.command_str = "cargo run".to_string();

    // Open launch modal creates draft from live session
    app.dispatch_action(AppAction::OpenLaunchModal);
    assert!(app.is_overlay_open(OverlayLayer::LaunchModal));
    assert!(app.launch_modal_draft.is_some());

    // Modify draft in form (e.g. user toggles to File source and types a path)
    if let Some(ref mut draft) = app.launch_modal_draft {
        draft.source_type = SourceType::File;
        draft.file_path = "/var/log/test.log".to_string();
    }

    // Live session must remain completely untouched while modal is uncommitted
    assert_eq!(app.session.source_config.source_type, SourceType::Process);
    assert_eq!(app.session.source_config.command_str, "cargo run");
    assert_eq!(app.session.source_config.file_path, "");

    // User cancels modal
    app.dispatch_action(AppAction::CloseLaunchModal);
    assert!(!app.is_overlay_open(OverlayLayer::LaunchModal));
    assert!(app.launch_modal_draft.is_none());

    // Live session remains intact
    assert_eq!(app.session.source_config.source_type, SourceType::Process);
    assert_eq!(app.session.source_config.command_str, "cargo run");
}

#[tokio::test]
async fn test_zed_style_draft_apply() {
    let mut app = create_test_app();
    app.session.source_config.source_type = SourceType::Process;
    app.session.source_config.command_str = "cargo run".to_string();

    // Open modal
    app.dispatch_action(AppAction::OpenLaunchModal);
    if let Some(ref mut draft) = app.launch_modal_draft {
        draft.source_type = SourceType::File;
        draft.file_path = "C:\\logs\\app.log".to_string();
        draft.capacity = 50_000;
    }

    // Apply
    app.dispatch_action(AppAction::ApplyLaunchModal);
    assert!(!app.is_overlay_open(OverlayLayer::LaunchModal));
    assert!(app.launch_modal_draft.is_none());

    // Live session has received the new config
    assert_eq!(app.session.source_config.source_type, SourceType::File);
    assert_eq!(app.session.source_config.file_path, "C:\\logs\\app.log");
    assert_eq!(app.session.source_config.capacity, 50_000);
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
    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_index, 1);

    app.dispatch_action(AppAction::OpenWorkspace(ws2));
    assert_eq!(app.sessions.len(), 3);
    assert_eq!(app.active_index, 2);

    // Cycle backwards
    app.dispatch_action(AppAction::CycleSession(false));
    assert_eq!(app.active_index, 1);

    // Cycle forwards
    app.dispatch_action(AppAction::CycleSession(true));
    assert_eq!(app.active_index, 2);

    // Switch directly
    app.dispatch_action(AppAction::SwitchSession(0));
    assert_eq!(app.active_index, 0);

    // Close session
    app.dispatch_action(AppAction::CloseSession(1));
    assert_eq!(app.sessions.len(), 2);
}

#[tokio::test]
async fn test_app_action_columns_modal_flow() {
    let mut app = create_test_app();
    let initial_cols = app.columns.columns.clone();

    // 1. Open columns modal
    app.dispatch_action(AppAction::OpenColumnsModal);
    assert!(app.columns.is_modal_open);
    assert!(app.columns.draft_columns.is_some());

    // 2. Modify draft
    if let Some(ref mut draft) = app.columns.draft_columns {
        draft[0].visible = false;
    }

    // Live columns should still be unchanged (Live vs Draft separation)
    assert_eq!(app.columns.columns, initial_cols);
    assert!(app.columns.columns[0].visible);

    // 3. Cancel / Close modal
    app.dispatch_action(AppAction::CloseColumnsModal);
    assert!(!app.columns.is_modal_open);
    assert!(app.columns.draft_columns.is_none());
    assert_eq!(app.columns.columns, initial_cols);

    // 4. Open again, modify and Apply
    app.dispatch_action(AppAction::OpenColumnsModal);
    if let Some(ref mut draft) = app.columns.draft_columns {
        draft[0].visible = false;
    }
    app.dispatch_action(AppAction::ApplyColumnsModal);
    assert!(!app.columns.is_modal_open);
    assert!(!app.columns.columns[0].visible);
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
    assert!(app.inspector.selected_log.is_some());
    assert_eq!(
        app.inspector.selected_log.as_ref().unwrap().message,
        "test message"
    );

    app.dispatch_action(AppAction::SelectLog(None));
    assert!(app.inspector.selected_log.is_none());

    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Unfiltered));
    assert_eq!(app.active_tab, ActiveTab::Unfiltered);

    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Filtered));
    assert_eq!(app.active_tab, ActiveTab::Filtered);
}

#[tokio::test]
async fn test_app_action_query_filter_and_clear() {
    let mut app = create_test_app();

    app.dispatch_action(AppAction::ApplyFilterTerm("level:error".to_string()));
    assert_eq!(app.search.query, "level:error");

    app.dispatch_action(AppAction::ExcludeFilterTerm("user_id:42".to_string()));
    assert_eq!(app.search.query, "level:error -user_id:42");

    app.dispatch_action(AppAction::ClearQuery);
    assert!(app.search.query.is_empty());
}

#[tokio::test]
async fn test_app_action_highlights() {
    let mut app = create_test_app();

    app.dispatch_action(AppAction::ToggleRowHighlight(100));
    assert!(app.is_row_highlighted(&100));

    app.dispatch_action(AppAction::ToggleTermHighlight("error".to_string()));
    assert!(app.inspector.highlighted_terms.contains("error"));

    app.dispatch_action(AppAction::ClearAllHighlights);
    assert!(!app.is_row_highlighted(&100));
    assert!(app.inspector.highlighted_terms.is_empty());
}

#[tokio::test]
async fn test_app_action_latch_and_unfiltered() {
    let mut app = create_test_app();

    // Latch toggle
    assert!(app.viewport.is_auto_scroll);
    app.dispatch_action(AppAction::ToggleLatch);
    assert!(!app.viewport.is_auto_scroll);
    app.dispatch_action(AppAction::ToggleLatch);
    assert!(app.viewport.is_auto_scroll);

    // Open & close unfiltered stream
    app.dispatch_action(AppAction::OpenUnfilteredStream(Some(42)));
    assert!(app.unfiltered.is_open);
    assert_eq!(app.active_tab, ActiveTab::Unfiltered);

    // Unfiltered live toggle
    assert!(!app.unfiltered.is_live);
    app.dispatch_action(AppAction::ToggleUnfilteredLive);
    assert!(app.unfiltered.is_live);

    app.dispatch_action(AppAction::RefreshUnfilteredSnapshot);

    app.dispatch_action(AppAction::FocusInMainAndClearFilter);
    assert!(!app.unfiltered.is_open);
    assert_eq!(app.active_tab, ActiveTab::Filtered);

    app.dispatch_action(AppAction::OpenUnfilteredStream(None));
    assert!(app.unfiltered.is_open);
    app.dispatch_action(AppAction::CloseUnfilteredStream);
    assert!(!app.unfiltered.is_open);
}

#[tokio::test]
async fn test_app_action_project_picker() {
    let mut app = create_test_app();

    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    app.dispatch_action(AppAction::ToggleProjectPicker);
    assert!(app.is_overlay_open(OverlayLayer::ProjectPicker));

    app.project_search_query = "search_test".to_string();
    app.dispatch_action(AppAction::CloseProjectPicker);
    assert!(!app.is_overlay_open(OverlayLayer::ProjectPicker));
    assert!(app.project_search_query.is_empty());
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
    app.inspector.selected_log = Some(test_log);
    assert!(app.inspector.selected_log.is_some());

    // 2. Add unfiltered stream on top
    app.open_unfiltered_stream(None);
    assert_eq!(app.active_tab, ActiveTab::Unfiltered);

    // 3. Add launch modal on top (stack)
    app.push_overlay(OverlayLayer::LaunchModal);

    // 4. Add columns modal on top (stack)
    app.columns.open_modal();
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
    assert_eq!(app.active_tab, ActiveTab::Unfiltered);

    // Pop 4: Unfiltered stream tab
    app.dispatch_action(AppAction::DismissTopLayer);
    assert_eq!(app.active_tab, ActiveTab::Filtered);
    assert!(app.inspector.selected_log.is_some());

    // Pop 5: Selected log inspector
    app.dispatch_action(AppAction::DismissTopLayer);
    assert!(app.inspector.selected_log.is_none());

    // Pop 6: Nothing left to pop
    assert!(!app.dismiss_top_layer());
}

#[test]
fn test_cli_args_parsing() {
    // 1. Test default arguments
    let defaults = CliArgs::try_parse_from(["uwu-gui"]).unwrap();
    assert_eq!(defaults.display_limit, 5000);
    assert_eq!(defaults.capacity, 200_000);
    assert_eq!(defaults.cmd, None);
    assert_eq!(defaults.file, None);
    assert_eq!(defaults.working_dir, None);
    assert_eq!(defaults.remote, None);
    assert_eq!(defaults.query, None);
    assert_eq!(defaults.path, None);
    assert!(defaults.trailing_cmd.is_empty());

    // 2. Test standard short flags, query and positional path
    let parsed = CliArgs::try_parse_from([
        "uwu-gui",
        "-n",
        "1234",
        "-C",
        "50000",
        "-c",
        "cargo run",
        "-d",
        "C:/my_project",
        "-q",
        "level:error",
        "server.log",
    ])
    .unwrap();
    assert_eq!(parsed.display_limit, 1234);
    assert_eq!(parsed.capacity, 50000);
    assert_eq!(parsed.cmd.as_deref(), Some("cargo run"));
    assert_eq!(parsed.working_dir.as_deref(), Some("C:/my_project"));
    assert_eq!(parsed.query.as_deref(), Some("level:error"));
    assert_eq!(parsed.path.as_deref(), Some("server.log"));

    // 3. Test short_alias -r for command
    let r_flag = CliArgs::try_parse_from(["uwu-gui", "-r", "python app.py"]).unwrap();
    assert_eq!(r_flag.cmd.as_deref(), Some("python app.py"));

    // 4. Test trailing command after `--`
    let trailing = CliArgs::try_parse_from([
        "uwu-gui",
        "-d",
        "/repo",
        "--",
        "cargo",
        "test",
        "--workspace",
    ])
    .unwrap();
    assert_eq!(trailing.working_dir.as_deref(), Some("/repo"));
    assert_eq!(
        trailing.trailing_cmd,
        vec![
            "cargo".to_string(),
            "test".to_string(),
            "--workspace".to_string()
        ]
    );

    // 5. Test standard remote flag
    let remote_clean = CliArgs::try_parse_from([
        "uwu-gui",
        "--cmd",
        "htop",
        "--remote",
        "Debian",
        "--dir",
        "/home/user",
    ])
    .unwrap();
    assert_eq!(remote_clean.cmd.as_deref(), Some("htop"));
    assert_eq!(remote_clean.remote.as_deref(), Some("Debian"));
    assert_eq!(remote_clean.working_dir.as_deref(), Some("/home/user"));

    // 6. Test backward compatibility aliases (wsl legacy)
    let wsl_legacy = CliArgs::try_parse_from([
        "uwu-gui",
        "--wsl-cmd",
        "tail -f log",
        "--wsl-distro",
        "Ubuntu-22.04",
        "--wsl-cwd",
        "/var/log",
    ])
    .unwrap();
    assert_eq!(wsl_legacy.cmd.as_deref(), Some("tail -f log"));
    assert_eq!(wsl_legacy.remote.as_deref(), Some("Ubuntu-22.04"));
    assert_eq!(wsl_legacy.working_dir.as_deref(), Some("/var/log"));
}

#[tokio::test]
async fn test_resolve_target_and_build_initial_session() {
    let store = WorkspaceStore::default();

    // Case 1: Remote CLI with command
    let cli_remote = CliArgs::try_parse_from([
        "uwu-gui",
        "--remote",
        "Ubuntu",
        "-d",
        "/var/log",
        "-c",
        "journalctl -f",
        "-q",
        "error",
    ])
    .unwrap();
    let (loc, src_type, cmd, file, has_custom) = cli_remote.resolve_target();
    assert!(loc.is_remote());
    assert_eq!(loc.working_dir(), "/var/log");
    assert_eq!(src_type, SourceType::Process);
    assert_eq!(cmd, "journalctl -f");
    assert_eq!(file, "");
    assert!(has_custom);

    let (session, custom_src, _) = UwuGuiApp::build_initial_session(&cli_remote, &store);
    assert!(custom_src);
    assert_eq!(session.session.location, loc);
    assert_eq!(session.view.search.query, "error");

    // Case 2: Positional log file on local
    let cli_file = CliArgs::try_parse_from(["uwu-gui", "production.log"]).unwrap();
    let (loc, src_type, cmd, file, has_custom) = cli_file.resolve_target();
    assert!(!loc.is_remote());
    assert_eq!(src_type, SourceType::File);
    assert_eq!(cmd, "");
    assert_eq!(file, "production.log");
    assert!(has_custom);

    let (session, custom_src, _) = UwuGuiApp::build_initial_session(&cli_file, &store);
    assert!(custom_src);
    assert_eq!(session.session.source_config.file_path, "production.log");
}

#[tokio::test]
async fn test_build_initial_session_preserves_saved_workspace_id_and_envs() {
    let mut store = WorkspaceStore::default();
    let mut saved_ws = Workspace::new(
        "SavedProj",
        WorkspaceLocation::local("D:\\my\\repo"),
        SourceType::Process,
    );
    let saved_uuid = saved_ws.id;
    saved_ws
        .env_vars
        .insert("ENV_KEY".to_string(), "ENV_VAL".to_string());
    store.recent_workspaces.push(saved_ws);

    let cli =
        CliArgs::try_parse_from(["uwu-gui", "-d", "D:\\my\\repo", "-c", "custom-cmd"]).unwrap();
    let (session, has_custom, saved_id) = UwuGuiApp::build_initial_session(&cli, &store);

    assert!(has_custom);
    assert_eq!(saved_id, Some(saved_uuid));
    assert_eq!(session.session.id, saved_uuid);
    assert_eq!(session.session.source_config.command_str, "custom-cmd");
    assert_eq!(
        session.session.env_vars.get("ENV_KEY").map(String::as_str),
        Some("ENV_VAL")
    );
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
    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_index, 1);
    // Kiểm tra kế thừa capacity và display_limit từ active session
    assert_eq!(app.session.source_config.capacity, 100);
    assert_eq!(app.session.display_limit, 50);

    app.open_or_switch_workspace(&ws2);
    assert_eq!(app.sessions.len(), 3);
    assert_eq!(app.active_index, 2);

    // 1. switch_session vào chính index hiện tại là no-op
    app.switch_session(2);
    assert_eq!(app.active_index, 2);

    // 2. close_session tab trước active_index sẽ giảm active_index đi 1
    app.close_session(0);
    assert_eq!(app.sessions.len(), 2);
    assert_eq!(app.active_index, 1);
    assert_eq!(app.session.name, "P2");

    // 3. Đóng hết các session -> tạo session mặc định và kế thừa capacity
    app.close_session(1);
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.active_index, 0);

    app.close_session(0);
    assert_eq!(app.sessions.len(), 1);
    assert_eq!(app.active_index, 0);
    assert_eq!(app.session.source_config.capacity, 100);
    assert_eq!(app.session.display_limit, 50);
}

#[tokio::test]
async fn test_on_exit_saves_current_workspace() {
    use eframe::App;
    let mut app = create_test_app();
    app.sessions[0].session.name = "ExitTestProj".to_string();
    app.sessions[0].view.search.query = "error_query".to_string();

    app.on_exit(None);

    let ws = app
        .store
        .recent_workspaces
        .iter()
        .find(|w| w.name == "ExitTestProj");
    assert!(ws.is_some());
    assert_eq!(ws.unwrap().last_query, "error_query");
}
