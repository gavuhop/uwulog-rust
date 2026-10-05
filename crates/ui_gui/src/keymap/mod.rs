//! Zed-Style 4-Layer Keybindings Manager (Re-exported from `uwu-core-keymap`).

pub use uwu_core_keymap::*;

use crate::actions::AppAction;

/// Extension trait ánh xạ `KeyAction` sang `AppAction` của tầng GUI logic.
pub trait KeyActionExt {
    fn to_app_action(&self) -> Option<AppAction>;
}

impl KeyActionExt for KeyAction {
    fn to_app_action(&self) -> Option<AppAction> {
        match self {
            Self::Dismiss => Some(AppAction::DismissTopLayer),
            Self::Quit => Some(AppAction::QuitApp),
            Self::ZoomIn => Some(AppAction::ZoomIn),
            Self::ZoomOut => Some(AppAction::ZoomOut),
            Self::ResetZoom => Some(AppAction::ResetZoom),
            Self::ToggleProjectPicker => Some(AppAction::ToggleProjectPicker),
            Self::PreviousSession => Some(AppAction::CycleSession(false)),
            Self::NextSession => Some(AppAction::CycleSession(true)),
            Self::ToggleRemoteServers => Some(AppAction::ToggleRemoteServersModal),
            Self::OpenLaunchModal => Some(AppAction::OpenLaunchModal),
            Self::OpenColumnsModal => Some(AppAction::OpenColumnsModal),
            Self::OpenKeymapModal => Some(AppAction::OpenKeymapModal),
            Self::RestartSource => Some(AppAction::RestartSource),
            Self::ToggleLatch => Some(AppAction::ToggleLatch),
            Self::ToggleStreamView => Some(AppAction::ToggleStreamView),
            Self::SelectMainView => Some(AppAction::SwitchTab(crate::state::ActiveTab::Filtered)),
            Self::SelectRawView => Some(AppAction::SwitchTab(crate::state::ActiveTab::Unfiltered)),
            Self::ViewRawContext => Some(AppAction::ViewRawContext),
            Self::FocusFilter => Some(AppAction::FocusSearch),
            Self::ToggleSearchHistory => Some(AppAction::ToggleSearchHistory),
            Self::CommitSearch => Some(AppAction::CommitSearch),
            Self::ClearSearch => Some(AppAction::ClearQuery),
            Self::SelectNext => Some(AppAction::AutocompleteNext),
            Self::SelectPrev => Some(AppAction::AutocompletePrev),
            Self::ConfirmSelection => Some(AppAction::AutocompleteConfirm),
            Self::ScrollLeft => Some(AppAction::ScrollTableLeft),
            Self::ScrollRight => Some(AppAction::ScrollTableRight),
            Self::PageUp => Some(AppAction::PageUp),
            Self::PageDown => Some(AppAction::PageDown),
            Self::ScrollToTop => Some(AppAction::ScrollToTop),
            Self::ScrollToBottom => Some(AppAction::ScrollToBottom),
            Self::CopySelection => Some(AppAction::CopySelectedLog),
            Self::Back | Self::TabComplete | Self::Unbind => None,
        }
    }
}
