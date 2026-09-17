use super::icon::IconName;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Response, Stroke, Ui, Vec2};

/// Pill badge hiển thị số lượng (log count, items count)
pub struct CountBadge<'a> {
    text: &'a str,
    icon: Option<IconName>,
    text_color: Option<Color32>,
    bg_color: Option<Color32>,
    framed: bool,
    tooltip: Option<&'a str>,
}

impl<'a> CountBadge<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            icon: None,
            text_color: None,
            bg_color: None,
            framed: true,
            tooltip: None,
        }
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn flat(mut self) -> Self {
        self.framed = false;
        self
    }

    pub fn text_color(mut self, color: Color32) -> Self {
        self.text_color = Some(color);
        self
    }

    pub fn bg_color(mut self, color: Color32) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn tooltip(mut self, tooltip: &'a str) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = ui.app_theme();
        let text_color = self.text_color.unwrap_or(theme.text.accent);
        let bg_color = self.bg_color.unwrap_or(theme.surfaces.surface0);

        let padding_x = if self.framed { 7.0 } else { 4.0 };
        let font_size = 11.0;
        let font_id = egui::FontId::monospace(font_size);

        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_string(), font_id, text_color);

        let icon_w = if self.icon.is_some() { 16.0 } else { 0.0 };
        let size = egui::vec2(galley.size().x + padding_x * 2.0 + icon_w, 24.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            if self.framed {
                ui.painter().rect(
                    rect,
                    CornerRadius::same(4),
                    bg_color,
                    Stroke::new(1.0, theme.surfaces.surface1),
                    egui::StrokeKind::Inside,
                );
            }

            let start_x = rect.min.x + padding_x;
            let text_x = if let Some(icon) = self.icon {
                let icon_r = Rect::from_center_size(
                    Pos2::new(start_x + 6.0, rect.center().y),
                    Vec2::splat(12.0),
                );
                icon.paint(ui.painter(), icon_r, text_color);
                start_x + 16.0
            } else {
                start_x
            };

            let text_pos = Pos2::new(text_x, rect.center().y - galley.size().y * 0.5);
            ui.painter().galley(text_pos, galley, text_color);
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
