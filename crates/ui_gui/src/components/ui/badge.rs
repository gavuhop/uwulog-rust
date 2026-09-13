//! Zed-style Badge and Status indicators: CountBadge, StatusDot.

use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Response, Stroke, Ui, Vec2};

/// Pill badge hiển thị số lượng (log count, items count)
pub struct CountBadge<'a> {
    text: &'a str,
    text_color: Color32,
    bg_color: Color32,
    framed: bool,
    tooltip: Option<&'a str>,
}

impl<'a> CountBadge<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            text_color: theme::TEXT_KEY,
            bg_color: theme::BG_SURFACE0,
            framed: true,
            tooltip: None,
        }
    }

    pub fn flat(mut self) -> Self {
        self.framed = false;
        self
    }

    pub fn text_color(mut self, color: Color32) -> Self {
        self.text_color = color;
        self
    }

    pub fn bg_color(mut self, color: Color32) -> Self {
        self.bg_color = color;
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let padding_x = if self.framed { 7.0 } else { 4.0 };
        let font_size = 11.0;
        let font_id = egui::FontId::monospace(font_size);

        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_string(), font_id, self.text_color);

        let size = egui::vec2(galley.size().x + padding_x * 2.0, 24.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            if self.framed {
                ui.painter().rect(
                    rect,
                    CornerRadius::same(4),
                    self.bg_color,
                    Stroke::new(1.0, theme::BG_SURFACE1),
                    egui::StrokeKind::Inside,
                );
            }

            let text_pos = Pos2::new(
                rect.min.x + padding_x,
                rect.center().y - galley.size().y * 0.5,
            );
            ui.painter().galley(text_pos, galley, self.text_color);
        }

        if let Some(tip) = self.tooltip {
            response.on_hover_text(tip)
        } else {
            response
        }
    }
}

/// Chấm tròn trạng thái (StatusDot)
pub struct StatusDot<'a> {
    color: Color32,
    size: f32,
    tooltip: Option<&'a str>,
}

impl<'a> StatusDot<'a> {
    pub fn new(color: Color32) -> Self {
        Self {
            color,
            size: 8.0,
            tooltip: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(self.size), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            let center = rect.center();
            ui.painter()
                .circle_filled(center, self.size * 0.5, self.color);
        }

        if let Some(tip) = self.tooltip {
            response.on_hover_text(tip)
        } else {
            response
        }
    }
}
