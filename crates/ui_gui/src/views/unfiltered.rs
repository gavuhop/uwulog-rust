use super::table::{render_log_table, TableMode};
use crate::app::UwuGuiApp;
use eframe::egui;

/// Renders the Raw / Unfiltered log stream table using the shared generalized table component
pub fn render_unfiltered_table(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    render_log_table(ui, app, TableMode::Unfiltered);
}
