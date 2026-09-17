use super::icon::{IconName, IconSize};
use crate::theme::{self, ActiveTheme};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Response, Stroke, Ui, Vec2};

/// Kiểu nút bấm tiêu chuẩn (Regular, Selected, Danger, Success, Ghost, Primary, Outline)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Primary,
    Ghost,
    Outline,
    Selected,
    Success,
    Danger,
}

pub const BUTTON_HEIGHT_NORMAL: f32 = 24.0;
pub const BUTTON_HEIGHT_SMALL: f32 = 20.0;

/// Icon được hiển thị trên nút bấm (hỗ trợ cả IconName chuẩn hóa và ký tự text)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonIcon<'a> {
    Named(IconName),
    Glyph(&'a str),
}

impl<'a> From<IconName> for ButtonIcon<'a> {
    fn from(name: IconName) -> Self {
        Self::Named(name)
    }
}

impl<'a> From<&'a str> for ButtonIcon<'a> {
    fn from(glyph: &'a str) -> Self {
        Self::Glyph(glyph)
    }
}

/// Nút bấm tổng quát với API dạng Builder (tương tự Button trong Zed UI)
pub struct AppButton<'a> {
    label: Option<&'a str>,
    icon: Option<ButtonIcon<'a>>,
    icon_size: Option<IconSize>,
    status_dot: Option<Color32>,
    tooltip: Option<&'a str>,
    variant: ButtonVariant,
    small: bool,
    fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
    text_color_override: Option<Color32>,
    bordered: bool,
    min_size: Option<Vec2>,
    min_width: Option<f32>,
    full_width: bool,
    align_left: bool,
}

