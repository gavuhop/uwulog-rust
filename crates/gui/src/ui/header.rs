use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Rounding, Stroke};

pub fn render_header(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.horizontal(|ui| {
        // App Title / Brand
        ui.label(
            egui::RichText::new("🐱 uwulog")
                .strong()
                .size(14.5)
                .color(theme::TEXT_KEY),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Search Input Box
        let search_response = ui.add(
            egui::TextEdit::singleline(&mut app.query)
                .hint_text("🔍 Filter query (e.g. level:error, status:500, time:now..10m)...")
                .desired_width(500.0)
                .font(egui::TextStyle::Monospace)
                .margin(egui::Margin::symmetric(10.0, 6.0)),
        );

        if search_response.changed() {
            app.trigger_full_search();
        }

        // Quick Clear button if query is not empty
        if !app.query.is_empty()
            && ui
                .button(
                    egui::RichText::new("✖")
                        .size(11.0)
                        .color(theme::TEXT_PRIMARY),
                )
                .on_hover_text("Clear filter")
                .clicked()
        {
            app.query.clear();
            app.trigger_full_search();
        }

        // Right-aligned Controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Source Parameters Modal Button
            let params_btn = egui::Button::new(
                egui::RichText::new("⚙ Params")
                    .strong()
                    .color(theme::TEXT_PRIMARY),
            )
            .fill(theme::BG_SURFACE0)
            .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
            .rounding(Rounding::same(4.0));

            if ui
                .add(params_btn)
                .on_hover_text("Configure engine buffer, display limits & sources")
                .clicked()
            {
                app.show_launch_modal = true;
            }

            ui.add_space(6.0);

            // Stop / Restart Source Button
            if app.is_source_running {
                let stop_btn = egui::Button::new(
                    egui::RichText::new("⏹ Stop Process")
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                )
                .fill(theme::BTN_STOP_BG)
                .stroke(Stroke::new(1.0, theme::BTN_STOP_BORDER))
                .rounding(Rounding::same(4.0));

                if ui
                    .add(stop_btn)
                    .on_hover_text("Stop running process source")
                    .clicked()
                {
                    app.stop_current_source();
                }
            } else {
                let restart_btn = egui::Button::new(
                    egui::RichText::new("🔄 Restart Source")
                        .color(theme::TEXT_PRIMARY)
                        .strong(),
                )
                .fill(theme::BTN_RESTART_BG)
                .stroke(Stroke::new(1.0, theme::BTN_RESTART_BORDER))
                .rounding(Rounding::same(4.0));

                if ui
                    .add(restart_btn)
                    .on_hover_text("Clear logs and restart source")
                    .clicked()
                {
                    app.restart_current_source();
                }
            }
        });
    });
}
