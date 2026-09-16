//! Quản lý nạp, nhập (import) và phát hiện (discovery) các Theme từ thư mục cấu hình bên ngoài.
//!
//! Thư mục theme mặc định:
//! - Windows: `%LOCALAPPDATA%\uwulog\themes`
//! - Unix/Linux/macOS: `~/.config/uwulog/themes`
//! - Có thể ghi đè qua biến môi trường `UWULOG_THEMES_DIR`.

use super::theme::Theme;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Mẫu theme JSON ví dụ để người dùng tham khảo và tự tạo theme mới
pub const TEMPLATE_JSON: &str = r##"{
  "id": "my-custom-theme",
  "name": "My Custom Theme",
  "appearance": "dark",
  "surfaces": {
    "base": "#14161f",
    "mantle": "#1a1d28",
    "crust": "#0e0f16",
    "surface0": "#282c3c",
    "surface1": "#343a4e"
  },
  "borders": {
    "border": "#282c3c",
    "border_subtle": "#1e2333",
    "border_focused": "#78a0d4",
    "border_selected": "#20293d"
  },
  "text": {
    "primary": "#c5cdd9",
    "muted": "#727a90",
    "placeholder": "#555d73",
    "accent": "#78a0d4"
  },
  "status": {
    "error": "#d96570",
    "warning": "#d4a359",
    "info": "#7ec787",
    "debug": "#b48ead",
    "success": "#a3be8c"
  },
  "log": {
    "row_hover": "#202636",
    "row_selected": "#2d354a",
    "row_highlight": "#3b3322",
    "term_highlight_bg": "#d08770",
    "term_highlight_text": "#14161f"
  }
}
"##;

/// Lấy đường dẫn tới thư mục chứa theme tùy chỉnh của người dùng
pub fn get_themes_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("UWULOG_THEMES_DIR") {
        if !override_dir.trim().is_empty() {
            return PathBuf::from(override_dir);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("uwulog").join("themes");
        }
    }

    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        return PathBuf::from(home)
            .join(".config")
            .join("uwulog")
            .join("themes");
    }

    PathBuf::from("themes")
}

/// Tên file theme mẫu mặc định luôn hiển thị sẵn ở thư mục themes để người dùng tham khảo/đổi tên
pub const TEMPLATE_FILE_NAME: &str = "custom-template.json";

/// Đảm bảo thư mục lưu trữ theme tồn tại
pub fn ensure_themes_dir() -> std::io::Result<PathBuf> {
    let dir = get_themes_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Đảm bảo file template mẫu luôn tồn tại trong thư mục theme.
/// Nếu người dùng đổi tên file hoặc xóa đi, lần chạy sau sẽ tự động tạo lại.
pub fn ensure_template_file() -> std::io::Result<PathBuf> {
    let dir = ensure_themes_dir()?;
    let template_file = dir.join(TEMPLATE_FILE_NAME);
    if !template_file.exists() {
        let _ = std::fs::write(&template_file, TEMPLATE_JSON);
    }
    let old_example = dir.join("custom-template.json.example");
    if old_example.exists() {
        let _ = std::fs::remove_file(old_example);
    }
    Ok(template_file)
}

/// Nạp một theme cụ thể từ thư mục theme nếu tồn tại trên đĩa và đăng ký vào registry
pub fn load_theme_by_id(id: &str) -> Option<Arc<Theme>> {
    if let Some(theme) = super::get_theme(id) {
        return Some(theme);
    }
    let dir = get_themes_dir();
    let file_path = dir.join(format!("{}.json", id));
    if file_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            if let Ok(theme) = Theme::from_json(&content) {
                let arc = Arc::new(theme.clone());
                super::register_theme(theme);
                return Some(arc);
            }
        }
    }
    None
}

/// Nạp tất cả các file JSON hợp lệ từ một thư mục chỉ định
pub fn load_themes_from_dir(dir: &Path) -> Vec<Theme> {
    let mut themes = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return themes,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                match Theme::from_json(&content) {
                    Ok(theme) => {
                        themes.push(theme);
                    }
                    Err(err) => {
                        eprintln!(
                            "uwulog: Bỏ qua theme không hợp lệ tại {}: {err}",
                            path.display()
                        );
                    }
                }
            }
        }
    }

    themes
}

/// Tự động quét và nạp toàn bộ các theme từ thư mục `themes/` vào ThemeRegistry.
/// Trả về số lượng theme ngoài đã được đăng ký thành công.
pub fn load_external_themes() -> usize {
    let dir = match ensure_themes_dir() {
        Ok(d) => d,
        Err(_) => get_themes_dir(),
    };
    if !dir.exists() {
        return 0;
    }

    let themes = load_themes_from_dir(&dir);
    let count = themes.len();
    for theme in themes {
        super::register_theme(theme);
    }
    count
}

/// Nhập một file theme JSON và lưu vào thư mục chỉ định, sau đó đăng ký vào ThemeRegistry
pub fn import_theme_file_to_dir(src: &Path, themes_dir: &Path) -> Result<Arc<Theme>> {
    let content = std::fs::read_to_string(src)
        .with_context(|| format!("Không thể đọc file theme từ '{}'", src.display()))?;

    import_theme_from_content_to_dir(&content, themes_dir)
}

