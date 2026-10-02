use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::app::UwuGuiApp;
use crate::state::{SuggestionItem, SuggestionKind};
use std::collections::HashMap;
use uwu_core_schema::{LogColor, LogEvent};

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
    let session = app.active_session_mut();

    session.apply_filter_term("level:error");
    assert_eq!(session.search.query, "level:error");

    session.apply_filter_term("tag:Auth");
    assert_eq!(session.search.query, "level:error tag:Auth");

    session.exclude_filter_term("healthcheck");
    assert_eq!(session.search.query, "level:error tag:Auth -healthcheck");

    session.exclude_filter_term("-already_negated");
    assert_eq!(
        session.search.query,
        "level:error tag:Auth -healthcheck -already_negated"
    );

    // Verify filter terms applied via context menu are recorded to history
    assert!(!session.search.history.entries.is_empty());
    assert_eq!(
        session.search.history.entries[0],
        "level:error tag:Auth -healthcheck -already_negated"
    );
}

#[tokio::test]
async fn test_apply_autocomplete_suggestion() {
    let mut app = create_test_app();
    let session = app.active_session_mut();

    let item_key = SuggestionItem {
        kind: SuggestionKind::Key,
        op_symbol: "",
        action_name: "level".to_string(),
        example_syntax: "level:".to_string(),
        insert_text: "level:".to_string(),
    };
    session.search.autocomplete.active_token_range = (0, 0);
    session.apply_autocomplete_suggestion(&item_key);
    assert_eq!(session.search.query, "level:");

    let item_val = SuggestionItem {
        kind: SuggestionKind::OperatorOrValue,
        op_symbol: "",
        action_name: "error".to_string(),
        example_syntax: "error".to_string(),
        insert_text: "error".to_string(),
    };
    session.search.autocomplete.active_token_range = (6, 6);
    session.apply_autocomplete_suggestion(&item_val);
    assert_eq!(session.search.query, "level:error");
    assert!(!session.search.autocomplete.is_open);
}

#[tokio::test]
async fn test_get_available_log_fields_inference() {
    let app = create_test_app();
    let fields = app.active_session().get_available_log_fields();
    assert!(!fields.is_empty());
    assert!(fields.iter().any(|(k, _)| k == "level"));
    assert!(fields.iter().any(|(k, _)| k == "timestamp"));
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

    let session = app.active_session_mut();
    session.sync_discovered_fields(&[log]);
    let fields = session.get_available_log_fields();
    assert!(fields.iter().any(|(k, _)| k == "level"));
}

#[tokio::test]
async fn test_app_action_query_filter_and_clear() {
    let mut app = create_test_app();

    app.dispatch_action(AppAction::ApplyFilterTerm("level:error".to_string()));
    assert_eq!(app.active_session().search.query, "level:error");

    app.dispatch_action(AppAction::ExcludeFilterTerm("user_id:42".to_string()));
    assert_eq!(app.active_session().search.query, "level:error -user_id:42");

    app.dispatch_action(AppAction::ClearQuery);
    assert!(app.active_session().search.query.is_empty());
}

#[tokio::test]
async fn test_apply_search_results_stale_generation_discarded() {
    let mut app = create_test_app();
    let session = app.active_session_mut();

    // Thiết lập active_query_id = 2
    session.view.search.active_query_id = 2;
    session.view.viewport.total_matched = 10;

    let stale_log = LogEvent::new(
        "2026-08-20T10:00:00Z",
        LogColor::Default,
        "stale log",
        HashMap::new(),
    );

    // 1. Event lỗi thời (query_id 1 < 2) -> phải bị bỏ qua, không ghi đè cache
    session.apply_search_results(1, 999, vec![stale_log]);
    assert_eq!(session.view.viewport.total_matched, 10);
    assert!(session.view.viewport.cached_logs.is_empty());

    // 2. Event hợp lệ (query_id 2 == 2) -> được áp dụng ngay trong O(1)
    let fresh_log = LogEvent::new(
        "2026-08-20T10:01:00Z",
        LogColor::Default,
        "fresh log",
        HashMap::new(),
    );
    session.apply_search_results(2, 42, vec![fresh_log]);
    assert_eq!(session.view.viewport.total_matched, 42);
    assert_eq!(session.view.viewport.cached_logs.len(), 1);
    assert_eq!(session.view.viewport.cached_logs[0].message, "fresh log");
}

