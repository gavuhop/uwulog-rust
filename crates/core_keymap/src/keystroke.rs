//! Keystroke Parser & Modifier Representation (Tầng 1 - Zed-Style Input Model).

use crate::key::{format_key, parse_key, Key};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystrokeParseError(pub String);

impl fmt::Display for KeystrokeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid keystroke format: \"{}\"", self.0)
    }
}

impl std::error::Error for KeystrokeParseError {}

/// Biểu diễn một tổ hợp phím gồm modifiers (Ctrl, Alt, Shift, Cmd/Win) và Key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Keystroke {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
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

    pub const fn ctrl(key: Key) -> Self {
        Self::new(key).with_ctrl()
    }

    pub const fn alt(key: Key) -> Self {
        Self::new(key).with_alt()
    }

    pub const fn shift(key: Key) -> Self {
        Self::new(key).with_shift()
    }

    pub const fn cmd(key: Key) -> Self {
        Self::new(key).with_cmd()
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
    /// hỗ trợ cả dấu `-` lẫn `+`:
    /// ví dụ: `"ctrl-shift-p"`, `"Ctrl+Shift+P"`, `"alt-p"`, `"ctrl-="`, `"ctrl--"`, `"escape"`, `"pageup"`.
    pub fn parse(s: &str) -> Result<Self, KeystrokeParseError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(KeystrokeParseError("Empty keystroke string".to_string()));
        }

        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut mac_cmd = false;

        // Xử lý trường hợp đặc biệt: chuỗi kết thúc bằng dấu trừ hoặc cộng (như "ctrl-", "ctrl+", "ctrl--", "-", "+")
        let (tokens, key_part): (Vec<&str>, &str) = if trimmed == "-" {
            (Vec::new(), "-")
        } else if trimmed == "+" {
            (Vec::new(), "+")
        } else if let Some(prefix) = trimmed.strip_suffix('-') {
            let parts: Vec<&str> = prefix.split(['-', '+']).filter(|p| !p.is_empty()).collect();
            (parts, "-")
        } else if let Some(prefix) = trimmed.strip_suffix('+') {
            let parts: Vec<&str> = prefix.split(['-', '+']).filter(|p| !p.is_empty()).collect();
            (parts, "+")
        } else {
            let mut parts: Vec<&str> = trimmed
                .split(['-', '+'])
                .filter(|p| !p.is_empty())
                .collect();
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

    /// Trả về chuỗi canonical identifier chuẩn dạng kebab-case (dùng cho serialize `keymap.json`).
    pub fn canonical_string(&self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("ctrl");
        }
        if self.alt {
            parts.push("alt");
        }
        if self.shift {
            parts.push("shift");
        }
        if self.mac_cmd {
            parts.push("cmd");
        }
        let key_str = match self.key {
            Key::Minus => "-",
            Key::Plus => "+",
            Key::Equals => "=",
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
            Key::A => "a",
            Key::B => "b",
            Key::C => "c",
            Key::D => "d",
            Key::E => "e",
            Key::F => "f",
            Key::G => "g",
            Key::H => "h",
            Key::I => "i",
            Key::J => "j",
            Key::K => "k",
            Key::L => "l",
            Key::M => "m",
            Key::N => "n",
            Key::O => "o",
            Key::P => "p",
            Key::Q => "q",
            Key::R => "r",
            Key::S => "s",
            Key::T => "t",
            Key::U => "u",
            Key::V => "v",
            Key::W => "w",
            Key::X => "x",
            Key::Y => "y",
            Key::Z => "z",
            Key::Escape => "escape",
            Key::Enter => "enter",
            Key::Tab => "tab",
            Key::Space => "space",
            Key::Backspace => "backspace",
            Key::Delete => "delete",
            Key::Insert => "insert",
            Key::Home => "home",
            Key::End => "end",
            Key::PageUp => "pageup",
            Key::PageDown => "pagedown",
            Key::ArrowUp => "up",
            Key::ArrowDown => "down",
            Key::ArrowLeft => "left",
            Key::ArrowRight => "right",
            Key::OpenBracket => "[",
            Key::CloseBracket => "]",
            Key::Comma => ",",
            Key::Period => ".",
            Key::Slash => "/",
            Key::Backslash => "\\",
            Key::Semicolon => ";",
            Key::Quote => "'",
            Key::F1 => "f1",
            Key::F2 => "f2",
            Key::F3 => "f3",
            Key::F4 => "f4",
            Key::F5 => "f5",
            Key::F6 => "f6",
            Key::F7 => "f7",
            Key::F8 => "f8",
            Key::F9 => "f9",
            Key::F10 => "f10",
            Key::F11 => "f11",
            Key::F12 => "f12",
        };
        parts.push(key_str);
        parts.join("-")
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

    // -----------------------------------------------------------------------
    // egui integration
    // -----------------------------------------------------------------------
    #[cfg(feature = "egui")]
    pub fn matches(&self, input: &egui::InputState) -> bool {
        // egui-winit phát sinh Event::Copy thay vì Event::Key khi bấm Ctrl+C / Cmd+C
        if self.key == Key::C
            && (self.ctrl || self.mac_cmd)
            && !self.alt
            && !self.shift
            && input.events.iter().any(|e| matches!(e, egui::Event::Copy))
        {
            return true;
        }

        if let Some(egui_key) = self.key.to_egui() {
            if !input.key_pressed(egui_key) {
                return false;
            }
            self.matches_modifiers(&input.modifiers)
        } else {
            false
        }
    }

    #[cfg(feature = "egui")]
    pub fn matches_modifiers(&self, mods: &egui::Modifiers) -> bool {
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
            !mods.mac_cmd
        };

        ctrl_match && alt_match && shift_match && mac_match
    }

    #[cfg(feature = "egui")]
    pub fn to_egui_modifiers(&self) -> egui::Modifiers {
        egui::Modifiers {
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
            mac_cmd: self.mac_cmd,
            command: self.ctrl || self.mac_cmd,
        }
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical_string())
    }
}

