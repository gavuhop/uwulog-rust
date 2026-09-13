//! Multiplatform Window Controls (Windows, macOS, Linux).
//! Clean modular architecture separating platform-specific rendering behaviors.

pub mod icons;
pub mod linux;
pub mod macos;
pub mod resize_borders;
pub mod windows;

pub use resize_borders::render_window_resize_borders;
pub use windows::{WINDOW_CONTROL_BTN_HEIGHT, WINDOW_CONTROL_BTN_WIDTH};

/// Chiều cao tiêu chuẩn của thanh titlebar / header (32px theo chuẩn Windows 10/11 và Zed)
pub const TITLEBAR_HEIGHT: f32 = 32.0;

use eframe::egui::Ui;

/// Nền tảng hiển thị nút điều khiển cửa sổ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlsPlatform {
    Windows,
    MacOs,
    Linux,
}

impl WindowControlsPlatform {
    /// Tự động nhận diện nền tảng theo Target OS của Rust.
    /// Hỗ trợ biến môi trường `UWU_WINDOW_CONTROLS_STYLE` ("windows", "macos", "linux") để preview linh hoạt trên mọi máy dev.
    pub fn current() -> Self {
        if let Ok(val) = std::env::var("UWU_WINDOW_CONTROLS_STYLE") {
            match val.to_lowercase().as_str() {
                "mac" | "macos" | "darwin" | "apple" => return Self::MacOs,
                "linux" | "gnome" | "adwaita" | "kde" => return Self::Linux,
                "windows" | "win" => return Self::Windows,
                _ => {}
            }
        }

        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            Self::Linux
        }
    }

    /// Lề panel header tương ứng với từng hệ điều hành
    pub fn header_panel_margin(&self) -> eframe::egui::Margin {
        match self {
            Self::MacOs => eframe::egui::Margin {
                left: 14.0,
                right: 10.0,
                top: 0.0,
                bottom: 0.0,
            },
            Self::Windows => eframe::egui::Margin {
                left: 10.0,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            Self::Linux => eframe::egui::Margin {
                left: 10.0,
                right: 8.0,
                top: 0.0,
                bottom: 0.0,
            },
        }
    }
}

/// Loại nút điều khiển cửa sổ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptionButtonType {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl CaptionButtonType {
    #[inline]
    pub(crate) fn segoe_glyph(&self) -> &'static str {
        match self {
            Self::Minimize => "\u{e921}", // ChromeMinimize
            Self::Restore => "\u{e923}",  // ChromeRestore
            Self::Maximize => "\u{e922}", // ChromeMaximize
            Self::Close => "\u{e8bb}",    // ChromeClose
        }
    }

    #[inline]
    pub(crate) fn tooltip(&self) -> &'static str {
        match self {
            Self::Minimize => "Minimize",
            Self::Restore => "Restore Down",
            Self::Maximize => "Maximize",
            Self::Close => "Close",
        }
    }
}

/// Nút điều khiển cửa sổ phía bên trái (dành riêng cho macOS Traffic Lights).
/// Trả về `true` nếu có render nút (trên macOS), `false` nếu là Windows / Linux.
pub fn render_left_window_controls(ui: &mut Ui) -> bool {
    if WindowControlsPlatform::current() != WindowControlsPlatform::MacOs {
        return false;
    }

    macos::render_macos_traffic_lights(ui, WINDOW_CONTROL_BTN_HEIGHT);
    true
}

/// Nút điều khiển cửa sổ phía bên phải (Windows / Linux).
/// Trả về `true` nếu có render nút (trên Windows / Linux), `false` nếu là macOS (vì macOS nằm ở bên trái).
pub fn render_right_window_controls(ui: &mut Ui) -> bool {
    let platform = WindowControlsPlatform::current();
    let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));

    match platform {
        WindowControlsPlatform::Windows => {
            windows::render_windows_window_controls(ui, is_maximized);
            true
        }
        WindowControlsPlatform::Linux => {
            linux::render_linux_window_controls(ui, is_maximized);
            true
        }
        WindowControlsPlatform::MacOs => false,
    }
}

/// Tương thích ngược: Mặc định gọi render_right_window_controls
pub fn render_window_controls(ui: &mut Ui) {
    render_right_window_controls(ui);
}
