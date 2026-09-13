//! Zed-style PopoverContainer primitive with positioning, click-outside and escape dismiss.

use crate::theme;
use eframe::egui::{self, Color32, Context, Id, Key, Order, Pos2, Rect, Rounding, Stroke, Ui};

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
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(theme::BG_MANTLE)
                    .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 4.0),
                        blur: 16.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(160),
                    })
                    .show(ui, |ui| {
                        ui.set_width(self.width);
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
