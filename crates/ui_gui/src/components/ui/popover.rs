//! Zed-style PopoverContainer primitive with positioning, click-outside and escape dismiss.

use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, Context, CornerRadius, Id, Key, Order, Pos2, Rect, Stroke, Ui};

pub struct PopoverContainer<'a> {
    id: Id,
    trigger_rect: Rect,
    width: f32,
    max_height: Option<f32>,
    custom_pos: Option<Pos2>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

pub struct PopoverResponse {
    pub closed: bool,
}

impl<'a> PopoverContainer<'a> {
    pub fn new(id_str: &'a str, trigger_rect: Rect) -> Self {
        Self {
            id: Id::new(id_str),
            trigger_rect,
            width: 300.0,
            max_height: None,
            custom_pos: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    pub fn custom_pos(mut self, pos: Pos2) -> Self {
        self.custom_pos = Some(pos);
        self
    }

    /// Render popover nổi ngay bên dưới trigger_rect kèm cơ chế tự đóng khi bấm Escape hoặc click ra ngoài
    pub fn show(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui)) -> PopoverResponse {
        let mut close_requested = false;

        // 1. Phím Escape đóng popover
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close_requested = true;
        }

        let pos = self
            .custom_pos
            .unwrap_or_else(|| Pos2::new(self.trigger_rect.min.x, self.trigger_rect.max.y + 4.0));

        let est_height = self.max_height.unwrap_or(360.0);
        let popup_rect = Rect::from_min_size(pos, egui::vec2(self.width, est_height));

        // 2. Click outside detection
        if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_click()) {
            if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !self.trigger_rect.contains(click_pos) && !popup_rect.contains(click_pos) {
                    close_requested = true;
                }
            }
        }

        // 3. Render Area
        egui::Area::new(self.id)
            .order(Order::Foreground)
            .fixed_pos(pos)
            .movable(false)
            .show(ctx, |ui| {
                let theme = ui.app_theme();
                let frame = egui::Frame::default()
                    .fill(theme.surfaces.mantle)
                    .stroke(Stroke::new(1.0, theme.borders.border))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(160),
                    });
                let total_margin = frame.total_margin();
                frame.show(ui, |ui| {
                    let inner_width = (self.width - total_margin.sum().x).max(0.0);
                    ui.set_width(inner_width);
                    if let Some(h) = self.max_height {
                        ui.set_max_height(h);
                    }
                    add_contents(ui);
                });
            });

        PopoverResponse {
            closed: close_requested,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popover_outer_width_matches_requested_width() {
        let ctx = egui::Context::default();
        let trigger_rect = Rect::from_min_size(Pos2::new(100.0, 50.0), egui::vec2(300.0, 32.0));
        let raw_input = egui::RawInput::default();
        let mut output = ctx.run_ui(raw_input, |ui| {
            let ctx = ui.ctx();
            let mut captured_row_rect = Rect::NOTHING;
            let _ = PopoverContainer::new("test_popover", trigger_rect)
                .width(300.0)
                .show(ctx, |ui| {
                    let desired_size = egui::vec2(ui.available_width(), 26.0);
                    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::click());
                    captured_row_rect = rect;
                });
            // In egui 0.31+, Frame::total_margin() accounts for inner_margin (8.0) + stroke (1.0) on each side (total 9.0 per side).
            // Inner content is width - 18.0 = 282.0.
            assert_eq!(captured_row_rect.width(), 300.0 - 18.0);
            assert_eq!(captured_row_rect.min.x, 100.0 + 9.0);
            assert_eq!(captured_row_rect.max.x, 100.0 + 300.0 - 9.0);
        });
        output.textures_delta.clear();
    }
}
