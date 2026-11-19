use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::state::ActiveTab;
use std::time::{Duration, Instant};
use uwu_core_schema::{RawLogEntry, RawPayload};

#[tokio::test]
async fn test_unfiltered_stream_open_close_and_focus() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    assert_eq!(session.view.active_tab, ActiveTab::Filtered);

    session.open_unfiltered_stream(Some(12345));
    assert_eq!(session.view.active_tab, ActiveTab::Unfiltered);
    assert!(session.view.has_raw_tab());
    assert_eq!(session.view.raw_tab().unwrap().target_id(), Some(12345));
    assert!(!session.view.raw_tab().unwrap().stream.is_auto_scroll);

    session.close_unfiltered_stream();
    assert_eq!(session.view.active_tab, ActiveTab::Filtered);
    assert!(!session.view.has_raw_tab());
}

#[tokio::test]
async fn test_unfiltered_live_tick_sync_both_branches() {
    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    // 1. Initial 5 logs
    for i in 0..5 {
        let log = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message {}", i)),
        };
        tx.send(log).await.unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    app.active_session_mut().trigger_full_search();
    assert_eq!(
        app.active_session()
            .view
            .main_tab()
            .stream
            .cached_logs
            .len(),
        5
    );

    // 2. Open unfiltered stream in LIVE mode
    app.active_session_mut().open_unfiltered_stream(None);
    assert!(
        app.active_session()
            .view
            .raw_tab()
            .unwrap()
            .stream
            .is_auto_scroll
    );
    assert_eq!(
        app.active_session()
            .view
            .raw_tab()
            .unwrap()
            .stream
            .cached_logs
            .len(),
        5
    );

    // 3. Ingest 5 more logs
    for i in 5..10 {
        let log = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message {}", i)),
        };
        tx.send(log).await.unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Force last_search_time backward to satisfy 150ms debounce in tick()
    app.active_session_mut().view.search.last_search_time =
        Instant::now() - Duration::from_millis(200);
    app.tick();

    // Both filtered and unfiltered live buffers must have received all 10 logs
    assert_eq!(
        app.active_session()
            .view
            .main_tab()
            .stream
            .cached_logs
            .len(),
        10
    );
    assert_eq!(
        app.active_session()
            .view
            .raw_tab()
            .unwrap()
            .stream
            .cached_logs
            .len(),
        10
    );
}

#[tokio::test]
async fn test_unfiltered_frozen_snapshot_no_drift() {
    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    for i in 0..5 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("{{\"level\":\"INFO\",\"message\":\"msg {}\"}}", i)),
        };
        let _ = tx.send(entry).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    app.active_session_mut().open_unfiltered_stream(Some(2));
    assert!(
        !app.active_session()
            .view
            .raw_tab()
            .unwrap()
            .stream
            .is_auto_scroll
    );
    let initial_count = app
        .active_session()
        .view
        .raw_tab()
        .unwrap()
        .stream
        .cached_logs
        .len();

    let new_entry = RawLogEntry {
        payload: RawPayload::Text("{\"level\":\"INFO\",\"message\":\"msg 99\"}".to_string()),
    };
    let _ = tx.send(new_entry).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    app.tick();
    assert_eq!(
        app.active_session()
            .view
            .raw_tab()
            .unwrap()
            .stream
            .cached_logs
            .len(),
        initial_count
    );
}

#[tokio::test]
async fn test_close_unfiltered_stream_frees_ram() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    session.open_unfiltered_stream(None);
    session.close_unfiltered_stream();
    assert!(!session.view.has_raw_tab());
}

#[tokio::test]
async fn test_switch_tab_preserves_picked_row_and_inspector() {
    let mut app = create_test_app();
    app.active_session_mut().view.search.query = "error".to_string();

    let event_main = uwu_core_schema::LogEvent::new(
        "2026-08-20T10:00:00Z",
        uwu_core_schema::LogColor::Green,
        "main log event",
        std::collections::HashMap::new(),
    )
    .with_id(101);

    let event_raw = uwu_core_schema::LogEvent::new(
        "2026-08-20T10:00:05Z",
        uwu_core_schema::LogColor::Red,
        "raw log event",
        std::collections::HashMap::new(),
    )
    .with_id(202);

    // 1. Ở Main (Filtered), pick dòng event_main
    app.dispatch_action(AppAction::SelectLog(Some(Box::new(event_main.clone()))));
    assert_eq!(
        app.active_session()
            .view
            .shared
            .inspector
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(101)
    );
    assert_eq!(
        app.active_session()
            .view
            .main_tab()
            .stream
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(101)
    );

    // 2. Chuyển sang Raw (Unfiltered)
    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Unfiltered));
    assert_eq!(app.active_session().view.active_tab, ActiveTab::Unfiltered);
    // Tab Raw chưa pick dòng nào -> inspector chưa có dòng nào
    assert!(app
        .active_session()
        .view
        .shared
        .inspector
        .selected_log
        .is_none());

    // 3. Ở Raw, pick dòng event_raw
    app.dispatch_action(AppAction::SelectLog(Some(Box::new(event_raw.clone()))));
    assert_eq!(
        app.active_session()
            .view
            .shared
            .inspector
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(202)
    );
    assert_eq!(
        app.active_session()
            .view
            .raw_tab()
            .unwrap()
            .stream
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(202)
    );

    // 4. Chuyển ngược về Main (Filtered)
    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Filtered));
    assert_eq!(app.active_session().view.active_tab, ActiveTab::Filtered);
    // Inspector tự động khôi phục đúng dòng đang pick của tab Main (event_main)!
    assert_eq!(
        app.active_session()
            .view
            .shared
            .inspector
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(101)
    );

    // 5. Chuyển lại sang Raw (Unfiltered)
    app.dispatch_action(AppAction::SwitchTab(ActiveTab::Unfiltered));
    assert_eq!(app.active_session().view.active_tab, ActiveTab::Unfiltered);
    // Inspector tự động khôi phục đúng dòng đang pick của tab Raw (event_raw)!
    assert_eq!(
        app.active_session()
            .view
            .shared
            .inspector
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(202)
    );
}

