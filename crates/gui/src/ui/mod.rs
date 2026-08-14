pub mod detail;
pub mod header;
pub mod launch_modal;
pub mod table;

use crate::app::UwuGuiApp;
use eframe::egui;

pub fn render_ui(ctx: &egui::Context, app: &mut UwuGuiApp) {
    // Top Panel: Search Bar, Fast Presets, Action Controls
    egui::TopBottomPanel::top("header_panel")
        .resizable(false)
        .show(ctx, |ui| {
            header::render_header(ui, app);
        });

    // Central Panel: Left Table View & Right Inspector Panel
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.selected_log.is_some() {
            egui::SidePanel::right("detail_inspector_panel")
                .resizable(true)
                .default_width(360.0)
                .min_width(280.0)
                .show_inside(ui, |ui| {
                    detail::render_detail(ui, app);
                });
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            table::render_table(ui, app);
        });
    });

    // Modal Dialog: Launch & Source Parameters
    launch_modal::render_launch_modal(ctx, app);
}
