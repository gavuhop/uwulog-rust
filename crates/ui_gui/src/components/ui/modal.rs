//! Zed-style ModalContainer primitive with backdrop scrim, header, body, and action footer.

use crate::theme;
use eframe::egui::{self, Align2, Color32, Context, Id, Key, Order, Rounding, Stroke, Vec2};

pub struct ModalContainer<'a> {
    id: Id,
    title: &'a str,
    subtitle: Option<&'a str>,
    width: f32,
    max_height: Option<f32>,
}

pub struct ModalResponse {
    pub closed: bool,
}

impl<'a> ModalContainer<'a> {
    pub fn new(id_str: &'a str, title: &'a str) -> Self {
        Self {
            id: Id::new(id_str),
            title,
            subtitle: None,
            width: 480.0,
            max_height: None,
        }
    }

    pub fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = Some(subtitle);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Render modal bao gồm backdrop scrim, window frame, header, body và footer
    pub fn show(
        self,
        ctx: &Context,
        add_body: impl FnOnce(&mut egui::Ui),
        add_footer: Option<impl FnOnce(&mut egui::Ui, &mut bool)>,
    ) -> ModalResponse {
        let mut close_requested = false;

        // 1. Kiểm tra phím Escape để đóng
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close_requested = true;
        }

        // 2. Lớp phủ nền tối (Backdrop scrim)
        let screen_rect = ctx.screen_rect();
        egui::Area::new(self.id.with("_backdrop"))
            .order(Order::Middle)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(150));
                if response.clicked() {
                    close_requested = true;
                }
            });

        // 3. Khung cửa sổ Modal nổi ở giữa màn hình
        let mut modal_window = egui::Window::new(self.title)
            .id(self.id)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .order(Order::Foreground)
            .fixed_size(egui::vec2(self.width, 0.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(theme::BG_MANTLE)
                    .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(16.0))
                    .shadow(egui::epaint::Shadow {
                        offset: egui::vec2(0.0, 8.0),
                        blur: 24.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(180),
                    }),
            );

        if let Some(h) = self.max_height {
            modal_window = modal_window.max_height(h);
        }

        modal_window.show(ctx, |ui| {
            // Header: Title + Subtitle + Close Button
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(self.title)
                            .strong()
                            .size(14.0)
                            .color(theme::TEXT_PRIMARY),
                    );
                    if let Some(sub) = self.subtitle {
                        ui.label(egui::RichText::new(sub).size(11.0).color(theme::TEXT_MUTED));
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if super::IconButton::new("✕")
                        .size(20.0)
                        .tooltip("Close (Esc)")
                        .show(ui)
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Body
            add_body(ui);

            // Footer (nếu có)
            if let Some(footer_fn) = add_footer {
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    footer_fn(ui, &mut close_requested);
                });
            }
        });

        ModalResponse {
            closed: close_requested,
        }
    }
}
