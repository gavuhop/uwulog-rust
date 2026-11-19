//! Action & Event contracts (Zed-style Command Pattern) và Clipboard/Context-menu helpers.

pub mod clipboard;
pub use clipboard::{
    active_text_selection, copy_and_close, copy_to_clipboard, extract_selected_text,
    render_filter_actions_menu, truncate_label, ActionContext,
};

use crate::state::ActiveTab;
use uwu_core_schema::LogEvent;
use uwu_core_workspace::Workspace;

/// Unified Action enum for high-level application & session state mutations
#[derive(Debug, Clone)]
pub enum AppAction {
    SwitchSession(usize),
    CloseSession(usize),
    CycleSession(bool),
    OpenWorkspace(Box<Workspace>),
    OpenContainerLogSession(Box<Workspace>),
    OpenWorkspaceInNewWindow(Box<Workspace>),
    OpenSessionInNewWindow(usize),
    LoadWorkspace(Box<Workspace>),
    DeleteWorkspace(uuid::Uuid),
    StartSource,
    StopSource,
    RestartSource,
    OpenLaunchModal,
    CloseLaunchModal,
    ApplyLaunchModal,
    OpenColumnsModal,
    CloseColumnsModal,
    ApplyColumnsModal,
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
    CopySelectedLog,
    SwitchTab(ActiveTab),
    ToggleStreamView,

    // Search Query & Filtering
    FocusSearch,
    ToggleSearchHistory,
    ApplyFilterTerm(String),
    ExcludeFilterTerm(String),
    ClearQuery,

    // Highlights
    ToggleRowHighlight(u64),
    ToggleTermHighlight(String),
    ClearAllHighlights,
    DismissFieldWarnings,

    // Stream & Latch Controls
    ToggleLatch,
    Unlatch,
    Latch,
    RefreshUnfilteredSnapshot,
    OpenUnfilteredStream(Option<u64>),
    ViewRawContext,
    CloseUnfilteredStream,
    FocusInMainAndClearFilter,
    LoadOlderLogs(usize),

    // Project Picker
    ToggleProjectPicker,
    CloseProjectPicker,

    // Remote Servers (WSL / SSH / Dev Container)
    OpenRemoteServersModal,
    CloseRemoteServersModal,
    ToggleRemoteServersModal,

    // Log Source / Container Switcher
    ToggleSourceSwitcher,
    CloseSourceSwitcher,

    // Main Menu & About
    ToggleMainMenu,
    CloseMainMenu,
    OpenAboutModal,
    CloseAboutModal,
    OpenKeymapModal,
    CloseKeymapModal,
    ApplyKeymapModal(Box<uwu_core_keymap::KeymapManager>),
    SwitchTheme(String),
    QuitApp,

    // Global Dismiss / Stack Pop
    DismissTopLayer,

    // Autocomplete & Search Navigation
    CommitSearch,
    AutocompleteNext,
    AutocompletePrev,
    AutocompleteConfirm,
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
