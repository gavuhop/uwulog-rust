//! Hierarchical Context Engine (Tầng 2 - Context-Aware Resolver).

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Định nghĩa các ngữ cảnh kích hoạt phím tắt (Key Context).
/// Khi người dùng nhấn phím, hệ thống sẽ ưu tiên kiểm tra ngữ cảnh sâu nhất (cụ thể nhất),
/// nếu không tìm thấy binding nào khớp mới fall-through về ngữ cảnh cha theo cây phân cấp hoặc `Global`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyContext {
    /// Áp dụng cho toàn bộ cửa sổ ứng dụng nếu không bị ngữ cảnh con ghi đè.
    #[default]
    Global,

    /// Bảng hiển thị logs đang được tương tác.
    Table,

    /// Ô tìm kiếm (Search Bar) đang nhận tiêu điểm (focus).
    SearchInput,

    /// Popup gợi ý tự động (Autocomplete) đang hiển thị bên trong ô tìm kiếm.
    Autocomplete,

    /// Một hộp thoại Modal thông thường đang hiển thị (Launch, Columns, About...).
    Modal,

    /// Cửa sổ quản lý WSL / Remote Servers đang mở (kế thừa từ Modal).
    RemoteServers,
}

impl KeyContext {
    /// Trả về ngữ cảnh cha trực tiếp trong cây phân cấp ngữ cảnh (Context Hierarchy).
    /// Chuỗi fall-through sẽ đi từ lá về gốc:
    /// `RemoteServers` -> `Modal` -> `Global`
    /// `Autocomplete` -> `SearchInput` -> `Global`
    pub const fn parent(&self) -> Option<Self> {
        match self {
            Self::Global => None,
            Self::Table => Some(Self::Global),
            Self::SearchInput => Some(Self::Global),
            Self::Autocomplete => Some(Self::SearchInput),
            Self::Modal => Some(Self::Global),
            Self::RemoteServers => Some(Self::Modal),
        }
    }

    /// Chuyển đổi tên chuỗi (phục vụ cấu hình JSON) sang `KeyContext`.
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().to_lowercase();
        match trimmed.as_str() {
            "global" => Some(Self::Global),
            "table" => Some(Self::Table),
            "search" | "searchinput" | "search_input" => Some(Self::SearchInput),
            "autocomplete" | "suggestion" => Some(Self::Autocomplete),
            "modal" | "dialog" => Some(Self::Modal),
            "remoteservers" | "remote_servers" | "wsl" => Some(Self::RemoteServers),
            _ => None,
        }
    }

    /// Tên chuẩn hoá (canonical name) của context.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Table => "Table",
            Self::SearchInput => "SearchInput",
            Self::Autocomplete => "Autocomplete",
            Self::Modal => "Modal",
            Self::RemoteServers => "RemoteServers",
        }
    }

    /// Danh sách tất cả các context có sẵn.
    pub const fn all() -> &'static [Self] {
        &[
            Self::Global,
            Self::Table,
            Self::SearchInput,
            Self::Autocomplete,
            Self::Modal,
            Self::RemoteServers,
        ]
    }
}

impl fmt::Display for KeyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for KeyContext {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("Unknown KeyContext: '{}'", s))
    }
}

impl Serialize for KeyContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for KeyContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).ok_or_else(|| {
            de::Error::custom(format!(
                "Unknown KeyContext '{}'. Valid values are: Global, Table, SearchInput, Autocomplete, Modal, RemoteServers",
                s
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_parse_and_as_str() {
        assert_eq!(KeyContext::parse("global"), Some(KeyContext::Global));
        assert_eq!(KeyContext::parse("table"), Some(KeyContext::Table));
        assert_eq!(
            KeyContext::parse("search_input"),
            Some(KeyContext::SearchInput)
        );
        assert_eq!(
            KeyContext::parse("autocomplete"),
            Some(KeyContext::Autocomplete)
        );
        assert_eq!(KeyContext::parse("modal"), Some(KeyContext::Modal));
        assert_eq!(KeyContext::parse("wsl"), Some(KeyContext::RemoteServers));
        assert_eq!(
            KeyContext::parse("remoteservers"),
            Some(KeyContext::RemoteServers)
        );

        assert_eq!(KeyContext::Global.as_str(), "Global");
        assert_eq!(KeyContext::Modal.as_str(), "Modal");
    }

    #[test]
    fn test_context_hierarchy() {
        assert_eq!(KeyContext::Global.parent(), None);
        assert_eq!(KeyContext::Table.parent(), Some(KeyContext::Global));
        assert_eq!(KeyContext::SearchInput.parent(), Some(KeyContext::Global));
        assert_eq!(
            KeyContext::Autocomplete.parent(),
            Some(KeyContext::SearchInput)
        );
        assert_eq!(KeyContext::Modal.parent(), Some(KeyContext::Global));
        assert_eq!(KeyContext::RemoteServers.parent(), Some(KeyContext::Modal));
    }

    #[test]
    fn test_context_serde() {
        let json = serde_json::to_string(&KeyContext::RemoteServers).unwrap();
        assert_eq!(json, "\"RemoteServers\"");

        let de: KeyContext = serde_json::from_str("\"remote_servers\"").unwrap();
        assert_eq!(de, KeyContext::RemoteServers);

        let de_wsl: KeyContext = serde_json::from_str("\"wsl\"").unwrap();
        assert_eq!(de_wsl, KeyContext::RemoteServers);
    }
}
