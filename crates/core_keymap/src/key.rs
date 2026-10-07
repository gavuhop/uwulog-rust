//! Universal Keyboard Key Representation.
//!
//! Tách biệt hoàn toàn khái niệm phím bấm khỏi UI framework cụ thể,
//! đồng thời cung cấp adapter chuyển đổi hai chiều cho `egui`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Đại diện cho các phím bấm vật lý tiêu chuẩn trên bàn phím.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Digits
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // Navigation & Editing
    Escape,
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Punctuation & Operators
    Equals,
    Plus,
    Minus,
    OpenBracket,
    CloseBracket,
    Comma,
    Period,
    Slash,
    Backslash,
    Semicolon,
    Quote,

    // Function Keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(format_key(*self))
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_key(&s).ok_or_else(|| serde::de::Error::custom(format!("Unknown key '{}'", s)))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format_key(*self))
    }
}

impl FromStr for Key {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_key(s).ok_or_else(|| format!("Unknown key '{}'", s))
    }
}

/// Helper ánh xạ chuỗi sang `Key` (hỗ trợ không phân biệt hoa thường và các alias thông dụng).
pub fn parse_key(s: &str) -> Option<Key> {
    let lower = s.trim().to_lowercase();
    match lower.as_str() {
        // Letters
        "a" => Some(Key::A),
        "b" => Some(Key::B),
        "c" => Some(Key::C),
        "d" => Some(Key::D),
        "e" => Some(Key::E),
        "f" => Some(Key::F),
        "g" => Some(Key::G),
        "h" => Some(Key::H),
        "i" => Some(Key::I),
        "j" => Some(Key::J),
        "k" => Some(Key::K),
        "l" => Some(Key::L),
        "m" => Some(Key::M),
        "n" => Some(Key::N),
        "o" => Some(Key::O),
        "p" => Some(Key::P),
        "q" => Some(Key::Q),
        "r" => Some(Key::R),
        "s" => Some(Key::S),
        "t" => Some(Key::T),
        "u" => Some(Key::U),
        "v" => Some(Key::V),
        "w" => Some(Key::W),
        "x" => Some(Key::X),
        "y" => Some(Key::Y),
        "z" => Some(Key::Z),

        // Digits
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),

        // Navigation & Editing
        "escape" | "esc" => Some(Key::Escape),
        "enter" | "return" => Some(Key::Enter),
        "tab" => Some(Key::Tab),
        "space" => Some(Key::Space),
        "backspace" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        "insert" => Some(Key::Insert),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "pageup" | "page_up" => Some(Key::PageUp),
        "pagedown" | "page_down" => Some(Key::PageDown),
        "up" | "arrowup" | "arrow_up" => Some(Key::ArrowUp),
        "down" | "arrowdown" | "arrow_down" => Some(Key::ArrowDown),
        "left" | "arrowleft" | "arrow_left" => Some(Key::ArrowLeft),
        "right" | "arrowright" | "arrow_right" => Some(Key::ArrowRight),

        // Punctuation & Operators
        "=" | "equals" | "equal" => Some(Key::Equals),
        "+" | "plus" => Some(Key::Plus),
        "-" | "minus" => Some(Key::Minus),
        "[" | "openbracket" => Some(Key::OpenBracket),
        "]" | "closebracket" => Some(Key::CloseBracket),
        "," | "comma" => Some(Key::Comma),
        "." | "period" => Some(Key::Period),
        "/" | "slash" => Some(Key::Slash),
        "\\" | "backslash" => Some(Key::Backslash),
        ";" | "semicolon" => Some(Key::Semicolon),
        "'" | "quote" => Some(Key::Quote),

        // Function Keys
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),

        _ => None,
    }
}

/// Helper format `Key` sang chuỗi thân thiện cho nhãn hiển thị (UI label).
pub fn format_key(key: Key) -> &'static str {
    match key {
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::ArrowUp => "Up",
        Key::Escape => "Esc",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::Enter => "Enter",
        Key::Space => "Space",
        Key::Insert => "Insert",
        Key::Delete => "Delete",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Minus => "-",
        Key::Plus => "+",
        Key::Equals => "=",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Backslash => "\\",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num7 => "7",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
    }
}

