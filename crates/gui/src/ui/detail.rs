use crate::app::UwuGuiApp;
use crate::ui::theme;
use eframe::egui::{self, Id, Rounding, Stroke};
use uwu_core::LogLevel;

enum FilterAction {
    Apply(String),
    Exclude(String),
}

enum HighlightAction {
    ToggleRow(uuid::Uuid),
    ToggleTerm(String),
    ClearAll,
}

struct DetailContext<'a> {
    highlighted_terms: &'a std::collections::HashSet<String>,
    filter_action: &'a mut Option<FilterAction>,
    highlight_action: &'a mut Option<HighlightAction>,
}

pub fn render_detail(ui: &mut egui::Ui, app: &mut UwuGuiApp) {
    let mut filter_action: Option<FilterAction> = None;
    let mut highlight_action: Option<HighlightAction> = None;
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
                    let mut msg = event.message.clone();
                    let msg_color = if is_colored {
                        log_color
                    } else {
                        theme::TEXT_PRIMARY
                    };

                    let msg_id = Id::new("detail_inspector_msg_box");
                    let highlighted_terms_ref = &app.highlighted_terms;
                    let mut msg_layouter = |ui: &egui::Ui, _text: &str, wrap_width: f32| {
                        let mut job = theme::create_highlighted_layout_job(
                            &event.message,
                            msg_color,
                            egui::FontId::monospace(11.5),
                            highlighted_terms_ref,
                        );
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut msg)
                            .id(msg_id)
                            .font(egui::TextStyle::Monospace)
                            .text_color(msg_color)
                            .desired_width(f32::INFINITY)
                            .frame(false)
                            .desired_rows(3)
                            .layouter(&mut msg_layouter),
                    );

                    let mut selected_text = None;
                    if let Some(state) = egui::text_edit::TextEditState::load(ui.ctx(), msg_id) {
                        if let Some(range) = state.cursor.char_range() {
                            let [min_c, max_c] = range.sorted();
                            if min_c.index < max_c.index {
                                let s = min_c.index;
                                let e = max_c.index;
                                let txt: String =
                                    msg.chars().skip(s).take(e.saturating_sub(s)).collect();
                                let trimmed = txt.trim().to_string();
                                if !trimmed.is_empty() {
                                    selected_text = Some(trimmed.clone());
                                    ui.ctx().data_mut(|d| d.insert_temp(msg_id, trimmed));
                                }
                            } else if resp.clicked()
                                && !ui.input(|i| {
                                    i.pointer.button_down(egui::PointerButton::Secondary)
                                })
                            {
                                ui.ctx().data_mut(|d| d.remove_temp::<String>(msg_id));
                            }
                        }
                    }

                    if selected_text.is_none() {
                        selected_text = ui.ctx().data(|d| d.get_temp::<String>(msg_id));
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
                                let term = UwuGuiApp::format_selection_term(sel);
                                filter_action = Some(FilterAction::Apply(term));
                                ui.close_menu();
                            }

                            if ui.button(format!("Exclude \"{}\"", display_sel)).clicked() {
                                let term = UwuGuiApp::format_selection_term(sel);
                                filter_action = Some(FilterAction::Exclude(term));
                                ui.close_menu();
                            }

                            let is_term_hl = app.is_term_highlighted(sel);
                            let hl_term_text = if is_term_hl {
                                format!("Unhighlight \"{}\"", display_sel)
                            } else {
                                format!("Highlight \"{}\"", display_sel)
                            };
                            if ui.button(hl_term_text).clicked() {
                                highlight_action = Some(HighlightAction::ToggleTerm(sel.clone()));
                                ui.close_menu();
                            }

                            ui.separator();
                        }

                        let display_msg = if msg.chars().count() > 25 {
                            format!("{}...", msg.chars().take(25).collect::<String>())
                        } else {
                            msg.clone()
                        };

                        if ui.button(format!("Filter \"{}\"", display_msg)).clicked() {
                            let term = UwuGuiApp::format_field_term("message", &msg);
                            filter_action = Some(FilterAction::Apply(term));
                            ui.close_menu();
                        }

                        if ui.button(format!("Exclude \"{}\"", display_msg)).clicked() {
                            let term = UwuGuiApp::format_field_term("message", &msg);
                            filter_action = Some(FilterAction::Exclude(term));
                            ui.close_menu();
                        }

                        let is_msg_hl = app.is_term_highlighted(&msg);
                        let hl_msg_text = if is_msg_hl {
                            format!("Unhighlight \"{}\"", display_msg)
                        } else {
                            format!("Highlight \"{}\"", display_msg)
                        };
                        if ui.button(hl_msg_text).clicked() {
                            highlight_action = Some(HighlightAction::ToggleTerm(msg.clone()));
                            ui.close_menu();
                        }

                        ui.separator();

                        if ui.button("Copy value").clicked() {
                            ui.ctx().output_mut(|o| o.copied_text = msg.clone());
                            ui.close_menu();
                        }
                    });
                });

                ui.add_space(8.0);

                // Parsed JSON / K-V Fields Card
                if !event.fields.is_empty() {
                    render_card(ui, "Parsed Fields", |ui| {
                        let mut ctx = DetailContext {
                            highlighted_terms: &app.highlighted_terms,
                            filter_action: &mut filter_action,
                            highlight_action: &mut highlight_action,
                        };

                        for (i, (key, val)) in event.fields.iter().enumerate() {
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
                        let copy_btn = egui::Button::new(
                            egui::RichText::new("Copy Raw")
                                .size(11.0)
                                .color(theme::TEXT_PRIMARY),
                        )
                        .fill(theme::BG_SURFACE0)
                        .stroke(Stroke::new(1.0, theme::BG_SURFACE1))
                        .rounding(Rounding::same(4.0));

                        if ui
                            .add(copy_btn)
                            .on_hover_text("Copy raw payload to clipboard")
                            .clicked()
                        {
                            ui.ctx().output_mut(|o| o.copied_text = event.raw.clone());
                        }
                    });
                    ui.add_space(4.0);

                    let mut raw = event.raw.clone();
                    let raw_id = Id::new("detail_inspector_raw_box");
                    let highlighted_terms_ref = &app.highlighted_terms;
                    let mut raw_layouter = |ui: &egui::Ui, _text: &str, wrap_width: f32| {
                        let mut job = theme::create_highlighted_layout_job(
                            &event.raw,
                            theme::TEXT_PRIMARY,
                            egui::FontId::monospace(11.5),
                            highlighted_terms_ref,
                        );
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut raw)
                            .id(raw_id)
                            .font(egui::TextStyle::Monospace)
                            .text_color(theme::TEXT_PRIMARY)
                            .desired_width(f32::INFINITY)
                            .frame(false)
                            .desired_rows(4)
                            .layouter(&mut raw_layouter),
                    );

                    let mut selected_text = None;
                    if let Some(state) = egui::text_edit::TextEditState::load(ui.ctx(), raw_id) {
                        if let Some(range) = state.cursor.char_range() {
                            let [min_c, max_c] = range.sorted();
                            if min_c.index < max_c.index {
                                let s = min_c.index;
                                let e = max_c.index;
                                let txt: String =
                                    raw.chars().skip(s).take(e.saturating_sub(s)).collect();
                                let trimmed = txt.trim().to_string();
                                if !trimmed.is_empty() {
                                    selected_text = Some(trimmed.clone());
                                    ui.ctx().data_mut(|d| d.insert_temp(raw_id, trimmed));
                                }
                            } else if resp.clicked()
                                && !ui.input(|i| {
                                    i.pointer.button_down(egui::PointerButton::Secondary)
                                })
                            {
                                ui.ctx().data_mut(|d| d.remove_temp::<String>(raw_id));
                            }
                        }
                    }

                    if selected_text.is_none() {
                        selected_text = ui.ctx().data(|d| d.get_temp::<String>(raw_id));
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
                                let term = UwuGuiApp::format_selection_term(sel);
                                filter_action = Some(FilterAction::Apply(term));
                                ui.close_menu();
                            }

                            if ui.button(format!("Exclude \"{}\"", display_sel)).clicked() {
                                let term = UwuGuiApp::format_selection_term(sel);
                                filter_action = Some(FilterAction::Exclude(term));
                                ui.close_menu();
                            }

                            let is_term_hl = app.is_term_highlighted(sel);
                            let hl_term_text = if is_term_hl {
                                format!("Unhighlight \"{}\"", display_sel)
                            } else {
                                format!("Highlight \"{}\"", display_sel)
                            };
                            if ui.button(hl_term_text).clicked() {
                                highlight_action = Some(HighlightAction::ToggleTerm(sel.clone()));
                                ui.close_menu();
                            }

                            ui.separator();
                        }

                        if ui.button("Copy value").clicked() {
                            ui.ctx().output_mut(|o| o.copied_text = raw.clone());
                            ui.close_menu();
                        }
                    });
                });
            });
    }

    if let Some(action) = filter_action {
        match action {
            FilterAction::Apply(term) => app.apply_filter_term(&term),
            FilterAction::Exclude(term) => app.exclude_filter_term(&term),
        }
    }

    if let Some(action) = highlight_action {
        match action {
            HighlightAction::ToggleRow(id) => app.toggle_row_highlight(id),
            HighlightAction::ToggleTerm(term) => app.toggle_term_highlight(&term),
            HighlightAction::ClearAll => app.clear_all_highlights(),
        }
    }

    if close_requested {
        app.selected_log = None;
    }
}

fn render_meta_field(
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

fn render_kv_field(ui: &mut egui::Ui, key: &str, val: &str, ctx: &mut DetailContext<'_>) {
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

fn render_card<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    let frame = egui::Frame::default()
        .fill(theme::BG_BASE)
        .rounding(Rounding::same(4.0))
        .inner_margin(egui::Margin::same(10.0))
        .stroke(Stroke::new(1.0, theme::BG_SURFACE0));

    frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                egui::RichText::new(title)
                    .size(12.0)
                    .strong()
                    .color(theme::TEXT_KEY),
            );
            ui.add_space(4.0);
            add_contents(ui);
        })
        .response
}