#[tokio::test]
async fn test_app_event_drain_pipeline() {
    use crate::actions::AppEvent;

    let mut app = create_test_app();
    let session_id = app.active_session().session.id;
    app.active_session_mut().view.search.active_query_id = 1;

    let log = LogEvent::new(
        "2026-08-20T10:05:00Z",
        LogColor::Default,
        "pipeline event test",
        HashMap::new(),
    );

    // Gửi sự kiện từ background channel
    app.event_tx
        .send(AppEvent::SearchResultsReady {
            session_id,
            query_id: 1,
            total_matched: 1,
            logs: vec![log],
        })
        .unwrap();

    // Drain events như trong UI loop
    while let Ok(event) = app.event_rx.try_recv() {
        app.apply_event(event);
    }

    assert_eq!(app.active_session().view.viewport.total_matched, 1);
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 1);
    assert_eq!(
        app.active_session().view.viewport.cached_logs[0].message,
        "pipeline event test"
    );
}

#[tokio::test]
async fn test_reducer_drops_out_of_order_query() {
    use crate::actions::AppEvent;

    let mut app = create_test_app();
    let session_id = app.active_session().session.id;

    // Giả sử user gõ liên tiếp: active_query_id đã tăng lên 3
    app.active_session_mut().view.search.active_query_id = 3;

    let stale_log = LogEvent::new(
        "2026-08-20T10:00:00Z",
        LogColor::Default,
        "stale from query 2",
        HashMap::new(),
    );

    // Worker của query 2 hoàn thành chậm hơn sau khi user đã sang query 3
    app.apply_event(AppEvent::SearchResultsReady {
        session_id,
        query_id: 2,
        total_matched: 50,
        logs: vec![stale_log],
    });

    // Kết quả của query 2 phải bị Reducer drop thẳng tay, không làm bẩn state
    assert_eq!(app.active_session().view.viewport.total_matched, 0);
    assert!(app.active_session().view.viewport.cached_logs.is_empty());

    // Worker của query 3 hoàn thành -> Reducer chấp nhận và cập nhật O(1)
    let valid_log = LogEvent::new(
        "2026-08-20T10:01:00Z",
        LogColor::Default,
        "valid from query 3",
        HashMap::new(),
    );
    app.apply_event(AppEvent::SearchResultsReady {
        session_id,
        query_id: 3,
        total_matched: 10,
        logs: vec![valid_log],
    });

    assert_eq!(app.active_session().view.viewport.total_matched, 10);
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 1);
    assert_eq!(
        app.active_session().view.viewport.cached_logs[0].message,
        "valid from query 3"
    );
}

#[tokio::test]
async fn test_reducer_incremental_events() {
    use crate::actions::AppEvent;

    let mut app = create_test_app();
    let session_id = app.active_session().session.id;

    let inc_log = LogEvent::new(
        "2026-08-20T10:10:00Z",
        LogColor::Default,
        "incremental match",
        HashMap::new(),
    );

    // Reducer xử lý IncrementalLogsReady
    app.apply_event(AppEvent::IncrementalLogsReady {
        session_id,
        matched: 1,
        logs: vec![inc_log],
        total_processed: 100,
    });

    assert_eq!(app.active_session().view.viewport.total_matched, 1);
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 1);
    assert_eq!(
        app.active_session().view.viewport.cached_logs[0].message,
        "incremental match"
    );
    assert_eq!(app.active_session().view.viewport.last_processed_count, 100);

    // Mở unfiltered view và test IncrementalUnfilteredReady
    app.active_session_mut().view.unfiltered.is_open = true;
    app.active_session_mut().view.unfiltered.is_live = true;

    let raw_log = LogEvent::new(
        "2026-08-20T10:10:05Z",
        LogColor::Default,
        "raw log unfiltered",
        HashMap::new(),
    );

    app.apply_event(AppEvent::IncrementalUnfilteredReady {
        session_id,
        logs: vec![raw_log],
    });

    assert_eq!(
        app.active_session().view.unfiltered.cached_unfiltered.len(),
        1
    );
    assert_eq!(
        app.active_session().view.unfiltered.cached_unfiltered[0].message,
        "raw log unfiltered"
    );
}

