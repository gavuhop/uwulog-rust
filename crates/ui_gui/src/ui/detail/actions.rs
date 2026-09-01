pub use crate::ui::actions::{dispatch_actions, truncate_label, FilterAction, HighlightAction};
use std::collections::HashSet;

/// Context passed down to inspector cards and fields
pub struct DetailContext<'a> {
    pub highlighted_terms: &'a HashSet<String>,
    pub filter_action: &'a mut Option<FilterAction>,
    pub highlight_action: &'a mut Option<HighlightAction>,
}
