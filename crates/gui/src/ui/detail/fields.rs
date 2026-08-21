use crate::app::UwuGuiApp;
use crate::ui::detail::actions::{DetailContext, FilterAction, HighlightAction};
use crate::ui::theme;
use eframe::egui;

pub fn render_meta_field(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    val_color: egui::Color32,
    mono: bool,
    ctx: &mut DetailContext<'_>,
) {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("{}: ", key),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(11.5),
            color: theme::TEXT_MUTED,
            ..Default::default()
        },
    );

    let val_font = if mono {
        egui::FontId::monospace(11.5)
    } else {
        egui::FontId::proportional(11.5)
    };

    let val_job =
        theme::create_highlighted_layout_job(val, val_color, val_font, ctx.highlighted_terms);
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

    let field_name = key.to_lowercase();
    let val_str = val.to_string();

    resp.context_menu(|ui| {
        ui.set_min_width(160.0);

        let display_val = if val_str.chars().count() > 25 {
            format!("{}...", val_str.chars().take(25).collect::<String>())
        } else {
            val_str.clone()
        };

        if ui.button(format!("Filter \"{}\"", display_val)).clicked() {
            let term = UwuGuiApp::format_field_term(&field_name, &val_str);
            *ctx.filter_action = Some(FilterAction::Apply(term));
            ui.close_menu();
        }

        if ui.button(format!("Exclude \"{}\"", display_val)).clicked() {
            let term = UwuGuiApp::format_field_term(&field_name, &val_str);
            *ctx.filter_action = Some(FilterAction::Exclude(term));
            ui.close_menu();
        }

        let is_val_hl = ctx.highlighted_terms.contains(&val_str.to_lowercase());
        let hl_btn_text = if is_val_hl {
            format!("Unhighlight \"{}\"", display_val)
        } else {
            format!("Highlight \"{}\"", display_val)
        };
        if ui.button(hl_btn_text).clicked() {
            *ctx.highlight_action = Some(HighlightAction::ToggleTerm(val_str.clone()));
            ui.close_menu();
        }

        ui.separator();

        if ui.button("Copy value").clicked() {
            ui.ctx().output_mut(|o| o.copied_text = val_str.clone());
            ui.close_menu();
        }
    });
}

pub fn render_kv_field(ui: &mut egui::Ui, key: &str, val: &str, ctx: &mut DetailContext<'_>) {
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

    let field_name = key.to_string();
    let val_str = val.to_string();

    resp.context_menu(|ui| {
        ui.set_min_width(160.0);

        let display_val = if val_str.chars().count() > 25 {
            format!("{}...", val_str.chars().take(25).collect::<String>())
        } else {
            val_str.clone()
        };

        if ui.button(format!("Filter \"{}\"", display_val)).clicked() {
            let term = UwuGuiApp::format_field_term(&field_name, &val_str);
            *ctx.filter_action = Some(FilterAction::Apply(term));
            ui.close_menu();
        }

        if ui.button(format!("Exclude \"{}\"", display_val)).clicked() {
            let term = UwuGuiApp::format_field_term(&field_name, &val_str);
            *ctx.filter_action = Some(FilterAction::Exclude(term));
            ui.close_menu();
        }

        let is_val_hl = ctx.highlighted_terms.contains(&val_str.to_lowercase());
        let hl_btn_text = if is_val_hl {
            format!("Unhighlight \"{}\"", display_val)
        } else {
            format!("Highlight \"{}\"", display_val)
        };
        if ui.button(hl_btn_text).clicked() {
            *ctx.highlight_action = Some(HighlightAction::ToggleTerm(val_str.clone()));
            ui.close_menu();
        }

        ui.separator();

        if ui.button("Copy value").clicked() {
            ui.ctx().output_mut(|o| o.copied_text = val_str.clone());
            ui.close_menu();
        }
    });
}
