use crate::ui::actions::ActionContext;
use crate::ui::table::context_menu::{render_cell_context_menu, CellMenuContext};
use crate::ui::theme;
use eframe::egui;
use uwu_core_schema::LogEvent;

use std::borrow::Cow;

/// Extracts formatted display text (with newlines replaced) and raw string value for a column with zero-allocation for common fields.
fn extract_cell_content<'a>(event: &'a LogEvent, col_name: &str) -> (Cow<'a, str>, Cow<'a, str>) {
    if let Some(cow) = event.get_field_cow(col_name) {
        if cow.contains('\n') {
            (Cow::Owned(cow.replace('\n', " ↵ ")), cow)
        } else {
            (cow.clone(), cow)
        }
    } else {
        (Cow::Borrowed("-"), Cow::Borrowed("-"))
    }
}

pub fn render_cell(
    ui: &mut egui::Ui,
    event: &LogEvent,
    col_name: &str,
    row_color: egui::Color32,
    is_selected: bool,
    is_row_highlighted: bool,
    ctx: &mut ActionContext<'_>,
) -> bool {
    let cell_rect = ui.max_rect();
    if is_row_highlighted {
        ui.painter()
            .rect_filled(cell_rect, egui::Rounding::ZERO, theme::BG_ROW_HIGHLIGHT);
    } else if is_selected {
        ui.painter()
            .rect_filled(cell_rect, egui::Rounding::ZERO, theme::BG_ROW_SELECTED);
    }

    let (cell_text, raw_cell_val) = extract_cell_content(event, col_name);
    let cell_id = ui.make_persistent_id((event.id, col_name));
    let mut text_val = cell_text.as_ref();

    let highlighted_terms_ref = ctx.highlighted_terms;
    let mut layouter = |ui: &egui::Ui, _text: &str, _wrap_width: f32| {
        let mut job = theme::create_highlighted_layout_job(
            &cell_text,
            row_color,
            egui::FontId::monospace(11.5),
            highlighted_terms_ref,
        );
        job.wrap.max_width = f32::INFINITY;
        ui.fonts(|f| f.layout_job(job))
    };

    let edit = egui::TextEdit::singleline(&mut text_val)
        .id(cell_id)
        .font(egui::FontId::monospace(11.5))
        .text_color(row_color)
        .frame(false)
        .clip_text(true)
        .desired_width(f32::INFINITY)
        .layouter(&mut layouter);

    let resp = ui.add(edit);
    let is_secondary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
    let clicked = resp.clicked() && !is_secondary_down;

    let selected_text = crate::ui::actions::extract_selected_text(
        ui.ctx(),
        cell_id,
        &cell_text,
        resp.clicked() && !is_secondary_down,
    );

    resp.context_menu(|ui| {
        let menu_ctx = CellMenuContext {
            col_name,
            raw_cell_val: &raw_cell_val,
            selected_text: selected_text.as_deref(),
            is_row_highlighted,
            event_id: event.id,
        };
        render_cell_context_menu(ui, menu_ctx, ctx);
    });

    clicked
}
