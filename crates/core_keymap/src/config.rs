//! Keymap Configuration & Persistence (Tầng 4 - User Overrides).

use crate::action::KeyAction;
use crate::context::KeyContext;
use crate::key::Key;
use crate::keystroke::Keystroke;
use crate::manager::KeymapManager;
use serde::{Deserialize, Deserializer, Serialize};
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
    #[serde(default)]
    pub context: KeyContext,

    /// Bảng ánh xạ phím bấm -> hành động.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindings: Option<BTreeMap<Keystroke, KeyAction>>,

    /// Danh sách unbind phím tắt (chấp nhận cả Array chuỗi `["alt-p"]` lẫn Map `{"alt-p": ...}`).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_unbind_list"
    )]
    pub unbind: Option<Vec<Keystroke>>,
}

fn deserialize_unbind_list<'de, D>(deserializer: D) -> Result<Option<Vec<Keystroke>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum UnbindHelper {
        List(Vec<Keystroke>),
        Map(BTreeMap<Keystroke, serde_json::Value>),
    }

    let helper = Option::<UnbindHelper>::deserialize(deserializer)?;
    match helper {
        None => Ok(None),
        Some(UnbindHelper::List(list)) => Ok(Some(list)),
        Some(UnbindHelper::Map(map)) => Ok(Some(map.into_keys().collect())),
    }
}

impl KeymapConfigFile {
    /// Áp dụng các cấu hình trong file này vào một `KeymapManager`.
    pub fn apply_to(&self, manager: &mut KeymapManager) {
        for section in &self.0 {
            let context = &section.context;

            // 1. Áp dụng unbind trước
            if let Some(ref unbinds) = section.unbind {
                for ks in unbinds {
                    manager.unbind_keystroke(*ks, context.clone());
                }
            }

            // 2. Áp dụng bindings mới
            if let Some(ref bindings) = section.bindings {
                for (ks, action) in bindings {
                    manager.bind_keystroke(*ks, action.clone(), context.clone());
                }
            }
        }
    }

    /// Sinh cấu hình mặc định dạng đối tượng để có thể xuất ra file JSON mẫu cho người dùng.
    /// Sử dụng typed Keystroke và KeyAction thay vì hardcoded string.
    pub fn generate_default_sample() -> Self {
        let mut global_bindings = BTreeMap::new();
        global_bindings.insert(Keystroke::alt(Key::P), KeyAction::ToggleProjectPicker);
        global_bindings.insert(Keystroke::ctrl(Key::PageUp), KeyAction::PreviousSession);
        global_bindings.insert(Keystroke::ctrl(Key::PageDown), KeyAction::NextSession);
        global_bindings.insert(Keystroke::ctrl(Key::Equals), KeyAction::ZoomIn);
        global_bindings.insert(Keystroke::ctrl(Key::Minus), KeyAction::ZoomOut);
        global_bindings.insert(Keystroke::ctrl(Key::Num0), KeyAction::ResetZoom);
        global_bindings.insert(Keystroke::new(Key::Escape), KeyAction::Dismiss);
        global_bindings.insert(Keystroke::ctrl(Key::F), KeyAction::FocusFilter);
        global_bindings.insert(Keystroke::ctrl(Key::H), KeyAction::ToggleSearchHistory);
        global_bindings.insert(Keystroke::ctrl(Key::R), KeyAction::RestartSource);
        global_bindings.insert(Keystroke::new(Key::F5), KeyAction::RestartSource);
        global_bindings.insert(Keystroke::alt(Key::R), KeyAction::OpenLaunchModal);
        global_bindings.insert(Keystroke::alt(Key::C), KeyAction::OpenColumnsModal);
        global_bindings.insert(Keystroke::ctrl(Key::Tab), KeyAction::ToggleStreamView);
        global_bindings.insert(Keystroke::alt(Key::Num1), KeyAction::SelectMainView);
        global_bindings.insert(Keystroke::alt(Key::Num2), KeyAction::SelectRawView);
        global_bindings.insert(Keystroke::alt(Key::V), KeyAction::ViewRawContext);

        let mut auto_bindings = BTreeMap::new();
        auto_bindings.insert(Keystroke::new(Key::ArrowDown), KeyAction::SelectNext);
        auto_bindings.insert(Keystroke::new(Key::ArrowUp), KeyAction::SelectPrev);
        auto_bindings.insert(Keystroke::new(Key::Enter), KeyAction::ConfirmSelection);
        auto_bindings.insert(Keystroke::new(Key::Tab), KeyAction::ConfirmSelection);

        let mut table_bindings = BTreeMap::new();
        table_bindings.insert(Keystroke::new(Key::ArrowDown), KeyAction::SelectNext);
        table_bindings.insert(Keystroke::new(Key::ArrowUp), KeyAction::SelectPrev);
        table_bindings.insert(Keystroke::new(Key::ArrowLeft), KeyAction::ScrollLeft);
        table_bindings.insert(Keystroke::new(Key::ArrowRight), KeyAction::ScrollRight);
        table_bindings.insert(Keystroke::new(Key::PageUp), KeyAction::PageUp);
        table_bindings.insert(Keystroke::new(Key::PageDown), KeyAction::PageDown);
        table_bindings.insert(Keystroke::ctrl(Key::Home), KeyAction::ScrollToTop);
        table_bindings.insert(Keystroke::ctrl(Key::ArrowUp), KeyAction::ScrollToTop);
        table_bindings.insert(Keystroke::ctrl(Key::End), KeyAction::ScrollToBottom);
        table_bindings.insert(Keystroke::ctrl(Key::ArrowDown), KeyAction::ScrollToBottom);
        table_bindings.insert(Keystroke::ctrl(Key::C), KeyAction::CopySelection);

        Self(vec![
            KeymapSection {
                context: KeyContext::Global,
                bindings: Some(global_bindings),
                unbind: None,
            },
            KeymapSection {
                context: KeyContext::Autocomplete,
                bindings: Some(auto_bindings),
                unbind: None,
            },
            KeymapSection {
                context: KeyContext::Table,
                bindings: Some(table_bindings),
                unbind: None,
            },
        ])
    }
}