impl<'a> Default for AppButton<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> AppButton<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            icon: None,
            icon_size: None,
            status_dot: None,
            tooltip: None,
            variant: ButtonVariant::Default,
            small: false,
            fill_override: None,
            stroke_override: None,
            text_color_override: None,
            bordered: false,
            min_size: None,
            min_width: None,
            full_width: false,
            align_left: false,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn icon(mut self, icon: impl Into<ButtonIcon<'a>>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn icon_size(mut self, size: impl Into<IconSize>) -> Self {
        self.icon_size = Some(size.into());
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn status_dot(mut self, color: Color32) -> Self {
        self.status_dot = Some(color);
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        if selected {
            self.variant = ButtonVariant::Selected;
        }
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn fill(mut self, color: Color32) -> Self {
        self.fill_override = Some(color);
        self
    }

    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke_override = Some(stroke);
        self
    }

    /// Bật viền tiêu chuẩn cho nút (stroke option)
    pub fn bordered(mut self) -> Self {
        self.bordered = true;
        self
    }

    pub fn text_color(mut self, color: Color32) -> Self {
        self.text_color_override = Some(color);
        self
    }

    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = Some(size);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn align_left(mut self) -> Self {
        self.align_left = true;
        self
    }

    /// Render nút bấm lên giao diện với kích thước và hover chuẩn
    pub fn show(self, ui: &mut Ui) -> Response {
        let (font_size, padding_x, height, rounding) = if self.small {
            (11.0f32, 6.0f32, BUTTON_HEIGHT_SMALL, CornerRadius::same(3))
        } else {
            (12.0f32, 8.0f32, BUTTON_HEIGHT_NORMAL, CornerRadius::same(4))
        };

        let font_id = egui::FontId::proportional(font_size);
        let icon_size_px =
            self.icon_size
                .map(|s| s.px())
                .unwrap_or(if self.small { 12.0 } else { 14.0 });

        let (desired_w, desired_h, label_galley) = match (self.icon, self.label) {
            (Some(ButtonIcon::Named(_)), Some(label)) => {
                let galley = ui.painter().layout_no_wrap(
                    label.to_string(),
                    font_id.clone(),
                    Color32::PLACEHOLDER,
                );
                let content_w = icon_size_px + 5.0 + galley.size().x;
                let min_w = if self.full_width {
                    ui.available_width()
                } else if let Some(w) = self.min_width {
                    w
                } else {
                    self.min_size.map(|s| s.x).unwrap_or(0.0)
                };
                let min_h = self.min_size.map(|s| s.y).unwrap_or(0.0);
                (
                    (content_w + padding_x * 2.0).max(min_w),
                    height.max(min_h),
                    Some(galley),
                )
            }
            (Some(ButtonIcon::Named(_)), None) => {
                let min_w = if self.full_width {
                    ui.available_width()
                } else if let Some(w) = self.min_width {
                    w
                } else {
                    self.min_size.map(|s| s.x).unwrap_or(0.0)
                };
                let min_h = self.min_size.map(|s| s.y).unwrap_or(0.0);
                let w = if self.min_size.is_some() {
                    min_w
                } else {
                    (icon_size_px + padding_x * 2.0).max(min_w)
                };
                let h = if self.min_size.is_some() {
                    min_h
                } else {
                    height.max(min_h)
                };
                (w, h, None)
            }
            (Some(ButtonIcon::Glyph(g)), Some(label)) => {
                let content_text = format!("{} {}", g, label);
                let galley = ui.painter().layout_no_wrap(
                    content_text,
                    font_id.clone(),
                    Color32::PLACEHOLDER,
                );
                let min_w = if self.full_width {
                    ui.available_width()
                } else if let Some(w) = self.min_width {
                    w
                } else {
                    self.min_size.map(|s| s.x).unwrap_or(0.0)
                };
                let min_h = self.min_size.map(|s| s.y).unwrap_or(0.0);
                (
                    (galley.size().x + padding_x * 2.0).max(min_w),
                    height.max(min_h),
                    Some(galley),
                )
            }
            (Some(ButtonIcon::Glyph(g)), None) => {
                let galley = ui.painter().layout_no_wrap(
                    g.to_string(),
                    font_id.clone(),
                    Color32::PLACEHOLDER,
                );
                let min_w = if self.full_width {
                    ui.available_width()
                } else if let Some(w) = self.min_width {
                    w
                } else {
                    self.min_size.map(|s| s.x).unwrap_or(0.0)
                };
                let min_h = self.min_size.map(|s| s.y).unwrap_or(0.0);
                let w = if self.min_size.is_some() {
                    min_w
                } else {
                    (galley.size().x + padding_x * 2.0).max(min_w)
                };
                let h = if self.min_size.is_some() {
                    min_h
                } else {
                    height.max(min_h)
                };
                (w, h, Some(galley))
            }
            (None, Some(label)) => {
                let galley = ui.painter().layout_no_wrap(
                    label.to_string(),
                    font_id.clone(),
                    Color32::PLACEHOLDER,
                );
                let min_w = if self.full_width {
                    ui.available_width()
                } else if let Some(w) = self.min_width {
                    w
                } else {
                    self.min_size.map(|s| s.x).unwrap_or(0.0)
                };
                let min_h = self.min_size.map(|s| s.y).unwrap_or(0.0);
                (
                    (galley.size().x + padding_x * 2.0).max(min_w),
                    height.max(min_h),
                    Some(galley),
                )
            }
            (None, None) => {
                let min_w = self
                    .min_width
                    .unwrap_or_else(|| self.min_size.map(|s| s.x).unwrap_or(0.0));
                let min_h = self.min_size.map(|s| s.y).unwrap_or(0.0);
                (min_w, min_h, None)
            }
        };

        let (rect, mut response) =
            ui.allocate_exact_size(egui::vec2(desired_w, desired_h), egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let is_hovered = response.hovered();
            let is_active = response.is_pointer_button_down_on();
            let theme = ui.app_theme();

            let (mut text_color, mut bg_color) = match self.variant {
                ButtonVariant::Default => {
                    if is_active || is_hovered {
                        (theme.text.primary, theme.surfaces.surface1)
                    } else {
                        (theme.text.primary, theme.surfaces.surface0)
                    }
                }
                ButtonVariant::Primary => {
                    if is_hovered {
                        (theme.text.primary, theme.selection.stroke)
                    } else {
                        (theme.text.primary, theme.selection.bg)
                    }
                }
                ButtonVariant::Ghost => {
                    if is_active {
                        (theme.text.primary, theme.surfaces.surface1)
                    } else if is_hovered {
                        (theme.text.primary, theme.surfaces.surface0)
                    } else {
                        (theme.text.muted, Color32::TRANSPARENT)
                    }
                }
                ButtonVariant::Outline => {
                    if is_active {
                        (theme.text.primary, theme.surfaces.surface1)
                    } else if is_hovered {
                        (theme.text.primary, theme.surfaces.surface0)
                    } else {
                        (theme.text.muted, Color32::TRANSPARENT)
                    }
                }
                ButtonVariant::Selected => (theme.text.accent, theme.surfaces.surface1),
                ButtonVariant::Success => (theme.status.info, theme.controls.restart_bg),
                ButtonVariant::Danger => (theme.status.error, theme.controls.stop_bg),
            };

            let stroke = if self.bordered && self.stroke_override.is_none() {
                Stroke::new(1.0, theme.surfaces.surface1)
            } else {
                self.stroke_override.unwrap_or(Stroke::NONE)
            };

            if let Some(fill) = self.fill_override {
                bg_color = fill;
            }
            if let Some(tc) = self.text_color_override {
                text_color = tc;
            }

            if bg_color != Color32::TRANSPARENT || stroke.width > 0.0 {
                ui.painter()
                    .rect(rect, rounding, bg_color, stroke, egui::StrokeKind::Inside);
            }

            if let Some(ButtonIcon::Named(icon_name)) = self.icon {
                if let Some(galley) = label_galley {
                    let content_w = icon_size_px + 5.0 + galley.size().x;
                    let (icon_rect, text_pos) = if self.align_left {
                        let icon_r = Rect::from_center_size(
                            Pos2::new(rect.min.x + padding_x + icon_size_px * 0.5, rect.center().y),
                            Vec2::splat(icon_size_px),
                        );
                        let text_p = Pos2::new(
                            rect.min.x + padding_x + icon_size_px + 5.0,
                            rect.center().y - galley.size().y * 0.5,
                        );
                        (icon_r, text_p)
                    } else {
                        let start_x = rect.center().x - content_w * 0.5;
                        let icon_r = Rect::from_center_size(
                            Pos2::new(start_x + icon_size_px * 0.5, rect.center().y),
                            Vec2::splat(icon_size_px),
                        );
                        let text_p = Pos2::new(
                            start_x + icon_size_px + 5.0,
                            rect.center().y - galley.size().y * 0.5,
                        );
                        (icon_r, text_p)
                    };
                    icon_name.paint(ui.painter(), icon_rect, text_color);
                    if let Some(dot_color) = self.status_dot {
                        let dot_radius = (icon_size_px * 0.16).clamp(2.0, 3.0);
                        let knockout_radius = dot_radius + 1.0;
                        let dot_center = Pos2::new(
                            icon_rect.max.x - dot_radius * 0.5,
                            icon_rect.max.y - dot_radius * 0.8,
                        );
                        let knockout_color = if bg_color != Color32::TRANSPARENT {
                            bg_color
                        } else {
                            theme.surfaces.mantle
                        };
                        ui.painter()
                            .circle_filled(dot_center, knockout_radius, knockout_color);
                        ui.painter()
                            .circle_filled(dot_center, dot_radius, dot_color);
                    }
                    ui.painter().galley(text_pos, galley, text_color);
                } else {
                    let icon_rect =
                        Rect::from_center_size(rect.center(), Vec2::splat(icon_size_px));
                    icon_name.paint(ui.painter(), icon_rect, text_color);
                    if let Some(dot_color) = self.status_dot {
                        let dot_radius = (icon_size_px * 0.16).clamp(2.0, 3.0);
                        let knockout_radius = dot_radius + 1.0;
                        let dot_center = Pos2::new(
                            icon_rect.max.x - dot_radius * 0.5,
                            icon_rect.max.y - dot_radius * 0.8,
                        );
                        let knockout_color = if bg_color != Color32::TRANSPARENT {
                            bg_color
                        } else {
                            theme.surfaces.mantle
                        };
                        ui.painter()
                            .circle_filled(dot_center, knockout_radius, knockout_color);
                        ui.painter()
                            .circle_filled(dot_center, dot_radius, dot_color);
                    }
                }
            } else if let Some(galley) = label_galley {
                let text_pos = if self.align_left {
                    Pos2::new(
                        rect.min.x + padding_x,
                        rect.center().y - galley.size().y * 0.5,
                    )
                } else {
                    Pos2::new(
                        rect.center().x - galley.size().x * 0.5,
                        rect.center().y - galley.size().y * 0.5,
                    )
                };
                ui.painter().galley(text_pos, galley, text_color);
            }
        }

        if let Some(tip) = self.tooltip {
            response = response.on_hover_text(tip);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response
    }
}

/// Nút icon vuông vắn chuyên dụng (IconButton trong Zed UI)
pub struct IconButton<'a> {
    icon: ButtonIcon<'a>,
    icon_size: Option<IconSize>,
    tooltip: Option<&'a str>,
    selected: bool,
    size: f32,
    variant: ButtonVariant,
    fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
    bordered: bool,
    text_color_override: Option<Color32>,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: impl Into<ButtonIcon<'a>>) -> Self {
        Self {
            icon: icon.into(),
            icon_size: None,
            tooltip: None,
            selected: false,
            size: 24.0,
            variant: ButtonVariant::Ghost,
            fill_override: None,
            stroke_override: None,
            bordered: false,
            text_color_override: None,
        }
    }

    pub fn icon_size(mut self, size: impl Into<IconSize>) -> Self {
        self.icon_size = Some(size.into());
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn fill(mut self, color: Color32) -> Self {
        self.fill_override = Some(color);
        self
    }

    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke_override = Some(stroke);
        self
    }

    /// Bật viền tiêu chuẩn cho nút icon (stroke option)
    pub fn bordered(mut self) -> Self {
        self.bordered = true;
        self
    }

    pub fn text_color(mut self, color: Color32) -> Self {
        self.text_color_override = Some(color);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let mut btn = AppButton::new()
            .icon(self.icon)
            .variant(self.variant)
            .selected(self.selected)
            .min_size(Vec2::splat(self.size));

        if self.bordered {
            btn = btn.bordered();
        }
        if let Some(is) = self.icon_size {
            btn = btn.icon_size(is);
        } else if self.size < 24.0 {
            btn = btn.small();
        }
        if let Some(tip) = self.tooltip {
            btn = btn.tooltip(tip);
        }
        if let Some(fill) = self.fill_override {
            btn = btn.fill(fill);
        }
        if let Some(stroke) = self.stroke_override {
            btn = btn.stroke(stroke);
        }
        if let Some(tc) = self.text_color_override {
            btn = btn.text_color(tc);
        }

        btn.show(ui)
    }
}

