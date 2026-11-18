use super::context_menu::{render_cell_context_menu, CellMenuContext};
use crate::actions::ActionContext;
use crate::theme::ActiveTheme;
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

pub struct CellRenderState {
    pub is_selected: bool,
    pub is_row_highlighted: bool,
    pub offset_x: f32,
    pub table_clip_rect: egui::Rect,
}

pub fn render_cell(
    ui: &mut egui::Ui,
    event: &LogEvent,
    col_name: &str,
    row_color: egui::Color32,
    state: CellRenderState,
    ctx: &mut ActionContext<'_>,
) -> bool {
    let theme = ui.app_theme();
    let cell_rect = ui.max_rect();
    let is_animating = state.offset_x.abs() > 0.01;
    let visual_rect = if is_animating {
        let moved = cell_rect.translate(egui::vec2(state.offset_x, 0.0));
        let mut anim_clip = moved.intersect(state.table_clip_rect);
        anim_clip.min.y = anim_clip.min.y.max(ui.clip_rect().min.y);
        anim_clip.max.y = anim_clip.max.y.min(ui.clip_rect().max.y);
        ui.set_clip_rect(anim_clip);
        moved
    } else {
        cell_rect
    };

    if state.is_row_highlighted {
        ui.painter().rect_filled(
            visual_rect,
            egui::CornerRadius::ZERO,
            theme.log.row_highlight,
        );
    } else if state.is_selected {
        ui.painter().rect_filled(
            visual_rect,
            egui::CornerRadius::ZERO,
            theme.log.row_selected,
        );
    } else if is_animating {
        ui.painter()
            .rect_filled(visual_rect, egui::CornerRadius::ZERO, theme.surfaces.base);
    }

    let (cell_text, raw_cell_val) = extract_cell_content(event, col_name);
    let cell_id = ui.make_persistent_id((event.id, col_name));
    let mut text_val = cell_text.as_ref();

    let highlighted_terms_ref = ctx.highlighted_terms;
    let mut layouter =
        |ui: &egui::Ui, _text: &dyn egui::text_edit::TextBuffer, _wrap_width: f32| {
            let mut job = crate::theme::create_highlighted_layout_job(
                &cell_text,
                row_color,
                egui::FontId::monospace(crate::theme::TABLE_FONT_SIZE),
                highlighted_terms_ref,
            );
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|f| f.layout_job(job))
        };

    let edit = egui::TextEdit::singleline(&mut text_val)
        .id(cell_id)
        .font(egui::FontId::monospace(crate::theme::TABLE_FONT_SIZE))
        .text_color(row_color)
        .frame(egui::Frame::NONE)
        .clip_text(true)
        .desired_width(f32::INFINITY)
        .layouter(&mut layouter);

    let resp = if is_animating {
        ui.new_child(
            egui::UiBuilder::new()
                .max_rect(visual_rect)
                .layout(*ui.layout()),
        )
        .add(edit)
    } else {
        ui.add(edit)
    };
    let is_secondary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
    let is_primary_clicked = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
    // Dùng ui.rect_contains_pointer thay vì raw cell_rect.contains để tôn trọng lớp popup/modal phía trên
    let pointer_in_cell = resp.hovered() || ui.rect_contains_pointer(visual_rect);
    let clicked = (resp.clicked() || (is_primary_clicked && pointer_in_cell)) && !is_secondary_down;

    let selected_text =
        crate::actions::extract_selected_text(ui.ctx(), cell_id, &cell_text, clicked);

    resp.context_menu(|ui| {
        let menu_ctx = CellMenuContext {
            col_name,
            raw_cell_val: &raw_cell_val,
            selected_text: selected_text.as_deref(),
            is_row_highlighted: state.is_row_highlighted,
            event_id: event.id,
        };
        render_cell_context_menu(ui, menu_ctx, ctx);
    });

    clicked
}