/// Nhập theme từ chuỗi nội dung JSON, lưu vào thư mục chỉ định và đăng ký vào ThemeRegistry
pub fn import_theme_from_content_to_dir(json_str: &str, themes_dir: &Path) -> Result<Arc<Theme>> {
    let theme = Theme::from_json(json_str)
        .context("Nội dung file không phải định dạng theme JSON hợp lệ")?;

    if theme.id.trim().is_empty() {
        anyhow::bail!("Mã theme (id) không được để trống");
    }

    if !themes_dir.exists() {
        std::fs::create_dir_all(themes_dir)?;
    }

    let safe_id: String = theme
        .id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let dest_path = themes_dir.join(format!("{}.json", safe_id));
    std::fs::write(&dest_path, json_str)
        .with_context(|| format!("Không thể lưu file theme tới '{}'", dest_path.display()))?;

    let arc = Arc::new(theme.clone());
    super::register_theme(theme);

    Ok(arc)
}

/// Nhập (import) một file theme JSON từ đường dẫn bất kỳ vào thư mục theme mặc định của uwulog,
/// kiểm tra tính hợp lệ cú pháp và đăng ký ngay vào ThemeRegistry.
pub fn import_theme_file(src: &Path) -> Result<Arc<Theme>> {
    let themes_dir =
        ensure_themes_dir().context("Không thể tạo hoặc truy cập thư mục lưu trữ theme")?;
    import_theme_file_to_dir(src, &themes_dir)
}

/// Nhập theme từ chuỗi nội dung JSON, lưu vào thư mục theme mặc định và đăng ký vào registry
pub fn import_theme_from_content(json_str: &str) -> Result<Arc<Theme>> {
    let themes_dir =
        ensure_themes_dir().context("Không thể tạo hoặc truy cập thư mục lưu trữ theme")?;
    import_theme_from_content_to_dir(json_str, &themes_dir)
}

/// Mở thư mục chứa theme trong trình quản lý tệp tin mặc định của hệ điều hành (Explorer, Finder, xdg-open)
pub fn open_themes_dir() -> std::io::Result<()> {
    let dir = ensure_themes_dir()?;

    #[cfg(target_os = "windows")]
    {
        let template_file = dir.join(TEMPLATE_FILE_NAME);
        if template_file.exists() {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", template_file.display()))
                .spawn();
            return Ok(());
        }

        std::process::Command::new("explorer").arg(&dir).spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&dir).spawn()?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(&dir).spawn()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_json_validity() {
        let theme = Theme::from_json(TEMPLATE_JSON).expect("Template JSON must parse cleanly");
        assert_eq!(theme.id, "my-custom-theme");
        assert_eq!(theme.name, "My Custom Theme");
        assert!(theme.appearance.is_dark());
    }

    #[test]
    fn test_load_themes_from_directory() {
        let temp_dir =
            std::env::temp_dir().join(format!("uwulog_theme_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let theme_file = temp_dir.join("test-theme.json");
        std::fs::write(&theme_file, TEMPLATE_JSON).unwrap();

        let loaded = load_themes_from_dir(&temp_dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "my-custom-theme");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_import_theme_from_content() {
        let temp_dir =
            std::env::temp_dir().join(format!("uwulog_import_test_{}", uuid::Uuid::new_v4()));

        let custom_json = TEMPLATE_JSON.replace("my-custom-theme", "imported-theme-123");
        let imported =
            import_theme_from_content_to_dir(&custom_json, &temp_dir).expect("Import must succeed");
        assert_eq!(imported.id, "imported-theme-123");

        let expected_file = temp_dir.join("imported-theme-123.json");
        assert!(expected_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_import_theme_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("uwulog_import_file_test_{}", uuid::Uuid::new_v4()));
        let source_dir =
            std::env::temp_dir().join(format!("uwulog_src_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&source_dir).unwrap();

        let src_file = source_dir.join("external-dracula.json");
        let custom_json = TEMPLATE_JSON.replace("my-custom-theme", "dracula-pro");
        std::fs::write(&src_file, custom_json).unwrap();

        let imported =
            import_theme_file_to_dir(&src_file, &temp_dir).expect("File import must succeed");
        assert_eq!(imported.id, "dracula-pro");

        let dest_file = temp_dir.join("dracula-pro.json");
        assert!(dest_file.exists());

        let _ = std::fs::remove_dir_all(&source_dir);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ensure_template_file_and_recreation() {
        let temp_dir =
            std::env::temp_dir().join(format!("uwulog_ensure_template_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let template_path = temp_dir.join(TEMPLATE_FILE_NAME);
        assert!(!template_path.exists());

        std::fs::write(&template_path, TEMPLATE_JSON).unwrap();
        assert!(template_path.exists());

        let renamed_path = temp_dir.join("my-renamed-theme.json");
        std::fs::rename(&template_path, &renamed_path).unwrap();
        assert!(!template_path.exists());
        assert!(renamed_path.exists());

        if !template_path.exists() {
            std::fs::write(&template_path, TEMPLATE_JSON).unwrap();
        }
        assert!(template_path.exists());
        assert!(renamed_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
