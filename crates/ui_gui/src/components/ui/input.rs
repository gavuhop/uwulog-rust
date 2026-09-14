//! Zed-style TextInput primitive: no stroke by default, BUTTON_HEIGHT_NORMAL height.

use super::button::BUTTON_HEIGHT_NORMAL;
use crate::theme;
use eframe::egui::{
    self, Color32, CornerRadius, FontSelection, Id, Margin, Response, RichText, Stroke, Ui,
};

/// Component TextInput chuẩn Zed UI:
/// - Mặc định không có stroke cả khi bình thường lẫn khi hover.
/// - Stroke là option (có thể bật thông qua `.stroke(stroke)` hoặc `.bordered()`).
/// - Chiều cao mặc định kế thừa từ `BUTTON_HEIGHT_NORMAL` (24.0px) của Button component.
/// - Tự động căn giữa chữ theo trục dọc (`vertical_align(Align::Center)`).
/// - Màu nền mặc định là `theme::BG_CRUST` (có thể đổi qua `.fill(color)` hoặc `.transparent()`).
pub struct TextInput<'a> {
    text: &'a mut String,
    hint_text: Option<RichText>,
    id: Option<Id>,
    width: Option<f32>,
    height: Option<f32>,
    fill: Option<Color32>,
    stroke: Option<Stroke>,
    corner_radius: CornerRadius,
    margin: Margin,
    font: Option<FontSelection>,
    text_color: Option<Color32>,
    password: bool,
    interactive: bool,
}

pub type AppInput<'a> = TextInput<'a>;

impl<'a> TextInput<'a> {
    pub fn new(text: &'a mut String) -> Self {
        Self {
            text,
            hint_text: None,
            id: None,
            width: None,
            height: None,
            fill: Some(theme::BG_CRUST),
            stroke: None,
            corner_radius: CornerRadius::same(4),
            margin: Margin::symmetric(8, 4),
            font: Some(FontSelection::from(egui::TextStyle::Monospace)),
            text_color: Some(theme::TEXT_PRIMARY),
            password: false,
            interactive: true,
        }
    }

    /// Thiết lập placeholder/hint text (tự động áp dụng màu theme::TEXT_PLACEHOLDER)
    pub fn hint_text(mut self, hint: impl AsRef<str>) -> Self {
        self.hint_text = Some(RichText::new(hint.as_ref()).color(theme::TEXT_PLACEHOLDER));
        self
    }

    /// Thiết lập placeholder/hint text dạng RichText tùy biến
    pub fn hint_rich_text(mut self, hint: RichText) -> Self {
        self.hint_text = Some(hint);
        self
    }

    /// Gán Id rõ ràng cho TextEdit (phục vụ focus tracking, cursor restore)
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Thiết lập chiều rộng mong muốn (mặc định lấy `ui.available_width()`)
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Thiết lập chiều cao mong muốn (mặc định là `BUTTON_HEIGHT_NORMAL` = 24.0px)
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Thiết lập màu nền (mặc định là `theme::BG_CRUST`)
    pub fn fill(mut self, fill: Color32) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Đặt nền trong suốt
    pub fn transparent(mut self) -> Self {
        self.fill = Some(Color32::TRANSPARENT);
        self
    }

    /// Tùy chọn border stroke (mặc định là không có stroke `Stroke::NONE`)
    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// Bật border 1px viền mặc định (`theme::BG_SURFACE1`)
    pub fn bordered(mut self) -> Self {
        self.stroke = Some(Stroke::new(1.0, theme::BG_SURFACE1));
        self
    }

    /// Thiết lập bo góc
    pub fn corner_radius(mut self, radius: CornerRadius) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Thiết lập lề trong
    pub fn margin(mut self, margin: impl Into<Margin>) -> Self {
        self.margin = margin.into();
        self
    }

    /// Tùy biến font (hỗ trợ cả FontId lẫn TextStyle)
    pub fn font(mut self, font: impl Into<FontSelection>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Tùy biến màu chữ
    pub fn text_color(mut self, color: Color32) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Chế độ nhập mật khẩu (che ký tự)
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    /// Cho phép hoặc vô hiệu hóa tương tác
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Render TextInput lên UI và trả về `Response`
    pub fn show(self, ui: &mut Ui) -> Response {
        let height = self.height.unwrap_or(BUTTON_HEIGHT_NORMAL);
        let width = self.width.unwrap_or_else(|| ui.available_width());

        let stroke = self.stroke.unwrap_or(Stroke::NONE);
        let fill = self.fill.unwrap_or(theme::BG_CRUST);

        let frame = egui::Frame::default()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(self.corner_radius)
            .inner_margin(self.margin);

        let mut edit = egui::TextEdit::singleline(self.text)
            .frame(frame)
            .vertical_align(egui::Align::Center)
            .desired_width(width)
            .margin(self.margin);

        if let Some(id) = self.id {
            edit = edit.id(id);
        }
        if let Some(hint) = self.hint_text {
            edit = edit.hint_text(hint);
        }
        if let Some(font) = self.font {
            edit = edit.font(font);
        }
        if let Some(color) = self.text_color {
            edit = edit.text_color(color);
        }
        if self.password {
            edit = edit.password(true);
        }
        if !self.interactive {
            edit = edit.interactive(false);
        }

        ui.add_sized([width, height], edit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_builder_defaults() {
        let mut text = String::from("hello");
        let input = TextInput::new(&mut text);
        assert_eq!(input.height, None);
        assert_eq!(input.stroke, None);
        assert_eq!(input.fill, Some(theme::BG_CRUST));
        assert_eq!(input.margin, Margin::symmetric(8, 4));
        assert_eq!(input.text_color, Some(theme::TEXT_PRIMARY));
        assert!(input.font.is_some());
    }

    #[test]
    fn test_text_input_bordered_option() {
        let mut text = String::new();
        let input = TextInput::new(&mut text).bordered();
        assert!(input.stroke.is_some());
        assert_eq!(input.stroke.unwrap().width, 1.0);
    }

    #[test]
    fn test_text_input_transparent_option() {
        let mut text = String::new();
        let input = TextInput::new(&mut text).transparent();
        assert_eq!(input.fill, Some(Color32::TRANSPARENT));
    }
}
