pub mod autocomplete;
pub mod columns_modal;
pub mod detail;
pub mod header;
pub mod history;
pub mod launch_modal;
pub mod table;
pub mod theme;

use crate::app::UwuGuiApp;
use eframe::egui;

pub fn render_ui(ctx: &egui::Context, app: &mut UwuGuiApp) {
    ctx.set_visuals(theme::create_visuals());

    // Phím Escape: Đóng history popup trước, rồi đến modal Params/Columns, rồi đến Log Inspector
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if app.history_state.is_open {
            app.history_state.close_popup();
        } else if app.column_state.is_modal_open {
            app.column_state.is_modal_open = false;
        } else if app.show_launch_modal {
            app.show_launch_modal = false;
        } else if app.selected_log.is_some() {
            app.selected_log = None;
        }
    }

    // Top Panel: Search Bar, Fast Presets, Action Controls
    egui::TopBottomPanel::top("header_panel")
        .frame(
            egui::Frame::default()
                .fill(theme::BG_MANTLE)
                .inner_margin(egui::Margin::symmetric(14.0, 8.0))
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
                .show_inside(ui, |ui| {
                    table::render_table(ui, app);
                });
        });

    // Modal Dialog: Launch & Source Parameters
    launch_modal::render_launch_modal(ctx, app);

    // Modal Dialog: Table Columns & Ordering
    columns_modal::render_columns_modal(ctx, app);
}
