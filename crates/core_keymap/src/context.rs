//! Dynamic Context Engine (Tầng 2 - Context-Aware Resolver chuẩn Zed).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

/// Định nghĩa ngữ cảnh kích hoạt phím tắt (Key Context).
/// Thay vì một enum đóng cứng, `KeyContext` sử dụng chuỗi định danh linh hoạt (Dynamic Context Tags / Hierarchical Paths)
/// tương tự kiến trúc `KeyContext` của Zed Editor.
///
/// Hỗ trợ cả các hằng số ngữ cảnh chuẩn (Global, Table, SearchInput, Modal...)
/// lẫn ngữ cảnh tùy biến người dùng định nghĩa ("Workspace > Table", "Plugin > Terminal", ...).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyContext(pub Cow<'static, str>);

const ALL_BUILTIN_CONTEXTS: &[KeyContext] = &[
    KeyContext::Global,
    KeyContext::Table,
    KeyContext::SearchInput,
    KeyContext::Autocomplete,
    KeyContext::Modal,
    KeyContext::RemoteServers,
];

#[allow(non_upper_case_globals)]
impl KeyContext {
    /// Áp dụng cho toàn bộ cửa sổ ứng dụng nếu không bị ngữ cảnh con ghi đè.
    pub const Global: Self = Self::new_static("Global");
    pub const GLOBAL: Self = Self::Global;

    /// Bảng hiển thị logs đang được tương tác.
    pub const Table: Self = Self::new_static("Table");
    pub const TABLE: Self = Self::Table;

    /// Ô tìm kiếm (Search Bar) đang nhận tiêu điểm (focus).
    pub const SearchInput: Self = Self::new_static("SearchInput");
    pub const SEARCH_INPUT: Self = Self::SearchInput;

    /// Popup gợi ý tự động (Autocomplete) đang hiển thị bên trong ô tìm kiếm.
    pub const Autocomplete: Self = Self::new_static("Autocomplete");
    pub const AUTOCOMPLETE: Self = Self::Autocomplete;

    /// Một hộp thoại Modal thông thường đang hiển thị (Launch, Columns, About...).
    pub const Modal: Self = Self::new_static("Modal");
    pub const MODAL: Self = Self::Modal;

    /// Cửa sổ quản lý WSL / Remote Servers đang mở (kế thừa từ Modal).
    pub const RemoteServers: Self = Self::new_static("RemoteServers");
    pub const REMOTE_SERVERS: Self = Self::RemoteServers;

    /// Khởi tạo một `KeyContext` từ chuỗi `&'static str` dùng trong const context.
    pub const fn new_static(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }

    /// Khởi tạo `KeyContext` từ chuỗi bất kỳ, tự động chuẩn hóa về hằng số chuẩn nếu khớp.
    pub fn new(s: impl Into<String>) -> Self {
        let s = s.into();
        let trimmed = s.trim();
        if let Some(canonical) = Self::parse_canonical(trimmed) {
            canonical
        } else {
            Self(Cow::Owned(trimmed.to_string()))
        }
    }

    /// Trả về ngữ cảnh cha trực tiếp trong cây phân cấp ngữ cảnh (Context Hierarchy).
    /// Chuỗi fall-through sẽ đi từ lá về gốc:
    /// `RemoteServers` -> `Modal` -> `Global`
    /// `Autocomplete` -> `SearchInput` -> `Global`
    /// `Workspace > Table` -> `Workspace` (Global)
    /// `A > B > C` -> `A > B` -> `A` -> `Global`
    pub fn parent(&self) -> Option<Self> {
        match self.as_str() {
            "Global" | "Workspace" => None,
            "Table" => Some(Self::Global),
            "SearchInput" => Some(Self::Global),
            "Autocomplete" => Some(Self::SearchInput),
            "Modal" => Some(Self::Global),
            "RemoteServers" => Some(Self::Modal),
            other => {
                if let Some((parent_part, _)) = other.rsplit_once(" > ") {
                    let parent_trimmed = parent_part.trim();
                    if !parent_trimmed.is_empty() {
                        return Some(Self::new(parent_trimmed));
                    }
                }
                if !other.eq_ignore_ascii_case("global") && !other.eq_ignore_ascii_case("workspace")
                {
                    Some(Self::Global)
                } else {
                    None
                }
            }
        }
    }

