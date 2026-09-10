use crate::app::AppAction;
use crate::ui::actions::{copy_and_close, render_filter_actions_menu, ActionContext};
use eframe::egui;

/// Parameters for rendering the right-click context menu of a table cell
pub struct CellMenuContext<'a> {
    pub col_name: &'a str,
    pub raw_cell_val: &'a str,
    pub selected_text: Option<&'a str>,
    pub is_row_highlighted: bool,
    pub event_id: u64,
}

pub fn render_cell_context_menu(
    ui: &mut egui::Ui,
    menu_ctx: CellMenuContext<'_>,
    render_ctx: &mut ActionContext<'_>,
) {
    ui.set_min_width(180.0);

    // 1. Nhóm từ bôi đen trong ô (nếu có lựa chọn bôi đen)
    if let Some(sel) = menu_ctx.selected_text {
        render_filter_actions_menu(ui, Some(menu_ctx.col_name), sel, render_ctx);
        ui.separator();
    }

    // 2. Nhóm thao tác với toàn bộ giá trị ô
    render_filter_actions_menu(
        ui,
        Some(menu_ctx.col_name),
        menu_ctx.raw_cell_val,
        render_ctx,
    );
    ui.separator();

    // 3. Nhóm thao tác Dòng & Toàn cục
    if ui.button("🔍 View in unfiltered stream").clicked() {
        *render_ctx.action = Some(AppAction::OpenUnfilteredStream(Some(menu_ctx.event_id)));
        ui.close_menu();
    }

    let highlight_label = if menu_ctx.is_row_highlighted {
        "Unhighlight row"
    } else {
        "Highlight row"
    };
    if ui.button(highlight_label).clicked() {
        *render_ctx.action = Some(AppAction::ToggleRowHighlight(menu_ctx.event_id));
        ui.close_menu();
    }

    if render_ctx.has_any_highlights && ui.button("Unhighlight all").clicked() {
        *render_ctx.action = Some(AppAction::ClearAllHighlights);
        ui.close_menu();
    }

    ui.separator();

    // 4. Copy giá trị vào Clipboard
    copy_and_close(ui, menu_ctx.raw_cell_val);
}
