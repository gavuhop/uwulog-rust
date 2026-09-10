use crate::app::{AppAction, UwuGuiApp};
use crate::ui::actions::{truncate_label, ActionContext};
use crate::ui::theme;
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

    let selected_text = crate::ui::actions::extract_selected_text(
        ui.ctx(),
        box_id,
        &val,
        resp.clicked() && !is_secondary_down,
    );

    resp.context_menu(|ui| {
        ui.set_min_width(180.0);

        // 1. Nhóm từ bôi đen trong ô (nếu có lựa chọn bôi đen)
        if let Some(ref sel) = selected_text {
            let display_sel = truncate_label(sel, 25);

            if ui.button(format!("Filter \"{}\"", display_sel)).clicked() {
                let term = if let Some(f_name) = field_name {
                    UwuGuiApp::format_field_term(f_name, sel)
                } else {
                    UwuGuiApp::format_selection_term(sel)
                };
                *ctx.action = Some(AppAction::ApplyFilterTerm(term));
                ui.close_menu();
            }

            if ui.button(format!("Exclude \"{}\"", display_sel)).clicked() {
                let term = if let Some(f_name) = field_name {
                    UwuGuiApp::format_field_term(f_name, sel)
                } else {
                    UwuGuiApp::format_selection_term(sel)
                };
                *ctx.action = Some(AppAction::ExcludeFilterTerm(term));
                ui.close_menu();
            }

            let sel_clean = sel.trim().to_lowercase();
            let is_term_hl = !sel_clean.is_empty() && ctx.highlighted_terms.contains(&sel_clean);
            let hl_term_text = if is_term_hl {
                format!("Unhighlight \"{}\"", display_sel)
            } else {
                format!("Highlight \"{}\"", display_sel)
            };
            if ui.button(hl_term_text).clicked() {
                *ctx.action = Some(AppAction::ToggleTermHighlight(sel.clone()));
                ui.close_menu();
            }

            ui.separator();
        }

        // 2. Nhóm toàn bộ nội dung trường (nếu có định nghĩa key)
        if let Some(f_name) = field_name {
            let display_msg = truncate_label(&val, 25);

            if ui.button(format!("Filter \"{}\"", display_msg)).clicked() {
                let term = UwuGuiApp::format_field_term(f_name, &val);
                *ctx.action = Some(AppAction::ApplyFilterTerm(term));
                ui.close_menu();
            }

            if ui.button(format!("Exclude \"{}\"", display_msg)).clicked() {
                let term = UwuGuiApp::format_field_term(f_name, &val);
                *ctx.action = Some(AppAction::ExcludeFilterTerm(term));
                ui.close_menu();
            }

            let val_clean = val.trim().to_lowercase();
            let is_msg_hl = !val_clean.is_empty() && ctx.highlighted_terms.contains(&val_clean);
            let hl_msg_text = if is_msg_hl {
                format!("Unhighlight \"{}\"", display_msg)
            } else {
                format!("Highlight \"{}\"", display_msg)
            };
            if ui.button(hl_msg_text).clicked() {
                *ctx.action = Some(AppAction::ToggleTermHighlight(val.clone()));
                ui.close_menu();
            }

            ui.separator();
        }

        // 3. Copy toàn bộ nội dung hộp văn bản
        if ui.button("Copy value").clicked() {
            ui.ctx().output_mut(|o| o.copied_text = val.clone());
            ui.close_menu();
        }
    });
}
