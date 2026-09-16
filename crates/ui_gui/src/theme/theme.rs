use super::tokens::*;
use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

/// Chế độ hiển thị giao diện (sáng hoặc tối)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Appearance {
    Dark,
    Light,
}

impl Appearance {
    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn is_light(&self) -> bool {
        matches!(self, Self::Light)
    }
}

/// Một bộ Theme hoàn chỉnh định nghĩa toàn bộ diện mạo ứng dụng
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
    pub surfaces: SurfaceColors,
    pub borders: BorderColors,
    pub text: TextColors,
    pub selection: SelectionColors,
    pub status: StatusColors,
    pub log: LogTableColors,
    pub controls: ControlColors,
    pub ansi: AnsiColors,
}

impl Theme {
    /// Chuyển đổi toàn bộ thông số màu của Theme thành cấu trúc Visuals chuẩn của egui
    pub fn create_visuals(&self) -> Visuals {
        let mut visuals = if self.appearance.is_dark() {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
        visuals.override_text_color = Some(self.text.primary);
        visuals.window_fill = self.surfaces.mantle;
        visuals.panel_fill = self.surfaces.mantle;
        visuals.faint_bg_color = self.surfaces.base;
        visuals.extreme_bg_color = self.surfaces.crust;
        visuals.code_bg_color = self.surfaces.crust;

        // Window & Dialog
        visuals.window_corner_radius = CornerRadius::same(6);
        visuals.window_stroke = Stroke::new(1.0, self.borders.border);

        // Non-interactive (nhãn tĩnh, container)
        visuals.widgets.noninteractive.bg_fill = self.surfaces.base;
        visuals.widgets.noninteractive.weak_bg_fill = self.surfaces.base;
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.borders.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text.primary);

        // Inactive (nút bấm bình thường khi chưa hover)
        visuals.widgets.inactive.bg_fill = self.surfaces.surface0;
        visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
        visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text.primary);

        // Hovered (khi rê chuột qua phần tử, splitter / separator)
        visuals.widgets.hovered.bg_fill = self.surfaces.surface1;
        visuals.widgets.hovered.weak_bg_fill = self.log.row_hover;
        visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, self.surfaces.surface1);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text.primary);

        // Active / Selected (khi nhấn giữ hoặc click chọn kéo thả)
        visuals.widgets.active.bg_fill = self.log.row_selected;
        visuals.widgets.active.weak_bg_fill = self.log.row_selected;
        visuals.widgets.active.corner_radius = CornerRadius::same(4);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, self.surfaces.surface1);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.text.primary);

        // Text Selection (bôi đen chọn chữ)
        visuals.selection.bg_fill = self.selection.bg;
        visuals.selection.stroke = Stroke::new(1.0, self.selection.stroke);

        visuals
    }

    /// Ánh xạ màu enum LogColor từ tầng core schema sang Color32 tương ứng của Theme
    pub fn log_color_to_egui(&self, color: uwu_core_schema::LogColor) -> Color32 {
        match color {
            uwu_core_schema::LogColor::Red => self.status.error,
            uwu_core_schema::LogColor::Yellow => self.status.warning,
            uwu_core_schema::LogColor::Green => self.status.info,
            uwu_core_schema::LogColor::Gray => self.status.debug,
            uwu_core_schema::LogColor::Default => self.text.primary,
        }
    }

    /// Trả về mã màu nền tiêu đề Window DWM (Windows 10/11) theo định dạng COLORREF (0x00BBGGRR)
    pub fn dwm_caption_color(&self) -> u32 {
        let c = self.surfaces.mantle;
        ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32)
    }

    /// Trả về mã màu chữ tiêu đề Window DWM (Windows 10/11) theo định dạng COLORREF (0x00BBGGRR)
    pub fn dwm_text_color(&self) -> u32 {
        let c = self.text.primary;
        ((c.b() as u32) << 16) | ((c.g() as u32) << 8) | (c.r() as u32)
    }
}
