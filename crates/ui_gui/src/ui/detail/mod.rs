pub mod actions;
pub mod card;
pub mod fields;
pub mod text_box;

use crate::app::UwuGuiApp;
use crate::ui::theme;
use actions::{dispatch_actions, DetailContext, FilterAction, HighlightAction};
use card::render_card;
use eframe::egui::{self, Id, Rounding, Stroke};
use fields::{render_kv_field, render_meta_field};
use text_box::render_text_box;
use uwu_core_schema::LogLevel;

pub fn render_detail(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let mut filter_action: Option<FilterAction> = None;
    let mut highlight_action: Option<HighlightAction> = None;
    let mut unfiltered_action: Option<crate::ui::actions::UnfilteredAction> = None;
    let mut close_requested = false;

    if let Some(event) = &app.selected_log {
        let is_highlighted = app.is_row_highlighted(&event.id);
        let event_id = event.id;
        let has_any_highlights = app.has_any_highlights();

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Log Inspector")
                    .strong()
                    .size(14.0)
                    .color(theme::TEXT_KEY),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let close_btn = egui::Button::new(
                    egui::RichText::new("Close")
                        .size(11.5)
                        .color(theme::TEXT_PRIMARY),
                )
                .fill(theme::BG_SURFACE0)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                .rounding(Rounding::same(4.0));

                if ui.add(close_btn).clicked() {
                    close_requested = true;
                }

                ui.add_space(4.0);

                let unfil_btn = egui::Button::new(
                    egui::RichText::new("🔍 Unfiltered")
                        .size(11.5)
                        .color(theme::TEXT_PRIMARY),
                )
                .fill(theme::BG_SURFACE0)
                .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                .rounding(Rounding::same(4.0));

                if ui
                    .add(unfil_btn)
                    .on_hover_text("View surrounding logs in full unfiltered stream")
                    .clicked()
                {
                    unfiltered_action = Some(crate::ui::actions::UnfilteredAction::Open(event_id));
                }

                ui.add_space(4.0);

                if has_any_highlights {
                    let unhl_all_btn = egui::Button::new(
                        egui::RichText::new("Unhighlight all")
                            .size(11.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(theme::BG_SURFACE0)
                    .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                    .rounding(Rounding::same(4.0));

                    if ui.add(unhl_all_btn).clicked() {
                        highlight_action = Some(HighlightAction::ClearAll);
                    }

                    ui.add_space(4.0);
                }

                let hl_text = if is_highlighted {
                    "Unhighlight row"
                } else {
                    "Highlight row"
                };

                let hl_btn = egui::Button::new(
                    egui::RichText::new(hl_text)
                        .size(11.5)
                        .color(theme::TEXT_PRIMARY),
                )
                .fill(if is_highlighted {
                    theme::BG_ROW_HIGHLIGHT
                } else {
                    theme::BG_SURFACE0
                })
                .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                .rounding(Rounding::same(4.0));

                if ui.add(hl_btn).clicked() {
                    highlight_action = Some(HighlightAction::ToggleRow(event_id));
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let (log_color, is_colored) = match event.level {
            LogLevel::Error | LogLevel::Fatal => (theme::COLOR_ERROR, true),
            LogLevel::Warn => (theme::COLOR_WARN, true),
            LogLevel::Info => (theme::COLOR_INFO, true),
            _ => (theme::TEXT_MUTED, false),
        };

        egui::ScrollArea::vertical()
            .id_salt("detail_inspector_scroll_area")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Metadata Card
                render_card(ui, "Metadata", |ui| {
                    let mut ctx = DetailContext {
                        highlighted_terms: &app.highlighted_terms,
                        filter_action: &mut filter_action,
                        highlight_action: &mut highlight_action,
                    };

                    render_meta_field(
                        ui,
                        "ID",
                        &event.id.to_string(),
                        theme::TEXT_PRIMARY,
                        true,
                        &mut ctx,
                    );
                    ui.add_space(5.0);
                    render_meta_field(
                        ui,
                        "Timestamp",
                        &event.timestamp,
                        theme::TEXT_PRIMARY,
                        true,
                        &mut ctx,
                    );
                    ui.add_space(5.0);
                    render_meta_field(
                        ui,
                        "Level",
                        &event.level.to_string(),
                        log_color,
                        false,
                        &mut ctx,
                    );
                });

                ui.add_space(8.0);

                // Message Card
                render_card(ui, "Message", |ui| {
                    let msg_color = if is_colored {
                        log_color
                    } else {
                        theme::TEXT_PRIMARY
                    };

                    let mut ctx = DetailContext {
                        highlighted_terms: &app.highlighted_terms,
                        filter_action: &mut filter_action,
                        highlight_action: &mut highlight_action,
                    };

                    render_text_box(
                        ui,
                        Id::new("detail_inspector_msg_box"),
                        &event.message,
                        msg_color,
                        3,
                        Some("message"),
                        &mut ctx,
                    );
                });

                ui.add_space(8.0);

                // Parsed JSON / K-V Fields Card (loại trừ các trường mặc định đã có card riêng)
                let custom_fields: Vec<(&String, &serde_json::Value)> = event
                    .fields
                    .iter()
                    .filter(|(k, _)| {
                        !matches!(k.as_str(), "timestamp" | "level" | "message" | "raw" | "id")
                            && uwu_core_schema::StandardField::from_alias(k).is_none()
                    })
                    .collect();

                if !custom_fields.is_empty() {
                    render_card(ui, "Parsed Fields", |ui| {
                        let mut ctx = DetailContext {
                            highlighted_terms: &app.highlighted_terms,
                            filter_action: &mut filter_action,
                            highlight_action: &mut highlight_action,
                        };

                        for (i, (key, val)) in custom_fields.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(6.0);
                            }
                            let val_str = match val {
                                serde_json::Value::String(s) => s.clone(),
                                _ => val.to_string(),
                            };
                            render_kv_field(ui, key, &val_str, &mut ctx);
                        }
                    });
                    ui.add_space(8.0);
                }

                // Raw Payload Card with Copy button
                render_card(ui, "Raw Payload", |ui| {
                    ui.horizontal(|ui| {
                        let copy_id = Id::new("copy_raw_flash");
                        let now = ui.input(|i| i.time);
                        let last_copy = ui.data(|d| d.get_temp::<f64>(copy_id)).unwrap_or(0.0);
                        let is_flashing = (now - last_copy) < 0.12;

                        if is_flashing {
                            ui.ctx().request_repaint();
                        }

                        let copy_btn =
                            egui::Button::new(egui::RichText::new("Copy Raw").size(11.0).color(
                                if is_flashing {
                                    theme::TEXT_KEY
                                } else {
                                    theme::TEXT_PRIMARY
                                },
                            ))
                            .fill(if is_flashing {
                                theme::BG_SURFACE1
                            } else {
                                theme::BG_SURFACE0
                            })
                            .stroke(Stroke::new(
                                1.0,
                                if is_flashing {
                                    theme::TEXT_KEY
                                } else {
                                    theme::BG_SURFACE1
                                },
                            ))
                            .rounding(Rounding::same(4.0));

                        let copy_resp = ui
                            .add(copy_btn)
                            .on_hover_text("Copy raw payload to clipboard");
                        if copy_resp.clicked() {
                            ui.data_mut(|d| d.insert_temp(copy_id, now));
                            ui.ctx().copy_text(event.raw.clone());
                            ui.ctx().request_repaint();
                        }
                    });
                    ui.add_space(4.0);

                    let mut ctx = DetailContext {
                        highlighted_terms: &app.highlighted_terms,
                        filter_action: &mut filter_action,
                        highlight_action: &mut highlight_action,
                    };

                    render_text_box(
                        ui,
                        Id::new("detail_inspector_raw_box"),
                        &event.raw,
                        theme::TEXT_PRIMARY,
                        4,
                        None,
                        &mut ctx,
                    );
                });
            });
    }

    dispatch_actions(app, filter_action, highlight_action, unfiltered_action);

    if close_requested {
        app.selected_log = None;
    }
}
