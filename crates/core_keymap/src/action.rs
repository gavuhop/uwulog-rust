//! Semantic Action Registry (Tầng 3 - Command Pattern).

use serde::{Deserialize, Serialize};

/// Danh mục các hành động ngữ nghĩa (Semantic Actions) có thể được gán phím tắt trong `uwulog`.
/// Định dạng tên canonical dạng `namespace::Action` tương tự như Zed Editor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyAction {
    // Window & View Controls
    #[serde(rename = "window::Dismiss")]
    Dismiss,
    #[serde(rename = "window::Quit")]
    Quit,
    #[serde(rename = "window::ZoomIn")]
    ZoomIn,
    #[serde(rename = "window::ZoomOut")]
    ZoomOut,
    #[serde(rename = "window::ResetZoom")]
    ResetZoom,

    // Workspace & Session Controls
    #[serde(rename = "workspace::ToggleProjectPicker")]
    ToggleProjectPicker,
    #[serde(rename = "workspace::PreviousSession")]
    PreviousSession,
    #[serde(rename = "workspace::NextSession")]
    NextSession,
    #[serde(rename = "workspace::ToggleRemoteServers")]
    ToggleRemoteServers,
    #[serde(rename = "workspace::OpenLaunchModal")]
    OpenLaunchModal,
    #[serde(rename = "workspace::OpenColumnsModal")]
    OpenColumnsModal,
    #[serde(rename = "workspace::ToggleLatch")]
    ToggleLatch,

    // Search Controls
    #[serde(rename = "search::Commit")]
    CommitSearch,
    #[serde(rename = "search::Clear")]
    ClearSearch,

    // Autocomplete & List Navigation
    #[serde(rename = "autocomplete::SelectNext")]
    SelectNext,
    #[serde(rename = "autocomplete::SelectPrev")]
    SelectPrev,
    #[serde(rename = "autocomplete::Confirm")]
    ConfirmSelection,

    // Navigation & Editing
    #[serde(rename = "menu::Back")]
    Back,
    #[serde(rename = "picker::TabComplete")]
    TabComplete,

    // Đặc biệt: Hủy gán phím tắt (Unbind)
    #[serde(rename = "unbind")]
    Unbind,
}

impl KeyAction {
    /// Phân tích cú pháp từ chuỗi định danh canonical.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "window::Dismiss" | "Dismiss" => Some(Self::Dismiss),
            "window::Quit" | "Quit" => Some(Self::Quit),
            "window::ZoomIn" | "ZoomIn" => Some(Self::ZoomIn),
            "window::ZoomOut" | "ZoomOut" => Some(Self::ZoomOut),
            "window::ResetZoom" | "ResetZoom" => Some(Self::ResetZoom),

            "workspace::ToggleProjectPicker" | "ToggleProjectPicker" => {
                Some(Self::ToggleProjectPicker)
            }
            "workspace::PreviousSession" | "PreviousSession" => Some(Self::PreviousSession),
            "workspace::NextSession" | "NextSession" => Some(Self::NextSession),
            "workspace::ToggleRemoteServers" | "ToggleRemoteServers" => {
                Some(Self::ToggleRemoteServers)
            }
            "workspace::OpenLaunchModal" | "OpenLaunchModal" => Some(Self::OpenLaunchModal),
            "workspace::OpenColumnsModal" | "OpenColumnsModal" => Some(Self::OpenColumnsModal),
            "workspace::ToggleLatch" | "ToggleLatch" => Some(Self::ToggleLatch),

            "search::Commit" | "CommitSearch" => Some(Self::CommitSearch),
            "search::Clear" | "ClearSearch" => Some(Self::ClearSearch),

            "autocomplete::SelectNext" | "SelectNext" | "menu::SelectNext" => {
                Some(Self::SelectNext)
            }
            "autocomplete::SelectPrev" | "SelectPrev" | "menu::SelectPrev" => {
                Some(Self::SelectPrev)
            }
            "autocomplete::Confirm" | "ConfirmSelection" | "menu::Confirm" => {
                Some(Self::ConfirmSelection)
            }

            "menu::Back" | "modal::Back" | "window::Back" | "Back" => Some(Self::Back),
            "picker::TabComplete" | "tab::Complete" | "TabComplete" => Some(Self::TabComplete),

            "unbind" | "Unbind" => Some(Self::Unbind),
            _ => None,
        }
    }

    /// Trả về chuỗi canonical identifier chuẩn.
    pub fn canonical_name(&self) -> &'static str {
        match self {
            Self::Dismiss => "window::Dismiss",
            Self::Quit => "window::Quit",
            Self::ZoomIn => "window::ZoomIn",
            Self::ZoomOut => "window::ZoomOut",
            Self::ResetZoom => "window::ResetZoom",

            Self::ToggleProjectPicker => "workspace::ToggleProjectPicker",
            Self::PreviousSession => "workspace::PreviousSession",
            Self::NextSession => "workspace::NextSession",
            Self::ToggleRemoteServers => "workspace::ToggleRemoteServers",
            Self::OpenLaunchModal => "workspace::OpenLaunchModal",
            Self::OpenColumnsModal => "workspace::OpenColumnsModal",
            Self::ToggleLatch => "workspace::ToggleLatch",

            Self::CommitSearch => "search::Commit",
            Self::ClearSearch => "search::Clear",

            Self::SelectNext => "autocomplete::SelectNext",
            Self::SelectPrev => "autocomplete::SelectPrev",
            Self::ConfirmSelection => "autocomplete::Confirm",

            Self::Back => "menu::Back",
            Self::TabComplete => "picker::TabComplete",

            Self::Unbind => "unbind",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_parse_and_canonical_name() {
        assert_eq!(
            KeyAction::parse("workspace::ToggleProjectPicker"),
            Some(KeyAction::ToggleProjectPicker)
        );
        assert_eq!(
            KeyAction::parse("ToggleProjectPicker"),
            Some(KeyAction::ToggleProjectPicker)
        );
        assert_eq!(
            KeyAction::parse("window::Dismiss"),
            Some(KeyAction::Dismiss)
        );
        assert_eq!(
            KeyAction::parse("autocomplete::Confirm"),
            Some(KeyAction::ConfirmSelection)
        );
        assert_eq!(KeyAction::parse("menu::Back"), Some(KeyAction::Back));
        assert_eq!(KeyAction::parse("unbind"), Some(KeyAction::Unbind));

        assert_eq!(
            KeyAction::ToggleProjectPicker.canonical_name(),
            "workspace::ToggleProjectPicker"
        );
        assert_eq!(KeyAction::Dismiss.canonical_name(), "window::Dismiss");
    }

    #[test]
    fn test_action_serde() {
        let action = KeyAction::ToggleProjectPicker;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"workspace::ToggleProjectPicker\"");
        let deserialized: KeyAction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);
    }
}
