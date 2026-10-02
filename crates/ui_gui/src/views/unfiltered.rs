use super::table::render_log_table;
use crate::actions::AppAction;
use crate::session::GuiSession;
use crate::state::ActiveTab;
use eframe::egui;

/// Renders the Raw / Unfiltered log stream table using the shared generalized table component
pub fn render_unfiltered_table(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    render_log_table(ui, session, ActiveTab::Unfiltered, dispatch);
}