    /// Trả về chuỗi đường dẫn phân cấp hiển thị chuẩn Zed:
    /// `Global` -> "Workspace"
    /// `Table` -> "Workspace > Table"
    /// `SearchInput` -> "Workspace > SearchBar"
    /// `Autocomplete` -> "Workspace > SearchBar > Autocomplete"
    /// `Modal` -> "Modal"
    /// `RemoteServers` -> "Modal > RemoteServers"
    pub fn display_path(&self) -> &str {
        match self.as_str() {
            "Global" | "Workspace" => "Workspace",
            "Table" => "Workspace > Table",
            "SearchInput" => "Workspace > SearchBar",
            "Autocomplete" => "Workspace > SearchBar > Autocomplete",
            "Modal" => "Modal",
            "RemoteServers" => "Modal > RemoteServers",
            other => other,
        }
    }

    /// Chuẩn hóa chuỗi tên context về các hằng số chuẩn nếu khớp.
    fn parse_canonical(trimmed: &str) -> Option<Self> {
        for ctx in Self::all() {
            if ctx.as_str().eq_ignore_ascii_case(trimmed)
                || ctx.display_path().eq_ignore_ascii_case(trimmed)
            {
                return Some(ctx.clone());
            }
        }

        let lower = trimmed.to_lowercase();
        match lower.as_str() {
            "workspace" | "global" => Some(Self::Global),
            "workspace > table" | "table" => Some(Self::Table),
            "workspace > searchbar" | "search" | "searchinput" | "search_input" => {
                Some(Self::SearchInput)
            }
            "workspace > searchbar > autocomplete" | "autocomplete" | "suggestion" => {
                Some(Self::Autocomplete)
            }
            "modal" | "dialog" => Some(Self::Modal),
            "modal > remoteservers" | "remoteservers" | "remote_servers" | "wsl" => {
                Some(Self::RemoteServers)
            }
            _ => None,
        }
    }

    /// Chuyển đổi tên chuỗi (phục vụ cấu hình JSON hoặc nhập text) sang `KeyContext`.
    /// Nếu là một chuỗi tùy biến không nằm trong built-ins, vẫn giữ nguyên ngữ cảnh động.
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(canonical) = Self::parse_canonical(trimmed) {
            Some(canonical)
        } else {
            Some(Self::new(trimmed))
        }
    }

    /// Kiểm tra xem context này có phải là một trong các built-in contexts chuẩn của ứng dụng hay không.
    pub fn is_builtin(&self) -> bool {
        ALL_BUILTIN_CONTEXTS.iter().any(|c| c == self)
    }

    /// Kiểm tra tính hợp lệ cú pháp của chuỗi Context:
    /// - Cho phép phân cách bởi `>` hoặc `::`
    /// - Từng phân đoạn không được rỗng (ví dụ: `A > > B`, `> A`, `A >` đều không hợp lệ)
    /// - Tên phân đoạn chỉ chứa chữ cái, chữ số, khoảng trắng, gạch dưới `_`, gạch nối `-`
    pub fn validate_syntax(s: &str) -> Result<String, ContextParseError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ContextParseError::Empty);
        }

        let delimiter = if trimmed.contains("::") { "::" } else { ">" };
        let parts: Vec<&str> = trimmed.split(delimiter).collect();

        let mut cleaned_segments = Vec::with_capacity(parts.len());
        for part in parts {
            let seg = part.trim();
            if seg.is_empty() {
                return Err(ContextParseError::EmptySegment);
            }
            for c in seg.chars() {
                if !c.is_alphanumeric() && c != '_' && c != '-' && c != ' ' {
                    return Err(ContextParseError::InvalidCharacter(c));
                }
            }
            cleaned_segments.push(seg.to_string());
        }

        Ok(cleaned_segments.join(" > "))
    }

    /// Chuẩn hóa chuỗi và parse thành KeyContext nếu cú pháp hợp lệ.
    pub fn parse_normalized(s: &str) -> Result<Self, ContextParseError> {
        let normalized = Self::validate_syntax(s)?;
        if let Some(canonical) = Self::parse_canonical(&normalized) {
            Ok(canonical)
        } else {
            Ok(Self::new(normalized))
        }
    }

    /// Tên chuẩn hoá (canonical name) của context.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    /// Danh sách tất cả các built-in contexts có sẵn.
    pub const fn all() -> &'static [Self] {
        ALL_BUILTIN_CONTEXTS
    }
}

