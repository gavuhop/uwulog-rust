use crate::actions::{copy_and_close, render_filter_actions_menu, ActionContext};
use crate::theme::{self, ActiveTheme};
use eframe::egui;

fn render_field_item(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    key_color: egui::Color32,
    val_color: egui::Color32,
    val_font: egui::FontId,
    ctx: &mut ActionContext<'_>,
) {
    ui.push_id(key, |ui| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("{}: ", key),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(12.0),
                color: key_color,
                ..Default::default()
            },
        );

        let val_job =
            theme::create_highlighted_layout_job(val, val_color, val_font, ctx.highlighted_terms);

        for section in val_job.sections {
            let slice = &val_job.text[section.byte_range.start.0..section.byte_range.end.0];
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

/// Renders a single metadata entry (e.g. ID, Timestamp, Level) with keyword highlighting and right-click quick filters.
pub fn render_meta_field(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    val_color: egui::Color32,
    mono: bool,
    ctx: &mut ActionContext<'_>,
) {
    let theme = ui.app_theme();
    let font_id = if mono {
        egui::FontId::monospace(12.0)
    } else {
        egui::FontId::proportional(12.0)
    };
    render_field_item(ui, key, val, theme.text.muted, val_color, font_id, ctx);
}

/// Renders a parsed JSON key-value field with syntax coloring, keyword highlighting, and right-click quick filters.
pub fn render_kv_field(ui: &mut egui::Ui, key: &str, val: &str, ctx: &mut ActionContext<'_>) {
    let theme = ui.app_theme();
    render_field_item(
        ui,
        key,
        val,
        theme.text.accent,
        theme.text.primary,
        egui::FontId::monospace(12.0),
        ctx,
    );
}

/// Renders the right-click quick action context menu for a log field
pub fn render_field_context_menu(
    ui: &mut egui::Ui,
    key: &str,
    val: &str,
    ctx: &mut ActionContext<'_>,
) {
    ui.set_min_width(160.0);
    render_filter_actions_menu(ui, Some(key), val, ctx);
    ui.separator();
    copy_and_close(ui, val);
}