// ---------------------------------------------------------------------------
// egui conversions
// ---------------------------------------------------------------------------
#[cfg(feature = "egui")]
impl Key {
    /// Chuyển đổi sang `egui::Key` nếu tương thích.
    pub fn to_egui(self) -> Option<egui::Key> {
        match self {
            Key::ArrowDown => Some(egui::Key::ArrowDown),
            Key::ArrowLeft => Some(egui::Key::ArrowLeft),
            Key::ArrowRight => Some(egui::Key::ArrowRight),
            Key::ArrowUp => Some(egui::Key::ArrowUp),
            Key::Escape => Some(egui::Key::Escape),
            Key::Tab => Some(egui::Key::Tab),
            Key::Backspace => Some(egui::Key::Backspace),
            Key::Enter => Some(egui::Key::Enter),
            Key::Space => Some(egui::Key::Space),
            Key::Insert => Some(egui::Key::Insert),
            Key::Delete => Some(egui::Key::Delete),
            Key::Home => Some(egui::Key::Home),
            Key::End => Some(egui::Key::End),
            Key::PageUp => Some(egui::Key::PageUp),
            Key::PageDown => Some(egui::Key::PageDown),
            Key::Minus => Some(egui::Key::Minus),
            Key::Plus => Some(egui::Key::Plus),
            Key::Equals => Some(egui::Key::Equals),
            Key::OpenBracket => Some(egui::Key::OpenBracket),
            Key::CloseBracket => Some(egui::Key::CloseBracket),
            Key::Comma => Some(egui::Key::Comma),
            Key::Period => Some(egui::Key::Period),
            Key::Slash => Some(egui::Key::Slash),
            Key::Backslash => Some(egui::Key::Backslash),
            Key::Semicolon => Some(egui::Key::Semicolon),
            Key::Quote => Some(egui::Key::Quote),
            Key::Num0 => Some(egui::Key::Num0),
            Key::Num1 => Some(egui::Key::Num1),
            Key::Num2 => Some(egui::Key::Num2),
            Key::Num3 => Some(egui::Key::Num3),
            Key::Num4 => Some(egui::Key::Num4),
            Key::Num5 => Some(egui::Key::Num5),
            Key::Num6 => Some(egui::Key::Num6),
            Key::Num7 => Some(egui::Key::Num7),
            Key::Num8 => Some(egui::Key::Num8),
            Key::Num9 => Some(egui::Key::Num9),
            Key::A => Some(egui::Key::A),
            Key::B => Some(egui::Key::B),
            Key::C => Some(egui::Key::C),
            Key::D => Some(egui::Key::D),
            Key::E => Some(egui::Key::E),
            Key::F => Some(egui::Key::F),
            Key::G => Some(egui::Key::G),
            Key::H => Some(egui::Key::H),
            Key::I => Some(egui::Key::I),
            Key::J => Some(egui::Key::J),
            Key::K => Some(egui::Key::K),
            Key::L => Some(egui::Key::L),
            Key::M => Some(egui::Key::M),
            Key::N => Some(egui::Key::N),
            Key::O => Some(egui::Key::O),
            Key::P => Some(egui::Key::P),
            Key::Q => Some(egui::Key::Q),
            Key::R => Some(egui::Key::R),
            Key::S => Some(egui::Key::S),
            Key::T => Some(egui::Key::T),
            Key::U => Some(egui::Key::U),
            Key::V => Some(egui::Key::V),
            Key::W => Some(egui::Key::W),
            Key::X => Some(egui::Key::X),
            Key::Y => Some(egui::Key::Y),
            Key::Z => Some(egui::Key::Z),
            Key::F1 => Some(egui::Key::F1),
            Key::F2 => Some(egui::Key::F2),
            Key::F3 => Some(egui::Key::F3),
            Key::F4 => Some(egui::Key::F4),
            Key::F5 => Some(egui::Key::F5),
            Key::F6 => Some(egui::Key::F6),
            Key::F7 => Some(egui::Key::F7),
            Key::F8 => Some(egui::Key::F8),
            Key::F9 => Some(egui::Key::F9),
            Key::F10 => Some(egui::Key::F10),
            Key::F11 => Some(egui::Key::F11),
            Key::F12 => Some(egui::Key::F12),
        }
    }