/// Lỗi cú pháp khi phân tích chuỗi đường dẫn Context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextParseError {
    Empty,
    EmptySegment,
    InvalidCharacter(char),
}

impl fmt::Display for ContextParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "Context cannot be empty"),
            Self::EmptySegment => write!(f, "Context path contains an empty segment"),
            Self::InvalidCharacter(c) => write!(f, "Invalid character '{c}' in context name"),
        }
    }
}

impl std::error::Error for ContextParseError {}

impl Default for KeyContext {
    fn default() -> Self {
        Self::Global
    }
}

impl fmt::Debug for KeyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyContext({})", self.as_str())
    }
}

impl fmt::Display for KeyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for KeyContext {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s).unwrap_or_else(|| Self::new(s)))
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
        Ok(Self::parse(&s).unwrap_or_else(|| Self::new(s)))
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
    fn test_dynamic_custom_context() {
        let custom = KeyContext::parse("Plugin > MyPanel").unwrap();
        assert_eq!(custom.display_path(), "Plugin > MyPanel");
        assert_eq!(custom.parent(), Some(KeyContext::new("Plugin")));
        assert_eq!(custom.parent().unwrap().parent(), Some(KeyContext::Global));
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

        let custom_de: KeyContext = serde_json::from_str("\"MyCustomContext\"").unwrap();
        assert_eq!(custom_de, KeyContext::new("MyCustomContext"));
    }

    #[test]
    fn test_context_validation_and_normalization() {
        assert_eq!(
            KeyContext::validate_syntax("Workspace > Table").unwrap(),
            "Workspace > Table"
        );
        assert_eq!(
            KeyContext::validate_syntax("workspace > table").unwrap(),
            "workspace > table"
        );
        assert_eq!(
            KeyContext::validate_syntax("Plugin::CustomView").unwrap(),
            "Plugin > CustomView"
        );

        // Canonical mapping via parse_normalized
        let normalized = KeyContext::parse_normalized("workspace > table").unwrap();
        assert_eq!(normalized, KeyContext::Table);
        assert!(normalized.is_builtin());

        let custom = KeyContext::parse_normalized("Plugin > Terminal").unwrap();
        assert_eq!(custom, KeyContext::new("Plugin > Terminal"));
        assert!(!custom.is_builtin());

        // Error cases
        assert_eq!(
            KeyContext::validate_syntax("   ").unwrap_err(),
            ContextParseError::Empty
        );
        assert_eq!(
            KeyContext::validate_syntax("Workspace > > Table").unwrap_err(),
            ContextParseError::EmptySegment
        );
        assert_eq!(
            KeyContext::validate_syntax("Workspace > ").unwrap_err(),
            ContextParseError::EmptySegment
        );
        assert_eq!(
            KeyContext::validate_syntax("> Workspace").unwrap_err(),
            ContextParseError::EmptySegment
        );
        assert_eq!(
            KeyContext::validate_syntax("Workspace @ Table").unwrap_err(),
            ContextParseError::InvalidCharacter('@')
        );
    }
}
