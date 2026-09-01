pub mod actions;
pub mod autocomplete;
pub mod columns_modal;
pub mod detail;
pub mod header;
pub mod history;
pub mod launch_modal;
pub mod project_picker;
pub mod table;
pub mod theme;
pub mod unfiltered_table;

use crate::app::UwuGuiApp;
use eframe::egui;

pub fn render_ui(ctx: &egui::Context, app: &mut UwuGuiApp) {
    ctx.set_visuals(theme::create_visuals());

    // Phím Escape: Đóng project picker / history popup trước, rồi đến modal Params/Columns, rồi đến Tab Unfiltered (trở về Filtered), rồi đến Log Inspector
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if app.project_picker_open {
            app.project_picker_open = false;
        } else if app.history_state.is_open {
            app.history_state.close_popup();
        } else if app.column_state.is_modal_open {
            app.column_state.is_modal_open = false;
        } else if app.show_launch_modal {
            app.show_launch_modal = false;
        } else if app.active_tab == crate::app::ActiveTab::Unfiltered {
            app.close_unfiltered_stream();
        } else if app.selected_log.is_some() {
            app.selected_log = None;
        }
    }

    // Top Panel: Unified 1-Tier Modern Custom Title & Header Bar
    egui::TopBottomPanel::top("header_panel")
        .frame(
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .inner_margin(egui::Margin {
                    left: 10.0,
                    right: 6.0,
                    top: 6.0,
                    bottom: 6.0,
                })
                .stroke(egui::Stroke::new(1.0, theme::BG_SURFACE0)),
        )
        .resizable(false)
        .show(ctx, |ui| {
            header::render_header(ui, app);
        });

    // Central Panel: Left Table View & Right Inspector Panel
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(theme::BG_BASE)
                .inner_margin(egui::Margin::same(0.0)),
        )
        .show(ctx, |ui| {
            if app.selected_log.is_some() {
                let screen_width = ctx.screen_rect().width();
                let panel_id = egui::Id::new("detail_inspector_panel");

                // Nếu tỷ lệ chưa hợp lệ, đặt mặc định 35% chiều rộng màn hình
                if !(0.15..=0.85).contains(&app.inspector_width_ratio) {
                    app.inspector_width_ratio = 0.35;
                }

                // Khi kích thước màn hình thay đổi (Resize cửa sổ hoặc Zoom In/Out):
                // Tự động cập nhật lại kích thước panel theo đúng tỷ lệ inspector_width_ratio!
                if app.prev_screen_width > 0.0 && (screen_width - app.prev_screen_width).abs() > 2.0
                {
                    let new_width = (screen_width * app.inspector_width_ratio)
                        .clamp(240.0, screen_width * 0.75);
                    ctx.data_mut(|d| {
                        if let Some(mut state) =
                            d.get_persisted::<egui::containers::panel::PanelState>(panel_id)
                        {
                            state.rect.min.x = state.rect.max.x - new_width;
                            d.insert_persisted(panel_id, state);
                        }
                    });
                }
                app.prev_screen_width = screen_width;

                let target_width =
                    (screen_width * app.inspector_width_ratio).clamp(240.0, screen_width * 0.75);
                let min_sidebar_width = 240.0_f32.min(screen_width * 0.4);
                let max_sidebar_width = (screen_width * 0.75).max(min_sidebar_width + 100.0);

                egui::SidePanel::right("detail_inspector_panel")
                    .frame(
                        egui::Frame::default()
                            .fill(theme::BG_MANTLE)
                            .inner_margin(egui::Margin::same(12.0))
                            .stroke(egui::Stroke::new(1.0, theme::BG_SURFACE0)),
                    )
                    .resizable(true)
                    .default_width(target_width)
                    .min_width(min_sidebar_width)
                    .max_width(max_sidebar_width)
                    .show_inside(ui, |ui| {
                        let actual_width = ui.available_width();
                        if actual_width > 50.0 && screen_width > 100.0 {
                            // Cập nhật tỷ lệ khi người dùng chủ động kéo dãn thanh Inspector
                            app.inspector_width_ratio =
                                (actual_width / screen_width).clamp(0.15, 0.75);
                        }
                        detail::render_detail(ui, app);
                    });
            }

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::default()
                        .fill(theme::BG_BASE)
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0)),
                )
                .show_inside(ui, |ui| match app.active_tab {
                    crate::app::ActiveTab::Filtered => {
                        ui.push_id("main_filtered_table_scope", |ui| {
                            table::render_table(ui, app);
                        });
                    }
                    crate::app::ActiveTab::Unfiltered => {
                        ui.push_id("unfiltered_table_scope", |ui| {
                            unfiltered_table::render_unfiltered_table(ui, app);
                        });
                    }
                });
        });

    // Modal Dialog: Launch & Source Parameters
    launch_modal::render_launch_modal(ctx, app);

    // Modal Dialog: Table Columns & Ordering
    columns_modal::render_columns_modal(ctx, app);

    // Window Resize Border Handles (Hỗ trợ kéo dãn / thu nhỏ 4 góc và 4 cạnh cửa sổ)
    render_window_resize_borders(ctx);
}

