//! Zed-Style 4-Layer Keybindings Manager (Re-exported from `uwu-core-keymap`).

pub use uwu_core_keymap::*;

use crate::actions::{AppAction, OverlayAction, SessionAction, WorkspaceAction};

/// Extension trait ánh xạ `KeyAction` sang `AppAction` của tầng GUI logic.
pub trait KeyActionExt {
    fn to_app_action(&self) -> Option<AppAction>;
}

impl KeyActionExt for KeyAction {
    fn to_app_action(&self) -> Option<AppAction> {
        match self {
            Self::Dismiss => Some(OverlayAction::DismissTopLayer.into()),
            Self::Quit => Some(AppAction::QuitApp),
            // Zoom được xử lý trực tiếp trên egui::Context trong app/keybindings.rs
            Self::ZoomIn | Self::ZoomOut | Self::ResetZoom => None,
            Self::ToggleProjectPicker => Some(OverlayAction::ToggleProjectPicker.into()),
            Self::PreviousSession => Some(WorkspaceAction::CycleSession(false).into()),
            Self::NextSession => Some(WorkspaceAction::CycleSession(true).into()),
            Self::ToggleRemoteServers => Some(OverlayAction::ToggleRemoteServers.into()),
            Self::OpenLaunchModal => Some(OverlayAction::OpenLaunch.into()),
            Self::OpenColumnsModal => Some(OverlayAction::OpenColumns.into()),
            Self::OpenKeymapModal => Some(OverlayAction::OpenKeymap.into()),
            Self::RestartSource => Some(SessionAction::RestartSource.into()),
            Self::ToggleLatch => Some(SessionAction::ToggleLatch.into()),
            Self::ToggleStreamView => Some(SessionAction::ToggleStreamView.into()),
            Self::SelectMainView => {
                Some(SessionAction::SwitchTab(crate::state::ActiveTab::Filtered).into())
            }
            Self::SelectRawView => {
                Some(SessionAction::SwitchTab(crate::state::ActiveTab::Unfiltered).into())
            }
            Self::ViewRawContext => Some(SessionAction::ViewRawContext.into()),
            Self::FocusFilter => Some(SessionAction::FocusSearch.into()),
            Self::ToggleSearchHistory => Some(SessionAction::ToggleSearchHistory.into()),
            Self::CommitSearch => Some(SessionAction::CommitSearch.into()),
            Self::ClearSearch => Some(SessionAction::ClearQuery.into()),
            Self::SelectNext => Some(SessionAction::AutocompleteNext.into()),
            Self::SelectPrev => Some(SessionAction::AutocompletePrev.into()),
            Self::ConfirmSelection => Some(SessionAction::AutocompleteConfirm.into()),
            Self::ScrollLeft => Some(SessionAction::ScrollTableLeft.into()),
            Self::ScrollRight => Some(SessionAction::ScrollTableRight.into()),
            Self::ScrollToLeft => Some(SessionAction::ScrollTableLeftMost.into()),
            Self::PageUp => Some(SessionAction::PageUp.into()),
            Self::PageDown => Some(SessionAction::PageDown.into()),
            Self::ScrollToTop => Some(SessionAction::ScrollToTop.into()),
            Self::ScrollToBottom => Some(SessionAction::ScrollToBottom.into()),
            Self::CopySelection => Some(SessionAction::CopySelectedLog.into()),
            Self::Back | Self::TabComplete | Self::Unbind => None,
        }
    }
}
