use super::test_helpers::create_test_app;
use crate::actions::AppAction;
use crate::state::ActiveTab;
use std::time::Duration;
use uwu_core_schema::{RawLogEntry, RawPayload};

#[tokio::test]
async fn test_latch_toggle() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    assert!(session.viewport.is_auto_scroll);

    session.unlatch();
    assert!(!session.viewport.is_auto_scroll);

    session.latch();
    assert!(session.viewport.is_auto_scroll);
    assert!(session.viewport.request_scroll_to_bottom);

    session.toggle_latch();
    assert!(!session.viewport.is_auto_scroll);

    session.toggle_latch();
    assert!(session.viewport.is_auto_scroll);
}

#[tokio::test]
async fn test_highlight_toggle_and_clear() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    let id1 = 1001u64;
    let id2 = 1002u64;

    assert!(!session.is_row_highlighted(&id1));
    assert!(!session.has_any_highlights());

    session.toggle_row_highlight(id1);
    assert!(session.is_row_highlighted(&id1));
    assert!(session.has_any_highlights());

    session.toggle_row_highlight(id2);
    assert!(session.is_row_highlighted(&id2));

    // Term highlight
    assert!(!session.is_term_highlighted("timeout"));
    session.toggle_term_highlight("timeout");
    assert!(session.is_term_highlighted("timeout"));
    assert!(session.is_term_highlighted("TIMEOUT")); // Case-insensitive
    assert!(session.has_any_highlights());

    session.toggle_term_highlight("timeout");
    assert!(!session.is_term_highlighted("timeout"));

    // Clear all
    session.toggle_term_highlight("error");
    session.clear_all_highlights();
    assert!(!session.has_any_highlights());
    assert!(!session.is_row_highlighted(&id1));
    assert!(!session.is_term_highlighted("error"));
}

#[tokio::test]
async fn test_unlatch_unfiltered_idempotency() {
    let mut app = create_test_app();
    let session = app.active_session_mut();
    session.open_unfiltered_stream(None);
    assert!(session.unfiltered.is_live);

    session.unlatch_unfiltered();
    assert!(!session.unfiltered.is_live);

    session.unlatch_unfiltered();
    assert!(!session.unfiltered.is_live);
}

#[tokio::test]
async fn test_repeated_unlatch_idempotency_preserves_pause_state() {
    let mut app = create_test_app();
    let tx = app.active_session().session.engine.get_channel();

    for i in 0..10 {
        let entry = RawLogEntry {
            payload: RawPayload::Text(format!("{{\"level\":\"INFO\",\"message\":\"msg {}\"}}", i)),
        };
        let _ = tx.send(entry).await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.tick();

    app.active_session_mut().unlatch();
    assert!(!app.active_session().viewport.is_auto_scroll);
    let paused_count = app.active_session().viewport.pause_snapshot.filtered_seen;

    app.active_session_mut().unlatch();
    assert_eq!(
        app.active_session().viewport.pause_snapshot.filtered_seen,
        paused_count
    );
}

#[tokio::test]
async fn test_app_action_highlights() {
    let mut app = create_test_app();

    app.dispatch_action(AppAction::ToggleRowHighlight(100));
    assert!(app.active_session().is_row_highlighted(&100));

    app.dispatch_action(AppAction::ToggleTermHighlight("error".to_string()));
    assert!(app
        .active_session()
        .inspector
        .highlighted_terms
        .contains("error"));

    app.dispatch_action(AppAction::ClearAllHighlights);
    assert!(!app.active_session().is_row_highlighted(&100));
    assert!(app.active_session().inspector.highlighted_terms.is_empty());
}

#[tokio::test]
async fn test_app_action_latch_and_unfiltered() {
    let mut app = create_test_app();

    // Latch toggle
    assert!(app.active_session().viewport.is_auto_scroll);
    app.dispatch_action(AppAction::ToggleLatch);
    assert!(!app.active_session().viewport.is_auto_scroll);
    app.dispatch_action(AppAction::ToggleLatch);
    assert!(app.active_session().viewport.is_auto_scroll);

    // Open & close unfiltered stream (yêu cầu có filter trước khi mở Unfiltered)
    app.active_session_mut().view.search.query = "level:error".to_string();
    app.dispatch_action(AppAction::OpenUnfilteredStream(Some(42)));
    assert!(app.active_session().unfiltered.is_open);
    assert_eq!(app.active_session().active_tab, ActiveTab::Unfiltered);

    // Unfiltered live toggle
    assert!(!app.active_session().unfiltered.is_live);
    app.dispatch_action(AppAction::ToggleUnfilteredLive);
    assert!(app.active_session().unfiltered.is_live);

    app.dispatch_action(AppAction::RefreshUnfilteredSnapshot);

    app.dispatch_action(AppAction::FocusInMainAndClearFilter);
    assert!(!app.active_session().unfiltered.is_open);
    assert_eq!(app.active_session().active_tab, ActiveTab::Filtered);

    app.active_session_mut().view.search.query = "level:error".to_string();
    app.dispatch_action(AppAction::OpenUnfilteredStream(None));
    assert!(app.active_session().unfiltered.is_open);
    app.dispatch_action(AppAction::CloseUnfilteredStream);
    assert!(!app.active_session().unfiltered.is_open);
}
