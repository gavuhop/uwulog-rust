use crate::actions::AppAction;
use crate::components::{render_card, AppButton, ButtonVariant, ModalContainer, TextInput};
use crate::theme;
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
                let draft = draft.as_mut().unwrap();

                // 1. System Engine Performance Card
                render_card(ui, "System Engine Performance", |ui| {
                    egui::Grid::new("engine_params_grid")
                        .num_columns(2)
                        .spacing([16.0, 10.0])
                        .show(ui, |ui| {
                            let right_col_w = 130.0;
                            let spacing_x = 16.0;
                            let left_col_w =
                                (ui.available_width() - right_col_w - spacing_x).max(180.0);

                            // Row 1: RingBuffer Capacity
                            ui.allocate_ui_with_layout(
                                egui::vec2(left_col_w, 24.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("RingBuffer Capacity (-C):")
                                            .color(theme::TEXT_PRIMARY),
                                    )
                                    .on_hover_text(
                                        "Maximum log events retained in circular memory buffer",
                                    );
                                },
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
                            ui.end_row();

                            // Row 2: Display Limit
                            ui.allocate_ui_with_layout(
                                egui::vec2(left_col_w, 24.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("Display Limit (-n):")
                                            .color(theme::TEXT_PRIMARY),
                                    )
                                    .on_hover_text("Maximum log events rendered in the table view");
                                },
                            );
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
                            ui.end_row();
                        });
                });

                ui.add_space(10.0);

                // 2. Data Ingestion Source Card
                render_card(ui, "Data Ingestion Source", |ui| {
                    // Segmented Control (Tabs) for Source Selection
                    let total_width = ui.available_width();
                    let seg_height = 28.0;
                    let seg_width = ((total_width - 4.0) * 0.5).floor();

                    egui::Frame::default()
                        .fill(theme::BG_CRUST)
                        .corner_radius(CornerRadius::same(5))
                        .inner_margin(egui::Margin::same(2))
                        .stroke(Stroke::new(1.0, theme::BG_SURFACE0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                                // Segment 1: Process Exec (-c)
                                let is_proc = draft.source_type == SourceType::Process;
                                let (rect1, resp1) = ui.allocate_exact_size(
                                    egui::vec2(seg_width, seg_height),
                                    egui::Sense::click(),
                                );
                                if resp1.clicked() {
                                    draft.source_type = SourceType::Process;
                                }
                                if resp1.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                let bg1 = if is_proc {
                                    theme::BG_SURFACE1
                                } else if resp1.hovered() {
                                    theme::BG_SURFACE0
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let stroke1 = if is_proc {
                                    Stroke::new(1.0, theme::TEXT_KEY)
                                } else {
                                    Stroke::NONE
                                };
                                let text_color1 = if is_proc {
                                    theme::TEXT_PRIMARY
                                } else {
                                    theme::TEXT_MUTED
                                };

                                ui.painter().rect(
                                    rect1,
                                    CornerRadius::same(4),
                                    bg1,
                                    stroke1,
                                    egui::StrokeKind::Inside,
                                );
                                ui.painter().text(
                                    rect1.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "⚡ Process Exec (-c)",
                                    egui::FontId::proportional(12.0),
                                    text_color1,
                                );

                                // Segment 2: File Tail (-f)
                                let is_file = draft.source_type == SourceType::File;
                                let (rect2, resp2) = ui.allocate_exact_size(
                                    egui::vec2(seg_width, seg_height),
                                    egui::Sense::click(),
                                );
                                if resp2.clicked() {
                                    draft.source_type = SourceType::File;
                                }
                                if resp2.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                let bg2 = if is_file {
                                    theme::BG_SURFACE1
                                } else if resp2.hovered() {
                                    theme::BG_SURFACE0
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let stroke2 = if is_file {
                                    Stroke::new(1.0, theme::TEXT_KEY)
                                } else {
                                    Stroke::NONE
                                };
                                let text_color2 = if is_file {
                                    theme::TEXT_PRIMARY
                                } else {
                                    theme::TEXT_MUTED
                                };

                                ui.painter().rect(
                                    rect2,
                                    CornerRadius::same(4),
                                    bg2,
                                    stroke2,
                                    egui::StrokeKind::Inside,
                                );
                                ui.painter().text(
                                    rect2.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "📄 File Tail (-f)",
                                    egui::FontId::proportional(12.0),
                                    text_color2,
                                );
                            });
                        });

                    ui.add_space(10.0);

                    if draft.source_type == SourceType::Process {
                        ui.label(
                            egui::RichText::new("Command string:")
                                .color(theme::TEXT_PRIMARY)
                                .size(11.5),
                        );
                        ui.add_space(4.0);
                        TextInput::new(&mut draft.command_str)
                            .hint_text("e.g. go run main.go")
                            .show(ui);
                    } else {
                        ui.label(
                            egui::RichText::new("Log file path:")
                                .color(theme::TEXT_PRIMARY)
                                .size(11.5),
                        );
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let browse_width = 84.0;
                            let spacing = ui.spacing().item_spacing.x;
                            let text_width =
                                (ui.available_width() - browse_width - spacing).max(100.0);

                            TextInput::new(&mut draft.file_path)
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
                        .icon("🚀")
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
