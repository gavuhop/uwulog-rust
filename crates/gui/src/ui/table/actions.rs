pub use crate::ui::actions::{dispatch_actions, truncate_label, FilterAction, HighlightAction};
use std::collections::HashSet;

/// Context passed down to table cells during row rendering
pub struct TableRenderContext<'a> {
    pub highlighted_terms: &'a HashSet<String>,
    pub has_any_highlights: bool,
    pub filter_action: &'a mut Option<FilterAction>,
    pub highlight_action: &'a mut Option<HighlightAction>,
}