#[tokio::test]
async fn test_view_in_raw_log_references_exact_spilled_disk_row() {
    use crate::actions::AppAction;
    use uwu_core_workspace::{SourceConfig, SourceType, Workspace, WorkspaceLocation};

    let mut app = create_test_app();
    let ws = Workspace::new(
        "spill-raw-ref-test",
        WorkspaceLocation::local("/tmp"),
        SourceType::Process,
    );
    app.dispatch_action(AppAction::OpenWorkspace(Box::new(ws)));

    // Khởi tạo WorkspaceSession với capacity = 10 để ép 30/40 dòng phải spill thực sự ra file Arrow IPC trên ổ cứng
    {
        let active = app.active_session_mut();
        active.session = uwu_core_workspace::WorkspaceSession::new(
            "spill-raw-ref-test",
            WorkspaceLocation::local("/tmp"),
            SourceConfig {
                capacity: 10,
                display_limit: 50,
                ..Default::default()
            },
        );
    }

    let tx = app.active_session().session.engine.get_channel();
    for seq in 0..40 {
        let level = if seq % 5 == 0 { "ERROR" } else { "INFO" };
        tx.send(RawLogEntry {
            payload: RawPayload::Json(serde_json::json!({
                "seq": seq,
                "level": level,
                "message": format!("log_seq_{:02}", seq),
            })),
        })
        .await
        .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(60)).await;
    app.active_session().session.engine.flush_spill();

    // RAM chỉ giữ 10 dòng cuối (30..39); 30 dòng đầu (0..29) đã nằm trong Cold Disk (.arrow)
    assert_eq!(app.active_session().session.engine.total_logs(), 10);
    assert_eq!(app.active_session().session.engine.total_spilled(), 30);

    // Lọc `level:error` trong tab Filtered -> chọn dòng `log_seq_05` (nằm hoàn toàn trên ổ đĩa)
    {
        let active = app.active_session_mut();
        active.view.search.query = "level:error".to_string();
        active.trigger_full_search();
    }
    let target_event = app
        .active_session()
        .view
        .main_tab()
        .stream
        .cached_logs
        .iter()
        .find(|e| e.message == "log_seq_05")
        .cloned()
        .expect("filtered logs must include spilled row log_seq_05 from disk");

    app.dispatch_action(AppAction::SelectLog(Some(Box::new(target_event.clone()))));
    app.dispatch_action(AppAction::ViewRawContext);

    let session = app.active_session();
    assert_eq!(session.view.active_tab, ActiveTab::Unfiltered);
    let raw = session.view.raw_tab().unwrap();
    assert!(!raw.stream.is_auto_scroll);
    assert_eq!(raw.target_id(), Some(target_event.id));

    let target_idx = match raw.stream.pending_scroll {
        Some(crate::state::ScrollCommand::Row(idx, _)) => idx,
        other => panic!("expected ScrollCommand::Row for spilled row, got {other:?}"),
    };
    let raw_row = &raw.stream.cached_logs[target_idx];
    assert_eq!(raw_row.id, target_event.id);
    assert_eq!(raw_row.message, "log_seq_05");

    // Kiểm tra ngữ cảnh Raw xung quanh dòng trên đĩa phải đúng thứ tự gốc (04 -> 05 -> 06)
    assert_eq!(raw.stream.cached_logs[target_idx - 1].message, "log_seq_04");
    assert_eq!(raw.stream.cached_logs[target_idx + 1].message, "log_seq_06");
    assert_eq!(
        session
            .view
            .shared
            .inspector
            .selected_log
            .as_ref()
            .map(|l| l.id),
        Some(target_event.id)
    );
}