impl FromStr for Keystroke {
    type Err = KeystrokeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for Keystroke {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.canonical_string())
    }
}

impl<'de> Deserialize<'de> for Keystroke {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(de::Error::custom)
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
        assert_eq!(p1.canonical_string(), "alt-p");

        let p2 = Keystroke::parse("ctrl-shift-f").unwrap();
        assert_eq!(p2.key, Key::F);
        assert!(p2.ctrl);
        assert!(p2.shift);
        assert!(!p2.alt);
        assert_eq!(p2.format_label(), "Ctrl+Shift+F");
        assert_eq!(p2.canonical_string(), "ctrl-shift-f");

        let p3 = Keystroke::parse("escape").unwrap();
        assert_eq!(p3.key, Key::Escape);
        assert!(!p3.ctrl);
        assert_eq!(p3.format_label(), "Esc");
        assert_eq!(p3.canonical_string(), "escape");

        let p4 = Keystroke::parse("ctrl-pageup").unwrap();
        assert_eq!(p4.key, Key::PageUp);
        assert!(p4.ctrl);
        assert_eq!(p4.format_label(), "Ctrl+PageUp");
        assert_eq!(p4.canonical_string(), "ctrl-pageup");

        let p5 = Keystroke::parse("ctrl-=").unwrap();
        assert_eq!(p5.key, Key::Equals);
        assert!(p5.ctrl);
        assert_eq!(p5.format_label(), "Ctrl+=");
        assert_eq!(p5.canonical_string(), "ctrl-=");

        let p6 = Keystroke::parse("ctrl--").unwrap();
        assert_eq!(p6.key, Key::Minus);
        assert!(p6.ctrl);
        assert_eq!(p6.format_label(), "Ctrl+-");
        assert_eq!(p6.canonical_string(), "ctrl--");

        // Test with '+' separator
        let p7 = Keystroke::parse("Ctrl+Shift+P").unwrap();
        assert_eq!(p7.key, Key::P);
        assert!(p7.ctrl);
        assert!(p7.shift);
        assert_eq!(p7.canonical_string(), "ctrl-shift-p");

        // Test with single trailing minus and plus
        let p8 = Keystroke::parse("ctrl-").unwrap();
        assert_eq!(p8.key, Key::Minus);
        assert!(p8.ctrl);

        let p9 = Keystroke::parse("ctrl+").unwrap();
        assert_eq!(p9.key, Key::Plus);
        assert!(p9.ctrl);
    }

    #[test]
    fn test_keystroke_serde() {
        let ks = Keystroke::ctrl(Key::P).with_shift();
        let json = serde_json::to_string(&ks).unwrap();
        assert_eq!(json, "\"ctrl-shift-p\"");

        let de: Keystroke = serde_json::from_str(&json).unwrap();
        assert_eq!(de, ks);
    }
}
