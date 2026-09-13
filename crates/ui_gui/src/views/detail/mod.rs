pub mod fields;
pub mod text_box;

use crate::actions::{ActionContext, AppAction};
use crate::components::render_card;
use crate::session::GuiSession;
use crate::theme;
use eframe::egui::{self, Id, Rounding, Stroke};
use fields::{render_kv_field, render_meta_field};
use std::collections::HashMap;
use text_box::render_text_box;
use uwu_core_schema::StandardField;

pub fn render_detail(
    ui: &mut egui::Ui,
    session: &mut GuiSession,
    dispatch: &mut impl FnMut(AppAction),
) {
    let mut action_to_dispatch: Option<AppAction> = None;

    if let Some(event) = &session.inspector.selected_log {
        let is_highlighted = session.is_row_highlighted(&event.id);
        let event_id = event.id;
        let has_any_highlights = session.has_any_highlights();

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
                    action_to_dispatch = Some(AppAction::SelectLog(None));
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
                    action_to_dispatch = Some(AppAction::OpenUnfilteredStream(Some(event_id)));
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
                        action_to_dispatch = Some(AppAction::ClearAllHighlights);
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
                    action_to_dispatch = Some(AppAction::ToggleRowHighlight(event_id));
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        let log_color = theme::log_color_to_egui(event.color);

        let has_any_highlights = session.has_any_highlights();

        egui::ScrollArea::vertical()
            .id_salt("detail_inspector_scroll_area")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let ts_key = event.semantic_key(StandardField::Timestamp);
                let lvl_key = event.semantic_key(StandardField::Level);

                // Metadata Card
                render_card(ui, "Metadata", |ui| {
                    let mut ctx = ActionContext {
                        highlighted_terms: &session.inspector.highlighted_terms,
                        has_any_highlights,
                        action: &mut action_to_dispatch,
                    };

                    render_meta_field(
                        ui,
                        ts_key,
                        &event.timestamp,
                        theme::TEXT_PRIMARY,
                        true,
                        &mut ctx,
                    );
                    if let Some(lvl_val) = event.get_field_cow(lvl_key) {
                        ui.add_space(5.0);
                        render_meta_field(ui, lvl_key, &lvl_val, log_color, false, &mut ctx);
                    }
                });

                ui.add_space(8.0);

                let msg_key = event.semantic_key(StandardField::Message);

                // Message Card
                render_card(ui, "Message", |ui| {
                    let msg_color = log_color;

                    let mut ctx = ActionContext {
                        highlighted_terms: &session.inspector.highlighted_terms,
                        has_any_highlights,
                        action: &mut action_to_dispatch,
                    };

                    render_text_box(
                        ui,
                        Id::new("detail_inspector_msg_box"),
                        &event.message,
                        msg_color,
                        3,
                        Some(msg_key),
                        &mut ctx,
                    );
                });

                ui.add_space(8.0);

                // Parsed JSON / K-V Fields Card (loại trừ các trường mặc định đã có card riêng)
                let custom_fields: Vec<(&String, &serde_json::Value)> = event
                    .fields
                    .iter()
                    .filter(|(k, _)| {
                        !matches!(k.as_str(), "timestamp" | "level" | "message" | "id")
                            && uwu_core_schema::StandardField::from_alias(k).is_none()
                    })
                    .collect();

                let custom_fields = cluster_log_fields(custom_fields);

                if !custom_fields.is_empty() {
                    render_card(ui, "Parsed Fields", |ui| {
                        let mut ctx = ActionContext {
                            highlighted_terms: &session.inspector.highlighted_terms,
                            has_any_highlights,
                            action: &mut action_to_dispatch,
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

                // Raw Payload Card with Raw/Beauty mode toggle & Copy button
                render_card(ui, "Raw Payload", |ui| {
                    let view_mode_id = Id::new("detail_payload_view_mode_is_beauty");
                    let mut is_beauty = ui
                        .data(|d| d.get_temp::<bool>(view_mode_id))
                        .unwrap_or(false);
                    let is_json = !event.fields.is_empty();

                    if !is_json {
                        is_beauty = false;
                    }

                    ui.horizontal(|ui| {
                        // 1. Nút "Raw"
                        let raw_btn = egui::Button::new(
                            egui::RichText::new("Raw").size(11.0).color(if !is_beauty {
                                theme::TEXT_KEY
                            } else {
                                theme::TEXT_PRIMARY
                            }),
                        )
                        .fill(if !is_beauty {
                            theme::BG_SURFACE1
                        } else {
                            theme::BG_SURFACE0
                        })
                        .stroke(Stroke::new(
                            1.0,
                            if !is_beauty {
                                theme::TEXT_KEY
                            } else {
                                theme::BG_SURFACE1
                            },
                        ))
                        .rounding(Rounding::same(4.0));

                        let raw_resp = ui.add(raw_btn);
                        if raw_resp.clicked() {
                            is_beauty = false;
                            ui.data_mut(|d| d.insert_temp(view_mode_id, false));
                        }

                        ui.add_space(4.0);

                        // 2. Nút "Beauty" (kế bên nút Raw)
                        let beauty_btn =
                            egui::Button::new(egui::RichText::new("Beauty").size(11.0).color(
                                if is_beauty {
                                    theme::TEXT_KEY
                                } else if is_json {
                                    theme::TEXT_PRIMARY
                                } else {
                                    theme::TEXT_MUTED
                                },
                            ))
                            .fill(if is_beauty {
                                theme::BG_SURFACE1
                            } else {
                                theme::BG_SURFACE0
                            })
                            .stroke(Stroke::new(
                                1.0,
                                if is_beauty {
                                    theme::TEXT_KEY
                                } else {
                                    theme::BG_SURFACE1
                                },
                            ))
                            .rounding(Rounding::same(4.0));

                        let beauty_resp = ui.add_enabled(is_json, beauty_btn);
                        if beauty_resp.clicked() {
                            is_beauty = true;
                            ui.data_mut(|d| d.insert_temp(view_mode_id, true));
                        }

                        ui.add_space(8.0);

                        // 3. Nút Copy
                        let copy_id = Id::new("copy_raw_flash");
                        let now = ui.input(|i| i.time);
                        let last_copy = ui.data(|d| d.get_temp::<f64>(copy_id)).unwrap_or(0.0);
                        let is_flashing = (now - last_copy) < 0.12;

                        if is_flashing {
                            ui.ctx().request_repaint();
                        }

                        let copy_label = if is_beauty { "Copy Beauty" } else { "Copy Raw" };
                        let copy_btn =
                            egui::Button::new(egui::RichText::new(copy_label).size(11.0).color(
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

                        let copy_text_val = if is_beauty {
                            event.beauty_display()
                        } else {
                            event.raw_display()
                        };

                        if ui.add(copy_btn).clicked() {
                            ui.data_mut(|d| d.insert_temp(copy_id, now));
                            ui.ctx().copy_text(copy_text_val.to_string());
                            ui.ctx().request_repaint();
                        }
                    });
                    ui.add_space(4.0);

                    let mut ctx = ActionContext {
                        highlighted_terms: &session.inspector.highlighted_terms,
                        has_any_highlights,
                        action: &mut action_to_dispatch,
                    };

                    let display_str = if is_beauty {
                        event.beauty_display()
                    } else {
                        event.raw_display()
                    };

                    render_text_box(
                        ui,
                        Id::new("detail_inspector_raw_box"),
                        &display_str,
                        theme::TEXT_PRIMARY,
                        if is_beauty { 10 } else { 4 },
                        None,
                        &mut ctx,
                    );
                });
            });
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}

/// Gom nhóm các trường theo cụm (cluster prefix) dựa trên thứ tự xuất hiện tự nhiên đầu tiên:
/// - Các trường cùng prefix (vd: metadata.*) được gom lại liền kề nhau ngay tại vị trí xuất hiện đầu tiên của prefix đó.
/// - Không đẩy cả cụm xuống cuối danh sách (giữ nguyên vị trí tự nhiên).
/// - Không sắp xếp theo bảng chữ cái toàn bộ danh sách (các trường độc lập giữ nguyên thứ tự).
/// - Bên trong cụm lồng nhau, các nhánh con được sắp xếp để các nhánh cùng cấp nằm cạnh nhau.
pub fn cluster_log_fields<'a, V>(fields: Vec<(&'a String, V)>) -> Vec<(&'a String, V)> {
    let mut order_of_prefixes = Vec::new();
    let mut groups: HashMap<&str, Vec<(&'a String, V)>> = HashMap::new();

    for item in fields {
        let prefix = if let Some(dot_idx) = item.0.find('.') {
            &item.0[..dot_idx]
        } else {
            item.0.as_str()
        };

        if !groups.contains_key(prefix) {
            order_of_prefixes.push(prefix);
        }
        groups.entry(prefix).or_default().push(item);
    }

    let mut result = Vec::new();
    for prefix in order_of_prefixes {
        if let Some(mut items) = groups.remove(prefix) {
            if items.len() > 1 && items.iter().any(|(k, _)| k.contains('.')) {
                items.sort_unstable_by(|a, b| a.0.cmp(b.0));
            }
            result.extend(items);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_log_fields_preserves_position_and_groups_prefix() {
        let k1 = "status".to_string();
        let k2 = "metadata.instance_id".to_string();
        let k3 = "latency".to_string();
        let k4 = "metadata.version".to_string();
        let k5 = "method".to_string();
        let k6 = "metadata.region".to_string();

        let fields: Vec<(&String, i32)> =
            vec![(&k1, 1), (&k2, 2), (&k3, 3), (&k4, 4), (&k5, 5), (&k6, 6)];

        let clustered = cluster_log_fields(fields);
        let keys: Vec<&str> = clustered.into_iter().map(|(k, _)| k.as_str()).collect();

        assert_eq!(
            keys,
            vec![
                "status",
                "metadata.instance_id",
                "metadata.region",
                "metadata.version",
                "latency",
                "method",
            ]
        );
    }

    #[test]
    fn test_parsed_fields_preserves_raw_keys_and_values() {
        use uwu_core_schema::{LogColor, LogEvent};

        let mut fields = HashMap::new();
        fields.insert("timestamp".to_string(), serde_json::json!("2026-09-11"));
        fields.insert("message".to_string(), serde_json::json!("msg"));
        fields.insert("id".to_string(), serde_json::json!(1));
        fields.insert("level".to_string(), serde_json::json!(30));
        fields.insert("user".to_string(), serde_json::json!("alice"));

        let event = LogEvent::new("2026-09-11", LogColor::Green, "msg", fields);

        let ts_key = event.semantic_key(StandardField::Timestamp);
        let msg_key = event.semantic_key(StandardField::Message);

        let custom_fields: Vec<(&String, &serde_json::Value)> = event
            .fields
            .iter()
            .filter(|(k, _)| {
                let s = k.as_str();
                s != "id" && s != ts_key && s != msg_key
            })
            .collect();

        let custom_fields = cluster_log_fields(custom_fields);
        let keys: Vec<&str> = custom_fields.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"level"));
        assert!(keys.contains(&"user"));
        assert!(!keys.contains(&"timestamp"));
        assert!(!keys.contains(&"message"));
        assert!(!keys.contains(&"id"));

        let lvl_field = custom_fields.iter().find(|(k, _)| *k == "level").unwrap();
        assert_eq!(lvl_field.1, &serde_json::json!(30));
        assert_eq!(uwu_core_schema::value_to_cow(lvl_field.1), "30");
    }
}
