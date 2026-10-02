use crate::actions::AppEvent;
use crate::app::UwuGuiApp;
use crate::session::GuiSession;
use uwu_core_schema::LogEvent;

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
                reduce_search_results(session, query_id, total_matched, logs);
            }
        }
        AppEvent::IncrementalLogsReady {
            session_id,
            matched,
            logs,
            total_processed,
        } => {
            if let Some(session) = find_session_mut(app, session_id) {
                reduce_incremental_logs(session, matched, logs, total_processed);
            }
        }
        AppEvent::IncrementalUnfilteredReady { session_id, logs } => {
            if let Some(session) = find_session_mut(app, session_id) {
                reduce_incremental_unfiltered(session, logs);
            }
        }
        AppEvent::ReversePaginationReady {
            session_id,
            before_id: _,
            logs,
            is_unfiltered,
            reached_oldest,
        } => {
            if let Some(session) = find_session_mut(app, session_id) {
                reduce_reverse_pagination(session, logs, is_unfiltered, reached_oldest);
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

/// Domain Slice: Xử lý kết quả tìm kiếm với out-of-order drop check
pub fn reduce_search_results(
    session: &mut GuiSession,
    query_id: u64,
    total_matched: usize,
    logs: Vec<LogEvent>,
) {
    // CANCELLATION & OUT-OF-ORDER CHECK:
    // Nếu kết quả trả về thuộc query cũ hơn query hiện tại, loại bỏ thẳng tay!
    if query_id < session.view.search.active_query_id {
        return;
    }
    session.apply_search_results(query_id, total_matched, logs);
}

/// Domain Slice: Xử lý log tăng dần cho luồng lọc (filtered stream)
pub fn reduce_incremental_logs(
    session: &mut GuiSession,
    matched: usize,
    logs: Vec<LogEvent>,
    total_processed: u64,
) {
    session.apply_incremental_logs(matched, logs, total_processed);
}

/// Domain Slice: Xử lý log tăng dần cho luồng thô (unfiltered stream)
pub fn reduce_incremental_unfiltered(session: &mut GuiSession, logs: Vec<LogEvent>) {
    session.apply_incremental_unfiltered(logs);
}

/// Domain Slice: Xử lý nạp trang ngược khi cuộn lên đỉnh
pub fn reduce_reverse_pagination(
    session: &mut GuiSession,
    logs: Vec<LogEvent>,
    is_unfiltered: bool,
    reached_oldest: bool,
) {
    session.apply_reverse_pagination(logs, is_unfiltered, reached_oldest);
}
