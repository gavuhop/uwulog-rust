//! Keystroke & Chord Parser (Tầng 1 - Zed-Style Input Model).

use crate::key::{format_key, parse_key, Key};
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
            true
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

    // -----------------------------------------------------------------------
    // crossterm integration
    // -----------------------------------------------------------------------
    #[cfg(feature = "crossterm")]
    pub fn matches_crossterm(&self, event: &crossterm::event::KeyEvent) -> bool {
        if let Some(expected_code) = self.key.to_crossterm() {
            if event.code != expected_code {
                return false;
            }
            self.matches_crossterm_modifiers(&event.modifiers)
        } else {
            false
        }
    }

    #[cfg(feature = "crossterm")]
    pub fn matches_crossterm_modifiers(&self, mods: &crossterm::event::KeyModifiers) -> bool {
        use crossterm::event::KeyModifiers;
        let ctrl_match = self.ctrl == mods.contains(KeyModifiers::CONTROL);
        let alt_match = self.alt == mods.contains(KeyModifiers::ALT);
        let shift_match = self.shift == mods.contains(KeyModifiers::SHIFT);
        let super_match = if self.mac_cmd {
            mods.contains(KeyModifiers::SUPER)
        } else {
            true
        };

        ctrl_match && alt_match && shift_match && super_match
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
