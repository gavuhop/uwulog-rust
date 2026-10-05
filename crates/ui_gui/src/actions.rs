//! UI utilities and action definitions (Zed-style Command Pattern).

use crate::state::{ActiveTab, SearchState};
use std::collections::HashSet;
use uwu_core_schema::LogEvent;
use uwu_core_workspace::{SourceConfig, Workspace};

/// Unified Action enum for high-level application & session state mutations
#[derive(Debug, Clone)]
pub enum AppAction {
    SwitchSession(usize),
    CloseSession(usize),
    CycleSession(bool),
    OpenWorkspace(Workspace),
    LoadWorkspace(Workspace),
    DeleteWorkspace(uuid::Uuid),
    StartSource,
    StopSource,
    RestartSource,
    OpenLaunchModal,
    CloseLaunchModal,
    ApplyLaunchModal,
    ApplyAndRestartSource(SourceConfig),
    OpenColumnsModal,
    CloseColumnsModal,
    ApplyColumnsModal,
    SelectLog(Option<LogEvent>),
    NavigateUp,
    NavigateDown,
    ScrollTableLeft,
    ScrollTableRight,
    PageUp,
    PageDown,
    ScrollToTop,
    ScrollToBottom,
    CopySelectedLog,
    SwitchTab(ActiveTab),
    ToggleStreamView,

    // Search Query & Filtering
    FocusSearch,
    ToggleSearchHistory,
    ApplyFilterTerm(String),
    ExcludeFilterTerm(String),
    ClearQuery,

    // Highlights
    ToggleRowHighlight(u64),
    ToggleTermHighlight(String),
    ClearAllHighlights,

    // Stream & Latch Controls
    ToggleLatch,
    Unlatch,
    Latch,
    ToggleUnfilteredLive,
    RefreshUnfilteredSnapshot,
    OpenUnfilteredStream(Option<u64>),
    ViewRawContext,
    CloseUnfilteredStream,
    FocusInMainAndClearFilter,
    LoadOlderLogs(usize),

    // Project Picker
    ToggleProjectPicker,
    CloseProjectPicker,

    // Remote Servers (WSL)
    OpenRemoteServersModal,
    CloseRemoteServersModal,
    ToggleRemoteServersModal,

    // Main Menu & About
    ToggleMainMenu,
    CloseMainMenu,
    OpenAboutModal,
    CloseAboutModal,
    OpenKeymapModal,
    CloseKeymapModal,
    ApplyKeymapModal(Box<uwu_core_keymap::KeymapManager>),
    SwitchTheme(String),
    QuitApp,

    // Global Dismiss / Stack Pop
    DismissTopLayer,

    // Viewport Zoom
    ZoomIn,
    ZoomOut,
    ResetZoom,

    // Autocomplete & Search Navigation
    CommitSearch,
    AutocompleteNext,
    AutocompletePrev,
    AutocompleteConfirm,
}

/// Luồng sự kiện bất đồng bộ gửi từ Background Workers về UI Thread (Worker -> UI Pipeline)
#[derive(Debug)]
pub enum AppEvent {
    SearchResultsReady {
        session_id: uuid::Uuid,
        query_id: u64,
        total_matched: usize,
        logs: Vec<LogEvent>,
    },
    IncrementalLogsReady {
        session_id: uuid::Uuid,
        matched: usize,
        logs: Vec<LogEvent>,
        total_processed: u64,
    },
    IncrementalUnfilteredReady {
        session_id: uuid::Uuid,
        logs: Vec<LogEvent>,
    },
    ReversePaginationReady {
        session_id: uuid::Uuid,
        logs: Vec<LogEvent>,
        is_unfiltered: bool,
        reached_oldest: bool,
    },
}

/// Unified render context passed down to subcomponents (tables, detail inspector, cells)
pub struct ActionContext<'a> {
    pub highlighted_terms: &'a HashSet<String>,
    pub has_any_highlights: bool,
    pub is_filtering: bool,
    pub is_unfiltered_tab: bool,
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
            let [min_c, max_c] = range.sorted_cursors();
            if min_c.index.0 < max_c.index.0 {
                let s = min_c.index.0;
                let e = max_c.index.0;
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

/// Renders standard context menu buttons: Filter, Exclude, and Toggle Keyword Highlight.
pub fn render_filter_actions_menu(
    ui: &mut eframe::egui::Ui,
    field_name: Option<&str>,
    text: &str,
    ctx: &mut ActionContext<'_>,
) {
    let display_text = truncate_label(text, 25);

    if ui.button(format!("Filter \"{}\"", display_text)).clicked() {
        let term = match field_name {
            Some(f) => SearchState::format_field_term(f, text),
            None => SearchState::format_selection_term(text),
        };
        *ctx.action = Some(AppAction::ApplyFilterTerm(term));
        ui.close();
    }

    if ui.button(format!("Exclude \"{}\"", display_text)).clicked() {
        let term = match field_name {
            Some(f) => SearchState::format_field_term(f, text),
            None => SearchState::format_selection_term(text),
        };
        *ctx.action = Some(AppAction::ExcludeFilterTerm(term));
        ui.close();
    }

    let text_clean = text.trim().to_lowercase();
    let is_hl = !text_clean.is_empty() && ctx.highlighted_terms.contains(&text_clean);
    let hl_text = if is_hl {
        format!("Unhighlight \"{}\"", display_text)
    } else {
        format!("Highlight \"{}\"", display_text)
    };
    if ui.button(hl_text).clicked() {
        *ctx.action = Some(AppAction::ToggleTermHighlight(text.to_string()));
        ui.close();
    }
}

/// Copies text to clipboard and closes current context menu.
pub fn copy_and_close(ui: &mut eframe::egui::Ui, text: &str) {
    if ui.button("Copy value").clicked() {
        ui.ctx().copy_text(text.to_string());
        ui.close();
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
