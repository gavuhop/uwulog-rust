//! UI utilities and string formatting helpers for actions.

use crate::app::AppAction;
use std::collections::HashSet;

/// Unified render context passed down to subcomponents (tables, detail inspector, cells)
pub struct ActionContext<'a> {
    pub highlighted_terms: &'a HashSet<String>,
    pub has_any_highlights: bool,
    pub action: &'a mut Option<AppAction>,
}

/// Formats a clean truncated label for context menus and tooltips (e.g. "very long tex...").
pub fn truncate_label(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        format!("{}...", text.chars().take(max_chars).collect::<String>())
    } else {
        text.to_string()
    }
}

/// Reads the currently selected text range in an egui TextEdit, or recovers it from temp storage on right click.
pub fn extract_selected_text(
    ctx: &eframe::egui::Context,
    id: eframe::egui::Id,
    full_text: &str,
    should_clear: bool,
) -> Option<String> {
    let mut selected_text = None;

    if let Some(state) = eframe::egui::text_edit::TextEditState::load(ctx, id) {
        if let Some(range) = state.cursor.char_range() {
            let [min_c, max_c] = range.sorted();
            if min_c.index < max_c.index {
                let s = min_c.index;
                let e = max_c.index;
                let txt: String = full_text
                    .chars()
                    .skip(s)
                    .take(e.saturating_sub(s))
                    .collect();
                let clean_txt = uwu_core_util::strip_ansi(&txt).replace(" ↵ ", " ");
                let trimmed = clean_txt.trim().to_string();
                if !trimmed.is_empty() {
                    selected_text = Some(trimmed.clone());
                    ctx.data_mut(|d| d.insert_temp(id, trimmed));
                }
            } else if should_clear {
                ctx.data_mut(|d| d.remove_temp::<String>(id));
            }
        }
    }

    if selected_text.is_none() {
        selected_text = ctx.data(|d| d.get_temp::<String>(id));
    }

    selected_text
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
