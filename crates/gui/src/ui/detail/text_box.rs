use crate::app::UwuGuiApp;
use crate::ui::detail::actions::{DetailContext, FilterAction, HighlightAction};
use crate::ui::theme;
use eframe::egui::{self, Id};

pub fn render_text_box(
    ui: &mut egui::Ui,
    box_id: Id,
    text: &str,
    text_color: egui::Color32,
    desired_rows: usize,
    field_name: Option<&str>,
    ctx: &mut DetailContext<'_>,
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

    let resp = ui.add(
        egui::TextEdit::multiline(&mut val)
            .id(box_id)
            .font(egui::TextStyle::Monospace)
            .text_color(text_color)
            .desired_width(f32::INFINITY)
            .frame(false)
            .desired_rows(desired_rows)
            .layouter(&mut layouter),
    );

    let mut selected_text = None;
    if let Some(state) = egui::text_edit::TextEditState::load(ui.ctx(), box_id) {
        if let Some(range) = state.cursor.char_range() {
            let [min_c, max_c] = range.sorted();
            if min_c.index < max_c.index {
                let s = min_c.index;
                let e = max_c.index;
                let txt: String = val.chars().skip(s).take(e.saturating_sub(s)).collect();
                let trimmed = txt.trim().to_string();
                if !trimmed.is_empty() {
                    selected_text = Some(trimmed.clone());
                    ui.ctx().data_mut(|d| d.insert_temp(box_id, trimmed));
                }
            } else if resp.clicked()
                && !ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary))
            {
                ui.ctx().data_mut(|d| d.remove_temp::<String>(box_id));
            }
        }
    }

    if selected_text.is_none() {
        selected_text = ui.ctx().data(|d| d.get_temp::<String>(box_id));
    }

    resp.context_menu(|ui| {
        ui.set_min_width(160.0);

        if let Some(ref sel) = selected_text {
            let display_sel = if sel.chars().count() > 25 {
                format!("{}...", sel.chars().take(25).collect::<String>())
            } else {
                sel.clone()
            };

            if ui.button(format!("Filter \"{}\"", display_sel)).clicked() {
                let term = if let Some(f_name) = field_name {
                    UwuGuiApp::format_field_term(f_name, sel)
                } else {
                    UwuGuiApp::format_selection_term(sel)
                };
                *ctx.filter_action = Some(FilterAction::Apply(term));
                ui.close_menu();
            }

            if ui.button(format!("Exclude \"{}\"", display_sel)).clicked() {
                let term = if let Some(f_name) = field_name {
                    UwuGuiApp::format_field_term(f_name, sel)
                } else {
                    UwuGuiApp::format_selection_term(sel)
                };
                *ctx.filter_action = Some(FilterAction::Exclude(term));
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
                *ctx.highlight_action = Some(HighlightAction::ToggleTerm(sel.clone()));
                ui.close_menu();
            }

            ui.separator();
        }

        if let Some(f_name) = field_name {
            let display_msg = if val.chars().count() > 25 {
                format!("{}...", val.chars().take(25).collect::<String>())
            } else {
                val.clone()
            };

            if ui.button(format!("Filter \"{}\"", display_msg)).clicked() {
                let term = UwuGuiApp::format_field_term(f_name, &val);
                *ctx.filter_action = Some(FilterAction::Apply(term));
                ui.close_menu();
            }

            if ui.button(format!("Exclude \"{}\"", display_msg)).clicked() {
                let term = UwuGuiApp::format_field_term(f_name, &val);
                *ctx.filter_action = Some(FilterAction::Exclude(term));
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
                *ctx.highlight_action = Some(HighlightAction::ToggleTerm(val.clone()));
                ui.close_menu();
            }

            ui.separator();
        }

        if ui.button("Copy value").clicked() {
            ui.ctx().output_mut(|o| o.copied_text = val.clone());
            ui.close_menu();
        }
    });
}
