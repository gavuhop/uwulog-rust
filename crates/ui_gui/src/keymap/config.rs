//! Keymap Configuration & Persistence (Tầng 4 - User Overrides).

use super::{action::KeyAction, context::KeyContext, manager::KeymapManager};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Đại diện cho toàn bộ file cấu hình `keymap.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct KeymapConfigFile(pub Vec<KeymapSection>);

/// Một phần (Section) trong file cấu hình phím tắt.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeymapSection {
    /// Ngữ cảnh kích hoạt, ví dụ: "Global", "Autocomplete", "SearchInput", "Modal".
    #[serde(default = "default_context_str")]
    pub context: String,

    /// Bảng ánh xạ chuỗi phím bấm -> tên hành động.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindings: Option<BTreeMap<String, String>>,

    /// Bảng unbind phím tắt mặc định.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unbind: Option<BTreeMap<String, String>>,
}

fn default_context_str() -> String {
    "Global".to_string()
}

impl KeymapConfigFile {
    /// Áp dụng các cấu hình trong file này vào một `KeymapManager`.
    pub fn apply_to(&self, manager: &mut KeymapManager) {
        for section in &self.0 {
            let context = KeyContext::parse(&section.context).unwrap_or(KeyContext::Global);

            // 1. Áp dụng unbind trước
            if let Some(ref unbinds) = section.unbind {
                for keystroke_str in unbinds.keys() {
                    manager.unbind(keystroke_str, context);
                }
            }

            // 2. Áp dụng bindings mới
            if let Some(ref bindings) = section.bindings {
                for (keystroke_str, action_str) in bindings {
                    if let Some(action) = KeyAction::parse(action_str) {
                        manager.bind(keystroke_str, action, context);
                    }
                }
            }
        }
    }

    /// Sinh cấu hình mặc định dạng đối tượng để có thể xuất ra file JSON mẫu cho người dùng.
    pub fn generate_default_sample() -> Self {
        let mut global_bindings = BTreeMap::new();
        global_bindings.insert(
            "alt-p".to_string(),
            "workspace::ToggleProjectPicker".to_string(),
        );
        global_bindings.insert(
            "ctrl-pageup".to_string(),
            "workspace::PreviousSession".to_string(),
        );
        global_bindings.insert(
            "ctrl-pagedown".to_string(),
            "workspace::NextSession".to_string(),
        );
        global_bindings.insert("ctrl-=".to_string(), "window::ZoomIn".to_string());
        global_bindings.insert("ctrl--".to_string(), "window::ZoomOut".to_string());
        global_bindings.insert("ctrl-0".to_string(), "window::ResetZoom".to_string());
        global_bindings.insert("escape".to_string(), "window::Dismiss".to_string());

        let mut auto_bindings = BTreeMap::new();
        auto_bindings.insert("down".to_string(), "autocomplete::SelectNext".to_string());
        auto_bindings.insert("up".to_string(), "autocomplete::SelectPrev".to_string());
        auto_bindings.insert("enter".to_string(), "autocomplete::Confirm".to_string());
        auto_bindings.insert("tab".to_string(), "autocomplete::Confirm".to_string());

        Self(vec![
            KeymapSection {
                context: "Global".to_string(),
                bindings: Some(global_bindings),
                unbind: None,
            },
            KeymapSection {
                context: "Autocomplete".to_string(),
                bindings: Some(auto_bindings),
                unbind: None,
            },
        ])
    }
}

/// Trả về đường dẫn mặc định của file cấu hình `keymap.json` trên hệ thống.
pub fn default_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("uwulog").join("keymap.json");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("uwulog")
                .join("keymap.json");
        }
    }

    PathBuf::from("keymap.json")
}

/// Nạp file cấu hình người dùng từ đường dẫn được chỉ định (hoặc từ đường dẫn mặc định).
pub fn load_user_keymap(manager: &mut KeymapManager, path: Option<&Path>) -> Result<bool, String> {
    let target_path = path.map(PathBuf::from).unwrap_or_else(default_config_path);

    if !target_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&target_path)
        .map_err(|e| format!("Failed to read keymap file '{:?}': {}", target_path, e))?;

    let config: KeymapConfigFile = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse keymap JSON '{:?}': {}", target_path, e))?;

    config.apply_to(manager);
    Ok(true)
}

/// Lưu file mẫu cấu hình phím tắt mặc định nếu file chưa tồn tại.
pub fn ensure_sample_config_file(path: Option<&Path>) -> Result<(), String> {
    let target_path = path.map(PathBuf::from).unwrap_or_else(default_config_path);

    if target_path.exists() {
        return Ok(());
    }

    if let Some(parent) = target_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let sample = KeymapConfigFile::generate_default_sample();
    let json = serde_json::to_string_pretty(&sample)
        .map_err(|e| format!("Failed to serialize sample keymap: {}", e))?;

    std::fs::write(&target_path, json).map_err(|e| {
        format!(
            "Failed to write sample keymap file '{:?}': {}",
            target_path, e
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_keymap_json() {
        let json_str = r#"[
            {
                "context": "Global",
                "bindings": {
                    "ctrl-shift-p": "workspace::ToggleProjectPicker"
                }
            },
            {
                "context": "Autocomplete",
                "bindings": {
                    "ctrl-n": "autocomplete::SelectNext",
                    "ctrl-p": "autocomplete::SelectPrev"
                }
            }
        ]"#;

        let config: KeymapConfigFile = serde_json::from_str(json_str).unwrap();
        assert_eq!(config.0.len(), 2);

        let mut manager = KeymapManager::new();
        config.apply_to(&mut manager);

        let label =
            manager.get_label_for_action(&KeyAction::ToggleProjectPicker, KeyContext::Global);
        assert_eq!(label.as_deref(), Some("Ctrl+Shift+P"));

        let next_label =
            manager.get_label_for_action(&KeyAction::SelectNext, KeyContext::Autocomplete);
        assert_eq!(next_label.as_deref(), Some("Ctrl+N"));
    }
}