/// Nút Tab phẳng có highlight active (TabButton)
pub struct TabButton<'a> {
    label: &'a str,
    active: bool,
    enabled: bool,
    badge_count: Option<usize>,
}

impl<'a> TabButton<'a> {
    pub fn new(label: &'a str, active: bool) -> Self {
        Self {
            label,
            active,
            enabled: true,
            badge_count: None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn badge(mut self, count: usize) -> Self {
        self.badge_count = Some(count);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let title = if let Some(count) = self.badge_count {
            format!("{} ({})", self.label, theme::format_number(count))
        } else {
            self.label.to_string()
        };

        let font_id = egui::FontId::proportional(11.0);
        let layout_galley =
            ui.painter()
                .layout_no_wrap(title.clone(), font_id.clone(), Color32::PLACEHOLDER);

        let padding_x = 8.0;
        let height = 24.0;
        let desired_size = egui::vec2(layout_galley.size().x + padding_x * 2.0, height);
        let (rect, mut response) = ui.allocate_exact_size(
            desired_size,
            if self.enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );

        if ui.is_rect_visible(rect) {
            let is_hovered = response.hovered();
            let is_active = response.is_pointer_button_down_on();
            let theme = ui.app_theme();

            let (text_color, bg_color, stroke) = if !self.enabled {
                (theme.text.muted, Color32::TRANSPARENT, Stroke::NONE)
            } else if self.active {
                (theme.text.accent, theme.surfaces.surface0, Stroke::NONE)
            } else if is_active {
                (theme.text.primary, theme.surfaces.surface1, Stroke::NONE)
            } else if is_hovered {
                (theme.text.primary, theme.surfaces.surface0, Stroke::NONE)
            } else {
                (theme.text.muted, Color32::TRANSPARENT, Stroke::NONE)
            };

            if bg_color != Color32::TRANSPARENT || stroke.width > 0.0 {
                ui.painter().rect(
                    rect,
                    CornerRadius::same(4),
                    bg_color,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }

            let galley = ui.painter().layout_no_wrap(title, font_id, text_color);
            let text_pos = egui::pos2(
                rect.center().x - galley.size().x * 0.5,
                rect.center().y - galley.size().y * 0.5,
            );
            ui.painter().galley(text_pos, galley, text_color);
        }

        if self.enabled {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_button_status_dot_builder() {
        let btn = AppButton::new()
            .label("Ubuntu")
            .icon(IconName::Linux)
            .status_dot(Color32::from_rgb(0, 255, 0));

        assert_eq!(btn.label, Some("Ubuntu"));
        assert_eq!(btn.icon, Some(ButtonIcon::Named(IconName::Linux)));
        assert_eq!(btn.status_dot, Some(Color32::from_rgb(0, 255, 0)));
        assert_eq!(btn.variant, ButtonVariant::Default);
    }
}