#[tokio::test]
async fn test_search_advance_query_and_early_cancellation_token() {
    let mut app = create_test_app();
    let search = &mut app.active_session_mut().view.search;

    assert_eq!(search.active_query_id, 0);

    let (q1_id, token1) = search.advance_query();
    assert_eq!(q1_id, 1);
    assert_eq!(token1.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Khi query 2 đến, token của worker 1 bị invalidate ngay lập tức
    let (q2_id, token2) = search.advance_query();
    assert_eq!(q2_id, 2);
    assert_eq!(token2.load(std::sync::atomic::Ordering::Relaxed), 2);
    // Worker 1 đọc token sẽ thấy != 1 và abort sớm
    assert_ne!(token1.load(std::sync::atomic::Ordering::Relaxed), q1_id);
}

#[tokio::test]
async fn test_phase0_drain_budget_guard() {
    use crate::actions::AppEvent;

    let mut app = create_test_app();
    app.active_session_mut().session.display_limit = 200;
    let session_id = app.active_session().session.id;

    // Gửi 100 sự kiện vào channel
    for i in 0..100 {
        let log = LogEvent::new(
            "2026-08-20T10:00:00Z",
            LogColor::Default,
            format!("log {i}"),
            HashMap::new(),
        );
        app.event_tx
            .send(AppEvent::IncrementalLogsReady {
                session_id,
                matched: 1,
                logs: vec![log],
                total_processed: i as u64 + 1,
            })
            .unwrap();
    }

    // Mô phỏng 1 frame drain có Frame-Budget Guard: tối đa 64 events
    const MAX_EVENTS_PER_FRAME: usize = 64;
    let mut drained = 0;
    while let Ok(event) = app.event_rx.try_recv() {
        app.apply_event(event);
        drained += 1;
        if drained >= MAX_EVENTS_PER_FRAME {
            break;
        }
    }

    // Frame đầu tiên chỉ được drain đúng 64 events để giữ 144 FPS
    assert_eq!(drained, 64);
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 64);

    // Các events còn lại vẫn nằm trong channel chờ frame tiếp theo
    let mut remaining = 0;
    while let Ok(event) = app.event_rx.try_recv() {
        app.apply_event(event);
        remaining += 1;
    }
    assert_eq!(remaining, 36);
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 100);
}

#[tokio::test]
async fn test_udf_reverse_pagination_worker_and_reducer() {
    use uwu_core_schema::{RawLogEntry, RawPayload};

    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    // 1. Nạp 20 bản ghi vào engine
    for i in 0..20 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("[INFO] Message index {:02}", i)),
        };
        tx.send(entry).await.unwrap();
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Giả lập display_limit = 5 (lấy 5 bản ghi mới nhất ban đầu: index 15..19)
    app.active_session_mut().session.display_limit = 5;
    app.active_session_mut().trigger_full_search();
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 5);

    // 2. Dispatch AppAction::LoadOlderLogs(5) qua UDF Dispatcher
    app.dispatch_action(AppAction::LoadOlderLogs(5));

    // Cờ is_loading_older phải được bật ngay lập tức để chặn spam worker
    assert!(app.active_session().view.viewport.is_loading_older);

    // 3. Đợi background worker xử lý và gửi AppEvent::ReversePaginationReady
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let event = app
        .event_rx
        .try_recv()
        .expect("Worker must send ReversePaginationReady");
    app.apply_event(event);

    // Sau khi Reducer áp dụng: cached_logs tăng lên 10 bản ghi, is_loading_older = false
    assert_eq!(app.active_session().view.viewport.cached_logs.len(), 10);
    assert!(!app.active_session().view.viewport.is_loading_older);
}

#[tokio::test]
async fn test_udf_unlatch_and_latch_actions() {
    let mut app = create_test_app();
    assert!(app.active_session().view.viewport.is_auto_scroll);

    // 1. Dispatch Unlatch action -> is_auto_scroll = false
    app.dispatch_action(AppAction::Unlatch);
    assert!(!app.active_session().view.viewport.is_auto_scroll);

    // 2. Dispatch Latch action -> is_auto_scroll = true, needs_search = true
    app.dispatch_action(AppAction::Latch);
    assert!(app.active_session().view.viewport.is_auto_scroll);
    assert!(app.active_session().view.search.needs_search);
}
