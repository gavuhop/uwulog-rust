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
            Self::ToggleLatch => Some(AppAction::ToggleLatch),
            Self::CommitSearch => Some(AppAction::CommitSearch),
            Self::ClearSearch => Some(AppAction::ClearQuery),
            Self::SelectNext => Some(AppAction::AutocompleteNext),
            Self::SelectPrev => Some(AppAction::AutocompletePrev),
            Self::ConfirmSelection => Some(AppAction::AutocompleteConfirm),
            Self::Back | Self::TabComplete | Self::Unbind => None,
        }
    }
}
