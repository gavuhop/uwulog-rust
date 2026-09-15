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