    /// Khởi tạo `Key` từ `egui::Key`.
    pub fn from_egui(k: egui::Key) -> Option<Self> {
        match k {
            egui::Key::ArrowDown => Some(Key::ArrowDown),
            egui::Key::ArrowLeft => Some(Key::ArrowLeft),
            egui::Key::ArrowRight => Some(Key::ArrowRight),
            egui::Key::ArrowUp => Some(Key::ArrowUp),
            egui::Key::Escape => Some(Key::Escape),
            egui::Key::Tab => Some(Key::Tab),
            egui::Key::Backspace => Some(Key::Backspace),
            egui::Key::Enter => Some(Key::Enter),
            egui::Key::Space => Some(Key::Space),
            egui::Key::Insert => Some(Key::Insert),
            egui::Key::Delete => Some(Key::Delete),
            egui::Key::Home => Some(Key::Home),
            egui::Key::End => Some(Key::End),
            egui::Key::PageUp => Some(Key::PageUp),
            egui::Key::PageDown => Some(Key::PageDown),
            egui::Key::Minus => Some(Key::Minus),
            egui::Key::Plus => Some(Key::Plus),
            egui::Key::Equals => Some(Key::Equals),
            egui::Key::OpenBracket => Some(Key::OpenBracket),
            egui::Key::CloseBracket => Some(Key::CloseBracket),
            egui::Key::Comma => Some(Key::Comma),
            egui::Key::Period => Some(Key::Period),
            egui::Key::Slash => Some(Key::Slash),
            egui::Key::Backslash => Some(Key::Backslash),
            egui::Key::Semicolon => Some(Key::Semicolon),
            egui::Key::Quote => Some(Key::Quote),
            egui::Key::Num0 => Some(Key::Num0),
            egui::Key::Num1 => Some(Key::Num1),
            egui::Key::Num2 => Some(Key::Num2),
            egui::Key::Num3 => Some(Key::Num3),
            egui::Key::Num4 => Some(Key::Num4),
            egui::Key::Num5 => Some(Key::Num5),
            egui::Key::Num6 => Some(Key::Num6),
            egui::Key::Num7 => Some(Key::Num7),
            egui::Key::Num8 => Some(Key::Num8),
            egui::Key::Num9 => Some(Key::Num9),
            egui::Key::A => Some(Key::A),
            egui::Key::B => Some(Key::B),
            egui::Key::C => Some(Key::C),
            egui::Key::D => Some(Key::D),
            egui::Key::E => Some(Key::E),
            egui::Key::F => Some(Key::F),
            egui::Key::G => Some(Key::G),
            egui::Key::H => Some(Key::H),
            egui::Key::I => Some(Key::I),
            egui::Key::J => Some(Key::J),
            egui::Key::K => Some(Key::K),
            egui::Key::L => Some(Key::L),
            egui::Key::M => Some(Key::M),
            egui::Key::N => Some(Key::N),
            egui::Key::O => Some(Key::O),
            egui::Key::P => Some(Key::P),
            egui::Key::Q => Some(Key::Q),
            egui::Key::R => Some(Key::R),
            egui::Key::S => Some(Key::S),
            egui::Key::T => Some(Key::T),
            egui::Key::U => Some(Key::U),
            egui::Key::V => Some(Key::V),
            egui::Key::W => Some(Key::W),
            egui::Key::X => Some(Key::X),
            egui::Key::Y => Some(Key::Y),
            egui::Key::Z => Some(Key::Z),
            egui::Key::F1 => Some(Key::F1),
            egui::Key::F2 => Some(Key::F2),
            egui::Key::F3 => Some(Key::F3),
            egui::Key::F4 => Some(Key::F4),
            egui::Key::F5 => Some(Key::F5),
            egui::Key::F6 => Some(Key::F6),
            egui::Key::F7 => Some(Key::F7),
            egui::Key::F8 => Some(Key::F8),
            egui::Key::F9 => Some(Key::F9),
            egui::Key::F10 => Some(Key::F10),
            egui::Key::F11 => Some(Key::F11),
            egui::Key::F12 => Some(Key::F12),
            _ => None,
        }
    }
}

#[cfg(feature = "egui")]
impl TryFrom<egui::Key> for Key {
    type Error = ();

    fn try_from(k: egui::Key) -> Result<Self, Self::Error> {
        Key::from_egui(k).ok_or(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_parse_and_format() {
        assert_eq!(parse_key("enter"), Some(Key::Enter));
        assert_eq!(parse_key("esc"), Some(Key::Escape));
        assert_eq!(parse_key("escape"), Some(Key::Escape));
        assert_eq!(parse_key("a"), Some(Key::A));
        assert_eq!(parse_key("Z"), Some(Key::Z));
        assert_eq!(parse_key("f1"), Some(Key::F1));
        assert_eq!(parse_key("f12"), Some(Key::F12));
        assert_eq!(parse_key("pageup"), Some(Key::PageUp));
        assert_eq!(parse_key("pagedown"), Some(Key::PageDown));
        assert_eq!(parse_key("="), Some(Key::Equals));
        assert_eq!(parse_key("-"), Some(Key::Minus));
        assert_eq!(parse_key("+"), Some(Key::Plus));

        assert_eq!(format_key(Key::Enter), "Enter");
        assert_eq!(format_key(Key::Escape), "Esc");
        assert_eq!(format_key(Key::A), "A");
        assert_eq!(format_key(Key::PageUp), "PageUp");
    }

    #[test]
    fn test_key_serde_roundtrip() {
        let key = Key::Enter;
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, "\"Enter\"");
        let deserialized: Key = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, key);

        let key_p: Key = serde_json::from_str("\"p\"").unwrap();
        assert_eq!(key_p, Key::P);
    }

    #[cfg(feature = "egui")]
    #[test]
    fn test_key_egui_conversion() {
        assert_eq!(Key::Enter.to_egui(), Some(egui::Key::Enter));
        assert_eq!(Key::from_egui(egui::Key::Enter), Some(Key::Enter));
        assert_eq!(Key::Escape.to_egui(), Some(egui::Key::Escape));
        assert_eq!(Key::P.to_egui(), Some(egui::Key::P));
    }
}
