use crate::ui::table::actions::TableRenderContext;
use crate::ui::table::context_menu::{render_cell_context_menu, CellMenuContext};
use crate::ui::theme;
use eframe::egui;
use uwu_core_schema::LogEvent;

use std::borrow::Cow;

/// Extracts formatted display text (with newlines replaced) and raw string value for a column with zero-allocation for common fields.
fn extract_cell_content<'a>(event: &'a LogEvent, col_name: &str) -> (Cow<'a, str>, Cow<'a, str>) {
    match col_name {
        "timestamp" => (
            Cow::Borrowed(&event.timestamp),
            Cow::Borrowed(&event.timestamp),
        ),
        "level" => (
            Cow::Borrowed(event.level.as_str()),
            Cow::Borrowed(event.level.as_str()),
        ),
        "message" => {
            if event.message.contains('\n') {
                (
                    Cow::Owned(event.message.replace('\n', " ↵ ")),
                    Cow::Borrowed(&event.message),
                )
            } else {
                (Cow::Borrowed(&event.message), Cow::Borrowed(&event.message))
            }
        }
        "source" | "source_id" => (
            Cow::Borrowed(&event.source_id),
            Cow::Borrowed(&event.source_id),
        ),
        custom_key => {
            if let Some(val) = event.fields.get(custom_key) {
                match val {
                    serde_json::Value::String(s) => {
                        (Cow::Borrowed(s.as_str()), Cow::Borrowed(s.as_str()))
                    }
                    _ => {
                        let s = val.to_string();
                        (Cow::Owned(s.clone()), Cow::Owned(s))
                    }
                }
            } else {
                (Cow::Borrowed("-"), Cow::Borrowed("-"))
            }
        }
    }
}

/// Reads the currently selected text range in the cell's TextEdit, or recovers it from temp storage on right click.
fn extract_selected_text(
    ui: &mut egui::Ui,
    cell_id: egui::Id,
    cell_text: &str,
    is_left_clicked: bool,
) -> Option<String> {
    let mut selected_text = None;

    if let Some(state) = egui::text_edit::TextEditState::load(ui.ctx(), cell_id) {
        if let Some(range) = state.cursor.char_range() {
            let [min_c, max_c] = range.sorted();
            if min_c.index < max_c.index {
                let s = min_c.index;
                let e = max_c.index;
                let txt: String = cell_text
                    .chars()
                    .skip(s)
                    .take(e.saturating_sub(s))
                    .collect();
                let clean_txt = uwu_core_util::strip_ansi(&txt).replace(" ↵ ", " ");
                let trimmed = clean_txt.trim().to_string();
                if !trimmed.is_empty() {
                    selected_text = Some(trimmed.clone());
                    ui.ctx().data_mut(|d| d.insert_temp(cell_id, trimmed));
                }
            } else if is_left_clicked {
                ui.ctx().data_mut(|d| d.remove_temp::<String>(cell_id));
            }
        }
    }

    if selected_text.is_none() {
        selected_text = ui.ctx().data(|d| d.get_temp::<String>(cell_id));
    }

    selected_text
}

pub fn render_cell(
    ui: &mut egui::Ui,
    event: &LogEvent,
    col_name: &str,
    row_color: egui::Color32,
    is_selected: bool,
    is_row_highlighted: bool,
    ctx: &mut TableRenderContext<'_>,
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

    let selected_text = extract_selected_text(
        ui,
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
