use crate::actions::AppAction;
use crate::components::{render_card, AppButton, ButtonVariant, ModalContainer, TextInput};
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, Stroke};
use uwu_core_workspace::{SourceConfig, SourceType};

pub fn render_launch_modal(
    ctx: &egui::Context,
    is_open: bool,
    draft: &mut Option<SourceConfig>,
    current_config: &SourceConfig,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    if draft.is_none() {
        *draft = Some(current_config.clone());
    }

    let mut action_to_dispatch: Option<AppAction> = None;

    let resp = ModalContainer::new("launch_modal_window", "Settings & Launch Parameters")
        .subtitle("Configure engine buffer, display limits & data ingestion sources")
        .width(500.0)
        .show(
            ctx,
            |ui| {
                let theme = ui.app_theme();
                let draft = draft.as_mut().unwrap();

                // 1. System Engine Performance Card
                render_card(ui, "System Engine Performance", |ui| {
                    ui.scope(|ui| {
                        let input_stroke = Stroke::new(1.0, theme.surfaces.surface1);
                        let widgets = &mut ui.visuals_mut().widgets;
                        widgets.inactive.bg_stroke = input_stroke;
                        widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                        widgets.inactive.bg_fill = Color32::TRANSPARENT;

                        widgets.hovered.bg_stroke = input_stroke;
                        widgets.hovered.weak_bg_fill = Color32::TRANSPARENT;
                        widgets.hovered.bg_fill = Color32::TRANSPARENT;

                        widgets.active.bg_stroke = input_stroke;
                        widgets.active.weak_bg_fill = Color32::TRANSPARENT;
                        widgets.active.bg_fill = Color32::TRANSPARENT;

                        let right_col_w = 130.0;

                        // Row 1: RingBuffer Capacity
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 24.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new("RingBuffer Capacity (-C):")
                                            .color(theme.text.primary),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .on_hover_text(
                                    "Maximum log events retained in circular memory buffer",
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_sized(
                                            [right_col_w, 24.0],
                                            egui::DragValue::new(&mut draft.capacity)
                                                .range(1_000..=1_000_000)
                                                .speed(5000),
                                        );
                                    },
                                );
                            },
                        );

                        ui.add_space(8.0);

                        // Row 2: Display Limit
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 24.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new("Display Limit (-n):")
                                            .color(theme.text.primary),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .on_hover_text("Maximum log events rendered in the table view");

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_sized(
                                            [right_col_w, 24.0],
                                            egui::DragValue::new(&mut draft.display_limit)
                                                .range(100..=50_000)
                                                .speed(500),
                                        );
                                    },
                                );
                            },
                        );
                    });
                });

                ui.add_space(10.0);

                // 2. Data Ingestion Source Card
                render_card(ui, "Data Ingestion Source", |ui| {
                    // Segmented Control (Tabs) for Source Selection
                    let total_width = ui.available_width();
                    let seg_height = 28.0;
                    let seg_width1 = (total_width * 0.5).floor();
                    let seg_width2 = total_width - seg_width1;

                    egui::Frame::default()
                        .fill(theme.surfaces.crust)
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::ZERO)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            ui.horizontal(|ui| {
                                // Segment 1: Process Exec (-c)
                                let is_proc = draft.source_type == SourceType::Process;
                                let (rect1, resp1) = ui.allocate_exact_size(
                                    egui::vec2(seg_width1, seg_height),
                                    egui::Sense::click(),
                                );
                                if resp1.clicked() {
                                    draft.source_type = SourceType::Process;
                                }
                                if resp1.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                let bg1 = if is_proc {
                                    theme.surfaces.surface1
                                } else if resp1.hovered() {
                                    theme.surfaces.surface0
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let text_color1 = if is_proc {
                                    theme.text.primary
                                } else {
                                    theme.text.muted
                                };

                                let radius1 = CornerRadius {
                                    nw: 4,
                                    sw: 4,
                                    ne: 4,
                                    se: 4,
                                };
                                ui.painter().rect_filled(rect1, radius1, bg1);
                                let galley1 = ui.painter().layout_no_wrap(
                                    "Process Exec (-c)".to_string(),
                                    egui::FontId::proportional(12.0),
                                    text_color1,
                                );
                                let total_w1 = 14.0 + 6.0 + galley1.size().x;
                                let icon_r1 = egui::Rect::from_center_size(
                                    egui::pos2(
                                        rect1.center().x - total_w1 * 0.5 + 7.0,
                                        rect1.center().y,
                                    ),
                                    egui::vec2(14.0, 14.0),
                                );
                                let text_pos1 = egui::pos2(
                                    rect1.center().x - total_w1 * 0.5 + 20.0,
                                    rect1.center().y - galley1.size().y * 0.5,
                                );
                                crate::components::ui::IconName::Terminal.paint(
                                    ui.painter(),
                                    icon_r1,
                                    text_color1,
                                );
                                ui.painter().galley(text_pos1, galley1, text_color1);

                                // Segment 2: File Tail (-f)
                                let is_file = draft.source_type == SourceType::File;
                                let (rect2, resp2) = ui.allocate_exact_size(
                                    egui::vec2(seg_width2, seg_height),
                                    egui::Sense::click(),
                                );
                                if resp2.clicked() {
                                    draft.source_type = SourceType::File;
                                }
                                if resp2.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                let bg2 = if is_file {
                                    theme.surfaces.surface1
                                } else if resp2.hovered() {
                                    theme.surfaces.surface0
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let text_color2 = if is_file {
                                    theme.text.primary
                                } else {
                                    theme.text.muted
                                };

                                let radius2 = CornerRadius {
                                    nw: 4,
                                    sw: 4,
                                    ne: 4,
                                    se: 4,
                                };
                                ui.painter().rect_filled(rect2, radius2, bg2);
                                let galley2 = ui.painter().layout_no_wrap(
                                    "File Tail (-f)".to_string(),
                                    egui::FontId::proportional(12.0),
                                    text_color2,
                                );
                                let total_w2 = 14.0 + 6.0 + galley2.size().x;
                                let icon_r2 = egui::Rect::from_center_size(
                                    egui::pos2(
                                        rect2.center().x - total_w2 * 0.5 + 7.0,
                                        rect2.center().y,
                                    ),
                                    egui::vec2(14.0, 14.0),
                                );
                                let text_pos2 = egui::pos2(
                                    rect2.center().x - total_w2 * 0.5 + 20.0,
                                    rect2.center().y - galley2.size().y * 0.5,
                                );
                                crate::components::ui::IconName::File.paint(
                                    ui.painter(),
                                    icon_r2,
                                    text_color2,
                                );
                                ui.painter().galley(text_pos2, galley2, text_color2);
                            });
                        });

                    ui.add_space(10.0);

                    if draft.source_type == SourceType::Process {
                        ui.label(
                            egui::RichText::new("Command string:")
                                .color(theme.text.primary)
                                .size(11.5),
                        );
                        ui.add_space(4.0);
                        let cmd_id = egui::Id::new("launch_modal_cmd_input");
                        TextInput::new(&mut draft.command_str)
                            .id(cmd_id)
                            .auto_focus(true)
                            .hint_text("e.g. go run main.go")
                            .show(ui);
                    } else {
                        ui.label(
                            egui::RichText::new("Log file path:")
                                .color(theme.text.primary)
                                .size(11.5),
                        );
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let browse_width = 84.0;
                            let spacing = ui.spacing().item_spacing.x;
                            let text_width =
                                (ui.available_width() - browse_width - spacing).max(100.0);

                            let path_id = egui::Id::new("launch_modal_path_input");
                            TextInput::new(&mut draft.file_path)
                                .id(path_id)
                                .auto_focus(true)
                                .hint_text("e.g. /var/log/app.log")
                                .width(text_width)
                                .show(ui);

                            let browse_clicked = AppButton::new()
                                .label("Browse...")
                                .min_size(egui::vec2(
                                    browse_width,
                                    crate::components::ui::button::BUTTON_HEIGHT_NORMAL,
                                ))
                                .show(ui)
                                .clicked();

                            if browse_clicked {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    draft.file_path = path.display().to_string();
                                }
                            }
                        });
                    }
                });
            },
            Some(|ui: &mut egui::Ui, close_req: &mut bool| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if AppButton::new()
                        .label("Apply & Restart")
                        .icon(crate::components::ui::IconName::Rocket)
                        .variant(ButtonVariant::Success)
                        .min_size(egui::vec2(130.0, 26.0))
                        .show(ui)
                        .clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        action_to_dispatch = Some(AppAction::ApplyLaunchModal);
                    }

                    ui.add_space(8.0);

                    if AppButton::new()
                        .label("Cancel")
                        .min_size(egui::vec2(80.0, 26.0))
                        .show(ui)
                        .clicked()
                    {
                        *close_req = true;
                    }
                });
            }),
        );

    if resp.closed {
        action_to_dispatch = Some(AppAction::CloseLaunchModal);
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}
