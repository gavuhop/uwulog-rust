use crate::app::{AppAction, UwuGuiApp};
use crate::ui::actions::{truncate_label, ActionContext};
use crate::ui::theme;
use eframe::egui;

/// Renders a single metadata entry (e.g. ID, Timestamp, Level) with keyword highlighting and right-click quick filters.
pub fn render_meta_field(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    val_color: egui::Color32,
    mono: bool,
    ctx: &mut ActionContext<'_>,
) {
    ui.push_id(key, |ui| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("{}: ", key),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(11.5),
                color: theme::TEXT_MUTED,
                ..Default::default()
            },
        );

        let hl_terms = ctx.highlighted_terms;
        let val_job = theme::create_highlighted_layout_job(
            val,
            val_color,
            if mono {
                egui::FontId::monospace(11.5)
            } else {
                egui::FontId::proportional(12.0)
            },
            hl_terms,
        );

        for section in val_job.sections {
            let slice = &val_job.text[section.byte_range];
            job.append(slice, section.leading_space, section.format);
        }

        let resp = ui.add(
            egui::Label::new(job)
                .wrap_mode(egui::TextWrapMode::Wrap)
                .selectable(true),
        );

        resp.context_menu(|ui| {
            render_field_context_menu(ui, key, val, ctx);
        });
    });
}

/// Renders a parsed JSON key-value field with syntax coloring, keyword highlighting, and right-click quick filters.
pub fn render_kv_field(ui: &mut egui::Ui, key: &str, val: &str, ctx: &mut ActionContext<'_>) {
    ui.push_id(key, |ui| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("{}: ", key),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(11.5),
                color: theme::TEXT_KEY,
                ..Default::default()
            },
        );

        let val_font = egui::FontId::proportional(11.5);
        let val_job = theme::create_highlighted_layout_job(
            val,
            theme::TEXT_PRIMARY,
            val_font,
            ctx.highlighted_terms,
        );
        for section in &val_job.sections {
            job.append(
                &val[section.byte_range.clone()],
                0.0,
                section.format.clone(),
            );
        }

        let resp = ui.add(
            egui::Label::new(job)
                .wrap_mode(egui::TextWrapMode::Wrap)
                .selectable(true),
        );

        resp.context_menu(|ui| {
            render_field_context_menu(ui, key, val, ctx);
        });
    });
}

/// Renders the right-click quick action context menu for a log field
pub fn render_field_context_menu(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    ctx: &mut ActionContext<'_>,
) {
    ui.set_min_width(160.0);

    let display_val = truncate_label(val, 25);

    if ui.button(format!("Filter \"{}\"", display_val)).clicked() {
        let term = UwuGuiApp::format_field_term(key, val);
        *ctx.action = Some(AppAction::ApplyFilterTerm(term));
        ui.close_menu();
    }

    if ui.button(format!("Exclude \"{}\"", display_val)).clicked() {
        let term = UwuGuiApp::format_field_term(key, val);
        *ctx.action = Some(AppAction::ExcludeFilterTerm(term));
        ui.close_menu();
    }

    let is_val_hl = ctx.highlighted_terms.contains(&val.to_lowercase());
    let hl_btn_text = if is_val_hl {
        format!("Unhighlight \"{}\"", display_val)
    } else {
        format!("Highlight \"{}\"", display_val)
    };
    if ui.button(hl_btn_text).clicked() {
        *ctx.action = Some(AppAction::ToggleTermHighlight(val.to_string()));
        ui.close_menu();
    }

    ui.separator();

    if ui.button("Copy value").clicked() {
        ui.ctx().output_mut(|o| o.copied_text = val.to_string());
        ui.close_menu();
    }
}
