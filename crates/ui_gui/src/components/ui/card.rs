use crate::theme::ActiveTheme;
use eframe::egui::{self, Stroke};

pub fn render_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let theme = ui.app_theme();
    let frame = egui::Frame::default()
        .fill(theme.surfaces.base)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(10))
        .stroke(Stroke::new(1.0, theme.borders.border));

    frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                egui::RichText::new(title)
                    .size(12.0)
                    .strong()
                    .color(theme.text.accent),
            );
            ui.add_space(4.0);
            add_contents(ui);
        })
        .response
}
