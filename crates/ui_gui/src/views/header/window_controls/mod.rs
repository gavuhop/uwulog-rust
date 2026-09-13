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

    /// Lề panel header theo từng hệ điều hành:
    /// Cả 3 nền tảng đều đặt `top: 0.0, bottom: 0.0` để chiều cao thanh header cố định chính xác 32px (TITLEBAR_HEIGHT).
    /// Các widget con 24px (Menu, Project, Tabs, Toolbar, Linux buttons) tự động căn giữa dọc hoàn hảo (cách trên 4px, dưới 4px)
    /// nhờ `Layout::left_to_right(Align::Center)`.
    pub fn header_panel_margin(&self) -> eframe::egui::Margin {
        match self {
            Self::Windows => eframe::egui::Margin {
                left: 10,
                right: 0,
                top: 0,
                bottom: 0,
            },
            Self::MacOs => eframe::egui::Margin {
                left: 12,
                right: 10,
                top: 0,
                bottom: 0,
            },
            Self::Linux => eframe::egui::Margin {
                left: 10,
                right: 8,
                top: 0,
                bottom: 0,
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

/// Xử lý tương tác kéo di chuyển cửa sổ và nhấp đúp để phóng to / thu nhỏ.
pub fn handle_titlebar_drag_interaction(
    ctx: &eframe::egui::Context,
    response: &eframe::egui::Response,
) {
    if response.double_clicked() {
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Maximized(!is_maximized));
    } else if response.drag_started() {
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::StartDrag);
    }
}

/// Vùng đệm titlebar cho phép kéo di chuyển cửa sổ và nhấp đúp để phóng to / thu nhỏ cửa sổ.
pub fn render_titlebar_drag_spacer(ui: &mut Ui, width: f32) -> eframe::egui::Response {
    let (_rect, mut response) = ui.allocate_exact_size(
        eframe::egui::vec2(width.max(0.0), TITLEBAR_HEIGHT),
        eframe::egui::Sense::click_and_drag(),
    );

    // Con trỏ chuột trên thanh tiêu đề giữ nguyên dạng mũi tên mặc định của hệ điều hành
    response = response.on_hover_cursor(eframe::egui::CursorIcon::Default);
    if response.hovered() {
        ui.ctx().set_cursor_icon(eframe::egui::CursorIcon::Default);
    }

    handle_titlebar_drag_interaction(ui.ctx(), &response);

    response
}
