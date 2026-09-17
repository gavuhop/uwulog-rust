//! Keystroke & Low-level Chord Parser (Tầng 1 - Zed-Style Input Model).

use eframe::egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystrokeParseError(pub String);

impl fmt::Display for KeystrokeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid keystroke format: \"{}\"", self.0)
    }
}

impl std::error::Error for KeystrokeParseError {}

/// Biểu diễn một tổ hợp phím gồm modifiers (Ctrl, Alt, Shift, Cmd/Win) và Key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Keystroke {
    #[serde(with = "key_serde")]
    pub key: Key,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub mac_cmd: bool,
}

impl Keystroke {
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            alt: false,
            shift: false,
            mac_cmd: false,
        }
    }

    pub const fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub const fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub const fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub const fn with_cmd(mut self) -> Self {
        self.mac_cmd = true;
        self
    }

    /// Phân tích cú pháp chuỗi phím canonical theo phong cách Zed:
    /// ví dụ: `"ctrl-shift-p"`, `"alt-p"`, `"ctrl-="`, `"ctrl--"`, `"escape"`, `"pageup"`.
    pub fn parse(s: &str) -> Result<Self, KeystrokeParseError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(KeystrokeParseError("Empty keystroke string".to_string()));
        }

        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut mac_cmd = false;

        // Xử lý trường hợp đặc biệt: chuỗi kết thúc bằng dấu trừ (như "ctrl--", "alt--", hoặc chỉ "-")
        let (tokens, key_part) = if let Some(prefix) = trimmed.strip_suffix("--") {
            let parts: Vec<&str> = prefix.split('-').filter(|p| !p.is_empty()).collect();
            (parts, "-")
        } else if trimmed == "-" {
            (Vec::new(), "-")
        } else {
            let mut parts: Vec<&str> = trimmed.split('-').collect();
            let key_str = parts.pop().unwrap_or("");
            (parts, key_str)
        };

        for modifier in tokens {
            match modifier.trim().to_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" | "opt" | "option" => alt = true,
                "shift" => shift = true,
                "cmd" | "command" | "super" | "win" => mac_cmd = true,
                other => {
                    return Err(KeystrokeParseError(format!("Unknown modifier '{}'", other)));
                }
            }
        }

        let key = parse_key(key_part)
            .ok_or_else(|| KeystrokeParseError(format!("Unknown key '{}'", key_part)))?;

        Ok(Self {
            key,
            ctrl,
            alt,
            shift,
            mac_cmd,
        })
    }

    /// Kiểm tra tổ hợp phím có khớp với `egui::InputState` trong frame hiện tại không.
    pub fn matches(&self, input: &eframe::egui::InputState) -> bool {
        if !input.key_pressed(self.key) {
            return false;
        }

        self.matches_modifiers(&input.modifiers)
    }

    /// Kiểm tra chỉ riêng phần modifiers
    pub fn matches_modifiers(&self, mods: &Modifiers) -> bool {
        // Trên Windows/Linux: Ctrl và Command thường đồng nghĩa trong egui
        let ctrl_match = if self.ctrl {
            mods.ctrl || mods.command
        } else {
            !mods.ctrl && (!mods.command || self.mac_cmd)
        };

        let alt_match = self.alt == mods.alt;
        let shift_match = self.shift == mods.shift;
        let mac_match = if self.mac_cmd {
            mods.mac_cmd || mods.command
        } else {
            true
        };

        ctrl_match && alt_match && shift_match && mac_match
    }

    /// Định dạng nhãn hiển thị thân thiện trên UI (ví dụ trên tooltip hoặc nút bấm): "Alt+P", "Ctrl+PageUp"
    pub fn format_label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.mac_cmd {
            parts.push("Cmd");
        }

        parts.push(format_key(self.key));
        parts.join("+")
    }
}

/// Helper ánh xạ chuỗi sang `egui::Key`
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

/// Helper format `egui::Key` sang tên chuỗi dễ đọc
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
        _ => "Key",
    }
}

mod key_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(key: &Key, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(format_key(*key))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Key, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_key(&s).ok_or_else(|| serde::de::Error::custom(format!("Unknown key '{}'", s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_keystrokes() {
        let p1 = Keystroke::parse("alt-p").unwrap();
        assert_eq!(p1.key, Key::P);
        assert!(p1.alt);
        assert!(!p1.ctrl);
        assert_eq!(p1.format_label(), "Alt+P");

        let p2 = Keystroke::parse("ctrl-shift-f").unwrap();
        assert_eq!(p2.key, Key::F);
        assert!(p2.ctrl);
        assert!(p2.shift);
        assert!(!p2.alt);
        assert_eq!(p2.format_label(), "Ctrl+Shift+F");

        let p3 = Keystroke::parse("escape").unwrap();
        assert_eq!(p3.key, Key::Escape);
        assert!(!p3.ctrl);
        assert_eq!(p3.format_label(), "Esc");

        let p4 = Keystroke::parse("ctrl-pageup").unwrap();
        assert_eq!(p4.key, Key::PageUp);
        assert!(p4.ctrl);
        assert_eq!(p4.format_label(), "Ctrl+PageUp");

        let p5 = Keystroke::parse("ctrl-=").unwrap();
        assert_eq!(p5.key, Key::Equals);
        assert!(p5.ctrl);
        assert_eq!(p5.format_label(), "Ctrl+=");

        let p6 = Keystroke::parse("ctrl--").unwrap();
        assert_eq!(p6.key, Key::Minus);
        assert!(p6.ctrl);
        assert_eq!(p6.format_label(), "Ctrl+-");
    }
}
