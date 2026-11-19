use crate::actions::{AppAction, OverlayAction};
use crate::components::{render_card, AppButton, ButtonVariant, ModalContainer, TextInput};
use crate::keymap::{KeyAction, KeyContext, KeymapManager};
use crate::theme::ActiveTheme;
use eframe::egui::{self, Color32, CornerRadius, Stroke};
use uwu_core_workspace::{SourceConfig, SourceType};

pub fn render_launch_modal(
    ctx: &egui::Context,
    is_open: bool,
    draft: &mut Option<SourceConfig>,
    current_config: &SourceConfig,
    keymap: &KeymapManager,
    dispatch: &mut impl FnMut(AppAction),
) {
    if !is_open {
        return;
    }

    let mut action_to_dispatch: Option<AppAction> = None;

    let resp = ModalContainer::new("launch_modal_window", "Settings & Launch Parameters")
        .subtitle("Configure engine buffer, display limits & data ingestion sources")
        .width(500.0)
        .show(
            ctx,
            |ui| {
                let theme = ui.app_theme();
                let draft = draft.get_or_insert_with(|| current_config.clone());

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
                                                .range(1_000..=50_000_000)
                                                .speed(5_000),
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
                    let segments = [
                        (
                            SourceType::Process,
                            "Process Exec (-c)",
                            crate::components::ui::IconName::Terminal,
                        ),
                        (
                            SourceType::File,
                            "File Tail (-f)",
                            crate::components::ui::IconName::File,
                        ),
                    ];
                    let seg_w = (total_width / segments.len() as f32).floor();

                    egui::Frame::default()
                        .fill(theme.surfaces.crust)
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::ZERO)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            ui.horizontal(|ui| {
                                for (idx, (seg_type, seg_label, seg_icon)) in
                                    segments.iter().enumerate()
                                {
                                    let w = if idx + 1 == segments.len() {
                                        total_width - seg_w * idx as f32
                                    } else {
                                        seg_w
                                    };
                                    let is_active = draft.source_type == *seg_type;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(w, seg_height),
                                        egui::Sense::click(),
                                    );
                                    if resp.clicked() {
                                        draft.source_type = *seg_type;
                                    }
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    let bg = if is_active {
                                        theme.surfaces.surface1
                                    } else if resp.hovered() {
                                        theme.surfaces.surface0
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    let text_color = if is_active {
                                        theme.text.primary
                                    } else {
                                        theme.text.muted
                                    };

                                    ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
                                    let galley = ui.painter().layout_no_wrap(
                                        (*seg_label).to_string(),
                                        egui::FontId::proportional(12.0),
                                        text_color,
                                    );
                                    let total_w = 14.0 + 6.0 + galley.size().x;
                                    let icon_r = egui::Rect::from_center_size(
                                        egui::pos2(
                                            rect.center().x - total_w * 0.5 + 7.0,
                                            rect.center().y,
                                        ),
                                        egui::vec2(14.0, 14.0),
                                    );
                                    let text_pos = egui::pos2(
                                        rect.center().x - total_w * 0.5 + 20.0,
                                        rect.center().y - galley.size().y * 0.5,
                                    );
                                    seg_icon.paint(ui.painter(), icon_r, text_color);
                                    ui.painter().galley(text_pos, galley, text_color);
                                }
                            });
                        });

                    ui.add_space(10.0);

                    match draft.source_type {
                        SourceType::Process => {
                            ui.label(
                                egui::RichText::new("Command string:")
                                    .color(theme.text.primary)
                                    .size(12.0),
                            );
                            ui.add_space(4.0);
                            let cmd_id = egui::Id::new("launch_modal_cmd_input");
                            TextInput::new(&mut draft.command_str)
                                .id(cmd_id)
                                .auto_focus(true)
                                .hint_text("e.g. go run main.go")
                                .show(ui);
                        }
                        SourceType::File => {
                            ui.label(
                                egui::RichText::new("Log file path:")
                                    .color(theme.text.primary)
                                    .size(12.0),
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
                        || keymap.consume_input(ui, KeyContext::Modal)
                            == Some(KeyAction::ConfirmSelection)
                    {
                        action_to_dispatch = Some(OverlayAction::ApplyLaunch.into());
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
        action_to_dispatch = Some(OverlayAction::CloseLaunch.into());
    }

    if let Some(action) = action_to_dispatch {
        dispatch(action);
    }
}
