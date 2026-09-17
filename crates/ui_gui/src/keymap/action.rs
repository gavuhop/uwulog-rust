//! Semantic Action Registry (Tầng 2 - Command Pattern).

use crate::actions::AppAction;
use serde::{Deserialize, Serialize};

/// Danh mục các hành động (Actions) có thể được gán phím tắt trong `uwulog`.
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

            "autocomplete::SelectNext" | "SelectNext" => Some(Self::SelectNext),
            "autocomplete::SelectPrev" | "SelectPrev" => Some(Self::SelectPrev),
            "autocomplete::Confirm" | "ConfirmSelection" => Some(Self::ConfirmSelection),

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

            Self::Unbind => "unbind",
        }
    }

    /// Chuyển đổi sang `AppAction` nếu action này thuộc quyền xử lý của hàng đợi ứng dụng chính.
    pub fn to_app_action(&self) -> Option<AppAction> {
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
            Self::Unbind => None,
        }
    }
}
