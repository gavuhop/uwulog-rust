//! Zed-style ModalContainer primitive with backdrop scrim, header, body, and action footer.

use crate::theme::ActiveTheme;
use eframe::egui::{self, Align2, Color32, Context, CornerRadius, Id, Key, Order, Stroke, Vec2};

pub struct ModalContainer<'a> {
    id: Id,
    title: &'a str,
    subtitle: Option<&'a str>,
    width: f32,
    default_height: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    resizable: bool,
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
            default_height: None,
            min_height: None,
            max_height: None,
            resizable: false,
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

    pub fn default_height(mut self, height: f32) -> Self {
        self.default_height = Some(height);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
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
        let screen_rect = ctx.viewport_rect();
        egui::Area::new(self.id.with("_backdrop"))
            .order(Order::Middle)
            .fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(150));
                if response.clicked() {
                    close_requested = true;
                }
            });

        let theme = ctx.app_theme();
        // Quản lý session của modal: khi modal được mở lại sau khi đóng,
        // làm mới generation để tính toán lại kích thước responsive và vị trí chính giữa màn hình
        let current_frame = ctx.cumulative_pass_nr();
        let last_frame_id = self.id.with("_last_frame_nr");
        let gen_id = self.id.with("_gen");
        let last_frame: Option<u64> = ctx.data(|d| d.get_temp(last_frame_id));
        let mut gen: u64 = ctx.data(|d| d.get_temp(gen_id)).unwrap_or(0);

        if self.resizable && last_frame != Some(current_frame.saturating_sub(1)) {
            gen = gen.wrapping_add(1);
            ctx.data_mut(|d| d.insert_temp(gen_id, gen));
        }
        ctx.data_mut(|d| d.insert_temp(last_frame_id, current_frame));

        let window_id = if self.resizable {
            self.id.with(gen)
        } else {
            self.id
        };

        // 3. Khung cửa sổ Modal nổi ở giữa màn hình
        let mut modal_window = egui::Window::new(self.title)
            .id(window_id)
            .title_bar(false)
            .collapsible(false)
            .order(Order::Foreground)
            .constrain_to(screen_rect)
            .frame(
                egui::Frame::default()
                    .fill(theme.surfaces.mantle)
                    .stroke(Stroke::new(1.0, theme.borders.border))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(16))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(50),
                    }),
            );

        if self.resizable {
            let max_w = (screen_rect.width() - 32.0).max(360.0);
            let max_h = (screen_rect.height() - 32.0).max(200.0);
            let min_w = 400.0_f32.min(max_w);
            let min_h = 240.0_f32.min(max_h);

            let initial_w = self.width.clamp(min_w, max_w);
            let initial_h = self.default_height.unwrap_or(540.0).clamp(min_h, max_h);

            modal_window = modal_window
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .default_size(egui::vec2(initial_w, initial_h))
                .min_width(min_w)
                .min_height(self.min_height.unwrap_or(min_h).min(max_h))
                .max_width(self.max_height.unwrap_or(max_w).min(max_w))
                .max_height(self.max_height.unwrap_or(max_h).min(max_h))
                .resizable(true);
        } else {
            modal_window = modal_window
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(egui::vec2(self.width, 0.0))
                .resizable(false);

            if let Some(h) = self.min_height {
                modal_window = modal_window.min_height(h);
            }

            if let Some(h) = self.max_height {
                modal_window = modal_window.max_height(h);
            }
        }

        modal_window.show(ctx, |ui| {
            ui.set_min_width(ui.available_width());
            // Header: Title + Subtitle (chỉ hiển thị nếu title không rỗng)
            if !self.title.is_empty() {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(self.title)
                            .strong()
                            .size(15.0)
                            .color(theme.text.primary),
                    );
                    if let Some(sub) = self.subtitle {
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(sub).size(11.0).color(theme.text.muted));
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
            }

            // Body & Footer
            if self.resizable && add_footer.is_some() {
                let footer_h = 50.0;
                let body_h = (ui.available_height() - footer_h).max(60.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), body_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| add_body(ui),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                if let Some(footer_fn) = add_footer {
                    ui.horizontal(|ui| {
                        footer_fn(ui, &mut close_requested);
                    });
                }
            } else {
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
            }
        });

        ModalResponse {
            closed: close_requested,
        }
    }
}
