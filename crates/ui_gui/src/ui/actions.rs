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
    ToggleRow(u64),
    ToggleTerm(String),
    ClearAll,
}

/// Actions for controlling the unfiltered stream split view
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnfilteredAction {
    Open(u64),
    Close,
    FocusInMain,
}

/// Dispatches requested filter, highlight, and unfiltered stream actions to the main application state.
pub fn dispatch_actions(
    app: &mut UwuGuiApp,
    filter_action: Option<FilterAction>,
    highlight_action: Option<HighlightAction>,
    unfiltered_action: Option<UnfilteredAction>,
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

    if let Some(action) = unfiltered_action {
        match action {
            UnfilteredAction::Open(id) => app.open_unfiltered_stream(Some(id)),
            UnfilteredAction::Close => app.close_unfiltered_stream(),
            UnfilteredAction::FocusInMain => app.focus_in_main_and_clear_filter(),
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
