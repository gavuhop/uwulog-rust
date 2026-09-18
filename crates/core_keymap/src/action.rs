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

    // Workspace & Session Controls
    ToggleProjectPicker => ("workspace::ToggleProjectPicker", ["ToggleProjectPicker"]),
    PreviousSession => ("workspace::PreviousSession", ["PreviousSession"]),
    NextSession => ("workspace::NextSession", ["NextSession"]),
    ToggleRemoteServers => ("workspace::ToggleRemoteServers", ["ToggleRemoteServers"]),
    OpenLaunchModal => ("workspace::OpenLaunchModal", ["OpenLaunchModal"]),
    OpenColumnsModal => ("workspace::OpenColumnsModal", ["OpenColumnsModal"]),
    ToggleLatch => ("workspace::ToggleLatch", ["ToggleLatch"]),

    // Search Controls
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
