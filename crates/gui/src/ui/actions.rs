use crate::app::UwuGuiApp;

/// Actions for search query filtering (Filter / Exclude)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterAction {
    Apply(String),
    Exclude(String),
}

/// Actions for UI highlighting (row bookmarking & keyword term highlights)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HighlightAction {
    ToggleRow(uuid::Uuid),
    ToggleTerm(String),
    ClearAll,
}

/// Dispatches requested filter and highlight actions to the main application state.
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

/// Formats a clean truncated label for context menus and tooltips (e.g. "very long tex...").
pub fn truncate_label(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        format!("{}...", text.chars().take(max_chars).collect::<String>())
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_label() {
        assert_eq!(truncate_label("short", 10), "short");
        assert_eq!(truncate_label("exact_len!", 10), "exact_len!");
        assert_eq!(
            truncate_label("this is a very long string", 10),
            "this is a ..."
        );
    }
}
