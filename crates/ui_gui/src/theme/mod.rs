pub mod ansi;
pub mod color;
pub mod fonts;
pub mod highlight;
pub mod loader;
pub mod presets;
pub mod registry;
pub mod schema;
#[allow(clippy::module_inception)]
pub mod theme;
pub mod tokens;
pub mod utils;

pub use ansi::*;
pub use color::*;
pub use fonts::*;
pub use highlight::*;
pub use loader::*;
pub use presets::*;
pub use registry::*;
pub use schema::*;
pub use theme::*;
pub use tokens::*;
pub use utils::*;

use eframe::egui;
use std::sync::{Arc, LazyLock, RwLock};

static THEME_REGISTRY: LazyLock<RwLock<ThemeRegistry>> =
    LazyLock::new(|| RwLock::new(ThemeRegistry::new()));

/// Lấy tham chiếu bất biến tới Theme đang được kích hoạt hiện tại
pub fn active() -> Arc<Theme> {
    THEME_REGISTRY.read().unwrap().active()
}

/// Chuyển đổi Theme đang hoạt động và cập nhật ngay lập tức egui Visuals cùng repaint
pub fn set_active_theme(id: &str, ctx: &egui::Context) -> bool {
    let mut reg = THEME_REGISTRY.write().unwrap();
    if reg.set_active(id) {
        let active_theme = reg.active();
        drop(reg);
        ctx.set_visuals(active_theme.create_visuals());
        ctx.request_repaint();
        true
    } else {
        false
    }
}

/// Đăng ký theme mới vào global ThemeRegistry
pub fn register_theme(theme: Theme) {
    THEME_REGISTRY.write().unwrap().register(theme);
}

/// Lấy danh sách toàn bộ theme có sẵn
pub fn list_themes() -> Vec<Arc<Theme>> {
    THEME_REGISTRY.read().unwrap().list()
}

/// Lấy theme theo ID từ registry nếu đã tồn tại
pub fn get_theme(id: &str) -> Option<Arc<Theme>> {
    THEME_REGISTRY.read().unwrap().get(id)
}

/// Lấy danh sách các preset có sẵn của hệ thống
pub fn list_builtin_themes() -> Vec<Arc<Theme>> {
    THEME_REGISTRY.read().unwrap().list_builtin()
}

/// Lấy danh sách các theme tùy biến nạp từ bên ngoài
pub fn list_custom_themes() -> Vec<Arc<Theme>> {
    THEME_REGISTRY.read().unwrap().list_custom()
}

/// Kiểm tra xem theme ID có phải preset có sẵn hay không
pub fn is_builtin_theme(id: &str) -> bool {
    THEME_REGISTRY.read().unwrap().is_builtin(id)
}

/// Chuyển đổi mã màu LogColor sang Color32 dựa trên Theme đang kích hoạt
pub fn log_color_to_egui(color: uwu_core_schema::LogColor) -> egui::Color32 {
    active().log_color_to_egui(color)
}

/// Áp dụng theme và fonts ban đầu khi khởi động ứng dụng
pub fn apply_theme(ctx: &egui::Context) {
    fonts::setup_fonts(ctx);
    ctx.set_visuals(active().create_visuals());
}

/// Áp dụng theme (màu nền, màu chữ, Dark/Light Mode) cho thanh tiêu đề gốc của Windows (DWM)
#[cfg(target_os = "windows")]
pub fn apply_windows_titlebar_theme(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let theme = active();
    if let Ok(window_handle) = cc.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw() {
            let hwnd = win32_handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                apply_windows_titlebar_theme_for_hwnd(hwnd, &theme);
            }
        }
    }
}

