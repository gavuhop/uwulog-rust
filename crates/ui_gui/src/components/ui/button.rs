//! Zed-style Button primitives: AppButton, IconButton, TabButton.

use crate::theme;
use eframe::egui::{self, Color32, Response, Rounding, Stroke, Ui, Vec2};

/// Kiểu nút bấm tiêu chuẩn (Regular, Selected, Danger, Success, Ghost, Primary)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Primary,
    Ghost,
    Selected,
    Success,
    Danger,
}

/// Nút bấm tổng quát với API dạng Builder (tương tự Button trong Zed UI)
pub struct AppButton<'a> {
    label: Option<&'a str>,
    icon: Option<&'a str>,
    tooltip: Option<&'a str>,
    variant: ButtonVariant,
    small: bool,
    fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
    min_size: Option<Vec2>,
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
            tooltip: None,
            variant: ButtonVariant::Default,
            small: false,
            fill_override: None,
            stroke_override: None,
            min_size: None,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
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

    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = Some(size);
        self
    }

    /// Render nút bấm lên giao diện
    pub fn show(self, ui: &mut Ui) -> Response {
        let (font_size, padding, rounding) = if self.small {
            (11.0, egui::vec2(6.0, 3.0), Rounding::same(3.0))
        } else {
            (12.0, egui::vec2(8.0, 4.0), Rounding::same(4.0))
        };

        let (text_color, bg_color, stroke) = match self.variant {
            ButtonVariant::Default => (
                theme::TEXT_PRIMARY,
                self.fill_override.unwrap_or(theme::BG_SURFACE0),
                self.stroke_override
                    .unwrap_or(Stroke::new(1.0, theme::BG_SURFACE1)),
            ),
            ButtonVariant::Primary => (
                Color32::WHITE,
                self.fill_override.unwrap_or(theme::BG_TEXT_SELECTION),
                self.stroke_override
                    .unwrap_or(Stroke::new(1.0, theme::STROKE_TEXT_SELECTION)),
            ),
            ButtonVariant::Ghost => (
                theme::TEXT_PRIMARY,
                self.fill_override.unwrap_or(Color32::TRANSPARENT),
                self.stroke_override.unwrap_or(Stroke::NONE),
            ),
            ButtonVariant::Selected => (
                theme::TEXT_KEY,
                self.fill_override.unwrap_or(theme::BG_SURFACE1),
                self.stroke_override
                    .unwrap_or(Stroke::new(1.0, theme::TEXT_KEY)),
            ),
            ButtonVariant::Success => (
                theme::TEXT_PRIMARY,
                self.fill_override.unwrap_or(theme::BTN_RESTART_BG),
                self.stroke_override
                    .unwrap_or(Stroke::new(1.0, theme::BTN_RESTART_BORDER)),
            ),
            ButtonVariant::Danger => (
                theme::TEXT_PRIMARY,
                self.fill_override.unwrap_or(theme::BTN_STOP_BG),
                self.stroke_override
                    .unwrap_or(Stroke::new(1.0, theme::BTN_STOP_BORDER)),
            ),
        };

        let content_text = match (self.icon, self.label) {
            (Some(icon), Some(label)) => format!("{} {}", icon, label),
            (Some(icon), None) => icon.to_string(),
            (None, Some(label)) => label.to_string(),
            (None, None) => String::new(),
        };

        let rich = egui::RichText::new(content_text)
            .size(font_size)
            .color(text_color);

        let mut btn = egui::Button::new(rich)
            .fill(bg_color)
            .stroke(stroke)
            .rounding(rounding);

        if let Some(size) = self.min_size {
            btn = btn.min_size(size);
        } else {
            btn = btn.min_size(egui::vec2(0.0, padding.y * 2.0 + font_size));
        }

        let resp = ui.add(btn);
        if let Some(tip) = self.tooltip {
            resp.on_hover_text(tip)
        } else {
            resp
        }
    }
}

/// Nút icon vuông vắn chuyên dụng (IconButton trong Zed UI)
pub struct IconButton<'a> {
    icon: &'a str,
    tooltip: Option<&'a str>,
    selected: bool,
    size: f32,
    fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: &'a str) -> Self {
        Self {
            icon,
            tooltip: None,
            selected: false,
            size: 24.0,
            fill_override: None,
            stroke_override: None,
        }
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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

    pub fn show(self, ui: &mut Ui) -> Response {
        let (text_color, bg_color, stroke) = if self.selected {
            (
                theme::TEXT_KEY,
                self.fill_override.unwrap_or(theme::BG_SURFACE1),
                self.stroke_override
                    .unwrap_or(Stroke::new(1.0, theme::TEXT_KEY)),
            )
        } else {
            (
                theme::TEXT_PRIMARY,
                self.fill_override.unwrap_or(Color32::TRANSPARENT),
                self.stroke_override.unwrap_or(Stroke::NONE),
            )
        };

        let rich = egui::RichText::new(self.icon)
            .size(self.size * 0.55)
            .color(text_color);

        let btn = egui::Button::new(rich)
            .fill(bg_color)
            .stroke(stroke)
            .rounding(Rounding::same(4.0))
            .min_size(Vec2::splat(self.size));

        let resp = ui.add(btn);
        if let Some(tip) = self.tooltip {
            resp.on_hover_text(tip)
        } else {
            resp
        }
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
        let text_color = if !self.enabled {
            theme::TEXT_MUTED
        } else if self.active {
            theme::TEXT_KEY
        } else {
            theme::TEXT_PRIMARY
        };

        let fill = if self.active {
            theme::BG_SURFACE1
        } else {
            Color32::TRANSPARENT
        };

        let title = if let Some(count) = self.badge_count {
            format!("{} ({})", self.label, theme::format_number(count))
        } else {
            self.label.to_string()
        };

        let rich = egui::RichText::new(title)
            .size(11.5)
            .strong()
            .color(text_color);

        let btn = egui::Button::new(rich)
            .fill(fill)
            .stroke(Stroke::NONE)
            .rounding(Rounding::same(4.0));

        ui.add_enabled(self.enabled, btn)
    }
}
