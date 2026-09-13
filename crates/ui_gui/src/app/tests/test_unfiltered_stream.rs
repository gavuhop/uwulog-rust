use super::test_helpers::create_test_app;
use crate::state::ActiveTab;
use std::time::{Duration, Instant};
use uwu_core_schema::{RawLogEntry, RawPayload};

#[tokio::test]
async fn test_unfiltered_stream_open_close_and_focus() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    assert_eq!(session.active_tab, ActiveTab::Filtered);

    session.open_unfiltered_stream(Some(12345));
    assert_eq!(session.active_tab, ActiveTab::Unfiltered);
    assert!(session.unfiltered.is_open);
    assert_eq!(session.unfiltered.target_id, Some(12345));
    assert!(!session.unfiltered.is_live);

    session.close_unfiltered_stream();
    assert_eq!(session.active_tab, ActiveTab::Filtered);
    assert!(!session.unfiltered.is_open);
    assert!(session.unfiltered.target_id.is_none());
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
    assert_eq!(app.active_session().viewport.cached_logs.len(), 5);

    // 2. Open unfiltered stream in LIVE mode
    app.active_session_mut().open_unfiltered_stream(None);
    assert!(app.active_session().unfiltered.is_live);
    assert_eq!(app.active_session().unfiltered.cached_unfiltered.len(), 5);

    // 3. Ingest 5 more logs
    for i in 5..10 {
        let log = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message {}", i)),
        };
        tx.send(log).await.unwrap();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Force last_search_time backward to satisfy 150ms debounce in tick()
    app.active_session_mut().search.last_search_time = Instant::now() - Duration::from_millis(200);
    app.tick();

    // Both filtered and unfiltered live buffers must have received all 10 logs
    assert_eq!(app.active_session().viewport.cached_logs.len(), 10);
    assert_eq!(app.active_session().unfiltered.cached_unfiltered.len(), 10);
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
    assert!(!app.active_session().unfiltered.is_live);
    let initial_count = app.active_session().unfiltered.cached_unfiltered.len();

    let new_entry = RawLogEntry {
        payload: RawPayload::Text("{\"level\":\"INFO\",\"message\":\"msg 99\"}".to_string()),
    };
    let _ = tx.send(new_entry).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    app.tick();
    assert_eq!(
        app.active_session().unfiltered.cached_unfiltered.len(),
        initial_count
    );
}

#[tokio::test]
async fn test_close_unfiltered_stream_frees_ram() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    session.open_unfiltered_stream(None);
    session.close_unfiltered_stream();
    assert!(session.unfiltered.cached_unfiltered.is_empty());
}
