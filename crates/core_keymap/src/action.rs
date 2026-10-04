//! Semantic Action Registry (Tầng 3 - Command Pattern).

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

macro_rules! define_actions {
    ($(
        $(#[$meta:meta])*
        $variant:ident => ($canonical:literal, [$( $alias:literal ),* $(,)?])
    ),* $(,)?) => {
        /// Danh mục các hành động ngữ nghĩa (Semantic Actions) có thể được gán phím tắt trong `uwulog`.
        /// Định dạng tên canonical dạng `namespace::Action` tương tự như Zed Editor.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum KeyAction {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        impl KeyAction {
            /// Trả về chuỗi canonical identifier chuẩn.
            pub const fn canonical_name(&self) -> &'static str {
                match self {
                    $(
                        Self::$variant => $canonical,
                    )*
                }
            }

            /// Phân tích cú pháp từ chuỗi định danh canonical hoặc alias thông dụng.
            pub fn parse(s: &str) -> Option<Self> {
                let trimmed = s.trim();
                match trimmed {
                    $(
                        $canonical $(| $alias)* => Some(Self::$variant),
                    )*
                    _ => None,
                }
            }
        }
    };
}

define_actions! {
    // Window & View Controls
    Dismiss => ("window::Dismiss", ["Dismiss"]),
    Quit => ("window::Quit", ["Quit"]),
    ZoomIn => ("window::ZoomIn", ["ZoomIn"]),
    ZoomOut => ("window::ZoomOut", ["ZoomOut"]),
    ResetZoom => ("window::ResetZoom", ["ResetZoom"]),
    OpenKeymapModal => ("window::OpenKeymapModal", ["OpenKeymapModal", "Keybindings", "Keymap"]),

    // Workspace & Session Controls
    ToggleProjectPicker => ("workspace::ToggleProjectPicker", ["ToggleProjectPicker"]),
    PreviousSession => ("workspace::PreviousSession", ["PreviousSession"]),
    NextSession => ("workspace::NextSession", ["NextSession"]),
    ToggleRemoteServers => ("workspace::ToggleRemoteServers", ["ToggleRemoteServers"]),
    OpenLaunchModal => ("workspace::OpenLaunchModal", ["OpenLaunchModal", "ConfigureSource", "RunCommandSettings"]),
    OpenColumnsModal => ("workspace::OpenColumnsModal", ["OpenColumnsModal", "ColumnsSettings", "ConfigureColumns"]),
    RestartSource => ("workspace::RestartSource", ["RestartSource", "RerunCommand", "Rerun", "Restart"]),
    ToggleLatch => ("workspace::ToggleLatch", ["ToggleLatch"]),

    // Search Controls
    FocusFilter => ("search::Focus", ["FocusFilter", "FocusSearch", "Filter"]),
    ToggleSearchHistory => ("search::ToggleHistory", ["ToggleSearchHistory", "SearchHistory", "FilterHistory"]),
    CommitSearch => ("search::Commit", ["CommitSearch"]),
    ClearSearch => ("search::Clear", ["ClearSearch"]),

    // Autocomplete & List Navigation
    SelectNext => ("autocomplete::SelectNext", ["SelectNext", "menu::SelectNext"]),
    SelectPrev => ("autocomplete::SelectPrev", ["SelectPrev", "menu::SelectPrev"]),
    ConfirmSelection => ("autocomplete::Confirm", ["ConfirmSelection", "menu::Confirm"]),

    // Navigation & Editing
    Back => ("menu::Back", ["modal::Back", "window::Back", "Back"]),
    TabComplete => ("picker::TabComplete", ["tab::Complete", "TabComplete"]),

    // Đặc biệt: Hủy gán phím tắt (Unbind)
    Unbind => ("unbind", ["Unbind"]),
}

