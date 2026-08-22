use crate::app::UwuGuiApp;
use crate::ui::table::actions::{
    truncate_label, FilterAction, HighlightAction, TableRenderContext,
};
use eframe::egui;

/// Parameters for rendering the right-click context menu of a table cell
pub struct CellMenuContext<'a> {
    pub col_name: &'a str,
    pub raw_cell_val: &'a str,
    pub selected_text: Option<&'a str>,
    pub is_row_highlighted: bool,
    pub event_id: uuid::Uuid,
}

pub fn render_cell_context_menu(
    ui: &mut egui::Ui,
    menu_ctx: CellMenuContext<'_>,
    render_ctx: &mut TableRenderContext<'_>,
) {
    ui.set_min_width(180.0);

    // 1. Nhóm từ bôi đen trong ô (nếu có lựa chọn bôi đen)
    if let Some(sel) = menu_ctx.selected_text {
        let display_sel = truncate_label(sel, 25);

        if ui.button(format!("Filter \"{}\"", display_sel)).clicked() {
            let term = UwuGuiApp::format_field_term(menu_ctx.col_name, sel);
            *render_ctx.filter_action = Some(FilterAction::Apply(term));
            ui.close_menu();
        }

        if ui.button(format!("Exclude \"{}\"", display_sel)).clicked() {
            let term = UwuGuiApp::format_field_term(menu_ctx.col_name, sel);
            *render_ctx.filter_action = Some(FilterAction::Exclude(term));
            ui.close_menu();
        }

        let sel_clean = sel.trim().to_lowercase();
        let is_term_hl = !sel_clean.is_empty() && render_ctx.highlighted_terms.contains(&sel_clean);
        let hl_term_text = if is_term_hl {
            format!("Unhighlight \"{}\"", display_sel)
        } else {
            format!("Highlight \"{}\"", display_sel)
        };
        if ui.button(hl_term_text).clicked() {
            *render_ctx.highlight_action = Some(HighlightAction::ToggleTerm(sel.to_string()));
            ui.close_menu();
        }

        ui.separator();
    }

    // 2. Nhóm thao tác với toàn bộ giá trị ô
    let display_val = truncate_label(menu_ctx.raw_cell_val, 25);

    if ui.button(format!("Filter \"{}\"", display_val)).clicked() {
        let term = UwuGuiApp::format_field_term(menu_ctx.col_name, menu_ctx.raw_cell_val);
        *render_ctx.filter_action = Some(FilterAction::Apply(term));
        ui.close_menu();
    }

    if ui.button(format!("Exclude \"{}\"", display_val)).clicked() {
        let term = UwuGuiApp::format_field_term(menu_ctx.col_name, menu_ctx.raw_cell_val);
        *render_ctx.filter_action = Some(FilterAction::Exclude(term));
        ui.close_menu();
    }

    let val_clean = menu_ctx.raw_cell_val.trim().to_lowercase();
    let is_cell_val_hl = !val_clean.is_empty() && render_ctx.highlighted_terms.contains(&val_clean);
    let hl_cell_text = if is_cell_val_hl {
        format!("Unhighlight \"{}\"", display_val)
    } else {
        format!("Highlight \"{}\"", display_val)
    };
    if ui.button(hl_cell_text).clicked() {
        *render_ctx.highlight_action = Some(HighlightAction::ToggleTerm(
            menu_ctx.raw_cell_val.to_string(),
        ));
        ui.close_menu();
    }

    ui.separator();

    // 3. Nhóm thao tác Dòng & Toàn cục
    if menu_ctx.is_row_highlighted {
        if ui.button("Unhighlight row").clicked() {
            *render_ctx.highlight_action = Some(HighlightAction::ToggleRow(menu_ctx.event_id));
            ui.close_menu();
        }
    } else if ui.button("Highlight row").clicked() {
        *render_ctx.highlight_action = Some(HighlightAction::ToggleRow(menu_ctx.event_id));
        ui.close_menu();
    }

    if render_ctx.has_any_highlights && ui.button("Unhighlight all").clicked() {
        *render_ctx.highlight_action = Some(HighlightAction::ClearAll);
        ui.close_menu();
    }

    ui.separator();

    // 4. Copy giá trị vào Clipboard
    if ui.button("Copy value").clicked() {
        ui.ctx().output_mut(|o| {
            o.copied_text = menu_ctx.raw_cell_val.to_string();
        });
        ui.close_menu();
    }
}
