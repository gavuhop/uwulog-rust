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

pub const BUTTON_HEIGHT_NORMAL: f32 = 24.0;
pub const BUTTON_HEIGHT_SMALL: f32 = 20.0;

/// Nút bấm tổng quát với API dạng Builder (tương tự Button trong Zed UI)
pub struct AppButton<'a> {
    label: Option<&'a str>,
    icon: Option<&'a str>,
    tooltip: Option<&'a str>,
    variant: ButtonVariant,
    small: bool,
    fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
    text_color_override: Option<Color32>,
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
            tooltip: None,
            variant: ButtonVariant::Default,
            small: false,
            fill_override: None,
            stroke_override: None,
            text_color_override: None,
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
            (11.0f32, 6.0f32, BUTTON_HEIGHT_SMALL, Rounding::same(3.0))
        } else {
            (12.0f32, 8.0f32, BUTTON_HEIGHT_NORMAL, Rounding::same(4.0))
        };

        let content_text = match (self.icon, self.label) {
            (Some(icon), Some(label)) => format!("{} {}", icon, label),
            (Some(icon), None) => icon.to_string(),
            (None, Some(label)) => label.to_string(),
            (None, None) => String::new(),
        };

        let font_id = egui::FontId::proportional(font_size);
        let layout_galley = ui.painter().layout_no_wrap(
            content_text.clone(),
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
        let desired_w = if self.label.is_none() && self.min_size.is_some() {
            min_w
        } else {
            (layout_galley.size().x + padding_x * 2.0).max(min_w)
        };
        let desired_h = if self.label.is_none() && self.min_size.is_some() {
            min_h
        } else {
            height.max(min_h)
        };

        let (rect, mut response) =
            ui.allocate_exact_size(egui::vec2(desired_w, desired_h), egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let is_hovered = response.hovered();
            let is_active = response.is_pointer_button_down_on();

            let (mut text_color, mut bg_color, mut stroke) = match self.variant {
                ButtonVariant::Default => {
                    if is_active {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_SURFACE1,
                            Stroke::new(1.0, theme::TEXT_KEY),
                        )
                    } else if is_hovered {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_SURFACE1,
                            Stroke::new(1.0, theme::BG_SURFACE1),
                        )
                    } else {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_SURFACE0,
                            Stroke::new(1.0, theme::BG_SURFACE1),
                        )
                    }
                }
                ButtonVariant::Primary => {
                    if is_active {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_TEXT_SELECTION,
                            Stroke::new(1.5, theme::TEXT_KEY),
                        )
                    } else if is_hovered {
                        (
                            theme::TEXT_PRIMARY,
                            theme::STROKE_TEXT_SELECTION,
                            Stroke::new(1.0, theme::TEXT_KEY),
                        )
                    } else {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_TEXT_SELECTION,
                            Stroke::new(1.0, theme::STROKE_TEXT_SELECTION),
                        )
                    }
                }
                ButtonVariant::Ghost => {
                    if is_active {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_SURFACE1,
                            Stroke::new(1.0, theme::BG_SURFACE1),
                        )
                    } else if is_hovered {
                        (
                            theme::TEXT_PRIMARY,
                            theme::BG_SURFACE0,
                            Stroke::new(1.0, theme::BG_SURFACE1),
                        )
                    } else {
                        (theme::TEXT_MUTED, Color32::TRANSPARENT, Stroke::NONE)
                    }
                }
                ButtonVariant::Selected => (
                    theme::TEXT_KEY,
                    theme::BG_SURFACE1,
                    Stroke::new(1.0, theme::TEXT_KEY),
                ),
                ButtonVariant::Success => {
                    let border = if is_hovered {
                        theme::COLOR_INFO
                    } else {
                        theme::BTN_RESTART_BORDER
                    };
                    (
                        theme::COLOR_INFO,
                        theme::BTN_RESTART_BG,
                        Stroke::new(1.0, border),
                    )
                }
                ButtonVariant::Danger => {
                    let border = if is_hovered {
                        theme::COLOR_ERROR
                    } else {
                        theme::BTN_STOP_BORDER
                    };
                    (
                        theme::COLOR_ERROR,
                        theme::BTN_STOP_BG,
                        Stroke::new(1.0, border),
                    )
                }
            };

            if let Some(fill) = self.fill_override {
                bg_color = fill;
            }
            if let Some(s) = self.stroke_override {
                stroke = s;
            }
            if let Some(tc) = self.text_color_override {
                text_color = tc;
            }

            if bg_color != Color32::TRANSPARENT || stroke.width > 0.0 {
                ui.painter().rect(rect, rounding, bg_color, stroke);
            }

            let galley = ui
                .painter()
                .layout_no_wrap(content_text, font_id, text_color);
            let text_pos = if self.align_left {
                egui::pos2(
                    rect.min.x + padding_x,
                    rect.center().y - galley.size().y * 0.5,
                )
            } else {
                egui::pos2(
                    rect.center().x - galley.size().x * 0.5,
                    rect.center().y - galley.size().y * 0.5,
                )
            };
            ui.painter().galley(text_pos, galley, text_color);
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
    icon: &'a str,
    tooltip: Option<&'a str>,
    selected: bool,
    size: f32,
    variant: ButtonVariant,
    fill_override: Option<Color32>,
    stroke_override: Option<Stroke>,
    text_color_override: Option<Color32>,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: &'a str) -> Self {
        Self {
            icon,
            tooltip: None,
            selected: false,
            size: 24.0,
            variant: ButtonVariant::Ghost,
            fill_override: None,
            stroke_override: None,
            text_color_override: None,
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

        if self.size < 24.0 {
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

        let font_id = egui::FontId::proportional(11.5);
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

            let (text_color, bg_color, stroke) = if !self.enabled {
                (theme::TEXT_MUTED, Color32::TRANSPARENT, Stroke::NONE)
            } else if self.active {
                (
                    theme::TEXT_KEY,
                    theme::BG_SURFACE0,
                    Stroke::new(1.0, theme::BG_SURFACE1),
                )
            } else if is_active {
                (theme::TEXT_PRIMARY, theme::BG_SURFACE1, Stroke::NONE)
            } else if is_hovered {
                (theme::TEXT_PRIMARY, theme::BG_SURFACE0, Stroke::NONE)
            } else {
                (theme::TEXT_MUTED, Color32::TRANSPARENT, Stroke::NONE)
            };

            if bg_color != Color32::TRANSPARENT || stroke.width > 0.0 {
                ui.painter()
                    .rect(rect, Rounding::same(4.0), bg_color, stroke);
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
