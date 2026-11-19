//! Action & Event contracts (Zed-style Command Pattern) và Clipboard/Context-menu helpers.

pub mod clipboard;
pub use clipboard::{
    active_text_selection, copy_and_close, copy_to_clipboard, extract_selected_text,
    render_filter_actions_menu, truncate_label, ActionContext,
};

use crate::state::ActiveTab;
use uwu_core_schema::LogEvent;
use uwu_core_workspace::Workspace;

/// Nhóm hành động cấp Workspace & Session Lifecycle.
#[derive(Debug, Clone)]
pub enum WorkspaceAction {
    SwitchSession(usize),
    CloseSession(usize),
    CycleSession(bool),
    Open(Box<Workspace>),
    OpenContainerLog(Box<Workspace>),
    OpenInNewWindow(Box<Workspace>),
    OpenSessionInNewWindow(usize),
    Load(Box<Workspace>),
    Delete(uuid::Uuid),
}

/// Nhóm hành động quản lý Overlay, Modal và Floating Picker.
#[derive(Debug, Clone)]
pub enum OverlayAction {
    OpenLaunch,
    CloseLaunch,
    ApplyLaunch,
    OpenColumns,
    CloseColumns,
    ApplyColumns,
    ToggleProjectPicker,
    CloseProjectPicker,
    OpenRemoteServers,
    CloseRemoteServers,
    ToggleRemoteServers,
    ToggleSourceSwitcher,
    CloseSourceSwitcher,
    ToggleMainMenu,
    CloseMainMenu,
    OpenAbout,
    CloseAbout,
    OpenKeymap,
    CloseKeymap,
    ApplyKeymap(Box<uwu_core_keymap::KeymapManager>),
    DismissTopLayer,
}

/// Nhóm hành động tác động trực tiếp lên Session / Viewer đang hoạt động.
#[derive(Debug, Clone)]
pub enum SessionAction {
    StartSource,
    StopSource,
    RestartSource,
    SelectLog(Option<Box<LogEvent>>),
    NavigateUp,
    NavigateDown,
    ScrollTableLeft,
    ScrollTableRight,
    ScrollTableLeftMost,
    PageUp,
    PageDown,
    ScrollToTop,
    ScrollToBottom,
    FocusSearch,
    CopySelectedLog,
    SwitchTab(ActiveTab),
    ToggleStreamView,
    ToggleSearchHistory,
    ApplyFilterTerm(String),
    ExcludeFilterTerm(String),
    ClearQuery,
    CommitSearch,
    AutocompleteNext,
    AutocompletePrev,
    AutocompleteConfirm,
    ToggleRowHighlight(u64),
    ToggleTermHighlight(String),
    ClearAllHighlights,
    DismissFieldWarnings,
    ToggleLatch,
    Unlatch,
    Latch,
    RefreshUnfilteredSnapshot,
    OpenUnfilteredStream(Option<u64>),
    ViewRawContext,
    CloseUnfilteredStream,
    FocusInMainAndClearFilter,
    LoadOlderLogs(usize),
}

/// Unified Action enum phân nhóm rạch ròi giữa Workspace, Overlay, Session và Global.
#[derive(Debug, Clone)]
pub enum AppAction {
    Workspace(WorkspaceAction),
    Overlay(OverlayAction),
    Session(SessionAction),
    SwitchTheme(String),
    QuitApp,
}

impl From<WorkspaceAction> for AppAction {
    #[inline]
    fn from(action: WorkspaceAction) -> Self {
        Self::Workspace(action)
    }
}

impl From<OverlayAction> for AppAction {
    #[inline]
    fn from(action: OverlayAction) -> Self {
        Self::Overlay(action)
    }
}

impl From<SessionAction> for AppAction {
    #[inline]
    fn from(action: SessionAction) -> Self {
        Self::Session(action)
    }
}

/// Luồng sự kiện bất đồng bộ gửi từ Background Workers về UI Thread (Worker -> UI Pipeline)
#[derive(Debug)]
pub enum AppEvent {
    SearchResultsReady {
        session_id: uuid::Uuid,
        query_id: u64,
        total_matched: usize,
        logs: Vec<LogEvent>,
        is_count_exact: bool,
    },
    SearchCountUpdated {
        session_id: uuid::Uuid,
        query_id: u64,
        total_matched: usize,
    },
    IncrementalLogsReady {
        session_id: uuid::Uuid,
        matched: usize,
        logs: Vec<LogEvent>,
        total_processed: u64,
    },
    IncrementalUnfilteredReady {
        session_id: uuid::Uuid,
        logs: Vec<LogEvent>,
    },
    ReversePaginationReady {
        session_id: uuid::Uuid,
        logs: Vec<LogEvent>,
        is_unfiltered: bool,
        reached_oldest: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_action_compact_size() {
        assert!(
            std::mem::size_of::<AppAction>() <= 64,
            "AppAction grew unexpectedly to {} bytes",
            std::mem::size_of::<AppAction>()
        );
    }
}