/// Trả về đường dẫn mặc định của file cấu hình `keymap.json` trên hệ thống.
pub fn default_config_path() -> PathBuf {
    // 1. Cho phép ghi đè thông qua biến môi trường (phục vụ test, CI hoặc portable mode)
    if let Ok(override_path) = std::env::var("UWULOG_KEYMAP_PATH") {
        if !override_path.trim().is_empty() {
            return PathBuf::from(override_path);
        }
    }

    // 2. Windows: %APPDATA% hoặc %LOCALAPPDATA%
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("uwulog").join("keymap.json");
        }
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(localappdata)
                .join("uwulog")
                .join("keymap.json");
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            return PathBuf::from(userprofile)
                .join(".config")
                .join("uwulog")
                .join("keymap.json");
        }
    }

    // 3. Unix/Linux/macOS: XDG Base Directory hoặc $HOME/.config
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("uwulog").join("keymap.json");
        }
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
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory '{:?}': {}", parent, e))?;
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

/// Ghi cấu hình phím tắt của người dùng từ KeymapManager xuống file keymap.json (hoặc đường dẫn được chỉ định).
pub fn save_user_keymap(manager: &KeymapManager, path: Option<&Path>) -> Result<(), String> {
    let target_path = path.map(PathBuf::from).unwrap_or_else(default_config_path);

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory '{:?}': {}", parent, e))?;
    }

    let config = manager.export_config();
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize keymap config: {}", e))?;

    std::fs::write(&target_path, json)
        .map_err(|e| format!("Failed to write keymap file '{:?}': {}", target_path, e))?;

    Ok(())
}

/// Đặt lại file cấu hình phím tắt về trạng thái mẫu mặc định.
pub fn reset_user_keymap_file(path: Option<&Path>) -> Result<(), String> {
    let target_path = path.map(PathBuf::from).unwrap_or_else(default_config_path);

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory '{:?}': {}", parent, e))?;
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

    #[test]
    fn test_parse_keymap_unbind_array_and_map() {
        // Hỗ trợ cả unbind dạng mảng lẫn dạng map
        let json_with_array = r#"[
            {
                "context": "Global",
                "unbind": ["alt-p", "ctrl-pageup"]
            }
        ]"#;
        let cfg1: KeymapConfigFile = serde_json::from_str(json_with_array).unwrap();
        assert_eq!(cfg1.0[0].unbind.as_ref().unwrap().len(), 2);

        let json_with_map = r#"[
            {
                "context": "Global",
                "unbind": {
                    "alt-p": "workspace::ToggleProjectPicker"
                }
            }
        ]"#;
        let cfg2: KeymapConfigFile = serde_json::from_str(json_with_map).unwrap();
        assert_eq!(cfg2.0[0].unbind.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_generate_default_sample_roundtrip() {
        let sample = KeymapConfigFile::generate_default_sample();
        let json = serde_json::to_string(&sample).unwrap();
        let de: KeymapConfigFile = serde_json::from_str(&json).unwrap();
        assert_eq!(de.0.len(), sample.0.len());
    }
}
