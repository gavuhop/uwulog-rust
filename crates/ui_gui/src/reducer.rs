use crate::actions::AppEvent;
use crate::app::UwuGuiApp;
use crate::session::GuiSession;

/// State Reducer: Xử lý tập trung các sự kiện bất đồng bộ trả về từ Background Workers
/// Chỉ duy nhất UI Thread sở hữu quyền ghi `&mut AppState` thông qua hàm reduce này (O(1)).
pub fn reduce(app: &mut UwuGuiApp, event: AppEvent) {
    match event {
        AppEvent::SearchResultsReady {
            session_id,
            query_id,
            total_matched,
            logs,
        } => {
            if let Some(session) = find_session_mut(app, session_id) {
                session.apply_search_results(query_id, total_matched, logs);
            }
        }
        AppEvent::IncrementalLogsReady {
            session_id,
            matched,
            logs,
            total_processed,
        } => {
            if let Some(session) = find_session_mut(app, session_id) {
                session.apply_incremental_logs(matched, logs, total_processed);
            }
        }
        AppEvent::IncrementalUnfilteredReady { session_id, logs } => {
            if let Some(session) = find_session_mut(app, session_id) {
                session.apply_incremental_unfiltered(logs);
            }
        }
        AppEvent::ReversePaginationReady {
            session_id,
            logs,
            is_unfiltered,
            reached_oldest,
        } => {
            if let Some(session) = find_session_mut(app, session_id) {
                session.apply_reverse_pagination(logs, is_unfiltered, reached_oldest);
            }
        }
    }
}

#[inline]
fn find_session_mut(app: &mut UwuGuiApp, session_id: uuid::Uuid) -> Option<&mut GuiSession> {
    app.workspaces
        .sessions
        .iter_mut()
        .find(|s| s.session.id == session_id)
}
