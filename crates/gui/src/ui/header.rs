use crate::app::UwuGuiApp;
use eframe::egui;

pub fn render_header(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("🔍 Filter:").strong());

        let search_response = ui.add(
            egui::TextEdit::singleline(&mut app.query)
                .hint_text("Enter query (e.g. level:error, timestamp:now..10m)...")
                .desired_width(450.0),
        );

        if search_response.changed() {
            app.trigger_full_search();
        }

        // Align right for control buttons
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(egui::RichText::new("⚙ Params & Source").strong())
                .clicked()
            {
                app.show_launch_modal = true;
            }

            ui.add_space(4.0);

            // Stop / Restart Source Button
            if app.is_source_running {
                let stop_btn = egui::Button::new(
                    egui::RichText::new("⏹ Stop Process").color(egui::Color32::RED),
                );
                if ui.add(stop_btn).clicked() {
                    app.stop_current_source();
                }
            } else {
                let restart_btn = egui::Button::new(
                    egui::RichText::new("🔄 Restart Source").color(egui::Color32::GOLD),
                );
                if ui.add(restart_btn).clicked() {
                    app.restart_current_source();
                }
            }
        });
    });
    ui.add_space(4.0);
}