/// Cập nhật trực tiếp thuộc tính DWM cho một HWND cụ thể
///
/// # Safety
/// Người gọi cần đảm bảo `hwnd` là một HWND hợp lệ được cấp phát bởi hệ thống Windows.
#[cfg(target_os = "windows")]
pub unsafe fn apply_windows_titlebar_theme_for_hwnd(
    hwnd: windows_sys::Win32::Foundation::HWND,
    theme: &Theme,
) {
    unsafe {
        use windows_sys::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
        };

        // 1. Kích hoạt Dark Mode hoặc Light Mode cho Title Bar
        let dark_mode: i32 = if theme.appearance.is_dark() { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_mode as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        // 2. Set màu nền thanh tiêu đề tương thích DWM COLORREF (0x00BBGGRR)
        let caption_color = theme.dwm_caption_color();
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            &caption_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // 3. Set màu chữ tiêu đề tương thích DWM COLORREF (0x00BBGGRR)
        let text_color = theme.dwm_text_color();
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR as u32,
            &text_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Trait hỗ trợ truy xuất Active Theme trực tiếp từ Context hoặc Ui
pub trait ActiveTheme {
    fn app_theme(&self) -> Arc<Theme>;
}

impl ActiveTheme for egui::Context {
    fn app_theme(&self) -> Arc<Theme> {
        active()
    }
}

impl ActiveTheme for egui::Ui {
    fn app_theme(&self) -> Arc<Theme> {
        active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_theme_and_switching() {
        let current = active();
        assert_eq!(current.id, "nord-dimmed");

        let ctx = egui::Context::default();
        assert!(set_active_theme("one-dark", &ctx));
        assert_eq!(active().id, "one-dark");

        // Trả lại mặc định để không ảnh hưởng test khác
        set_active_theme("nord-dimmed", &ctx);
        assert_eq!(active().id, "nord-dimmed");
    }

    #[test]
    fn test_theme_visuals_and_font_styles() {
        use eframe::egui::Color32;

        let ctx = egui::Context::default();
        apply_theme(&ctx);

        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let visuals = ui.visuals();
                assert_eq!(
                    visuals.widgets.noninteractive.bg_stroke,
                    egui::Stroke::new(1.0, Color32::from_rgb(0x28, 0x2c, 0x3c)),
                    "Separator stroke must match Nord Dimmed BG_SURFACE0 (#282c3c)"
                );
                assert_eq!(
                    visuals.widgets.hovered.bg_stroke,
                    egui::Stroke::new(1.0, Color32::from_rgb(0x34, 0x3a, 0x4e)),
                    "Hovered separator stroke must match surfaces.surface1 (#343a4e)"
                );
                assert_eq!(
                    visuals.widgets.active.bg_stroke,
                    egui::Stroke::new(1.0, Color32::from_rgb(0x34, 0x3a, 0x4e)),
                    "Active separator stroke must match surfaces.surface1 (#343a4e)"
                );
                assert_eq!(
                    visuals.override_text_color,
                    Some(Color32::from_rgb(0xc5, 0xcd, 0xd9)),
                    "Override text color must match Nord Dimmed TEXT_PRIMARY (#c5cdd9)"
                );
                assert_eq!(
                    visuals.text_cursor.stroke,
                    egui::Stroke::new(2.0, Color32::from_rgb(192, 222, 255)),
                    "Text cursor stroke must match pre-merge Visuals::dark cursor (#c0deff)"
                );

                let text_styles = &ui.style().text_styles;
                assert_eq!(
                    text_styles.get(&egui::TextStyle::Monospace).unwrap().size,
                    12.0
                );
                assert_eq!(text_styles.get(&egui::TextStyle::Body).unwrap().size, 12.0);
                assert_eq!(
                    text_styles.get(&egui::TextStyle::Heading).unwrap().size,
                    15.0
                );
                assert_eq!(
                    text_styles.get(&egui::TextStyle::Button).unwrap().size,
                    12.0
                );
                assert_eq!(text_styles.get(&egui::TextStyle::Small).unwrap().size, 11.0);
            });
        });
        output.textures_delta.clear();
    }
}
