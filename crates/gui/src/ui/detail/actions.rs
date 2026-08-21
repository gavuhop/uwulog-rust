use crate::app::UwuGuiApp;
use std::collections::HashSet;

pub enum FilterAction {
    Apply(String),
    Exclude(String),
}

pub enum HighlightAction {
    ToggleRow(uuid::Uuid),
    ToggleTerm(String),
    ClearAll,
}

pub struct DetailContext<'a> {
    pub highlighted_terms: &'a HashSet<String>,
    pub filter_action: &'a mut Option<FilterAction>,
    pub highlight_action: &'a mut Option<HighlightAction>,
}

pub fn dispatch_actions(
    app: &mut UwuGuiApp,
    filter_action: Option<FilterAction>,
    highlight_action: Option<HighlightAction>,
) {
    if let Some(action) = filter_action {
        match action {
            FilterAction::Apply(term) => app.apply_filter_term(&term),
            FilterAction::Exclude(term) => app.exclude_filter_term(&term),
        }
    }

    if let Some(action) = highlight_action {
        match action {
            HighlightAction::ToggleRow(id) => app.toggle_row_highlight(id),
            HighlightAction::ToggleTerm(term) => app.toggle_term_highlight(&term),
            HighlightAction::ClearAll => app.clear_all_highlights(),
        }
    }
}