impl KeyAction {
    /// Danh sách tất cả các hành động có thể gán phím tắt trong ứng dụng.
    pub const fn all() -> &'static [Self] {
        &[
            Self::OpenKeymapModal,
            Self::ToggleProjectPicker,
            Self::PreviousSession,
            Self::NextSession,
            Self::ToggleRemoteServers,
            Self::OpenLaunchModal,
            Self::OpenColumnsModal,
            Self::RestartSource,
            Self::ToggleLatch,
            Self::FocusFilter,
            Self::ToggleSearchHistory,
            Self::CommitSearch,
            Self::ClearSearch,
            Self::ZoomIn,
            Self::ZoomOut,
            Self::ResetZoom,
            Self::Dismiss,
            Self::Quit,
            Self::SelectNext,
            Self::SelectPrev,
            Self::ConfirmSelection,
            Self::Back,
            Self::TabComplete,
        ]
    }

    /// Nhóm phân loại chức năng (Category).
    pub const fn category(&self) -> &'static str {
        match self {
            Self::OpenKeymapModal
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ResetZoom
            | Self::Dismiss
            | Self::Quit => "Window",

            Self::ToggleProjectPicker
            | Self::PreviousSession
            | Self::NextSession
            | Self::ToggleRemoteServers
            | Self::OpenLaunchModal
            | Self::OpenColumnsModal
            | Self::RestartSource
            | Self::ToggleLatch => "Workspace",

            Self::FocusFilter
            | Self::ToggleSearchHistory
            | Self::CommitSearch
            | Self::ClearSearch => "Search",

            Self::SelectNext
            | Self::SelectPrev
            | Self::ConfirmSelection
            | Self::Back
            | Self::TabComplete => "Navigation",

            Self::Unbind => "System",
        }
    }

    /// Tên hiển thị người dùng thân thiện (Human-Readable Display Name).
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::OpenKeymapModal => "Keyboard Shortcuts",
            Self::Dismiss => "Dismiss Top Layer",
            Self::Quit => "Quit Application",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ResetZoom => "Reset Zoom (100%)",

            Self::ToggleProjectPicker => "Switch Project / Workspace",
            Self::PreviousSession => "Previous Workspace Session",
            Self::NextSession => "Next Workspace Session",
            Self::ToggleRemoteServers => "Remote Servers & WSL",
            Self::OpenLaunchModal => "Run Command & Source Settings",
            Self::OpenColumnsModal => "Customize Table Columns",
            Self::RestartSource => "Rerun Command / Restart Source",
            Self::ToggleLatch => "Toggle Latch / Follow Mode",

            Self::FocusFilter => "Focus Search / Filter Bar",
            Self::ToggleSearchHistory => "Search & Filter History",
            Self::CommitSearch => "Commit Search Query",
            Self::ClearSearch => "Clear Search Bar",

            Self::SelectNext => "Select Next Item",
            Self::SelectPrev => "Select Previous Item",
            Self::ConfirmSelection => "Confirm Selection",
            Self::Back => "Back / Previous Screen",
            Self::TabComplete => "Tab Autocomplete",
            Self::Unbind => "Unbind Shortcut",
        }
    }

    /// Mô tả chi tiết hành động.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::OpenKeymapModal => "Open keyboard shortcuts management dialog",
            Self::Dismiss => "Close top modal, context menu, or popup dialog",
            Self::Quit => "Exit and quit the application",
            Self::ZoomIn => "Increase interface scale and font size",
            Self::ZoomOut => "Decrease interface scale and font size",
            Self::ResetZoom => "Reset interface scale back to default 100%",

            Self::ToggleProjectPicker => "Open the Zed-style workspace and project switcher",
            Self::PreviousSession => "Cycle to the previous active workspace tab",
            Self::NextSession => "Cycle to the next active workspace tab",
            Self::ToggleRemoteServers => "Open the WSL distribution and remote server picker",
            Self::OpenLaunchModal => "Open log source and run command configuration modal",
            Self::OpenColumnsModal => "Open column visibility and reordering dialog",
            Self::RestartSource => "Restart running command source and re-fetch log stream",
            Self::ToggleLatch => "Toggle auto-scrolling to the latest live log entries",

            Self::FocusFilter => "Focus the search and filter query input",
            Self::ToggleSearchHistory => "Toggle filter and search query history dropdown",
            Self::CommitSearch => "Execute and apply filter query to the log stream",
            Self::ClearSearch => "Clear all text in the search input field",

            Self::SelectNext => "Move selection highlight down to next item",
            Self::SelectPrev => "Move selection highlight up to previous item",
            Self::ConfirmSelection => "Confirm the highlighted item or autocomplete token",
            Self::Back => "Navigate back to previous screen or close modal",
            Self::TabComplete => "Accept suggested autocomplete text",
            Self::Unbind => "Disable or unbind an existing key combination",
        }
    }

    /// Chuyển đổi tên Action canonical sang tên humanized chuẩn Zed Editor (dựa trên thuật toán `command_palette::humanize_action_name` của Zed)
    /// Ví dụ: `workspace::ToggleProjectPicker` -> `workspace: toggle project picker`
    pub fn humanized_name(&self) -> String {
        let canonical = self.canonical_name();
        let mut result = String::with_capacity(canonical.len() + 4);
        let mut prev_char: Option<char> = None;
        let mut in_name_part = false;

        for c in canonical.chars() {
            if c == ':' {
                if !result.ends_with(':') {
                    result.push(':');
                } else if !result.ends_with(": ") {
                    result.push(' ');
                    in_name_part = true;
                }
            } else if in_name_part {
                if c.is_uppercase() {
                    if let Some(p) = prev_char {
                        if !p.is_uppercase() && p != ' ' && p != ':' {
                            result.push(' ');
                        }
                    }
                    for lower in c.to_lowercase() {
                        result.push(lower);
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
            prev_char = Some(c);
        }

        result
    }
}

impl fmt::Display for KeyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical_name())
    }
}

impl FromStr for KeyAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown KeyAction: '{}'", s))
    }
}

impl Serialize for KeyAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.canonical_name())
    }
}

impl<'de> Deserialize<'de> for KeyAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).ok_or_else(|| de::Error::custom(format!("Unknown KeyAction '{}'", s)))
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
        assert_eq!(
            format!("{}", KeyAction::ToggleProjectPicker),
            "workspace::ToggleProjectPicker"
        );
    }

    #[test]
    fn test_action_serde() {
        let action = KeyAction::ToggleProjectPicker;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"workspace::ToggleProjectPicker\"");
        let deserialized: KeyAction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);

        let deserialized_alias: KeyAction =
            serde_json::from_str("\"ToggleProjectPicker\"").unwrap();
        assert_eq!(deserialized_alias, action);
    }
}
