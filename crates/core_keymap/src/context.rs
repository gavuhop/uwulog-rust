//! Hierarchical Context Engine (Tầng 2 - Context-Aware Resolver).

use serde::{Deserialize, Serialize};

/// Định nghĩa các ngữ cảnh kích hoạt phím tắt (Key Context).
/// Khi người dùng nhấn phím, hệ thống sẽ ưu tiên kiểm tra ngữ cảnh sâu nhất (cụ thể nhất),
/// nếu không tìm thấy binding nào khớp mới fall-through về ngữ cảnh cha hoặc `Global`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum KeyContext {
    /// Áp dụng cho toàn bộ cửa sổ ứng dụng nếu không bị ngữ cảnh con ghi đè.
    #[default]
    Global,

    /// Bảng hiển thị logs đang được tương tác.
    Table,

    /// Ô tìm kiếm (Search Bar) đang nhận tiêu điểm (focus).
    SearchInput,

    /// Popup gợi ý tự động (Autocomplete) đang hiển thị.
    Autocomplete,

    /// Một hộp thoại Modal thông thường đang hiển thị (Launch, Columns, About...).
    Modal,

    /// Cửa sổ quản lý WSL / Remote Servers đang mở (có Back Stack nội bộ).
    RemoteServers,
}

impl KeyContext {
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
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Table => "Table",
            Self::SearchInput => "SearchInput",
            Self::Autocomplete => "Autocomplete",
            Self::Modal => "Modal",
            Self::RemoteServers => "RemoteServers",
        }
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
}