/// Hỗ trợ kéo dãn / thu nhỏ cửa sổ tùy ý từ 4 góc và 4 cạnh viền màn hình (Edge & Corner Resizing)
fn render_window_resize_borders(ctx: &egui::Context) {
    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if is_maximized {
        return;
    }

    let screen_rect = ctx.screen_rect();
    let border_thickness = 6.0;
    let corner_size = 14.0;

    let corners_and_edges = [
        // 4 Góc (Ưu tiên kiểm tra góc trước vì diện tích góc bao gồm cả phần giao nhau)
        (
            egui::Rect::from_min_max(
                screen_rect.min,
                screen_rect.min + egui::vec2(corner_size, corner_size),
            ),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNorthWest,
        ),
        (
            egui::Rect::from_min_max(
                egui::pos2(screen_rect.max.x - corner_size, screen_rect.min.y),
                egui::pos2(screen_rect.max.x, screen_rect.min.y + corner_size),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNorthEast,
        ),
        (
            egui::Rect::from_min_max(
                egui::pos2(screen_rect.min.x, screen_rect.max.y - corner_size),
                egui::pos2(screen_rect.min.x + corner_size, screen_rect.max.y),
            ),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeSouthWest,
        ),
        (
            egui::Rect::from_min_max(
                screen_rect.max - egui::vec2(corner_size, corner_size),
                screen_rect.max,
            ),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeSouthEast,
        ),
        // 4 Cạnh viền
        (
            egui::Rect::from_min_max(
                screen_rect.min + egui::vec2(corner_size, 0.0),
                egui::pos2(
                    screen_rect.max.x - corner_size,
                    screen_rect.min.y + border_thickness,
                ),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeNorth,
        ),
        (
            egui::Rect::from_min_max(
                egui::pos2(
                    screen_rect.min.x + corner_size,
                    screen_rect.max.y - border_thickness,
                ),
                egui::pos2(screen_rect.max.x - corner_size, screen_rect.max.y),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeSouth,
        ),
        (
            egui::Rect::from_min_max(
                screen_rect.min + egui::vec2(0.0, corner_size),
                egui::pos2(
                    screen_rect.min.x + border_thickness,
                    screen_rect.max.y - corner_size,
                ),
            ),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeWest,
        ),
        (
            egui::Rect::from_min_max(
                egui::pos2(
                    screen_rect.max.x - border_thickness,
                    screen_rect.min.y + corner_size,
                ),
                egui::pos2(screen_rect.max.x, screen_rect.max.y - corner_size),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeEast,
        ),
    ];

    let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
    let pointer_pressed = ctx.input(|i| i.pointer.primary_pressed());

    if let Some(pos) = pointer_pos {
        for (rect, direction, cursor) in corners_and_edges {
            if rect.contains(pos) {
                ctx.set_cursor_icon(cursor);
                if pointer_pressed {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
                break;
            }
        }
    }
}
