pub mod detail;
pub mod header;
pub mod launch_modal;
pub mod table;
pub mod theme;

use crate::app::UwuGuiApp;
use eframe::egui;

pub fn render_ui(ctx: &egui::Context, app: &mut UwuGuiApp) {
    // Phím Escape: Đóng modal Params trước, nếu không mở modal thì đóng Log Inspector
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if app.show_launch_modal {
            app.show_launch_modal = false;
        } else if app.selected_log.is_some() {
            app.selected_log = None;
        }
    }

    // Top Panel: Search Bar, Fast Presets, Action Controls
    egui::TopBottomPanel::top("header_panel")
        .frame(
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                .stroke(egui::Stroke::new(1.0, theme::BG_SURFACE0)),
        )
        .resizable(false)
        .show(ctx, |ui| {
            header::render_header(ui, app);
        });

    // Central Panel: Left Table View & Right Inspector Panel
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(theme::BG_BASE)
                .inner_margin(egui::Margin::same(0.0)),
        )
        .show(ctx, |ui| {
            if app.selected_log.is_some() {
                egui::SidePanel::right("detail_inspector_panel")
                    .frame(
                        egui::Frame::default()
                            .fill(theme::BG_MANTLE)
                            .inner_margin(egui::Margin::same(12.0))
                            .stroke(egui::Stroke::new(1.0, theme::BG_SURFACE0)),
                    )
                    .resizable(true)
                    .default_width(380.0)
                    .min_width(300.0)
                    .show_inside(ui, |ui| {
                        detail::render_detail(ui, app);
                    });
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::default()
                        .fill(theme::BG_BASE)
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0)),
                )
                .show_inside(ui, |ui| {
                    table::render_table(ui, app);
                });
        });

    // Modal Dialog: Launch & Source Parameters
    launch_modal::render_launch_modal(ctx, app);
}
