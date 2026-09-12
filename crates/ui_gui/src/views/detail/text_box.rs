use crate::actions::{copy_and_close, render_filter_actions_menu, ActionContext};
use crate::theme;
use eframe::egui::{self, Id};

/// Renders a multiline selectable text area (used for Message and Raw Payload cards) with selection-based quick filters.
pub fn render_text_box(
    ui: &mut egui::Ui,
    box_id: Id,
    text: &str,
    text_color: egui::Color32,
    desired_rows: usize,
    field_name: Option<&str>,
    ctx: &mut ActionContext<'_>,
) {
    let mut val = text.to_string();
    let highlighted_terms_ref = ctx.highlighted_terms;
    let mut layouter = |ui: &egui::Ui, _text: &str, wrap_width: f32| {
        let mut job = theme::create_highlighted_layout_job(
            text,
            text_color,
            egui::FontId::monospace(11.5),
            highlighted_terms_ref,
        );
        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    };

    let edit = egui::TextEdit::multiline(&mut val)
        .id(box_id)
        .font(egui::FontId::monospace(11.5))
        .text_color(text_color)
        .frame(false)
        .desired_width(f32::INFINITY)
        .desired_rows(desired_rows)
        .layouter(&mut layouter);

    let resp = ui.add(edit);

    let is_secondary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));

    let selected_text = crate::actions::extract_selected_text(
        ui.ctx(),
        box_id,
        &val,
        resp.clicked() && !is_secondary_down,
    );

    resp.context_menu(|ui| {
        ui.set_min_width(180.0);

        // 1. Nhóm từ bôi đen trong ô (nếu có lựa chọn bôi đen)
        if let Some(ref sel) = selected_text {
            render_filter_actions_menu(ui, field_name, sel, ctx);
            ui.separator();
        }

        // 2. Nhóm toàn bộ nội dung trường (nếu có định nghĩa key)
        if let Some(f_name) = field_name {
            render_filter_actions_menu(ui, Some(f_name), &val, ctx);
            ui.separator();
        }

        // 3. Copy toàn bộ nội dung hộp văn bản
        copy_and_close(ui, &val);
    });
}
