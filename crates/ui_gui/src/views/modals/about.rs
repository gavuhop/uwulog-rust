use crate::actions::AppAction;
use crate::components::ui::{AppButton, ModalContainer};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui;

/// Render modal giới thiệu phiên bản và thông tin ứng dụng uwulog
pub fn render_about_modal(
    ctx: &egui::Context,
    is_open: bool,
    keymap: &KeymapManager,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    let resp = ModalContainer::new("about_uwulog_modal", "About uwulog")
        .width(360.0)
        .show(
            ctx,
            |ui| {
                let theme = ui.app_theme();
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("🐱 uwulog")
                            .size(20.0)
                            .strong()
                            .color(theme.text.accent),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .size(11.0)
                            .color(theme.text.muted),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "High-performance Realtime Log Viewer & Workspace Monitor built with Rust.",
                        )
                        .size(12.0)
                        .color(theme.text.primary),
                    );
                });
            },
            Some(|ui: &mut egui::Ui, close_req: &mut bool| {
                ui.vertical_centered(|ui| {
                    if AppButton::new()
                        .label("Close")
                        .show(ui)
                        .clicked()
                        || keymap.consume_input(ui, KeyContext::Modal)
                            == Some(KeyAction::ConfirmSelection)
                    {
                        *close_req = true;
                    }
                });
            }),
        );

    if resp.closed {
        dispatch(AppAction::CloseAboutModal);
    }
}
